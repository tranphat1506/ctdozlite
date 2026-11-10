//! Multi-account management for ctdozlite.
//!
//! Stores isolated account sessions, thread caches, and history databases in `accounts/<uin>.*`.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AccountProfile {
    pub uin: String,
    pub display_name: String,
    pub avatar: Option<String>,
    pub last_active_ts: i64,
    pub is_current: bool,
}

pub fn accounts_dir() -> PathBuf {
    PathBuf::from("accounts")
}

/// List all saved accounts in the `accounts/` directory.
pub fn list_saved_accounts(current_uin: &str) -> Vec<AccountProfile> {
    let dir = accounts_dir();
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return out;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) == Some("json") {
            let file_stem = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or_default();
            if file_stem.ends_with("_threads") {
                continue;
            }
            if let Ok(text) = std::fs::read_to_string(&path) {
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(&text) {
                    if let Some(login) = val.get("login") {
                        let uin = login
                            .get("uin")
                            .and_then(|v| v.as_str())
                            .unwrap_or_default()
                            .to_string();
                        if !uin.is_empty() {
                            let display_name = login
                                .get("display_name")
                                .and_then(|v| v.as_str())
                                .unwrap_or("Người dùng Zalo")
                                .to_string();
                            let avatar = login
                                .get("avatar")
                                .and_then(|v| v.as_str())
                                .map(str::to_string);
                            let is_current = uin == current_uin;
                            let last_active_ts = entry
                                .metadata()
                                .and_then(|m| m.modified())
                                .ok()
                                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                                .map(|d| d.as_secs() as i64)
                                .unwrap_or(0);
                            out.push(AccountProfile {
                                uin,
                                display_name,
                                avatar,
                                last_active_ts,
                                is_current,
                            });
                        }
                    }
                }
            }
        }
    }
    out.sort_by(|a, b| b.last_active_ts.cmp(&a.last_active_ts));
    out
}

/// Save the current session and caches into `accounts/<uin>.*`
pub fn save_current_account_snapshot(uin: &str) -> Result<()> {
    if uin.is_empty() {
        return Ok(());
    }
    let dir = accounts_dir();
    std::fs::create_dir_all(&dir)?;
    if Path::new("session.json").exists() {
        let _ = std::fs::copy("session.json", dir.join(format!("{uin}.json")));
    }
    if Path::new("threads_cache.json").exists() {
        let _ = std::fs::copy(
            "threads_cache.json",
            dir.join(format!("{uin}_threads.json")),
        );
    }
    if Path::new("zalo_history.db").exists() {
        let _ = std::fs::copy("zalo_history.db", dir.join(format!("{uin}_history.db")));
    }
    Ok(())
}

/// Switch to a target account:
/// 1. Saves current account snapshot to accounts/<current_uin>.*
/// 2. Restores accounts/<target_uin>.* into session.json, threads_cache.json, zalo_history.db
pub fn switch_to_account(current_uin: &str, target_uin: &str) -> Result<()> {
    if target_uin.is_empty() {
        anyhow::bail!("Target UIN is empty");
    }
    let dir = accounts_dir();
    if !current_uin.is_empty() {
        let _ = save_current_account_snapshot(current_uin);
    }
    let _ = std::fs::remove_file("session.json");
    let _ = std::fs::remove_file("threads_cache.json");
    let _ = std::fs::remove_file("zalo_history.db");

    let target_session = dir.join(format!("{target_uin}.json"));
    if !target_session.exists() {
        anyhow::bail!("Không tìm thấy dữ liệu phiên của tài khoản {target_uin}");
    }
    std::fs::copy(&target_session, "session.json")?;

    let target_threads = dir.join(format!("{target_uin}_threads.json"));
    if target_threads.exists() {
        let _ = std::fs::copy(&target_threads, "threads_cache.json");
    }

    let target_db = dir.join(format!("{target_uin}_history.db"));
    if target_db.exists() {
        let _ = std::fs::copy(&target_db, "zalo_history.db");
    }
    Ok(())
}

/// Delete a saved account from `accounts/`
pub fn delete_saved_account(uin: &str) -> Result<()> {
    if uin.is_empty() {
        return Ok(());
    }
    let dir = accounts_dir();
    let _ = std::fs::remove_file(dir.join(format!("{uin}.json")));
    let _ = std::fs::remove_file(dir.join(format!("{uin}_threads.json")));
    let _ = std::fs::remove_file(dir.join(format!("{uin}_history.db")));
    Ok(())
}
