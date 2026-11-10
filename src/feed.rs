//! Social Feed (Nhật ký) & Social Profile module for Zalo PC
//! -----------------------------------------------------------
//! Cung cấp cấu trúc dữ liệu và các API tương tác mạng xã hội:
//! - Lấy hồ sơ chi tiết bạn bè (Ảnh bìa Cover HD, Avatar, Ngày sinh, Bio)
//! - Lấy kho ảnh Nhật ký cá nhân (CDN zpsocial)
//! - Đóng gói binary packet cho Socket Command 50001 (Get Feed / Post Feed)

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedAuthor {
    pub uid: String,
    pub display_name: String,
    pub avatar_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedMedia {
    pub photo_id: String,
    pub media_type: String, // "image" | "video"
    pub url: String,
    pub thumbnail: String,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedReaction {
    pub like_count: u32,
    pub comment_count: u32,
    pub is_liked: bool,
    pub top_reactions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedComment {
    pub comment_id: String,
    pub author_name: String,
    pub author_uid: String,
    pub content: String,
    pub timestamp_str: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedItem {
    pub feed_id: String,
    pub author: FeedAuthor,
    pub date_str: String,
    pub content: String,
    pub status_badge: Option<String>,
    pub media_list: Vec<FeedMedia>,
    pub reaction: FeedReaction,
    pub comments: Vec<FeedComment>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SocialUserProfile {
    pub uid: String,
    pub username: String,
    pub display_name: String,
    pub zalo_name: String,
    pub avatar_url: String,
    pub cover_url: String,
    pub gender: i32,
    pub sdob: String,
    pub status: String,
    pub is_friend: bool,
    pub last_action_time: Option<u64>,
}

impl SocialUserProfile {
    pub fn from_json(uid: &str, val: &Value) -> Option<Self> {
        let obj = val.as_object()?;
        let display_name = obj
            .get("displayName")
            .and_then(Value::as_str)
            .unwrap_or(uid)
            .to_string();
        let zalo_name = obj
            .get("zaloName")
            .and_then(Value::as_str)
            .unwrap_or(&display_name)
            .to_string();
        let username = obj
            .get("username")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let avatar_url = obj
            .get("avatar")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let cover_url = obj
            .get("cover")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let gender = obj.get("gender").and_then(Value::as_i64).unwrap_or(0) as i32;
        let sdob = obj
            .get("sdob")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let status = obj
            .get("status")
            .or_else(|| obj.get("statusMsg"))
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let is_friend = obj.get("isFr").and_then(Value::as_i64).unwrap_or(0) == 1;
        let last_action_time = obj.get("lastActionTime").and_then(Value::as_u64);

        Some(Self {
            uid: uid.to_string(),
            username,
            display_name,
            zalo_name,
            avatar_url,
            cover_url,
            gender,
            sdob,
            status,
            is_friend,
            last_action_time,
        })
    }
}

/// Tiện ích đóng gói nhị phân cho Socket Command 50001 (Feed)
pub mod packet {
    pub const CMD_FEED_TIMELINE: u16 = 50001;
    pub const SUBCMD_GET_FEED: u32 = 0x00150102;
    pub const SUBCMD_POST_FEED: u32 = 0x00220411;
    pub const SUBCMD_EDIT_FEED: u32 = 0x26580022;

    /// Tạo payload nhị phân cho lệnh GET_FEED (lấy dòng thời gian)
    pub fn build_get_feed_payload(
        owner_id: u64,
        viewer_id: u64,
        tab: u32,
        page: u32,
        count: u32,
    ) -> Vec<u8> {
        let mut buf = Vec::with_capacity(50);
        buf.push(0x01); // header_magic
        buf.extend_from_slice(&2u32.to_be_bytes()); // feed_version = 2
        buf.extend_from_slice(&tab.to_be_bytes());
        buf.extend_from_slice(&page.to_be_bytes());
        buf.extend_from_slice(&count.to_be_bytes());
        buf.extend_from_slice(&0u64.to_be_bytes()); // last_feed_id_server
        buf.extend_from_slice(&0u64.to_be_bytes()); // last_feed_id_local
        buf.extend_from_slice(&owner_id.to_be_bytes());
        buf.extend_from_slice(&viewer_id.to_be_bytes());
        buf.extend_from_slice(&0u16.to_be_bytes()); // filter_flags
        buf
    }

    /// Đóng gói payload vào khung RequestPacket
    pub fn build_request_packet(cmd: u16, subcmd: u32, payload: &[u8]) -> Vec<u8> {
        let body_len = 2 + 4 + payload.len();
        let total_len = body_len as u32;
        let mut packet = Vec::with_capacity(4 + body_len);
        packet.extend_from_slice(&total_len.to_be_bytes());
        packet.extend_from_slice(&cmd.to_be_bytes());
        packet.extend_from_slice(&subcmd.to_be_bytes());
        packet.extend_from_slice(payload);
        packet
    }
}

// Mở rộng các phương thức mạng xã hội cho Session
impl crate::session::Session {
    /// Lấy hồ sơ chi tiết bạn bè (Ảnh bìa Cover HD, Avatar, Ngày sinh, Giới tính, Bio)
    pub async fn get_friend_social_profile(&self, target_uid: &str) -> Result<SocialUserProfile> {
        let body = json!({
            "avatar_size": 240,
            "friend_ids": [target_uid],
            "imei": self.imei,
        });

        let enc = self.encoded_params(&body)?;
        let url = format!(
            "{}/api/social/friend/getprofiles?{}&params={enc}&os=8",
            self.domains.profile,
            self.common_params()
        );

        let resp = self.get(&url).await?;
        let dec = self.maybe_decrypt(resp, self.session_key_bytes()).await;

        if dec.get("error_code").and_then(Value::as_i64) == Some(0) {
            if let Some(prof_obj) = dec.get("data").and_then(|d| d.get(target_uid)) {
                return SocialUserProfile::from_json(target_uid, prof_obj)
                    .ok_or_else(|| anyhow!("Failed to parse profile JSON for {target_uid}"));
            }
        }

        Err(anyhow!(
            "Server returned error or missing data for uid {target_uid}: {dec}"
        ))
    }

    /// Lấy kho ảnh Nhật ký cá nhân (CDN zpsocial)
    pub async fn get_my_social_avatar_list(&self, page: u32, count: u32) -> Result<Vec<FeedMedia>> {
        let body = json!({
            "page": page,
            "count": count,
            "imei": self.imei,
        });

        let enc = self.encoded_params(&body)?;
        let url = format!(
            "{}/api/social/avatar-list?{}&params={enc}&os=8",
            self.domains.profile,
            self.common_params()
        );

        let resp = self.get(&url).await?;
        let dec = self.maybe_decrypt(resp, self.session_key_bytes()).await;

        let mut photos = Vec::new();
        if let Some(photo_arr) = dec
            .get("data")
            .and_then(|d| d.get("photos"))
            .and_then(Value::as_array)
        {
            for p in photo_arr {
                let photo_id = p
                    .get("photoId")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                let url = p
                    .get("url")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                let thumbnail = p
                    .get("thumbnail")
                    .and_then(Value::as_str)
                    .unwrap_or(&url)
                    .to_string();
                if !url.is_empty() {
                    photos.push(FeedMedia {
                        photo_id,
                        media_type: "image".to_string(),
                        url,
                        thumbnail,
                        width: 0,
                        height: 0,
                    });
                }
            }
        }

        Ok(photos)
    }

    /// Lấy hàng loạt hồ sơ mạng xã hội của nhiều bạn bè (`POST /api/social/friend/getprofiles/v2`)
    pub async fn get_friend_social_profiles_batch(
        &self,
        target_uids: &[String],
    ) -> Result<Vec<SocialUserProfile>> {
        if target_uids.is_empty() {
            return Ok(Vec::new());
        }
        let pversion_map: Vec<String> = target_uids.iter().map(|u| format!("{u}_0")).collect();
        let body = json!({
            "phonebook_version": 0,
            "friend_pversion_map": pversion_map,
            "avatar_size": 240,
            "language": "vi",
            "show_online_status": 1,
            "imei": self.imei,
        });
        let enc = self.encoded_params(&body)?;
        let url = format!(
            "{}/api/social/friend/getprofiles/v2?{}",
            self.domains.profile,
            self.common_params()
        );
        let form = format!("params={enc}");
        let resp = self.post_form(&url, form).await?;
        let dec = self.maybe_decrypt(resp, self.session_key_bytes()).await;

        let mut profiles = Vec::new();
        if let Some(changed) = dec
            .get("data")
            .and_then(|d| d.get("changed_profiles").or(Some(d)))
            .and_then(Value::as_object)
        {
            for (uid, pval) in changed {
                if let Some(p) = SocialUserProfile::from_json(uid, pval) {
                    profiles.push(p);
                }
            }
        }
        Ok(profiles)
    }

    /// Tổng hợp Dòng thời gian (Nhật ký) của bạn bè + kho ảnh cá nhân thành danh sách `FeedItem`
    pub async fn fetch_timeline_feed(
        &self,
        friend_uids: &[String],
        _shared_media_by_uid: &std::collections::HashMap<String, Vec<FeedMedia>>,
    ) -> Result<(Vec<SocialUserProfile>, Vec<FeedItem>)> {
        let mut items: Vec<FeedItem> = Vec::new();

        // 1. Lấy hồ sơ cá nhân & kho ảnh đại diện/nhật ký của chính mình
        let my_uid = self
            .login
            .as_ref()
            .map(|l| l.uin.clone())
            .unwrap_or_default();
        let my_name = self
            .login
            .as_ref()
            .map(|l| l.display_name.clone())
            .unwrap_or_else(|| "Tôi".to_string());
        let my_ava = self
            .login
            .as_ref()
            .map(|l| l.avatar.clone())
            .unwrap_or_default();

        if let Ok(my_photos) = self.get_my_social_avatar_list(1, 20).await {
            if !my_photos.is_empty() {
                items.push(FeedItem {
                    feed_id: format!("my_album_{my_uid}"),
                    author: FeedAuthor {
                        uid: my_uid.clone(),
                        display_name: my_name,
                        avatar_url: my_ava,
                    },
                    date_str: "Kho ảnh cá nhân".to_string(),
                    content: format!("Album ảnh đại diện mạng xã hội ({} ảnh)", my_photos.len()),
                    status_badge: Some("Album ảnh".to_string()),
                    media_list: my_photos,
                    reaction: FeedReaction {
                        like_count: 0,
                        comment_count: 0,
                        is_liked: false,
                        top_reactions: Vec::new(),
                    },
                    comments: Vec::new(),
                });
            }
        }

        // 2. Lấy hàng loạt hồ sơ mạng xã hội của bạn bè (tối đa 40 người đầu tiên)
        let batch_ids: Vec<String> = friend_uids
            .iter()
            .filter(|u| !u.is_empty() && *u != &my_uid)
            .take(40)
            .cloned()
            .collect();
        let mut profiles = self
            .get_friend_social_profiles_batch(&batch_ids)
            .await
            .unwrap_or_default();

        // Fallback: nếu getprofiles/v2 trả rỗng thì gọi get_friend_social_profile cho 8 bạn đầu
        if profiles.is_empty() {
            for uid in batch_ids.iter().take(8) {
                if let Ok(p) = self.get_friend_social_profile(uid).await {
                    profiles.push(p);
                }
            }
        }

        // Sắp xếp theo thời gian hoạt động gần nhất
        profiles.sort_by(|a, b| {
            b.last_action_time
                .unwrap_or(0)
                .cmp(&a.last_action_time.unwrap_or(0))
        });

        // 3. Nạp bài đăng Nhật ký từ cơ sở dữ liệu tin nhắn/nhật ký của chính ctdozlite
        // Tuyệt đối không đọc đường dẫn cứng trong test_lab/ để không làm sai lệch dữ liệu của người dùng khác.
        let local_db =
            std::env::var("ZALO_TIMELINE_DB").unwrap_or_else(|_| "zalo_history.db".to_string());
        if std::path::Path::new(&local_db).exists() {
            if let Ok(mut real_feeds) = load_real_timeline_from_sqlite(&local_db, "") {
                if !real_feeds.is_empty() {
                    for rf in &mut real_feeds {
                        if let Some(prof) = profiles.iter().find(|p| {
                            p.display_name
                                .trim()
                                .eq_ignore_ascii_case(rf.author.display_name.trim())
                                || p.zalo_name
                                    .trim()
                                    .eq_ignore_ascii_case(rf.author.display_name.trim())
                        }) {
                            rf.author.uid = prof.uid.clone();
                            if !prof.avatar_url.is_empty() {
                                rf.author.avatar_url = prof.avatar_url.clone();
                            }
                        }
                    }
                    items.append(&mut real_feeds);
                }
            }
        }

        // 4. Tuyệt đối không tự bịa bài viết hoặc tự tạo thẻ giả lập.
        // Trả về đúng danh sách hồ sơ bạn bè và bài viết thực sự có trong database của user hiện tại.
        Ok((profiles, items))
    }
}

/// Nạp dữ liệu bài viết thật từ bảng `timeline_feed_1` trong cơ sở dữ liệu SQLite cục bộ
pub fn load_real_timeline_from_sqlite(db_path: &str, target_uid: &str) -> Result<Vec<FeedItem>> {
    let conn = rusqlite::Connection::open(db_path)?;
    crate::store::configure_connection(&conn)?;
    // Kiểm tra xem bảng timeline_feed_1 có tồn tại trong database này không
    let table_exists: bool = conn
        .query_row(
            "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='timeline_feed_1'",
            [],
            |row| row.get(0),
        )
        .map(|count: i64| count > 0)
        .unwrap_or(false);

    if !table_exists {
        return Ok(Vec::new());
    }

    let mut stmt = conn.prepare("SELECT content FROM timeline_feed_1")?;
    let rows = stmt.query_map([], |row| {
        let content_str: String = row.get(0)?;
        Ok(content_str)
    })?;

    let mut feeds = Vec::new();
    for r in rows {
        let content_str = r?;
        if let Ok(val) = serde_json::from_str::<Value>(&content_str) {
            if let Some(item) = val.get("item") {
                let author_uid = item
                    .get("header")
                    .and_then(|h| h.get("uid"))
                    .and_then(Value::as_str)
                    .unwrap_or("");
                let author_dpn = item
                    .get("header")
                    .and_then(|h| h.get("dpn"))
                    .and_then(Value::as_str)
                    .unwrap_or("");

                // Kiểm tra khớp UID hoặc khớp tên nếu có bộ lọc target_uid
                let matches_target = target_uid.is_empty()
                    || author_uid == target_uid
                    || author_dpn.trim().eq_ignore_ascii_case(target_uid.trim());

                if matches_target {
                    let fid = item
                        .get("fid")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string();
                    let cts = item.get("cts").and_then(Value::as_i64).unwrap_or(0);
                    let date_str = chrono::DateTime::from_timestamp_millis(cts)
                        .map(|dt| dt.format("%d/%m/%Y lúc %H:%M:%S").to_string())
                        .unwrap_or_else(String::new);

                    let header = item.get("header").cloned().unwrap_or(json!({}));
                    let author = FeedAuthor {
                        uid: author_uid.to_string(),
                        display_name: author_dpn.to_string(),
                        avatar_url: header
                            .get("avt")
                            .and_then(Value::as_str)
                            .unwrap_or("")
                            .to_string(),
                    };

                    let content_obj = item.get("content").cloned().unwrap_or(json!({}));
                    let caption = content_obj
                        .get("caption")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string();

                    let mut media_list = Vec::new();
                    if let Some(photo) = content_obj.get("photo") {
                        let photoid = photo
                            .get("photoid")
                            .and_then(Value::as_str)
                            .unwrap_or("")
                            .to_string();
                        let origin = photo
                            .get("origin")
                            .and_then(Value::as_str)
                            .unwrap_or("")
                            .to_string();
                        if !origin.is_empty() {
                            let origin_jpg = if origin.ends_with(".jxl") {
                                format!("{}.jpg", &origin[..origin.len() - 4])
                            } else {
                                origin
                            };
                            let dim = content_obj
                                .get("photo_dimension")
                                .cloned()
                                .unwrap_or(json!({}));
                            let width =
                                dim.get("width").and_then(Value::as_u64).unwrap_or(0) as u32;
                            let height =
                                dim.get("height").and_then(Value::as_u64).unwrap_or(0) as u32;
                            media_list.push(FeedMedia {
                                photo_id: photoid,
                                media_type: "image".to_string(),
                                url: origin_jpg.clone(),
                                thumbnail: origin_jpg,
                                width,
                                height,
                            });
                        }
                    }

                    let footer = item.get("footer").cloned().unwrap_or(json!({}));
                    let numlike = footer.get("numlike").and_then(Value::as_u64).unwrap_or(0) as u32;
                    let numcmt = footer.get("numcmt").and_then(Value::as_u64).unwrap_or(0) as u32;

                    feeds.push(FeedItem {
                        feed_id: fid,
                        author,
                        date_str,
                        content: caption,
                        status_badge: None,
                        media_list,
                        reaction: FeedReaction {
                            like_count: numlike,
                            comment_count: numcmt,
                            is_liked: false,
                            top_reactions: vec!["Thích".to_string()],
                        },
                        comments: Vec::new(),
                    });
                }
            }
        }
    }

    // Sắp xếp bài đăng mới nhất (cts lớn nhất) lên đầu danh sách
    feeds.sort_by(|a, b| b.feed_id.cmp(&a.feed_id));

    Ok(feeds)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn social_profile_and_packet_50001_encode_correctly() {
        let payload = packet::build_get_feed_payload(435550501, 435550501, 0, 1, 20);
        assert_eq!(payload.len(), 51);
        assert_eq!(payload[0], 0x01);
        let pkt = packet::build_request_packet(
            packet::CMD_FEED_TIMELINE,
            packet::SUBCMD_GET_FEED,
            &payload,
        );
        assert_eq!(pkt.len(), 4 + 2 + 4 + 51);
        assert_eq!(u16::from_be_bytes([pkt[4], pkt[5]]), 50001);

        let prof_val = json!({
            "displayName": "Nguyễn Thành",
            "zaloName": "Thanh",
            "avatar": "https://s120.ava.talk.zdn.vn/a.jpg",
            "cover": "https://cover-talk.zadn.vn/c.jpg",
            "gender": 0,
            "sdob": "01/01/2000",
            "status": "Zalo Rust",
            "isFr": 1
        });
        let p = SocialUserProfile::from_json("435550501", &prof_val).unwrap();
        assert_eq!(p.display_name, "Nguyễn Thành");
        assert!(p.is_friend);
        assert_eq!(p.cover_url, "https://cover-talk.zadn.vn/c.jpg");
    }

    #[test]
    fn load_real_dtc_timeline_from_sqlite_works() {
        let db_path = "test_lab/databases/zalo";
        if std::path::Path::new(db_path).exists() {
            let feeds = load_real_timeline_from_sqlite(db_path, "257404931").unwrap();
            assert_eq!(feeds.len(), 2, "Phải nạp được đúng 2 bài viết thật của Đtc");

            // Kiểm tra bài đăng 1 (FPT University)
            let f1 = feeds
                .iter()
                .find(|f| f.feed_id == "18223066329")
                .expect("Phải có feed 18223066329");
            assert_eq!(f1.author.display_name, "Đtc");
            assert_eq!(f1.reaction.like_count, 16);
            assert_eq!(f1.reaction.comment_count, 6);
            assert!(f1.content.contains("FPT University"));
            assert!(!f1.media_list.is_empty());
            assert!(f1.media_list[0].url.ends_with(".jpg"));

            // Kiểm tra bài đăng 2 (Bão lũ an toàn)
            let f2 = feeds
                .iter()
                .find(|f| f.feed_id == "18193461996")
                .expect("Phải có feed 18193461996");
            assert_eq!(f2.author.display_name, "Đtc");
            assert_eq!(f2.reaction.like_count, 6);
            assert_eq!(f2.reaction.comment_count, 0);
            assert!(f2.content.contains("an toàn"));
            assert!(!f2.media_list.is_empty());
            assert!(f2.media_list[0].url.ends_with(".jpg"));
        }
    }
}
