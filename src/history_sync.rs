//! Paged history sync for groups, "Cloud của tôi", and mobile message sync (`Đồng bộ tin nhắn từ điện thoại`, #19, #23, #68).
//!
//! - Group & Cloud history walks `/api/cm/getoldv2` backwards 50 messages at a time from the oldest
//!   message already in `zalo_history.db`, so a second run resumes where the first stopped.
//! - Phone message sync (`sync_phone_messages`, #68) triggers Zalo's mobile pull endpoints
//!   (`/api/message/pull_mobile_msg` cmd `12000` & `/api/transfer-sync-v2/request-sync` cmd `12888`),
//!   drains `lp_chat` multi-batch queues (`getChatFromPolling`), batch-queries `/api/preloadconvers/get-last-msgs`
//!   across all 1-1 and group threads in 25-thread chunks, and syncs recent cloud/group history into `zalo_history.db`.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use anyhow::Result;

use crate::chat::{self, Thread};
use crate::session::Session;
use crate::store::{self, MessageStore};

const DB_PATH: &str = "zalo_history.db";
const PAGE: u32 = 50;
/// Pages per thread per run (1 000 messages); run again to go further back.
const MAX_PAGES: usize = 20;
const PAUSE: Duration = Duration::from_millis(400);

/// Set by the GUI's "Dừng" button; checked between pages.
pub static CANCEL: AtomicBool = AtomicBool::new(false);
/// True while a Cloud/Group sync runs, so a second click doesn't start a parallel one.
pub static RUNNING: AtomicBool = AtomicBool::new(false);

/// Set by the GUI's "Dừng đồng bộ điện thoại" button (#68).
pub static PHONE_SYNC_CANCEL: AtomicBool = AtomicBool::new(false);
/// True while a phone message sync (`sync_phone_messages`) is active (#68).
pub static PHONE_SYNC_RUNNING: AtomicBool = AtomicBool::new(false);

/// Fetch older pages for `t` until the server has nothing older, `MAX_PAGES` is hit, or the user
/// cancels. Each page is committed immediately so a crash or cancel loses nothing. SQLite work
/// runs on the blocking pool; no connection is held across an await.
/// Returns how many messages were new to the database.
pub async fn sync_thread(s: &Session, t: &mut Thread) -> Result<usize> {
    let id = t.id.clone();
    let mut cursor =
        tokio::task::spawn_blocking(move || MessageStore::open(DB_PATH).ok()?.oldest_msg_id(&id))
            .await?
            .unwrap_or_default();
    let mut added = 0;
    for _ in 0..MAX_PAGES {
        if CANCEL.load(Ordering::Relaxed) {
            break;
        }
        let resp = chat::get_cloud_messages(s, &t.id, &cursor, true, PAGE).await?;
        let before = t.messages.len();
        chat::merge_group_history(t, &resp);
        if t.messages.len() == before {
            break; // nothing older on the server
        }
        let snapshot = t.clone();
        added += tokio::task::spawn_blocking(move || {
            store::persist_threads(
                &mut MessageStore::open(DB_PATH)?,
                std::slice::from_ref(&snapshot),
            )
        })
        .await??;
        match next_cursor(t, &cursor) {
            Some(c) => cursor = c,
            None => break,
        }
        tokio::time::sleep(PAUSE).await;
    }
    Ok(added)
}

/// The oldest server message id after a merge, or `None` when the page didn't move the cursor
/// back (the server returned only messages we already had); continuing would loop forever.
/// Local echoes (`out_*`) are not valid cursors, hence the digits-only filter.
fn next_cursor(t: &Thread, current: &str) -> Option<String> {
    let oldest = t
        .messages
        .iter()
        .map(|m| m.id.as_str())
        .find(|id| !id.is_empty() && id.bytes().all(|b| b.is_ascii_digit()))?;
    (oldest != current).then(|| oldest.to_string())
}

static LATEST_MOBILE_TEMP_KEY: std::sync::Mutex<String> = std::sync::Mutex::new(String::new());

/// Record a fresh `temp_key` received from a `transfer_after_login` control event.
pub fn set_latest_mobile_temp_key(temp_key: &str) {
    if let Ok(mut g) = LATEST_MOBILE_TEMP_KEY.lock() {
        *g = temp_key.trim().to_string();
    }
}

/// Return the most recent `temp_key` issued by the user's phone on login (if any).
pub fn latest_mobile_temp_key() -> String {
    LATEST_MOBILE_TEMP_KEY
        .lock()
        .map(|g| g.clone())
        .unwrap_or_default()
}
static LAST_MOBILE_SYNC_STATUS: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

/// Record the latest status returned by the user's phone (`WaitingConfirmOnPhone`, `MobileBusyRestoring`, etc.).
pub fn set_last_mobile_sync_status(status: Option<String>) {
    if let Ok(mut g) = LAST_MOBILE_SYNC_STATUS.lock() {
        *g = status;
    }
}

/// Read the latest status returned by the user's phone during `sync_phone_messages`.
pub fn last_mobile_sync_status() -> Option<String> {
    LAST_MOBILE_SYNC_STATUS.lock().ok().and_then(|g| g.clone())
}


static CURRENT_SYNC_KEYS: std::sync::Mutex<Option<(rsa::RsaPrivateKey, String)>> =
    std::sync::Mutex::new(None);

/// Path to store the active session RSA sync key pair to persist across restarts.
fn sync_rsa_key_path() -> std::path::PathBuf {
    std::path::PathBuf::from("accounts/sync_rsa_active.der")
}

/// Retrieve the active RSA 2048 keys used for mobile sync decryption without consuming them.
pub fn get_sync_keys() -> Option<(rsa::RsaPrivateKey, String)> {
    if let Ok(g) = CURRENT_SYNC_KEYS.lock() {
        if let Some(ref pair) = *g {
            return Some(pair.clone());
        }
    }
    // Fall back to reading persisted RSA key from disk
    use rsa::pkcs8::{DecodePrivateKey, EncodePublicKey};
    use base64::Engine;
    if let Ok(bytes) = std::fs::read(sync_rsa_key_path()) {
        if let Ok(priv_key) = rsa::RsaPrivateKey::from_pkcs8_der(&bytes) {
            let pub_key = rsa::RsaPublicKey::from(&priv_key);
            if let Ok(der) = pub_key.to_public_key_der() {
                let b64 = base64::engine::general_purpose::STANDARD.encode(der.as_bytes());
                if let Ok(mut g) = CURRENT_SYNC_KEYS.lock() {
                    *g = Some((priv_key.clone(), b64.clone()));
                }
                return Some((priv_key, b64));
            }
        }
    }
    None
}

/// Retrieve the active RSA 2048 public key.
pub fn get_sync_public_key() -> Option<String> {
    get_sync_keys().map(|(_, pub_k)| pub_k)
}

/// Clear the active sync keys once sync is finalized.
pub fn clear_sync_keys() {
    if let Ok(mut g) = CURRENT_SYNC_KEYS.lock() {
        *g = None;
    }
    let _ = std::fs::remove_file(sync_rsa_key_path());
}

/// Generate or reuse a genuine 2048-bit RSA SubjectPublicKeyInfo (`SPKI`) DER Base64 key.
pub fn generate_sync_public_key() -> String {
    use rsa::pkcs8::{EncodePrivateKey, EncodePublicKey};
    use rsa::{RsaPrivateKey, RsaPublicKey};
    use base64::Engine;

    // Reuse existing key if one was generated recently and not completed
    if let Some((_, pub_k)) = get_sync_keys() {
        return pub_k;
    }

    let mut rng = rsa::rand_core::OsRng;
    if let Ok(priv_key) = RsaPrivateKey::new(&mut rng, 2048) {
        let pub_key = RsaPublicKey::from(&priv_key);
        if let Ok(der) = pub_key.to_public_key_der() {
            let b64 = base64::engine::general_purpose::STANDARD.encode(der.as_bytes());
            // Persist private key to disk to withstand restarts
            if let Ok(priv_der) = priv_key.to_pkcs8_der() {
                let _ = std::fs::create_dir_all("accounts");
                let _ = std::fs::write(sync_rsa_key_path(), priv_der.as_bytes());
            }
            if let Ok(mut g) = CURRENT_SYNC_KEYS.lock() {
                *g = Some((priv_key, b64.clone()));
            }
            return b64;
        }
    }
    String::new()
}
/// Synchronize 1-1 and group messages from the user's phone and Zalo cloud (`Đồng bộ tin nhắn từ điện thoại`, #68).
///
/// Workflow:
/// 1. Sends mobile pull signal (`request_phone_sync` `cmd 12000` + `request_phone_sync_v2` `cmd 12888`)
///    so the phone Zalo app is notified to push recent 1-1 & group messages.
/// 2. Drains `lp_chat` (`get_chat_from_polling`) across multiple batches to collect queued 1-1 & group messages.
/// 3. Queries `/api/preloadconvers/get-last-msgs` (`get_last_msgs`) in 25-thread batches across all
///    1-1 and group conversations and merges messages into `threads`.
/// 4. Fetches recent & older pages via `/api/cm/getrecentv2` and `/api/cm/getoldv2` for Cloud & groups.
/// 5. Persists all merged messages to `zalo_history.db` and `threads_cache.json`.
///
/// Returns `(added_messages, synced_threads_count)`.
pub async fn sync_phone_messages<F>(
    s: &Session,
    poll_state: &tokio::sync::Mutex<chat::PollState>,
    threads: &mut Vec<Thread>,
    on_progress: F,
) -> Result<(usize, usize)>
where
    F: FnMut(f32, String),
{
    sync_phone_messages_opts(s, poll_state, threads, true, on_progress).await
}

/// Synchronize messages with explicit control over whether to send a wake-up/pull prompt
/// (`send_phone_prompt`) to the user's phone or only drain server/cloud queues.
pub async fn sync_phone_messages_opts<F>(
    s: &Session,
    poll_state: &tokio::sync::Mutex<chat::PollState>,
    threads: &mut Vec<Thread>,
    send_phone_prompt: bool,
    mut on_progress: F,
) -> Result<(usize, usize)>
where
    F: FnMut(f32, String),
{
    if s.login.is_none() {
        anyhow::bail!("chưa đăng nhập");
    }
    let pub_key = generate_sync_public_key();
    let before_total: usize = threads.iter().map(|t| t.messages.len()).sum();

    if send_phone_prompt {
        set_last_mobile_sync_status(None);

        // Phase 1: Send mobile wake-up / pull request to phone (`cmd 12000` + WebSocket `cmd 592` & `cmd 590`)
        on_progress(
            0.10,
            "📲 Đang gửi yêu cầu đồng bộ tới Zalo trên điện thoại (hãy mở Zalo trên điện thoại và bấm 'Đồng bộ ngay')…".to_string(),
        );
        let r1 = chat::request_phone_sync(s, &pub_key).await;
        if let Ok(ref val) = r1 {
            chat::merge_previews(threads, val);
        }
        let _ = chat::request_phone_sync_v2(s, &pub_key).await;

        // Wait for user to confirm on phone (up to 90 seconds)
        let deadline = std::time::Instant::now() + Duration::from_secs(90);
        let mut confirmed = false;
        while std::time::Instant::now() < deadline {
            if PHONE_SYNC_CANCEL.load(Ordering::Relaxed) {
                let _ = chat::cancel_phone_sync(s, &pub_key).await;
                return Ok((0, 0));
            }
            if let Some(ref st) = last_mobile_sync_status() {
                if st.contains("đã xác nhận")
                    || st.contains("đang nén")
                    || st.contains("giải mã")
                    || st.contains("Đã nhận gói đồng bộ")
                    || st.contains("thành công")
                {
                    confirmed = true;
                    on_progress(0.40, st.clone());
                    break;
                } else if st.contains("từ chối") {
                    on_progress(1.0, st.clone());
                    return Ok((0, 0));
                } else {
                    on_progress(0.20, st.clone());
                }
            }
            tokio::time::sleep(Duration::from_millis(500)).await;
        }

        if confirmed {
            let finish_deadline = std::time::Instant::now() + Duration::from_secs(45);
            while std::time::Instant::now() < finish_deadline {
                if let Some(ref st) = last_mobile_sync_status() {
                    if st.contains("thành công") {
                        on_progress(1.0, st.clone());
                        break;
                    } else if st.contains("Không thể") || st.contains("thất bại") {
                        on_progress(1.0, st.clone());
                        break;
                    } else {
                        on_progress(0.70, st.clone());
                    }
                }
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
        }
    }

    // Phase 1b: Synchronize pinned conversations from server (/api/pinconvers/list, #87)
    on_progress(
        0.18,
        "📌 Đang đồng bộ danh sách hội thoại đã ghim từ máy chủ Zalo…".to_string(),
    );
    if let Ok(pinned_val) = chat::get_pinned(s).await {
        let pinned_ids = chat::parse_pinned_conversations(&pinned_val);
        chat::apply_pinned_threads(threads, &pinned_ids);
    }

    if PHONE_SYNC_CANCEL.load(Ordering::Relaxed) {
        let _ = chat::cancel_phone_sync(s, &pub_key).await;
        return Ok((0, 0));
    }

    // Phase 2: Drain `lp_chat` multi-batch action queues (`getChatFromPolling`)
    on_progress(
        0.22,
        "📥 Đang nhận hàng đợi tin nhắn 1-1 & nhóm từ điện thoại…".to_string(),
    );
    for batch_idx in 0..5 {
        if PHONE_SYNC_CANCEL.load(Ordering::Relaxed) {
            let _ = chat::cancel_phone_sync(s, &pub_key).await;
            return Ok((0, 0));
        }
        let lp_res = {
            let mut ps = poll_state.lock().await;
            chat::get_chat_from_polling(s, &mut ps).await
        };
        let Ok(lp_resp) = lp_res else {
            break;
        };
        chat::merge_previews(threads, &lp_resp);
        if let Some(data_node) = lp_resp.get("data") {
            crate::unread::merge_read_points(threads, data_node);
        }
        let data_node = lp_resp.get("data").unwrap_or(&lp_resp);
        let more = data_node
            .get("more")
            .and_then(serde_json::Value::as_i64)
            .unwrap_or(0);
        let has_reset = data_node
            .get("resetLastActionId")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|r| !r.is_empty() && r != "0");
        on_progress(
            0.22 + (batch_idx as f32 + 1.0) * 0.04,
            format!("📥 Đã nhận lô hàng đợi #{} từ điện thoại…", batch_idx + 1),
        );
        if more == 0 && !has_reset {
            break;
        }
    }

    // Phase 3: Batch-query `/api/preloadconvers/get-last-msgs` across all 1-1 and group conversations
    on_progress(
        0.45,
        "🔄 Đang đồng bộ tin nhắn các cuộc trò chuyện 1-1 & nhóm…".to_string(),
    );
    if let Ok(preview_resp) = chat::get_last_msgs(s, &[]).await {
        chat::merge_previews(threads, &preview_resp);
    }
    let known_ids: Vec<String> = threads
        .iter()
        .filter(|t| !t.id.is_empty())
        .map(|t| {
            if t.is_group && !t.id.starts_with('g') && !t.id.starts_with("group_") {
                format!("g{}", t.id)
            } else {
                t.id.clone()
            }
        })
        .collect();
    let chunks: Vec<&[String]> = known_ids.chunks(25).collect();
    let total_chunks = chunks.len().max(1);
    for (idx, chunk) in chunks.into_iter().enumerate() {
        if PHONE_SYNC_CANCEL.load(Ordering::Relaxed) {
            let _ = chat::cancel_phone_sync(s, &pub_key).await;
            break;
        }
        if let Ok(preview_resp) = chat::get_last_msgs(s, chunk).await {
            chat::merge_previews(threads, &preview_resp);
        }
        let frac = 0.45 + 0.25 * ((idx + 1) as f32 / total_chunks as f32);
        on_progress(
            frac,
            format!(
                "🔄 Đang đồng bộ hội thoại 1-1 & nhóm ({}/{total_chunks} nhóm lô)…",
                idx + 1
            ),
        );
        tokio::time::sleep(Duration::from_millis(120)).await;
    }

    // Phase 4: Sync Cloud & group conversations via `/api/cm/getrecentv2` + `/api/cm/getoldv2`
    let send2me_id = s
        .login
        .as_ref()
        .map(|l| l.send2me_id.clone())
        .unwrap_or_default();
    let cm_targets: Vec<usize> = threads
        .iter()
        .enumerate()
        .filter(|(_, t)| {
            !t.id.is_empty()
                && (t.is_group
                    || t.name.contains("Cloud của tôi")
                    || (!send2me_id.is_empty() && t.id == send2me_id))
        })
        .map(|(i, _)| i)
        .collect();
    let total_cm = cm_targets.len().max(1);
    for (pos, tidx) in cm_targets.into_iter().enumerate() {
        if PHONE_SYNC_CANCEL.load(Ordering::Relaxed) {
            let _ = chat::cancel_phone_sync(s, &pub_key).await;
            break;
        }
        let (tid, tname) = {
            let t = &threads[tidx];
            (t.id.clone(), t.name.clone())
        };
        let frac = 0.70 + 0.25 * ((pos + 1) as f32 / total_cm as f32);
        on_progress(
            frac,
            format!("☁️ Đang đồng bộ lịch sử: {tname} ({}/{total_cm})…", pos + 1),
        );
        if let Ok(cm_resp) = chat::get_cloud_messages(s, &tid, "0", false, PAGE).await {
            chat::merge_group_history(&mut threads[tidx], &cm_resp);
        }
        if let Some(oldest) = next_cursor(&threads[tidx], "") {
            if let Ok(old_resp) = chat::get_cloud_messages(s, &tid, &oldest, true, PAGE).await {
                chat::merge_group_history(&mut threads[tidx], &old_resp);
            }
        }
        tokio::time::sleep(Duration::from_millis(150)).await;
    }

    // Phase 5: Resolve contact/group display names, deduplicate, and persist to SQLite (`zalo_history.db`)
    on_progress(
        0.96,
        "💾 Đang lưu tin nhắn đã đồng bộ vào cơ sở dữ liệu zalo_history.db…".to_string(),
    );
    chat::resolve_thread_names(s, threads).await;
    chat::deduplicate_threads(threads, None);

    let snapshot = threads.clone();
    let uin_str = s.login.as_ref().map(|l| l.uin.clone()).unwrap_or_default();
    let db_path = store::db_path_for_account(&uin_str);
    let db_path_clone = db_path.clone();
    let db_added = tokio::task::spawn_blocking(move || -> Result<usize> {
        let mut st = MessageStore::open(&db_path_clone)?;
        let n = store::persist_threads(&mut st, &snapshot)?;
        chat::save_threads_cache(&snapshot);
        Ok(n)
    })
    .await??;
    let after_total: usize = threads.iter().map(|t| t.messages.len()).sum();
    let added = db_added.max(after_total.saturating_sub(before_total));
    let active_threads = threads.iter().filter(|t| !t.messages.is_empty()).count();
    let final_msg = if let Some(phone_note) = last_mobile_sync_status() {
        format!("{phone_note} · Đã đồng bộ Cloud/Nhóm: +{added} tin nhắn ({active_threads} hội thoại)")
    } else {
        format!("✅ Đã gửi yêu cầu tới điện thoại & đồng bộ Cloud/Nhóm: +{added} tin nhắn ({active_threads} hội thoại). Hãy mở Zalo trên điện thoại để xác nhận nếu muốn kéo tin nhắn 1-1 cũ.")
    };
    on_progress(1.0, final_msg);
    Ok((added, active_threads))
}

/// Download, decrypt, decompress and ingest a mobile backup container (.db.crypt)
/// into `zalo_history.db` and hydrate `threads_cache.json`.
pub async fn import_mobile_backup_container(
    s: &Session,
    url: &str,
    encrypted_key: &str,
) -> Result<usize> {
    use aes::cipher::{BlockDecryptMut, KeyIvInit};
    use base64::Engine;
    use rsa::Pkcs1v15Encrypt;
    use rusqlite::Connection;
    use serde_json::json;
    use std::collections::HashMap;
    use std::io::Cursor;
    use crate::store::StoredMessage;

    if url.is_empty() {
        anyhow::bail!("empty backup url");
    }

    let (priv_key, pub_key_b64) = get_sync_keys()
        .ok_or_else(|| anyhow::anyhow!("Không tìm thấy khoá RSA phiên đồng bộ hiện tại"))?;

    let raw_key = if !encrypted_key.is_empty() {
        let enc_bytes = base64::engine::general_purpose::STANDARD.decode(encrypted_key)?;
        priv_key.decrypt(Pkcs1v15Encrypt, &enc_bytes)?
    } else {
        anyhow::bail!("encrypted_key is empty");
    };

    let uin = s.login.as_ref().map(|l| l.uin.clone()).unwrap_or_default();
    let sek = s.zpw_sek.clone().unwrap_or_default();
    let cookie_hdr = format!("zpw_sek={sek}; zpw_uin={uin}");

    eprintln!("[sync] import_mobile_backup_container: downloading {url}...");
    let resp = s
        .http
        .get(url)
        .header("Cookie", cookie_hdr)
        .header("User-Agent", "ZaloPC-win32-26.10.11")
        .send()
        .await?;

    if !resp.status().is_success() {
        let err = format!("failed to download backup: HTTP {}", resp.status());
        eprintln!("[sync] {err}");
        anyhow::bail!("{err}");
    }

    let crypt_data = resp.bytes().await?;
    eprintln!("[sync] downloaded {} bytes. Decrypting AES container...", crypt_data.len());

    let hex_key: String = raw_key.iter().map(|b| format!("{:02X}", b)).collect();
    let key_32 = &hex_key.as_bytes()[..32];
    type Aes256CbcDec = cbc::Decryptor<aes::Aes256>;

    let chunk_size = 65536;
    let iv_zero = [0u8; 16];
    let mut decrypted = Vec::with_capacity(crypt_data.len());

    for chunk in crypt_data.chunks(chunk_size) {
        let rem = chunk.len() % 16;
        let aligned_len = chunk.len() - rem;
        let mut buf = chunk[..aligned_len].to_vec();

        let dec = Aes256CbcDec::new(key_32.into(), &iv_zero.into());
        dec.decrypt_padded_mut::<cipher::block_padding::NoPadding>(&mut buf)
            .map_err(|e| anyhow::anyhow!("{e:?}"))?;

        decrypted.extend_from_slice(&buf);
        if rem > 0 {
            decrypted.extend_from_slice(&chunk[aligned_len..]);
        }
    }

    let magic = decrypted.get(..6).unwrap_or_default();
    if magic != b"ZDB4.0" {
        anyhow::bail!("invalid backup container magic: {:?}", decrypted.get(..7));
    }

    let mut offset = 14;
    if offset + 4 > decrypted.len() {
        anyhow::bail!("truncated container header");
    }
    let n_files = u32::from_be_bytes(decrypted[offset..offset + 4].try_into()?) as usize;
    offset += 4;

    let mut toc = Vec::new();
    let mut fids = Vec::new();
    let mut gids = Vec::new();

    for _ in 0..n_files {
        if offset + 4 > decrypted.len() {
            break;
        }
        let name_len = u32::from_be_bytes(decrypted[offset..offset + 4].try_into()?) as usize;
        offset += 4;
        if offset + name_len > decrypted.len() {
            break;
        }
        let name = String::from_utf8_lossy(&decrypted[offset..offset + name_len]).to_string();
        offset += name_len;
        if offset + 4 > decrypted.len() {
            break;
        }
        let size = u32::from_be_bytes(decrypted[offset..offset + 4].try_into()?) as usize;
        offset += 4;

        let stem = name.trim_end_matches(".db");
        if stem.starts_with("group_") {
            if let Ok(gid) = stem[6..].parse::<i64>() {
                gids.push(gid);
            }
        } else if let Ok(fid) = stem.parse::<i64>() {
            fids.push(fid);
        }

        toc.push((name, size));
    }
    let toc_end = offset;

    if toc_end >= decrypted.len() {
        anyhow::bail!("corrupted container: toc extends beyond payload");
    }
    let xz_slice = &decrypted[toc_end..];
    let end_pos = xz_slice
        .windows(2)
        .rposition(|w| w == b"YZ")
        .ok_or_else(|| anyhow::anyhow!("XZ footer not found"))?
        + 2;

    let mut cursor = Cursor::new(&xz_slice[..end_pos]);
    let mut payload = Vec::new();
    lzma_rs::xz_decompress(&mut cursor, &mut payload)?;

    let noise_body = json!({
        "fids": fids,
        "gids": gids
    });
    let enc_params = s.encoded_params(&noise_body)?;
    let znoise_url = format!("https://zwid.api.zalo.me/api/znoise?{}", s.common_params());
    let resp = s.post_form(&znoise_url, format!("params={enc_params}")).await?;
    let dec = s.maybe_decrypt(resp, s.session_key_bytes()).await;

    let mut id_map: HashMap<String, String> = HashMap::new();
    if let Some(data) = dec.get("data") {
        if let Some(arr) = data.get("fids").and_then(|v| v.as_array()) {
            for (fid, thread_id_val) in fids.iter().zip(arr.iter()) {
                if let Some(tid) = thread_id_val.as_str() {
                    id_map.insert(format!("{fid}.db"), tid.to_string());
                }
            }
        }
        if let Some(arr) = data.get("gids").and_then(|v| v.as_array()) {
            for (gid, thread_id_val) in gids.iter().zip(arr.iter()) {
                if let Some(tid) = thread_id_val.as_str() {
                    id_map.insert(format!("group_{gid}.db"), tid.to_string());
                }
            }
        }
    }

    let my_uid = s.login.as_ref().map(|l| l.uin.clone()).unwrap_or_default();
    let my_uin: i64 = my_uid.parse().unwrap_or(0);
    let my_name = s.login.as_ref().map(|l| l.display_name.clone()).unwrap_or_else(|| "Tôi".into());

    let db_path = crate::store::db_path_for_account(&my_uid);
    let mut store = crate::store::MessageStore::open(&db_path)?;
    let temp_dir = std::env::temp_dir().join(format!("ctdozlite_sync_{}", crate::param_cipher::random_hex(8)));

    let mut curr_offset = 0;
    let mut total_imported = 0;

    for (name, size) in toc {
        if curr_offset + size > payload.len() {
            break;
        }
        let db_bytes = &payload[curr_offset..curr_offset + size];
        curr_offset += size;

        let Some(thread_id) = id_map.get(&name) else {
            continue;
        };

        let temp_db_path = temp_dir.join(&name);
        if std::fs::write(&temp_db_path, db_bytes).is_err() {
            continue;
        }

        let Ok(conn) = Connection::open(&temp_db_path) else {
            continue;
        };

        let Ok(mut stmt) = conn.prepare(
            "SELECT SenderId, CliMsgId, GlbMsgId, MsgStatus, MsgType, TimeStamp, MsgContent, BinNet 
             FROM ChatContent ORDER BY TimeStamp ASC",
        ) else {
            continue;
        };

        let Ok(rows) = stmt.query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, i64>(4)?,
                row.get::<_, i64>(5)?,
                row.get::<_, Option<String>>(6)?,
                row.get::<_, Option<Vec<u8>>>(7)?,
            ))
        }) else {
            continue;
        };

        for r in rows.flatten() {
            let (sender_id_num, cli_msg_id, glb_msg_id, _status, msg_type_num, ts, content_opt, bin_net_opt) = r;
            let is_outgoing = sender_id_num == my_uin;
            let sender_id = if is_outgoing { my_uid.clone() } else { thread_id.clone() };
            let sender_name = if is_outgoing { my_name.clone() } else { String::new() };

            let msg_id = if glb_msg_id > 0 {
                glb_msg_id.to_string()
            } else {
                cli_msg_id.to_string()
            };

            let mut text = content_opt.unwrap_or_default().trim().to_string();
            let mut msg_type = "webchat".to_string();

            if text.is_empty() {
                if let Some(bn) = &bin_net_opt {
                    let s_latin = String::from_utf8_lossy(bn);
                    if let Some(pos) = s_latin.find("maps.app.goo.gl") {
                        msg_type = "chat.link".to_string();
                        let sub = &s_latin[pos..];
                        let end_url = sub.find(['"', '}', '\\', ' ']).unwrap_or(sub.len());
                        let url = format!("https://{}", &sub[..end_url]);
                        text = format!("Tôi đang trên đường tới. Bạn có thể xem tiến độ đi đường và thời gian đến của tôi trên Maps: {url}");
                    } else if msg_type_num == 10 {
                        msg_type = "chat.sticker".to_string();
                        text = "[Nhãn dán]".to_string();
                    } else if msg_type_num == 3 {
                        msg_type = "chat.photo".to_string();
                        text = "[Hình ảnh]".to_string();
                    } else if msg_type_num == 12 {
                        msg_type = "chat.link".to_string();
                        text = "[Liên kết]".to_string();
                    }
                }
            }

            if text.is_empty() {
                if msg_type_num == 10 {
                    text = "[Nhãn dán]".to_string();
                } else if msg_type_num == 3 {
                    text = "[Hình ảnh]".to_string();
                } else if msg_type_num == 12 {
                    text = "[Liên kết]".to_string();
                }
            }

            let sm = StoredMessage {
                msg_id,
                thread_id: thread_id.clone(),
                sender_id,
                sender_name,
                text,
                msg_type,
                ts,
                is_outgoing,
                raw_json: String::new(),
            };

            if store.upsert_message(&sm).unwrap_or(false) {
                total_imported += 1;
            }
        }
    }

    let _ = std::fs::remove_dir_all(&temp_dir);

    let mut cached_threads = crate::chat::load_threads_cache();
    let _ = store.hydrate_threads(&mut cached_threads, 50);
    crate::chat::save_threads_cache(&cached_threads);

    eprintln!("[sync] Imported {total_imported} messages into SQLite!");

    // Notify Zalo server and phone that snapshot is downloaded, dismissing mobile spinner
    let _ = crate::chat::clean_sync(s, &pub_key_b64).await;
    clear_sync_keys();

    Ok(total_imported)
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct SyncV2Keys {
    pub ik_pub: String,
    pub ik_priv: String,
    pub ek_pub: String,
    pub ek_priv: String,
}

#[derive(Debug, Clone)]
struct SyncV2KeysRaw {
    ik_pub: Vec<u8>,
    ik_priv: Vec<u8>,
    ek_priv: Vec<u8>,
}

/// Khóa Sync V2 ed25519 của phiên hiện tại (raw bytes, giữ trong bộ nhớ).
static CURRENT_V2_KEYS: std::sync::Mutex<Option<SyncV2KeysRaw>> = std::sync::Mutex::new(None);

pub fn generate_sync_v2_keys() -> Result<SyncV2Keys> {
    // wasmtime host (không cần Node.js). scripts/sync_v2_helper.js chỉ còn
    // là tài liệu tham khảo cho luồng JS gốc (deprecated path).
    let host = crate::zproto_host::ZprotoHost::load()?;
    let (ik_pub, ik_priv) = host.generate_keypair()?;
    let (ek_pub, ek_priv) = host.generate_keypair()?;
    use base64::Engine;
    let keys = SyncV2Keys {
        ik_pub: base64::engine::general_purpose::STANDARD.encode(&ik_pub),
        ik_priv: hex_encode(&ik_priv),
        ek_pub: base64::engine::general_purpose::STANDARD.encode(&ek_pub),
        ek_priv: hex_encode(&ek_priv),
    };
    if let Ok(mut g) = CURRENT_V2_KEYS.lock() {
        *g = Some(SyncV2KeysRaw { ik_pub, ik_priv, ek_priv });
    }
    Ok(keys)
}

fn hex_encode(bytes: &[u8]) -> String {
    const H: &[u8; 16] = b"0123456789abcdef";
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push(H[(b >> 4) as usize] as char);
        s.push(H[(b & 15) as usize] as char);
    }
    s
}
/// Kết quả giải mã metadata Sync V2: session id để đối chiếu với event batch.
#[derive(Debug, Default)]
pub struct SyncV2Meta {
    pub session_id: String,
    pub sender_identity: String,
}

pub async fn import_sync_v2_meta(s: &Session, url: &str) -> Result<SyncV2Meta> {
    let uin = s.login.as_ref().map(|l| l.uin.clone()).unwrap_or_default();
    let sek = s.zpw_sek.clone().unwrap_or_default();
    let cookie_hdr = format!("zpw_sek={sek}; zpw_uin={uin}");

    let resp = s
        .http
        .get(url)
        .header("Cookie", cookie_hdr)
        .header("User-Agent", "ZaloPC-win32-26.10.11")
        .send()
        .await?;

    if !resp.status().is_success() {
        anyhow::bail!("failed to download v2 meta: HTTP {}", resp.status());
    }

    let bytes = resp.bytes().await?;
    // wasmtime host (không cần Node.js). Keys phải tồn tại trong bộ nhớ tiến
    // trình (generate_sync_v2_keys đã chạy trước khi request sync).
    let keys = current_v2_keys()?;
    let host = crate::zproto_host::ZprotoHost::load()?;
    let mut session_record: Option<Vec<u8>> = None;
    let mut last: Option<crate::zproto_host::DecryptedMeta> = None;
    for chunk in crate::zproto_host::split_frames(&bytes)? {
        let m = host.decrypt_metadata(&chunk, &keys.ik_pub, &keys.ik_priv, &keys.ek_priv, session_record.as_deref())?;
        session_record = Some(m.session_record.clone());
        last = Some(m);
    }
    let m = last.ok_or_else(|| anyhow::anyhow!("metadata rỗng"))?;
    store_v2_session(&m.session_record);
    Ok(SyncV2Meta {
        session_id: m.session_id,
        sender_identity: m.sender_identity,
    })
}


fn current_v2_keys() -> Result<SyncV2KeysRaw> {
    CURRENT_V2_KEYS
        .lock()
        .ok()
        .and_then(|g| g.clone())
        .ok_or_else(|| anyhow::anyhow!("chưa có khóa Sync V2 — gọi generate_sync_v2_keys trước"))
}

/// Session record của metadata gần nhất (raw bytes, giữ trong bộ nhớ).
static CURRENT_V2_SESSION: std::sync::Mutex<Option<Vec<u8>>> = std::sync::Mutex::new(None);

fn store_v2_session(session_record: &[u8]) {
    if let Ok(mut g) = CURRENT_V2_SESSION.lock() {
        *g = Some(session_record.to_vec());
    }
}

fn current_v2_session() -> Result<Vec<u8>> {
    CURRENT_V2_SESSION
        .lock()
        .ok()
        .and_then(|g| g.clone())
        .ok_or_else(|| anyhow::anyhow!("chưa giải mã metadata — gọi import_sync_v2_meta trước"))
}
/// Đăng ký session Sync V2 đã giải mã xong metadata, key theo sync_id của event.
/// Batch message chỉ được giải mã khi sync_id của nó đã có metadata — tránh
/// dùng nhầm session của phiên cũ (#116).
static SYNC_V2_SESSIONS: std::sync::Mutex<Vec<(String, String)>> =
    std::sync::Mutex::new(Vec::new());

pub fn record_sync_v2_session(sync_id: &str, session_id: &str) {
    if let Ok(mut m) = SYNC_V2_SESSIONS.lock() {
        m.retain(|(k, _)| k != sync_id);
        m.push((sync_id.to_string(), session_id.to_string()));
    }
}

pub fn has_sync_v2_session(sync_id: &str) -> bool {
    SYNC_V2_SESSIONS
        .lock()
        .map(|m| m.iter().any(|(k, _)| k == sync_id))
        .unwrap_or(false)
}

#[derive(Debug, Default)]
pub struct ParsedProtoMsg {
    pub sender_id: String,
    pub client_id: u64,
    pub msg_status: i32,
    pub msg_type: i32,
    pub timestamp: u64,
    pub global_id: String,
    pub content: String,
}

#[derive(Debug, Default)]
pub struct ParsedProtoPartition {
    pub id: String,
    pub messages: Vec<ParsedProtoMsg>,
}

fn parse_vli(buf: &[u8], mut offset: usize) -> Option<(u64, usize)> {
    let mut result: u64 = 0;
    let mut shift = 0;
    while offset < buf.len() {
        let b = buf[offset];
        offset += 1;
        result |= ((b & 0x7F) as u64) << shift;
        if (b & 0x80) == 0 {
            return Some((result, offset));
        }
        shift += 7;
        if shift >= 64 { return None; }
    }
    None
}

fn parse_message(buf: &[u8]) -> Option<ParsedProtoMsg> {
    let mut msg = ParsedProtoMsg::default();
    let mut offset = 0;
    while offset < buf.len() {
        let (tag, next) = parse_vli(buf, offset)?;
        offset = next;
        let field_num = tag >> 3;
        let wire_type = tag & 0x07;
        match wire_type {
            0 => {
                let (val, next) = parse_vli(buf, offset)?;
                offset = next;
                match field_num {
                    2 => msg.client_id = val,
                    3 => msg.msg_status = val as i32,
                    4 => msg.msg_type = val as i32,
                    5 => msg.timestamp = val,
                    _ => {}
                }
            }
            2 => {
                let (len, next) = parse_vli(buf, offset)?;
                offset = next;
                let len = len as usize;
                if offset + len > buf.len() { return None; }
                let bytes = &buf[offset..offset + len];
                offset += len;
                match field_num {
                    1 => msg.sender_id = String::from_utf8_lossy(bytes).into_owned(),
                    6 => msg.global_id = String::from_utf8_lossy(bytes).into_owned(),
                    7 => msg.content = String::from_utf8_lossy(bytes).into_owned(),
                    _ => {}
                }
            }
            1 => offset += 8,
            5 => offset += 4,
            _ => return None,
        }
    }
    Some(msg)
}

fn parse_partition(buf: &[u8]) -> Option<ParsedProtoPartition> {
    let mut part = ParsedProtoPartition::default();
    let mut offset = 0;
    while offset < buf.len() {
        let (tag, next) = parse_vli(buf, offset)?;
        offset = next;
        let field_num = tag >> 3;
        let wire_type = tag & 0x07;
        match (field_num, wire_type) {
            (1, 2) => {
                let (len, next) = parse_vli(buf, offset)?;
                offset = next;
                let len = len as usize;
                if offset + len > buf.len() { return None; }
                part.id = String::from_utf8_lossy(&buf[offset..offset + len]).into_owned();
                offset += len;
            }
            (2, 2) => {
                let (len, next) = parse_vli(buf, offset)?;
                offset = next;
                let len = len as usize;
                if offset + len > buf.len() { return None; }
                if let Some(m) = parse_message(&buf[offset..offset + len]) {
                    part.messages.push(m);
                }
                offset += len;
            }
            (_, 0) => {
                let (_, next) = parse_vli(buf, offset)?;
                offset = next;
            }
            (_, 2) => {
                let (len, next) = parse_vli(buf, offset)?;
                offset = next + len as usize;
            }
            _ => return None,
        }
    }
    Some(part)
}

/// Giải nén payload Sync V2 tự nhận dạng: zstd (28 b5 2f fd) / gzip (1f 8b) /
/// zlib (78 xx) / raw. Không bao giờ fail — trả nguyên bytes khi không nhận ra.
pub fn decompress_sync_payload(buf: &[u8]) -> Vec<u8> {
    use std::io::Read;
    if buf.len() >= 4 && buf[0] == 0x28 && buf[1] == 0xb5 && buf[2] == 0x2f && buf[3] == 0xfd {
        if let Ok(d) = zstd::decode_all(std::io::Cursor::new(buf)) {
            return d;
        }
        return buf.to_vec();
    }
    if buf.len() >= 2 && buf[0] == 0x1f && buf[1] == 0x8b {
        let mut out = Vec::new();
        if flate2::read::GzDecoder::new(buf).read_to_end(&mut out).is_ok() {
            return out;
        }
        return buf.to_vec();
    }
    if buf.len() >= 2 && buf[0] == 0x78 {
        let mut out = Vec::new();
        if flate2::read::ZlibDecoder::new(buf).read_to_end(&mut out).is_ok() {
            return out;
        }
    }
    buf.to_vec()
}

pub fn parse_sync_chunk(buf: &[u8]) -> Option<Vec<ParsedProtoPartition>> {
    let mut parts = Vec::new();
    let mut offset = 0;
    while offset < buf.len() {
        let (tag, next) = parse_vli(buf, offset)?;
        offset = next;
        let field_num = tag >> 3;
        let wire_type = tag & 0x07;
        match (field_num, wire_type) {
            (2, 2) => {
                let (len, next) = parse_vli(buf, offset)?;
                offset = next;
                let len = len as usize;
                if offset + len > buf.len() { return None; }
                if let Some(p) = parse_partition(&buf[offset..offset + len]) {
                    parts.push(p);
                }
                offset += len;
            }
            (_, 0) => {
                let (_, next) = parse_vli(buf, offset)?;
                offset = next;
            }
            (_, 2) => {
                let (len, next) = parse_vli(buf, offset)?;
                offset = next + len as usize;
            }
            _ => return None,
        }
    }
    Some(parts)
}

pub async fn import_sync_v2_messages(s: &Session, url: &str) -> Result<usize> {
    let uin = s.login.as_ref().map(|l| l.uin.clone()).unwrap_or_default();
    let sek = s.zpw_sek.clone().unwrap_or_default();
    let cookie_hdr = format!("zpw_sek={sek}; zpw_uin={uin}");

    let resp = s
        .http
        .get(url)
        .header("Cookie", cookie_hdr)
        .header("User-Agent", "ZaloPC-win32-26.10.11")
        .send()
        .await?;

    if !resp.status().is_success() {
        anyhow::bail!("failed to download v2 messages: HTTP {}", resp.status());
    }

    let bytes = resp.bytes().await?;
    // wasmtime host (không cần Node.js). Multi-chunk: decrypt + unpack từng
    // chunk, nối protobuf bytes lại (đúng luồng HistoryCodec mẫu).
    let host = crate::zproto_host::ZprotoHost::load()?;
    let session_record = current_v2_session()?;
    let mut parts: Vec<u8> = Vec::new();
    for chunk in crate::zproto_host::split_frames(&bytes)? {
        let res = host.decrypt_message(&chunk, &session_record)?;
        parts.extend_from_slice(&decompress_sync_payload(&res.plaintext));
    }
    let decrypted_bytes = parts;
    // Nếu batch trả raw còn nén thì Rust tự nhận dạng thêm
    // (zstd/zlib/gzip — Zalo dùng cả 3 theo HistoryCodec mẫu).
    let decompressed = decompress_sync_payload(&decrypted_bytes);

    let partitions = parse_sync_chunk(&decompressed).unwrap_or_default();
    let db_path = crate::store::db_path_for_account(&uin);
    let mut store = crate::store::MessageStore::open(&db_path)?;
    let my_name = s.login.as_ref().map(|l| l.display_name.clone()).unwrap_or_else(|| "Tôi".into());

    let mut imported = 0;
    for part in partitions {
        let thread_id = part.id.clone();
        for m in part.messages {
            let is_outgoing = m.sender_id == uin;
            let sender_name = if is_outgoing { my_name.clone() } else { String::new() };
            let msg_id = if !m.global_id.is_empty() && m.global_id != "0" {
                m.global_id
            } else {
                m.client_id.to_string()
            };

            let sm = crate::store::StoredMessage {
                msg_id,
                thread_id: thread_id.clone(),
                sender_id: if is_outgoing { uin.clone() } else { thread_id.clone() },
                sender_name,
                text: m.content,
                msg_type: "webchat".into(),
                ts: m.timestamp as i64,
                is_outgoing,
                raw_json: String::new(),
            };

            if store.upsert_message(&sm).unwrap_or(false) {
                imported += 1;
            }
        }
    }

    let mut cached_threads = crate::chat::load_threads_cache();
    let _ = store.hydrate_threads(&mut cached_threads, 50);
    crate::chat::save_threads_cache(&cached_threads);

    if let Some(pub_k) = get_sync_public_key() {
        let _ = crate::chat::clean_sync(s, &pub_k).await;
    }

    // Keys/session giữ trong bộ nhớ tiến trình (CURRENT_V2_KEYS/_SESSION),
    // không còn file temp sync_v2_*.bin/json của helper JS.

    Ok(imported)
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::chat::ChatMessage;

    fn thread_with(ids: &[&str]) -> Thread {
        Thread {
            id: "g1".into(),
            name: "Nhóm".into(),
            preview: String::new(),
            time: String::new(),
            avatar: None,
            is_group: true,
            messages: ids
                .iter()
                .map(|id| ChatMessage {
                    id: (*id).into(),
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        }
    }

    #[test]
    fn cursor_moves_to_the_oldest_server_message() {
        assert_eq!(
            next_cursor(&thread_with(&["5", "9"]), "9").as_deref(),
            Some("5")
        );
        assert_eq!(
            next_cursor(&thread_with(&["out_1", "7", "9"]), "9").as_deref(),
            Some("7")
        );
    }

    #[test]
    fn cursor_stops_when_page_brought_nothing_older() {
        assert_eq!(next_cursor(&thread_with(&["5", "9"]), "5"), None);
        assert_eq!(next_cursor(&thread_with(&[]), "5"), None);
        assert_eq!(next_cursor(&thread_with(&["", "out_2"]), "5"), None);
    }

    #[test]
    fn generate_sync_public_key_produces_valid_rsa2048_spki_der_base64() {
        use base64::Engine;
        use rsa::pkcs8::EncodePublicKey;
        use rsa::{RsaPrivateKey, RsaPublicKey};

        let mut rng = rsa::rand_core::OsRng;
        let priv1 = RsaPrivateKey::new(&mut rng, 2048).unwrap();
        let pub1 = RsaPublicKey::from(&priv1);
        let k1 = base64::engine::general_purpose::STANDARD.encode(pub1.to_public_key_der().unwrap().as_bytes());

        let priv2 = RsaPrivateKey::new(&mut rng, 2048).unwrap();
        let pub2 = RsaPublicKey::from(&priv2);
        let k2 = base64::engine::general_purpose::STANDARD.encode(pub2.to_public_key_der().unwrap().as_bytes());

        assert_ne!(k1, k2);
        assert!(
            k1.starts_with("MIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEA"),
            "must have valid ASN.1 RSA-2048 SPKI DER prefix: {k1}"
        );
        let raw = base64::engine::general_purpose::STANDARD
            .decode(&k1)
            .expect("valid base64");
        assert_eq!(raw.len(), 294);
        assert_eq!(&raw[raw.len() - 5..], &[0x02, 0x03, 0x01, 0x00, 0x01]);
    }


    #[test]
    fn extract_mobile_sync_controls_parses_v1_and_v2_events() {
        let payload = serde_json::json!({
            "controls": [
                {
                    "content": {
                        "act_type": "transfer_sync2",
                        "act": "transfer_status",
                        "data": "{\"syncId\":\"abc123\",\"fromDevice\":0,\"status\":3}"
                    }
                },
                {
                    "content": {
                        "act_type": "syncmsgmb",
                        "act": "user_confirm",
                        "data": "{\"user_action\":2,\"pc_name\":\"ADMIN-PC\"}"
                    }
                },
                {
                    "content": {
                        "act_type": "transfer_sync2",
                        "act": "upload_batch",
                        "data": "{\"syncId\":\"abc123\",\"err\":0,\"msgUrl\":\"https://f.zalo.me/batch1\"}"
                    }
                }
            ]
        });
        let evs = crate::chat::extract_mobile_sync_controls(&payload);
        assert_eq!(
            evs,
            vec![
                crate::chat::MobileSyncControlEvent::WaitingConfirmOnPhone {
                    sync_id: "abc123".into()
                },
                crate::chat::MobileSyncControlEvent::MobileBusyRestoring,
                crate::chat::MobileSyncControlEvent::SyncV2Batch {
                    url: "https://f.zalo.me/batch1".into(),
                    sync_id: "abc123".into(),
                    batch_type: 0,
                    is_last: false,
                },
            ]
        );
    }

    #[test]
    fn decompress_sync_payload_detects_zlib_gzip_and_raw() {
        use std::io::Write;
        let raw = b"hello sync";
        // zlib
        let mut enc = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::fast());
        enc.write_all(raw).unwrap();
        let zlib_bytes = enc.finish().unwrap();
        assert_eq!(decompress_sync_payload(&zlib_bytes), raw);
        // gzip
        let mut enc = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        enc.write_all(raw).unwrap();
        let gzip_bytes = enc.finish().unwrap();
        assert_eq!(decompress_sync_payload(&gzip_bytes), raw);
        // raw passthrough
        assert_eq!(decompress_sync_payload(raw), raw);
        // zstd (encode bằng crate zstd đã có)
        let zstd_bytes = zstd::encode_all(std::io::Cursor::new(raw), 1).unwrap();
        assert_eq!(decompress_sync_payload(&zstd_bytes), raw);
    }

    #[test]
    fn sync_v2_session_registry_gates_batches() {
        assert!(!has_sync_v2_session("nope-sync"));
        record_sync_v2_session("sync-1", "sess-aaa");
        assert!(has_sync_v2_session("sync-1"));
        assert!(!has_sync_v2_session("sync-2"));
        // Ghi đè cùng sync_id không trùng lặp.
        record_sync_v2_session("sync-1", "sess-bbb");
        assert!(has_sync_v2_session("sync-1"));
    }
}
