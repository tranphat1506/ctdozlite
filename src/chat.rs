//! Chat data: friend/conversation list + last messages, all over REST.
//!
//! Notes: `re-notes/02_chat_data.md`.

use anyhow::Result;
use serde_json::{json, Value};

use crate::session::{obj, Session};

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct ChatMessage {
    #[serde(default)]
    pub kind: crate::message_model::MessageKind,
    #[serde(default)]
    pub wire_ref: crate::message_model::WireMessageRef,
    #[serde(default)]
    pub unread_policy: crate::message_model::UnreadPolicy,
    #[serde(default)]
    pub delivery: crate::message_model::DeliveryState,
    pub id: String,
    pub sender: String,
    pub text: String,
    pub time: String,
    pub is_outgoing: bool,
    pub ts: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sticker_id: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sticker_cate_id: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sticker_type: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reply_to_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reply_to_sender: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reply_to_text: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reactions: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reaction_summaries: Vec<crate::reactions::ReactionSummary>,
    #[serde(default)]
    pub reaction_snapshot: bool,
    #[serde(default)]
    pub reaction_authoritative_snapshot: bool,
    #[serde(skip)]
    pub reaction_snapshot_fresh: bool,
    #[serde(default)]
    pub reaction_snapshot_provenance: crate::reactions::ReactionSnapshotProvenance,
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub reaction_versions: std::collections::BTreeMap<String, crate::reactions::ReactionVersion>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub client_message_id: String,
    #[serde(default)]
    pub is_pinned: bool,
    /// Group-board topic id backing a remote pin (`/api/board/topic/createv2`), needed to unpin.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub board_topic_id: String,
    #[serde(default)]
    pub is_starred: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub link_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub link_title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub link_desc: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub link_thumb: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub link_src: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub history_source: Option<Value>,
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub resolved_mention_uids: std::collections::BTreeMap<String, String>,
}

impl ChatMessage {
    pub fn merge_message_metadata(&mut self, incoming: &Self) -> bool {
        let previous_mentions = self.resolved_mention_uids.clone();
        self.resolved_mention_uids
            .extend(incoming.resolved_mention_uids.clone());
        let before = (
            self.kind,
            self.wire_ref.clone(),
            self.unread_policy.clone(),
            self.delivery,
            self.history_source.clone(),
        );
        if incoming.kind != crate::message_model::MessageKind::Unknown {
            self.kind = incoming.kind;
        }
        if !incoming.wire_ref.fields.is_empty() {
            let mut fields = self.wire_ref.fields.clone();
            fields.extend(incoming.wire_ref.fields.clone());
            self.wire_ref = crate::message_model::WireMessageRef::from_raw(&Value::Object(
                fields.into_iter().collect(),
            ));
            if incoming.unread_policy.params_ext_present {
                self.unread_policy = incoming.unread_policy.clone();
            }
        }
        if incoming.history_source.is_some() {
            self.history_source = incoming.history_source.clone();
        }
        self.delivery = self.delivery.merge(incoming.delivery);
        previous_mentions != self.resolved_mention_uids
            || before
                != (
                    self.kind,
                    self.wire_ref.clone(),
                    self.unread_policy.clone(),
                    self.delivery,
                    self.history_source.clone(),
                )
    }

    /// Keep aggregate counts and event replay cursors across cache/network merges.
    pub fn merge_reaction_metadata(&mut self, incoming: &Self) -> bool {
        let before_total = crate::reactions::total_count(&self.reaction_summaries);
        let changed = self.merge_reaction_metadata_inner(incoming);
        crate::reactions::trace_count_change(
            &format!(
                "merge_reaction_metadata incoming_total={} incoming_snapshot={} incoming_authoritative={} incoming_versions={}",
                crate::reactions::total_count(&incoming.reaction_summaries),
                incoming.reaction_snapshot,
                incoming.reaction_authoritative_snapshot,
                incoming.reaction_versions.len()
            ),
            &self.id,
            before_total,
            crate::reactions::total_count(&self.reaction_summaries),
        );
        changed
    }

    fn merge_reaction_metadata_inner(&mut self, incoming: &Self) -> bool {
        let model_changed = self.merge_message_metadata(incoming);
        let previous_provenance = self.reaction_snapshot_provenance.clone();
        let before = (
            self.reaction_summaries.clone(),
            self.reactions.clone(),
            self.client_message_id.clone(),
            self.reaction_snapshot,
            self.reaction_authoritative_snapshot,
            self.reaction_versions.clone(),
        );
        if self.client_message_id.is_empty() {
            self.client_message_id = incoming.client_message_id.clone();
        }
        let accept_snapshot = incoming.reaction_authoritative_snapshot
            && (incoming.reaction_snapshot_fresh || !self.reaction_authoritative_snapshot)
            && self.reaction_snapshot_provenance.accepts_snapshot(
                &incoming.reaction_snapshot_provenance,
                &self.reaction_versions,
                incoming.reaction_snapshot_fresh,
            );

        if accept_snapshot {
            self.reaction_snapshot_provenance = incoming.reaction_snapshot_provenance.clone();
            self.reaction_summaries = incoming.reaction_summaries.clone();
            self.reaction_authoritative_snapshot = true;
            self.reaction_snapshot = true;
            for (user, version) in &mut self.reaction_versions {
                version.covered_events.append(&mut version.events);
                version.baseline = incoming
                    .reaction_summaries
                    .iter()
                    .filter_map(|s| Some((s.r_type?, *s.reactor_counts.get(user)?)))
                    .collect();
                version.ledger_initialized = true;
                version.snapshot_baseline = true;
            }
            for (user, incoming_version) in &incoming.reaction_versions {
                let covered = self
                    .reaction_versions
                    .get(user)
                    .map(|v| v.covered_events.clone())
                    .unwrap_or_default();
                let mut version = incoming_version.clone();
                version.covered_events.extend(covered);
                if incoming.reaction_snapshot_fresh {
                    version.covered_events.append(&mut version.events);
                    version.baseline = incoming
                        .reaction_summaries
                        .iter()
                        .filter_map(|s| Some((s.r_type?, *s.reactor_counts.get(user)?)))
                        .collect();
                    version.ledger_initialized = true;
                    version.snapshot_baseline = true;
                }
                self.reaction_versions.insert(user.clone(), version);
            }
        }
        if !self.reaction_authoritative_snapshot
            && !incoming.reaction_authoritative_snapshot
            && (incoming.reaction_snapshot || !incoming.reaction_summaries.is_empty())
            && self.reaction_versions.is_empty()
        {
            self.reaction_summaries = incoming.reaction_summaries.clone();
            self.reactions = crate::reactions::legacy_emojis(&self.reaction_summaries);
            self.reaction_snapshot = incoming.reaction_snapshot;
        }
        if !self.reaction_authoritative_snapshot
            && !incoming.reaction_authoritative_snapshot
            && !self.reaction_versions.is_empty()
            && (incoming.reaction_snapshot || !incoming.reaction_summaries.is_empty())
        {
            crate::reactions::merge_untracked_snapshot(
                &mut self.reaction_summaries,
                &incoming.reaction_summaries,
                &self.reaction_versions,
                incoming.reaction_snapshot && incoming.reaction_versions.is_empty(),
            );
        }
        for (user, version) in &incoming.reaction_versions {
            if accept_snapshot {
                continue;
            }
            if !self.reaction_versions.contains_key(user) {
                if self.reaction_authoritative_snapshot {
                    for event in &version.events {
                        if event.r_type >= -1 {
                            self.apply_reaction(event);
                        }
                    }
                    continue;
                }
                self.reaction_versions.insert(user.clone(), version.clone());
                if version.ledger_initialized {
                    version.rebuild(&mut self.reaction_summaries, user);
                }
            } else {
                for event in &version.events {
                    if event.r_type >= -1 {
                        self.apply_reaction(event);
                    }
                }
            }
        }
        if !self.reaction_summaries.is_empty()
            || incoming.reaction_snapshot
            || !incoming.reaction_versions.is_empty()
        {
            self.reactions = crate::reactions::legacy_emojis(&self.reaction_summaries);
        } else if self.reactions.is_empty()
            && self.reaction_versions.is_empty()
            && !self.reaction_snapshot
        {
            self.reactions = incoming.reactions.clone();
        }
        model_changed
            || previous_provenance != self.reaction_snapshot_provenance
            || before
                != (
                    self.reaction_summaries.clone(),
                    self.reactions.clone(),
                    self.client_message_id.clone(),
                    self.reaction_snapshot,
                    self.reaction_authoritative_snapshot,
                    self.reaction_versions.clone(),
                )
    }

    pub fn apply_reaction(&mut self, event: &crate::reactions::ReactionEvent) -> bool {
        let before_total = crate::reactions::total_count(&self.reaction_summaries);
        let changed = self.apply_reaction_inner(event);
        crate::reactions::trace_count_change(
            &format!(
                "apply_reaction event={} sender={}",
                event.event_id, event.sender_id
            ),
            &self.id,
            before_total,
            crate::reactions::total_count(&self.reaction_summaries),
        );
        changed
    }

    fn apply_reaction_inner(&mut self, event: &crate::reactions::ReactionEvent) -> bool {
        if event.sender_id.is_empty() || event.r_type < -1 {
            return false;
        }
        if self.reaction_summaries.is_empty() && !self.reactions.is_empty() {
            self.reaction_summaries =
                crate::reactions::parse_reactions(&json!({"reactions":self.reactions}));
        }
        let changed = self
            .reaction_versions
            .entry(event.sender_id.clone())
            .or_default()
            .record(
                &mut self.reaction_summaries,
                event,
                self.reaction_authoritative_snapshot,
            );
        self.reactions = crate::reactions::legacy_emojis(&self.reaction_summaries);
        self.reaction_snapshot = true;
        changed
    }

    /// Repair cached media metadata without changing captions or signed URL bytes.
    pub fn hydrate_media_metadata(&mut self) -> bool {
        if self.text == "[Tin nhắn đã thu hồi]" {
            return false;
        }
        let before = (
            self.link_url.clone(),
            self.link_thumb.clone(),
            self.link_src.clone(),
        );
        let source_kind = self.history_source.as_ref().and_then(|source| {
            let kind = source
                .get("msgType")
                .and_then(|v| v.as_i64().or_else(|| v.as_str()?.parse().ok()));
            match kind {
                Some(2..=4) => Some("Hình ảnh Zalo"),
                Some(19) => Some("Video Zalo"),
                _ => None,
            }
        });
        if let Some(source) = &self.history_source {
            if let Some(attachment) = source
                .get("meta")
                .and_then(|m| m.get("attachsList"))
                .and_then(Value::as_array)
                .and_then(|a| a.first())
            {
                let (url, _, _, thumb, _) = extract_link_info(attachment);
                if self.link_url.is_none() {
                    self.link_url = url;
                }
                if self.link_thumb.is_none() {
                    self.link_thumb = thumb;
                }
            }
        }
        let label_kind = match self.text.trim() {
            "[Photo]" | "[Hình ảnh]" => Some("Hình ảnh Zalo"),
            "[Video]" => Some("Video Zalo"),
            _ => None,
        };
        if let Some(kind) = source_kind.or(label_kind) {
            self.link_src = Some(kind.into());
            if kind == "Hình ảnh Zalo" && self.link_thumb.is_none() {
                self.link_thumb = self.link_url.clone();
            }
        }
        before
            != (
                self.link_url.clone(),
                self.link_thumb.clone(),
                self.link_src.clone(),
            )
    }

    /// Fill independently missing card fields when an existing URL already exists.
    // The parser layer wires this helper in the dependent protocol change.
    #[allow(dead_code)]
    fn merge_media_metadata(&mut self, incoming: &Self) -> bool {
        if self.text == "[Tin nhắn đã thu hồi]" {
            return false;
        }
        let before = (
            self.link_url.clone(),
            self.link_title.clone(),
            self.link_desc.clone(),
            self.link_thumb.clone(),
            self.link_src.clone(),
        );
        if self.link_title.is_none() {
            self.link_title = incoming.link_title.clone();
        }
        if self.link_desc.is_none() {
            self.link_desc = incoming.link_desc.clone();
        }
        if self.link_thumb.is_none() {
            self.link_thumb = incoming.link_thumb.clone();
        }
        if self.link_src.is_none()
            || matches!(
                incoming.link_src.as_deref(),
                Some("Hình ảnh Zalo" | "Video Zalo")
            )
        {
            self.link_src = incoming.link_src.clone().or_else(|| self.link_src.clone());
        }
        self.hydrate_media_metadata();
        before
            != (
                self.link_url.clone(),
                self.link_title.clone(),
                self.link_desc.clone(),
                self.link_thumb.clone(),
                self.link_src.clone(),
            )
    }

    /// Compute the static PNG URL for a Zalo sticker (`compact-app-pc.b7bec208c4c8f73e09f8.js:7739583` & `18589292`).
    pub fn sticker_image_url(&self) -> Option<String> {
        let sid = self.sticker_id?;
        if sid == 0 {
            return None;
        }
        if self.sticker_cate_id == Some(0) && (1..=30).contains(&sid) {
            Some(format!(
                "https://stc-chat.zdn.vn/images/stickers/default/thumb/{sid}.png"
            ))
        } else {
            Some(format!(
                "https://api.zaloapp.com/api/emoticon/sticker/webpc?eid={sid}&size=130"
            ))
        }
    }

    pub fn sticker_cache_key(&self) -> Option<String> {
        let sid = self.sticker_id?;
        if sid == 0 {
            return None;
        }
        let cid = self.sticker_cate_id.unwrap_or(0);
        Some(format!("stk_{cid}_{sid}"))
    }
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct Thread {
    #[serde(default)]
    pub presentation: SidebarPresentation,
    pub id: String,
    pub name: String,
    pub preview: String,
    pub time: String,
    pub avatar: Option<String>,
    pub is_group: bool,
    pub messages: Vec<ChatMessage>,
    #[serde(default)]
    pub unread: crate::unread::UnreadState,
}

/// Durable sidebar metadata, including metadata-only startup snapshots.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct SidebarPresentation {
    /// Whether the current user is restricted from posting in this group.
    pub posting_restricted: Option<bool>,
    pub members: Vec<SidebarMember>,
    /// Server totalMember only; never inferred from message senders or avatar samples.
    pub member_count: Option<u64>,
    pub last_message_ts: i64,
    pub last_message_id: String,
    pub last_sender: String,
    pub last_sender_id: String,
    pub last_outgoing: bool,
    pub pinned: bool,
    pub muted: bool,
    pub hidden: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct SidebarMember {
    pub id: String,
    pub name: String,
    pub avatar: Option<String>,
    #[serde(default)]
    pub role: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SidebarFontStyle {
    Regular,
    Semibold,
}

impl Thread {
    pub fn sidebar_font_style(&self) -> SidebarFontStyle {
        if self.unread.has_badge() {
            SidebarFontStyle::Semibold
        } else {
            SidebarFontStyle::Regular
        }
    }

    pub fn refresh_sidebar_presentation(&mut self) {
        if let Some(message) = self
            .messages
            .iter()
            .max_by_key(|m| crate::clock::normalize_ts_ms(m.ts))
        {
            let ts = crate::clock::normalize_ts_ms(message.ts);
            if ts >= crate::clock::normalize_ts_ms(self.presentation.last_message_ts) {
                let sender_id = sidebar_sender_id(message)
                    .or_else(|| {
                        (message.id == self.presentation.last_message_id
                            && ts
                                == crate::clock::normalize_ts_ms(self.presentation.last_message_ts))
                        .then(|| self.presentation.last_sender_id.clone())
                    })
                    .unwrap_or_default();
                let keep_resolved = message.id == self.presentation.last_message_id
                    && ts == crate::clock::normalize_ts_ms(self.presentation.last_message_ts)
                    && sender_id == self.presentation.last_sender_id
                    && !self.presentation.last_sender.is_empty()
                    && !self
                        .presentation
                        .last_sender
                        .bytes()
                        .all(|b| b.is_ascii_digit())
                    && message.sender.bytes().all(|b| b.is_ascii_digit());
                self.presentation.last_message_ts = message.ts;
                self.presentation.last_message_id = message.id.clone();
                if !keep_resolved {
                    self.presentation.last_sender = message.sender.clone();
                }
                self.presentation.last_sender_id = sender_id;
                self.presentation.last_outgoing = message.is_outgoing;
            }
        }
    }

    pub fn sidebar_preview(&self) -> String {
        let latest = self
            .messages
            .iter()
            .max_by_key(|m| crate::clock::normalize_ts_ms(m.ts))
            .filter(|m| {
                crate::clock::normalize_ts_ms(m.ts)
                    > crate::clock::normalize_ts_ms(self.presentation.last_message_ts)
                    || self.presentation.last_sender.is_empty()
            });
        let (sender, outgoing) = latest
            .map(|m| (m.sender.as_str(), m.is_outgoing))
            .unwrap_or((
                &self.presentation.last_sender,
                self.presentation.last_outgoing,
            ));
        let sender = self
            .presentation
            .members
            .iter()
            .find(|member| member.id == sender && !member.name.trim().is_empty())
            .map(|member| member.name.as_str())
            .unwrap_or(sender);
        let text = self.preview.replace('\n', " ");
        if text.is_empty() {
            return text;
        }
        if outgoing {
            if text.starts_with("Bạn: ") {
                text
            } else {
                format!("Bạn: {text}")
            }
        } else if self.is_group
            && !sender.trim().is_empty()
            && !sender.trim().chars().all(|c| c.is_ascii_digit())
        {
            let prefix = format!("{}: ", sender.trim());
            if text.starts_with(&prefix) {
                text
            } else {
                format!("{prefix}{text}")
            }
        } else {
            text
        }
    }

    pub fn sidebar_activity_ts(&self) -> i64 {
        self.messages
            .iter()
            .map(|m| crate::clock::normalize_ts_ms(m.ts))
            .chain(std::iter::once(crate::clock::normalize_ts_ms(
                self.presentation.last_message_ts,
            )))
            .max()
            .unwrap_or(0)
    }

    pub fn sidebar_timestamp(&self, now_ms: i64) -> String {
        let ts = self.sidebar_activity_ts();
        if ts > 0 {
            crate::clock::sidebar_time_at(ts, now_ms)
        } else {
            self.time.clone()
        }
    }

    /// Parse only group roster metadata observed in the original getmg-v2 consumer.
    pub fn apply_group_sidebar_metadata(&mut self, info: &Value) {
        if let Some(count) = info
            .get("totalMember")
            .and_then(|v| v.as_u64().or_else(|| v.as_str()?.parse().ok()))
        {
            self.presentation.member_count = Some(count);
        }
        let mut ids = Vec::new();
        if let Some(list) = info.get("memVerList").and_then(Value::as_array) {
            for item in list.iter().filter_map(Value::as_str) {
                let id = item.split('_').next().unwrap_or_default();
                if !id.is_empty() && !ids.iter().any(|s| s == id) {
                    ids.push(id.to_owned());
                }
            }
        } else if let Some(map) = info.get("memberVerMap").and_then(Value::as_object) {
            ids.extend(map.keys().cloned());
        }
        let mut leaders = Vec::new();
        if let Some(creator) = pick_str(info, &["creatorId"]) {
            leaders.push(creator);
        }
        if let Some(admins) = info.get("adminIds").and_then(Value::as_array) {
            leaders.extend(admins.iter().filter_map(Value::as_str).map(str::to_owned));
        }
        let mut ordered = Vec::new();
        for id in leaders.into_iter().chain(ids.iter().cloned()) {
            if ids.contains(&id) && !ordered.contains(&id) {
                ordered.push(id);
            }
        }
        ids = ordered;
        let top = info.get("topMember").and_then(Value::as_array);
        if let Some(top) = top {
            for profile in top {
                if let Some(id) = pick_str(profile, &["id", "userId", "uid"]) {
                    if !ids.contains(&id) {
                        ids.push(id);
                    }
                }
            }
        }
        if !ids.is_empty() {
            let previous = std::mem::take(&mut self.presentation.members);
            self.presentation.members =
                ids.into_iter()
                    .map(|id| {
                        let profile = top.and_then(|list| {
                            list.iter().find(|p| {
                                pick_str(p, &["id", "userId", "uid"]).as_ref() == Some(&id)
                            })
                        });
                        let mut member = previous.iter().find(|m| m.id == id).cloned().unwrap_or(
                            SidebarMember {
                                id,
                                ..Default::default()
                            },
                        );
                        if let Some(profile) = profile {
                            if let Some(name) =
                                pick_str(profile, &["displayName", "zaloName", "name"])
                            {
                                member.name = name;
                            }
                            if let Some(avatar) = pick_str(profile, &["avatar", "avt"]) {
                                member.avatar = Some(avatar);
                            }
                        }
                        member
                    })
                    .collect();
        }
    }
}

pub fn sidebar_sender_id(message: &ChatMessage) -> Option<String> {
    let raw = message.wire_ref.sender.as_ref().or_else(|| {
        message
            .history_source
            .as_ref()
            .and_then(|v| v.get("senderId"))
    })?;
    let id = match raw {
        Value::String(v) => v.clone(),
        Value::Number(v) if v.is_u64() => v.to_string(),
        _ => return None,
    };
    (!id.is_empty() && id != "0" && id.bytes().all(|b| b.is_ascii_digit())).then_some(id)
}

/// Return `true` if a thread name is empty or a generic placeholder (`"Người dùng ..."`, `"Nhóm ..."`).
pub fn is_generic_thread_name(name: &str) -> bool {
    let t = name.trim();
    t.is_empty()
        || t.starts_with("Người dùng ")
        || t.starts_with("Nhóm ")
        || t.chars().all(|c| c.is_ascii_digit())
}

/// Extract the stable avatar filename (e.g. `"00000000000000000000000000000001.jpg"`) from a Zalo CDN URL.
pub fn avatar_file_key(avatar: Option<&str>) -> Option<String> {
    let url = avatar?.trim();
    if url.is_empty() {
        return None;
    }
    let path = url.split('?').next()?;
    let fname = path.rsplit('/').next()?.trim();
    if fname.len() >= 12
        && (fname.ends_with(".jpg") || fname.ends_with(".png") || fname.ends_with(".webp"))
    {
        Some(fname.to_ascii_lowercase())
    } else {
        None
    }
}

/// Extract the `?time=...` expiry timestamp from a Zalo avatar URL so newer session avatars win.
pub fn avatar_url_time(avatar: Option<&str>) -> i64 {
    let Some(url) = avatar else {
        return 0;
    };
    let mut max_ts = 0i64;
    for seg in url.split(';') {
        let Some(query) = seg.split('?').nth(1) else {
            continue;
        };
        for pair in query.split('&') {
            if let Some(val) = pair.strip_prefix("time=") {
                let digits: String = val.chars().take_while(|c| c.is_ascii_digit()).collect();
                if let Ok(ts) = digits.parse::<i64>() {
                    if ts > max_ts {
                        max_ts = ts;
                    }
                }
            }
        }
    }
    max_ts
}

/// Count how many real `http(s)://` image URLs (excluding `/default`) an avatar string contains.
pub fn avatar_http_url_count(avatar: Option<&str>) -> usize {
    let Some(raw) = avatar.map(str::trim).filter(|s| !s.is_empty()) else {
        return 0;
    };
    if let Some(rest) = raw.strip_prefix("multi:") {
        rest.split(';')
            .filter_map(|seg| seg.split_once('|').map(|(_, u)| u.trim()))
            .filter(|u| {
                (u.starts_with("http://") || u.starts_with("https://")) && !u.ends_with("/default")
            })
            .count()
    } else if (raw.starts_with("http://") || raw.starts_with("https://"))
        && !raw.ends_with("/default")
    {
        1
    } else {
        0
    }
}

/// Normalize an avatar string by extracting stable image filenames (`avatar_file_key`) so `s120`
/// vs `s160` CDN subdomains and rotating `?key=...&time=...` tokens compare equal.
fn normalize_avatar_signature(avatar: &str) -> String {
    let raw = avatar.trim();
    if let Some(rest) = raw.strip_prefix("multi:") {
        let mut out = String::from("multi:");
        for (idx, seg) in rest.split(';').enumerate() {
            if idx > 0 {
                out.push(';');
            }
            if let Some((name, url)) = seg.split_once('|') {
                let u_trim = url.trim();
                let file_key = avatar_file_key(Some(u_trim))
                    .unwrap_or_else(|| u_trim.split('?').next().unwrap_or("").to_string());
                out.push_str(name.trim());
                out.push('|');
                out.push_str(&file_key);
            } else {
                out.push_str(seg.trim());
            }
        }
        out
    } else {
        avatar_file_key(Some(raw))
            .unwrap_or_else(|| raw.split('?').next().unwrap_or(raw).to_string())
    }
}

/// Decide whether `incoming` should replace `current` avatar without downgrading a rich
/// `multi:` composite avatar (with member photo URLs) or flipping between `s120` and `s160` URLs.
pub fn should_replace_avatar(current: Option<&str>, incoming: Option<&str>) -> bool {
    let Some(inc) = incoming.map(str::trim).filter(|s| !s.is_empty()) else {
        return false;
    };
    let Some(cur) = current.map(str::trim).filter(|s| !s.is_empty()) else {
        return true;
    };
    let cur_urls = avatar_http_url_count(Some(cur));
    let inc_urls = avatar_http_url_count(Some(inc));
    if inc_urls < cur_urls {
        return false;
    }
    if inc_urls > cur_urls {
        return true;
    }
    if cur.starts_with("multi:") && inc.starts_with("multi:") && cur_urls > 0 {
        return false;
    }
    if normalize_avatar_signature(cur) == normalize_avatar_signature(inc) {
        return false;
    }
    avatar_url_time(Some(inc)) > avatar_url_time(Some(cur))
}

/// Return `true` if two `Thread` entries represent the same underlying contact or group,
/// even if Zalo issued a new session-scoped `userId` after a QR re-login.
///
/// Two threads with distinct non-empty IDs are NEVER treated as the same contact solely
/// because their display names match (#63 — multiple Zalo contacts/groups frequently share
/// the same display name). Across different IDs, only the singleton "Cloud của tôi" thread
/// or contacts sharing the same non-default CDN avatar hash (`avatar_file_key`) and name match.
pub fn threads_represent_same_contact(a: &Thread, b: &Thread) -> bool {
    if !a.id.is_empty() && a.id == b.id {
        return true;
    }
    if a.is_group != b.is_group {
        return false;
    }
    if !a.is_group && a.name.contains("Cloud của tôi") && b.name.contains("Cloud của tôi") {
        return true;
    }
    if let (Some(ka), Some(kb)) = (
        avatar_file_key(a.avatar.as_deref()),
        avatar_file_key(b.avatar.as_deref()),
    ) {
        if !ka.contains("default") && ka == kb {
            return is_generic_thread_name(&a.name)
                || is_generic_thread_name(&b.name)
                || a.name.trim() == b.name.trim();
        }
        return false;
    }
    false
}

fn wire_identity(value: Option<&Value>) -> Option<String> {
    match value? {
        Value::String(value) if !value.trim().is_empty() && value != "0" => Some(value.clone()),
        Value::Number(value) => value.as_u64().filter(|id| *id > 0).map(|id| id.to_string()),
        _ => None,
    }
}

fn wire_sender(value: Option<&Value>) -> Option<String> {
    match value? {
        Value::String(value) if !value.is_empty() => Some(value.clone()),
        Value::Number(value) => value.as_u64().map(|id| id.to_string()),
        _ => None,
    }
}

fn server_message_id(message: &ChatMessage) -> Option<String> {
    let wire_server = wire_identity(message.wire_ref.server_id.as_ref());
    let wire_client = wire_identity(message.wire_ref.client_id.as_ref());
    if message.id.starts_with("out_")
        || message.id.is_empty()
        || wire_client.as_deref() == Some(message.id.as_str())
    {
        wire_server
    } else {
        wire_identity(Some(&Value::String(message.id.clone())))
    }
}

fn client_message_id(message: &ChatMessage) -> Option<String> {
    let wire = wire_identity(message.wire_ref.client_id.as_ref());
    if message.client_message_id.trim().is_empty() || message.client_message_id == "0" {
        wire
    } else if wire
        .as_ref()
        .is_some_and(|id| id != &message.client_message_id)
    {
        None
    } else {
        Some(message.client_message_id.clone())
    }
}

#[derive(Default)]
struct MessageIdentityIndex {
    rows: std::collections::HashMap<(u8, String), std::collections::HashSet<usize>>,
}

impl MessageIdentityIndex {
    fn keys(message: &ChatMessage) -> Vec<(u8, String)> {
        let mut keys = Vec::with_capacity(3);
        if let Some(id) = server_message_id(message) {
            keys.push((0, id));
        } else if message.id.starts_with("out_") {
            keys.push((1, message.id.clone()));
        }
        if !message.id.starts_with("out_") {
            if let Some(id) = wire_identity(Some(&Value::String(message.id.clone()))) {
                // Old caches only retain this ID; it may have come from cliMsgId.
                keys.push((3, id));
            }
        }
        if let Some(id) = client_message_id(message) {
            keys.push((2, id));
        }
        keys
    }

    fn insert(&mut self, message: &ChatMessage, index: usize) {
        for key in Self::keys(message) {
            self.rows.entry(key).or_default().insert(index);
        }
    }

    fn remove(&mut self, message: &ChatMessage, index: usize) {
        for key in Self::keys(message) {
            if let Some(rows) = self.rows.get_mut(&key) {
                rows.remove(&index);
            }
        }
    }

    fn find(&self, messages: &[ChatMessage], incoming: &ChatMessage) -> Option<usize> {
        for key in Self::keys(incoming) {
            let Some(rows) = self.rows.get(&key).filter(|rows| !rows.is_empty()) else {
                continue;
            };
            // A colliding identity is not evidence for selecting any particular row.
            if rows.len() != 1 {
                return None;
            }
            let index = *rows.iter().next()?;
            let existing = &messages[index];
            if existing.is_outgoing != incoming.is_outgoing {
                return None;
            }
            if let (Some(a), Some(b)) = (
                wire_sender(existing.wire_ref.sender.as_ref()),
                wire_sender(incoming.wire_ref.sender.as_ref()),
            ) {
                if a != b {
                    return None;
                }
            }
            if key.0 != 2 {
                if key.0 == 3 {
                    if let (Some(a), Some(b)) = (
                        wire_identity(existing.wire_ref.server_id.as_ref()),
                        wire_identity(incoming.wire_ref.server_id.as_ref()),
                    ) {
                        if a != b {
                            return None;
                        }
                    }
                }
                return Some(index);
            }
            // Client-only packets can replay before a server ID is known. A client
            // reference never collapses two distinct pending or server messages.
            let existing_server = server_message_id(existing);
            let incoming_server = server_message_id(incoming);
            let existing_local = existing.id.starts_with("out_") && existing_server.is_none();
            let incoming_local = incoming.id.starts_with("out_") && incoming_server.is_none();
            let client_only_replay = existing_server.is_none()
                && incoming_server.is_none()
                && (existing.id.is_empty() || incoming.id.is_empty());
            return (client_only_replay
                || (existing.is_outgoing
                    && ((existing_local && incoming_server.is_some())
                        || (incoming_local && existing_server.is_some()))))
            .then_some(index);
        }
        None
    }
}

/// Merge a local send acknowledgement without replacing an existing server clock.
pub(crate) fn merge_sent_message(thread: &mut Thread, mut message: ChatMessage) {
    let mut identities = MessageIdentityIndex::default();
    for (row, existing) in thread.messages.iter().enumerate() {
        identities.insert(existing, row);
    }
    if identities
        .find(&thread.messages, &message)
        .is_some_and(|row| server_message_id(&thread.messages[row]).is_some())
    {
        // The acknowledgement receipt time is not the server's message timestamp.
        message.ts = 0;
        message.time.clear();
    }
    merge_thread_into(
        thread,
        Thread {
            messages: vec![message],
            ..Default::default()
        },
        false,
    );
}

/// Merge `incoming` thread into `target`. When `incoming_id_is_authoritative` is `true`,
/// `target.id` is migrated to `incoming.id` so actions (chat/VoIP call) always target the active session's `userId`.
pub fn merge_thread_into(
    target: &mut Thread,
    incoming: Thread,
    incoming_id_is_authoritative: bool,
) {
    if incoming.presentation.posting_restricted.is_some() {
        target.presentation.posting_restricted = incoming.presentation.posting_restricted;
    }
    if incoming_id_is_authoritative && !incoming.id.is_empty() && incoming.id != "0" {
        target.id = incoming.id;
    }
    if is_generic_thread_name(&target.name) && !incoming.name.is_empty() {
        target.name = incoming.name;
    }
    if should_replace_avatar(target.avatar.as_deref(), incoming.avatar.as_deref()) {
        target.avatar = incoming.avatar;
    }
    target.is_group = target.is_group || incoming.is_group;
    target.presentation.pinned = target.presentation.pinned || incoming.presentation.pinned;
    target.presentation.muted = target.presentation.muted || incoming.presentation.muted;
    target.presentation.hidden = target.presentation.hidden || incoming.presentation.hidden;

    // Repeated text and timestamps are not identities. Keep ambiguous keys instead of
    // choosing the first candidate, especially when multiple pending sends collide.
    let mut identities = MessageIdentityIndex::default();
    for (index, message) in target.messages.iter().enumerate() {
        identities.insert(message, index);
    }

    for msg in incoming.messages {
        if msg.text == "sendBubbleMessage" {
            continue;
        }
        let found = identities.find(&target.messages, &msg);
        if let Some(idx) = found {
            let existing_msg = &mut target.messages[idx];
            identities.remove(existing_msg, idx);
            existing_msg.merge_reaction_metadata(&msg);
            if existing_msg.id.starts_with("out_")
                && !msg.id.is_empty()
                && !msg.id.starts_with("out_")
            {
                existing_msg.id = msg.id.clone();
            }
            if msg.ts > 0 && existing_msg.ts != msg.ts {
                existing_msg.ts = msg.ts;
                existing_msg.time = msg.time;
            }
            if msg.text.trim() == "[Tin nhắn đã thu hồi]" {
                if !existing_msg.text.starts_with("[Đã thu hồi]") {
                    if existing_msg.text.trim().is_empty() {
                        existing_msg.text = "[Tin nhắn đã thu hồi]".to_string();
                    } else {
                        existing_msg.text = format!("[Đã thu hồi] {}", existing_msg.text);
                    }
                }
                // Anti-recall: Retain sticker, link_url, link_thumb so user can still view recalled photos & media!
            }
            if existing_msg.sticker_id.is_none() && msg.sticker_id.is_some() {
                existing_msg.sticker_id = msg.sticker_id;
                existing_msg.sticker_cate_id = msg.sticker_cate_id;
                existing_msg.sticker_type = msg.sticker_type;
            }
            if existing_msg.reply_to_text.is_none() && msg.reply_to_text.is_some() {
                existing_msg.reply_to_id = msg.reply_to_id;
                existing_msg.reply_to_sender = msg.reply_to_sender;
                existing_msg.reply_to_text = msg.reply_to_text;
            }
            if existing_msg.link_url.is_none() && msg.link_url.is_some() {
                existing_msg.link_url = msg.link_url;
                existing_msg.link_title = msg.link_title;
                existing_msg.link_desc = msg.link_desc;
                existing_msg.link_thumb = msg.link_thumb;
                existing_msg.link_src = msg.link_src;
                if !msg.text.is_empty() {
                    existing_msg.text = msg.text;
                }
            }
            if existing_msg.reactions.is_empty() && !msg.reactions.is_empty() {
                existing_msg.reactions = msg.reactions;
            }
            existing_msg.is_pinned = existing_msg.is_pinned || msg.is_pinned;
            existing_msg.is_starred = existing_msg.is_starred || msg.is_starred;
            identities.insert(existing_msg, idx);
        } else {
            let idx = target.messages.len();
            identities.insert(&msg, idx);
            target.messages.push(msg);
        }
    }

    target.messages.retain(|m| m.text != "sendBubbleMessage");
    target.messages.sort_by_key(|m| m.ts);

    let mut latest_incoming_ts = 0i64;
    for m in target.messages.iter() {
        if !m.is_outgoing && m.ts > latest_incoming_ts {
            latest_incoming_ts = m.ts;
        }
    }
    if latest_incoming_ts > 0 {
        for m in target.messages.iter_mut() {
            if m.is_outgoing && m.ts <= latest_incoming_ts {
                m.delivery = crate::message_model::DeliveryState::Read;
            }
        }
    }

    if let Some(newest) = target.messages.last() {
        if !newest.text.is_empty() {
            target.preview = newest.text.replace('\n', " ");
            target.time = newest.time.clone();
        }
    } else if !incoming.preview.is_empty() && target.preview.is_empty() {
        target.preview = incoming.preview;
        if !incoming.time.is_empty() {
            target.time = incoming.time;
        }
    } else if target.time.is_empty() && !incoming.time.is_empty() {
        target.time = incoming.time;
    }
}

/// Deduplicate `threads` in-place, merging entries that represent the same contact/group
/// (by `id`, avatar hash, or non-generic display name) and preserving the authoritative/newer `id`.
pub fn deduplicate_threads(
    threads: &mut Vec<Thread>,
    authoritative_ids: Option<&std::collections::HashSet<String>>,
) {
    let mut merged: Vec<Thread> = Vec::with_capacity(threads.len());
    for mut t in threads.drain(..) {
        if t.id.is_empty() || t.id == "0" {
            continue;
        }
        t.messages.retain(|m| m.text != "sendBubbleMessage");
        if let Some(newest) = t.messages.last() {
            if !newest.text.is_empty() && (t.preview.is_empty() || t.preview == "sendBubbleMessage")
            {
                t.preview = newest.text.replace('\n', " ");
                t.time = newest.time.clone();
            }
        }
        if let Some(existing) = merged.iter_mut().find(|e| {
            if e.id == t.id {
                return true;
            }
            if let Some(auth) = authoritative_ids {
                if auth.contains(&e.id) && auth.contains(&t.id) {
                    return false;
                }
            }
            threads_represent_same_contact(e, &t)
        }) {
            let incoming_auth = if let Some(auth) = authoritative_ids {
                auth.contains(&t.id) && !auth.contains(&existing.id)
            } else {
                // In cache order, later entries or entries with newer avatar URL timestamps come from newer sessions
                let t_time = avatar_url_time(t.avatar.as_deref());
                let e_time = avatar_url_time(existing.avatar.as_deref());
                if t_time != e_time {
                    t_time > e_time
                } else {
                    true
                }
            };
            merge_thread_into(existing, t, incoming_auth);
        } else {
            merged.push(t);
        }
    }
    let cloud_msg_ids: std::collections::HashSet<String> = merged
        .iter()
        .filter(|t| t.name.contains("Cloud của tôi"))
        .flat_map(|t| t.messages.iter())
        .filter_map(|m| {
            if !m.id.is_empty() && !m.id.starts_with("out_") && !m.id.starts_with("local_") {
                Some(m.id.clone())
            } else {
                None
            }
        })
        .collect();
    if !cloud_msg_ids.is_empty() {
        for t in merged.iter_mut() {
            if !t.name.contains("Cloud của tôi") {
                let before = t.messages.len();
                t.messages.retain(|m| !cloud_msg_ids.contains(&m.id));
                if t.messages.len() != before {
                    if let Some(newest) = t.messages.last() {
                        t.preview = newest.text.replace('\n', " ");
                        t.time = newest.time.clone();
                    } else {
                        t.preview.clear();
                        t.time.clear();
                    }
                }
            }
        }
    }
    *threads = merged;
}

fn active_uin_from_session() -> Option<String> {
    if let Ok(text) = std::fs::read_to_string("session.json") {
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(&text) {
            if let Some(uin) = val.pointer("/login/uin").and_then(|v| v.as_str()) {
                if !uin.is_empty() {
                    return Some(uin.to_string());
                }
            }
        }
    }
    None
}

/// Load locally cached conversations and message history scoped to active account.
pub fn load_threads_cache() -> Vec<Thread> {
    if let Some(uin) = active_uin_from_session() {
        let acc_path = format!("accounts/{uin}_threads.json");
        if let Ok(text) = std::fs::read_to_string(&acc_path) {
            if let Ok(mut threads) = serde_json::from_str::<Vec<Thread>>(&text) {
                deduplicate_threads(&mut threads, None);
                return threads;
            }
        }
    }
    if std::path::Path::new("session.json").exists() {
        if let Ok(text) = std::fs::read_to_string("threads_cache.json") {
            if let Ok(mut threads) = serde_json::from_str::<Vec<Thread>>(&text) {
                deduplicate_threads(&mut threads, None);
                return threads;
            }
        }
    }
    Vec::new()
}

/// Snapshot of the thread list for an instant first paint. Only the newest
/// `PRELOAD_MSGS` messages per thread are kept: full history lives in SQLite, and
/// rewriting every message as pretty JSON on each change cost more than the SQLite write.
pub fn save_threads_cache(threads: &[Thread]) {
    let keep = crate::store::MessageStore::PRELOAD_MSGS;
    let slim: Vec<Thread> = threads
        .iter()
        .map(|t| Thread {
            presentation: t.presentation.clone(),
            unread: t.unread.clone(),
            id: t.id.clone(),
            name: t.name.clone(),
            preview: t.preview.clone(),
            time: t.time.clone(),
            avatar: t.avatar.clone(),
            is_group: t.is_group,
            messages: t.messages[t.messages.len().saturating_sub(keep)..].to_vec(),
        })
        .collect();
    if let Ok(text) = serde_json::to_string(&slim) {
        let _ = std::fs::write("threads_cache.json", &text);
        if let Some(uin) = active_uin_from_session() {
            let _ = std::fs::create_dir_all("accounts");
            let _ = std::fs::write(format!("accounts/{uin}_threads.json"), text);
        }
    }
}

/// `/api/social/friend/getfriends` — the friend list.
///
/// Uses `s.domains.friend` (defaulting to `tt-friend-wpa.chat.zalo.me`), falling back
/// to `s.domains.profile` only if `domains.friend` is empty.
pub async fn get_friends(s: &Session) -> Result<Vec<Thread>> {
    if s.login.is_none() {
        anyhow::bail!("chưa đăng nhập");
    }
    let body = obj(&[
        ("incInvalid", json!(1)),
        ("page", json!(1)),
        ("count", json!(20000)),
        ("avatar_size", json!(160)),
        ("actiontime", json!(0)),
        ("imei", json!(s.imei)),
    ]);
    let params = match s.encoded_params(&body) {
        Ok(p) => format!("params={p}"),
        Err(e) => {
            crate::logger::log_error("chat::get_friends::encoded_params", &e.to_string());
            eprintln!("! get_friends: no session key for encoded_params ({e}), falling back to plain_params");
            Session::plain_params(&body)
        }
    };
    let path_and_query = format!(
        "/api/social/friend/getfriends?{}{}",
        s.common_params(),
        if params.is_empty() {
            String::new()
        } else {
            format!("&{params}")
        }
    );
    let raw = s
        .get_endpoint("friend", &path_and_query)
        .await
        .map_err(|e| e.context("không tải được danh sách bạn bè"))?;
    let resp = s.maybe_decrypt(raw, s.session_key_bytes()).await;
    let data_root = resp.get("data").unwrap_or(&resp);
    let mut threads: Vec<Thread> = match data_root {
        Value::Array(items) => items.iter().filter_map(parse_thread).collect(),
        Value::Object(o) if o.contains_key("friends") => o["friends"]
            .as_array()
            .map(|a| a.iter().filter_map(parse_thread).collect())
            .unwrap_or_default(),
        Value::Object(o) if o.get("data").and_then(Value::as_array).is_some() => o["data"]
            .as_array()
            .map(|a| a.iter().filter_map(parse_thread).collect())
            .unwrap_or_default(),
        _ => Vec::new(),
    };

    // If Cloud của tôi (send2me) is available, place it at the top
    if let Some(li) = &s.login {
        if !li.send2me_id.is_empty() && !threads.iter().any(|t| t.id == li.send2me_id) {
            threads.insert(
                0,
                Thread {
                    presentation: Default::default(),
                    unread: Default::default(),
                    id: li.send2me_id.clone(),
                    name: "Cloud của tôi (Lưu trữ)".into(),
                    preview: "".into(),
                    time: String::new(),
                    avatar: None,
                    is_group: false,
                    messages: Vec::new(),
                },
            );
        }
    }

    // Hydrate with locally persisted history from SQLite scoped to this account
    let uin = s.login.as_ref().map(|l| l.uin.as_str()).unwrap_or_default();
    let db_path = crate::store::db_path_for_account(uin);
    crate::store::load_history_into_threads(
        &db_path,
        &mut threads,
        crate::store::MessageStore::PRELOAD_MSGS,
    );

    // Merge user groups (`getlg/v4` + `getmg-v2`) into threads list, upgrading any placeholder group names
    if let Ok(mut groups) = get_groups(s).await {
        for g in groups.drain(..) {
            if let Some(existing) = threads.iter_mut().find(|t| t.id == g.id) {
                merge_thread_into(existing, g, true);
            } else {
                threads.push(g);
            }
        }
    }

    resolve_thread_names(s, &mut threads).await;

    Ok(threads)
}

fn parse_thread(v: &Value) -> Option<Thread> {
    let id = pick_str(v, &["userId", "uid", "id", "friendId", "globalId"])?;
    let name = pick_str(v, &["displayName", "zaloName", "display_name", "name"])
        .or_else(|| {
            v.get("profile_displayName")
                .or_else(|| v.get("profile").and_then(|p| p.get("displayName")))
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .unwrap_or_else(|| id.clone());
    let avatar = pick_str(v, &["avatar", "avt", "profile_avatar"]);
    let is_group = is_group_flag(v);
    let last_action_ts = v
        .get("lastActionTime")
        .and_then(|val| match val {
            Value::Number(n) => n.as_i64(),
            Value::String(s) => s.parse::<i64>().ok(),
            _ => None,
        })
        .unwrap_or(0);
    let time = if last_action_ts > 0 {
        crate::clock::message_time(last_action_ts)
    } else {
        String::new()
    };

    let mut presentation = SidebarPresentation::default();
    presentation.last_message_ts = last_action_ts;

    Some(Thread {
        presentation,
        unread: Default::default(),
        id,
        name,
        preview: String::new(),
        time,
        avatar,
        is_group,
        messages: Vec::new(),
    })
}

pub(crate) fn pick_str(v: &Value, keys: &[&str]) -> Option<String> {
    for k in keys {
        match v.get(*k) {
            Some(Value::String(s)) if !s.is_empty() => return Some(s.clone()),
            Some(Value::Number(n)) => return Some(n.to_string()),
            _ => {}
        }
    }
    None
}

/// Allocate a millisecond-shaped request ID once per send, including simultaneous sends.
pub(crate) fn next_client_message_id() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static LAST_ID: AtomicU64 = AtomicU64::new(0);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0);
    let previous = LAST_ID
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |last| {
            Some(now.max(last.saturating_add(1)))
        })
        .unwrap_or_else(|last| last);
    now.max(previous.saturating_add(1))
}

/// Convert inline Markdown (`**bold**`, `__underline__`, `~~strikethrough~~`, `*italic*`) and
/// line-prefix styles (`## `, `- `, `1. `) into Zalo PC's native `(clean_text, Option<textProperties>)`
/// format (`{"styles":[{"start":N,"len":M,"st":"b,i,u,s,f_18,lst_1,lst_2"}],"ver":0}` in `compact-app-pc.js`).
pub fn extract_zalo_rtf_text_properties(raw: &str) -> (String, Option<String>) {
    let mut out_text = String::with_capacity(raw.len());
    let mut styles: Vec<Value> = Vec::new();

    let lines: Vec<&str> = raw.split('\n').collect();
    for (line_idx, raw_line) in lines.iter().enumerate() {
        if line_idx > 0 {
            out_text.push('\n');
        }
        let mut line = *raw_line;
        let mut line_tags: Vec<&'static str> = Vec::new();
        if let Some(rest) = line.strip_prefix("### ") {
            line = rest;
            line_tags.push("b");
            line_tags.push("f_16");
        } else if let Some(rest) = line.strip_prefix("## ") {
            line = rest;
            line_tags.push("b");
            line_tags.push("f_18");
        } else if let Some(rest) = line.strip_prefix("# ") {
            line = rest;
            line_tags.push("b");
            line_tags.push("f_18");
        } else if let Some(rest) = line.strip_prefix("- ").or_else(|| line.strip_prefix("* ")) {
            line = rest;
            line_tags.push("lst_1");
        } else if let Some(rest) = line.strip_prefix("1. ") {
            line = rest;
            line_tags.push("lst_2");
        }

        let line_start_u16 = out_text.encode_utf16().count();
        let mut rest = line;
        while !rest.is_empty() {
            let mut matched_span = false;
            for (delim, st_tag) in [("`", ""), ("**", "b"), ("__", "u"), ("~~", "s"), ("*", "i")] {
                if rest.starts_with(delim) {
                    let dlen = delim.len();
                    let after = &rest[dlen..];
                    if let Some(end_pos) = after.find(delim) {
                        let inner = &after[..end_pos];
                        if !inner.is_empty() && !inner.contains('\n') {
                            let span_start = out_text.encode_utf16().count();
                            let span_len = inner.encode_utf16().count();
                            out_text.push_str(inner);
                            if !st_tag.is_empty() && span_len > 0 {
                                styles.push(json!({
                                    "start": span_start,
                                    "len": span_len,
                                    "st": st_tag,
                                }));
                            }
                            rest = &after[end_pos + dlen..];
                            matched_span = true;
                            break;
                        }
                    }
                }
            }
            if matched_span {
                continue;
            }
            let mut chars = rest.chars();
            if let Some(ch) = chars.next() {
                out_text.push(ch);
                rest = chars.as_str();
            } else {
                break;
            }
        }
        let line_len_u16 = out_text
            .encode_utf16()
            .count()
            .saturating_sub(line_start_u16);
        if !line_tags.is_empty() && line_len_u16 > 0 {
            styles.push(json!({
                "start": line_start_u16,
                "len": line_len_u16,
                "st": line_tags.join(","),
            }));
        }
    }

    if styles.is_empty() {
        (raw.to_string(), None)
    } else {
        let prop_str = json!({
            "styles": styles,
            "ver": 0,
        })
        .to_string();
        (out_text, Some(prop_str))
    }
}

/// Reconstruct Markdown markers from Zalo's incoming `rtf` object (`{"action":"rtf","title":...,"params":"{\"styles\":[...]}"}`)
/// so incoming rich-text messages sent from official Zalo PC / Mobile render with bold, underline, strikethrough, and lists.
fn format_incoming_zalo_rtf(title: &str, params_str: &str) -> String {
    let Ok(p_val) = serde_json::from_str::<Value>(params_str.trim()) else {
        return title.to_string();
    };
    let Some(styles) = p_val.get("styles").and_then(Value::as_array) else {
        return title.to_string();
    };
    if styles.is_empty() {
        return title.to_string();
    }
    let u16_units: Vec<u16> = title.encode_utf16().collect();
    let mut spans: Vec<(usize, usize, String)> = styles
        .iter()
        .filter_map(|s| {
            let start = s.get("start")?.as_u64()? as usize;
            let len = s.get("len")?.as_u64()? as usize;
            let st = s.get("st")?.as_str()?.to_string();
            if len > 0 && start + len <= u16_units.len() {
                Some((start, start + len, st))
            } else {
                None
            }
        })
        .collect();
    if spans.is_empty() {
        return title.to_string();
    }
    spans.sort_by_key(|x| x.0);
    let mut out = String::with_capacity(title.len() + spans.len() * 4);
    let mut cursor = 0usize;
    for (s, e, st) in spans {
        if s < cursor {
            continue;
        }
        if s > cursor {
            out.push_str(&String::from_utf16_lossy(&u16_units[cursor..s]));
        }
        let seg = String::from_utf16_lossy(&u16_units[s..e]);
        let tags: Vec<&str> = st.split(',').collect();
        let is_bold = tags.contains(&"b") || tags.iter().any(|t| t.starts_with("f_"));
        let is_italic = tags.contains(&"i");
        let is_under = tags.contains(&"u");
        let is_strike = tags.contains(&"s");
        let mut wrapped = seg;
        if is_italic && !is_bold {
            wrapped = format!("*{wrapped}*");
        }
        if is_bold {
            wrapped = format!("**{wrapped}**");
        }
        if is_under {
            wrapped = format!("__{wrapped}__");
        }
        if is_strike {
            wrapped = format!("~~{wrapped}~~");
        }
        out.push_str(&wrapped);
        cursor = e;
    }
    if cursor < u16_units.len() {
        out.push_str(&String::from_utf16_lossy(&u16_units[cursor..]));
    }
    out
}

/// Send a 1-1 text message: `POST {domains.chat}/api/message/sms?{common}`
pub async fn send_message(s: &Session, to_id: &str, text: &str) -> Result<Value> {
    send_message_with_client_id(s, to_id, text, next_client_message_id()).await
}

/// Preserve the request client ID for reconciliation with a later server echo.
pub(crate) async fn send_message_with_client_id(
    s: &Session,
    to_id: &str,
    text: &str,
    now: u64,
) -> Result<Value> {
    if s.login.is_none() {
        anyhow::bail!("chưa đăng nhập");
    }
    let (clean_text, text_props_opt) = extract_zalo_rtf_text_properties(text);
    let mut body = obj(&[
        ("toid", json!(to_id)),
        ("message", json!(clean_text)),
        ("clientId", json!(now)),
        ("imei", json!(s.imei)),
    ]);
    if let (Some(tp), Some(map)) = (text_props_opt, body.as_object_mut()) {
        map.insert("textProperties".into(), json!(tp));
    }
    let params = s.encoded_params(&body)?;

    let host = if s.domains.chat.contains("-msg-chat.")
        || s.domains.chat.contains("tt-chat2-wpa")
        || s.domains.chat.is_empty()
    {
        "https://tt-chat3-wpa.api.zaloapp.com"
    } else {
        &s.domains.chat
    };

    let url = format!("{host}/api/message/sms?{}", s.common_params());
    let form = format!("params={params}");
    let resp = s.post_form(&url, form).await?;
    let resp = s.maybe_decrypt(resp, s.session_key_bytes()).await;
    if let Some(code) = resp.get("error_code").and_then(Value::as_i64) {
        if code != 0 {
            let msg = resp
                .get("error_message")
                .and_then(Value::as_str)
                .unwrap_or("unknown");
            return Err(crate::session::SessionError::new(code, msg).into());
        }
    }
    Ok(resp)
}

/// Send a group text message: `POST {domains.group}/api/group/sendmsg?{common}`
pub async fn send_group_message(s: &Session, grid: &str, text: &str) -> Result<Value> {
    send_group_message_with_client_id(s, grid, text, next_client_message_id()).await
}

/// Preserve the request client ID for reconciliation with a later server echo.
pub(crate) async fn send_group_message_with_client_id(
    s: &Session,
    grid: &str,
    text: &str,
    now: u64,
) -> Result<Value> {
    if s.login.is_none() {
        anyhow::bail!("chưa đăng nhập");
    }
    let clean_grid = grid
        .trim_start_matches("group_")
        .trim_start_matches('g')
        .trim_start_matches('G');
    let (clean_text, text_props_opt) = extract_zalo_rtf_text_properties(text);
    let mut body = obj(&[
        ("grid", json!(clean_grid)),
        ("message", json!(clean_text)),
        ("clientId", json!(now)),
        ("imei", json!(s.imei)),
        ("visibility", json!(0)),
    ]);
    if let (Some(tp), Some(map)) = (text_props_opt, body.as_object_mut()) {
        map.insert("textProperties".into(), json!(tp));
    }
    let params = s.encoded_params(&body)?;
    let url = format!(
        "{}/api/group/sendmsg?{}",
        s.domains.group,
        s.common_params()
    );
    let form = format!("params={params}");
    let resp = s.post_form(&url, form).await?;
    let resp = s.maybe_decrypt(resp, s.session_key_bytes()).await;
    if let Some(code) = resp.get("error_code").and_then(Value::as_i64) {
        if code != 0 {
            let msg = resp
                .get("error_message")
                .and_then(Value::as_str)
                .unwrap_or("unknown");
            return Err(crate::session::SessionError::new(code, msg).into());
        }
    }
    Ok(resp)
}

/// Format a byte count into a human-readable size string (e.g. `"1.4 MB"`, `"320 KB"`).
pub fn format_file_size(bytes: u64) -> String {
    if bytes >= 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else if bytes >= 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{bytes} B")
    }
}

fn md5_hex_bytes(bytes: &[u8]) -> String {
    use md5::{Digest, Md5};
    let mut h = Md5::new();
    h.update(bytes);
    let out = h.finalize();
    out.iter().fold(String::with_capacity(32), |mut acc, b| {
        use std::fmt::Write;
        let _ = write!(acc, "{b:02x}");
        acc
    })
}

/// How long `send_attachment` waits for the `file_done` control event of an async file upload.
const FILE_DONE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(90);
/// How long an unclaimed `file_done` event is kept for a waiter that registers late.
const FILE_DONE_EARLY_TTL: std::time::Duration = std::time::Duration::from_secs(120);

/// Async file uploads waiting for their `act_type: "file_done"` control event, keyed by `fileId`.
///
/// `asyncfile/upload` only returns a `fileId`; Zalo pushes the final `fileUrl` later over the
/// WebSocket (`cmd 601`) or `lp_control`, and `asyncfile/msg` is rejected without it.
#[derive(Default)]
struct FileDoneRegistry {
    waiters: std::collections::HashMap<String, tokio::sync::oneshot::Sender<String>>,
    /// Events that arrived before their waiter registered (`fileId` -> (`fileUrl`, received at)).
    early: std::collections::HashMap<String, (String, std::time::Instant)>,
}

static FILE_DONE_REGISTRY: std::sync::LazyLock<std::sync::Mutex<FileDoneRegistry>> =
    std::sync::LazyLock::new(|| std::sync::Mutex::new(FileDoneRegistry::default()));

/// Set once any control-event consumer (WebSocket or `lp_control` poll) feeds `file_done` events,
/// so front-ends without one (TUI) skip the wait instead of stalling for `FILE_DONE_TIMEOUT`.
static FILE_DONE_LISTENER_ACTIVE: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

fn file_id_key(v: &Value) -> Option<String> {
    match v {
        Value::String(s) if !s.trim().is_empty() => Some(s.trim().to_string()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

/// Parse `act_type: "file_done"` items (`{fileId, data: {url}}`) into `(fileId, fileUrl)` pairs.
pub fn extract_file_done_controls(data_node: &Value) -> Vec<(String, String)> {
    let mut candidates: Vec<&Value> = Vec::new();
    if let Some(arr) = data_node.get("controls").and_then(Value::as_array) {
        candidates.extend(arr.iter());
    } else if let Some(arr) = data_node.as_array() {
        for item in arr {
            if let Some(sub) = item.get("controls").and_then(Value::as_array) {
                candidates.extend(sub.iter());
            } else {
                candidates.push(item);
            }
        }
    } else {
        candidates.push(data_node);
    }

    let mut out = Vec::new();
    for item in candidates {
        let c = item.get("content").unwrap_or(item);
        if c.get("act_type").and_then(Value::as_str) != Some("file_done") {
            continue;
        }
        let raw_data = c.get("data").unwrap_or(&Value::Null);
        let parsed_data: Value;
        let d = if let Some(s) = raw_data.as_str() {
            parsed_data = serde_json::from_str::<Value>(s).unwrap_or(Value::Null);
            &parsed_data
        } else {
            raw_data
        };
        let Some(file_id) = c
            .get("fileId")
            .or_else(|| d.get("fileId"))
            .and_then(file_id_key)
        else {
            continue;
        };
        let Some(url) = pick_str(d, &["url", "fileUrl"]) else {
            continue;
        };
        out.push((file_id, url));
    }
    out
}

/// Resolve pending async file uploads from a decrypted control payload (WebSocket `cmd 601` or
/// `lp_control`). Unclaimed events are kept briefly for uploads whose waiter registers late.
pub fn dispatch_file_done_controls(data_node: &Value) {
    FILE_DONE_LISTENER_ACTIVE.store(true, std::sync::atomic::Ordering::Relaxed);
    let events = extract_file_done_controls(data_node);
    if events.is_empty() {
        return;
    }
    let Ok(mut reg) = FILE_DONE_REGISTRY.lock() else {
        crate::logger::log_error("chat::file_done", "registry mutex poisoned");
        return;
    };
    reg.early
        .retain(|_, (_, at)| at.elapsed() < FILE_DONE_EARLY_TTL);
    for (file_id, url) in events {
        match reg.waiters.remove(&file_id) {
            Some(tx) => {
                let _ = tx.send(url);
            }
            None => {
                reg.early.insert(file_id, (url, std::time::Instant::now()));
            }
        }
    }
}

fn register_file_done_waiter(file_id: &str) -> tokio::sync::oneshot::Receiver<String> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    match FILE_DONE_REGISTRY.lock() {
        Ok(mut reg) => match reg.early.remove(file_id) {
            Some((url, _)) => {
                let _ = tx.send(url);
            }
            None => {
                reg.waiters.insert(file_id.to_string(), tx);
            }
        },
        // Dropping `tx` makes the receiver resolve immediately with an error.
        Err(_) => crate::logger::log_error("chat::file_done", "registry mutex poisoned"),
    }
    rx
}

/// Wait up to `timeout` for the `file_done` event of `file_id`; `None` on timeout.
async fn wait_for_file_done(file_id: &str, timeout: std::time::Duration) -> Option<String> {
    let rx = register_file_done_waiter(file_id);
    let res = tokio::time::timeout(timeout, rx).await;
    if let Ok(mut reg) = FILE_DONE_REGISTRY.lock() {
        reg.waiters.remove(file_id);
    }
    res.ok().and_then(Result::ok)
}

/// Upload and send a photo, video, or general file attachment to a 1-1 or group conversation.
///
/// Implements Zalo PC 26.10.11's two-stage chunked upload (`photo_original/upload` -> `photo_original/send`
/// for images; `asyncfile/upload` -> `asyncfile/msg` for videos/files) with automatic fallback to
/// single-shot `message/photo` (`11152`) and `message/sharefile_full` (`11146`).
pub async fn send_attachment(
    s: &Session,
    to_id: &str,
    is_group: bool,
    file_path: &std::path::Path,
    caption: &str,
) -> Result<(Value, ChatMessage)> {
    send_attachment_ext(s, to_id, is_group, file_path, caption, true).await
}

pub async fn send_attachment_ext(
    s: &Session,
    to_id: &str,
    is_group: bool,
    file_path: &std::path::Path,
    caption: &str,
    is_original_hd: bool,
) -> Result<(Value, ChatMessage)> {
    if s.login.is_none() {
        anyhow::bail!("chưa đăng nhập");
    }
    let file_bytes = std::fs::read(file_path)?;
    if file_bytes.is_empty() {
        anyhow::bail!("Tập tin rỗng (0 byte)");
    }
    let file_name = file_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("attachment.bin")
        .to_string();
    let ext = file_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    let is_image = matches!(
        ext.as_str(),
        "png" | "jpg" | "jpeg" | "webp" | "gif" | "bmp"
    );
    let is_video = matches!(ext.as_str(), "mp4" | "mov" | "mkv" | "webm" | "avi");

    let file_host = if s.domains.file.is_empty() {
        "https://tt-files-wpa.zaloapp.com"
    } else {
        s.domains.file.trim_end_matches('/')
    };
    let clean_target = if is_group {
        to_id
            .trim_start_matches("group_")
            .trim_start_matches('g')
            .trim_start_matches('G')
    } else {
        to_id
    };
    let client_id = next_client_message_id();
    let total_size = file_bytes.len() as u64;
    let chunk_size: usize = 1_048_576; // 1 MiB chunks
    let total_chunks = file_bytes.len().div_ceil(chunk_size).max(1);
    let target_type = if is_group { 11 } else { 2 };

    if is_image {
        let (width, height) = image::load_from_memory(&file_bytes)
            .map(|img| (img.width(), img.height()))
            .unwrap_or((800, 600));
        let mime = match ext.as_str() {
            "png" => "image/png",
            "webp" => "image/webp",
            "gif" => "image/gif",
            "bmp" => "image/bmp",
            _ => "image/jpeg",
        };
        let upload_ep = if is_group {
            "/api/group/photo_original/upload"
        } else {
            "/api/message/photo_original/upload"
        };

        let mut last_up_dec = Value::Null;
        for (idx, chunk) in file_bytes.chunks(chunk_size).enumerate() {
            let chunk_id = idx + 1;
            let mut m = serde_json::Map::new();
            m.insert("totalChunk".into(), json!(total_chunks));
            m.insert("fileName".into(), json!(file_name));
            m.insert("clientId".into(), json!(client_id));
            m.insert("totalSize".into(), json!(total_size));
            m.insert("imei".into(), json!(s.imei));
            m.insert("isE2EE".into(), json!(0));
            m.insert("jxl".into(), json!(0));
            m.insert("chunkId".into(), json!(chunk_id));
            if is_group {
                m.insert("grid".into(), json!(clean_target));
            } else {
                m.insert("toid".into(), json!(clean_target));
            }
            let params = s.encoded_params(&Value::Object(m))?;
            let url = format!(
                "{file_host}{upload_ep}?{}&params={params}&type={target_type}",
                s.common_params()
            );
            let up_raw = s
                .post_multipart(&url, "chunkContent", &file_name, mime, chunk)
                .await?;
            last_up_dec = s.maybe_decrypt(up_raw, s.session_key_bytes()).await;
        }

        let data_node = last_up_dec.get("data").unwrap_or(&last_up_dec);
        let photo_id = pick_str(data_node, &["photoId", "clientFileId"]);
        let normal_url =
            pick_str(data_node, &["normalUrl", "hdUrl", "oriUrl", "url"]).unwrap_or_default();
        let hd_url = pick_str(data_node, &["hdUrl", "normalUrl", "oriUrl"])
            .unwrap_or_else(|| normal_url.clone());
        let ori_url = pick_str(data_node, &["oriUrl", "hdUrl", "normalUrl"])
            .unwrap_or_else(|| normal_url.clone());
        let thumb_url = pick_str(data_node, &["thumbUrl", "normalUrl", "hdUrl"])
            .unwrap_or_else(|| normal_url.clone());

        let send_resp = if let Some(pid) = photo_id.filter(|_| !normal_url.is_empty()) {
            let send_ep = if is_group {
                "/api/group/photo_original/send"
            } else {
                "/api/message/photo_original/send"
            };
            let pid_val = pid
                .parse::<u64>()
                .map(|n| json!(n))
                .unwrap_or_else(|_| json!(pid));
            let mut send_obj = serde_json::Map::new();
            send_obj.insert("photoId".into(), pid_val);
            send_obj.insert("clientId".into(), json!(client_id));
            send_obj.insert("desc".into(), json!(caption));
            send_obj.insert("width".into(), json!(width));
            send_obj.insert("height".into(), json!(height));
            send_obj.insert("rawUrl".into(), json!(normal_url));
            send_obj.insert("hdUrl".into(), json!(hd_url));
            send_obj.insert("thumbUrl".into(), json!(thumb_url));
            send_obj.insert(
                "oriUrl".into(),
                json!(if is_original_hd {
                    &ori_url
                } else {
                    &normal_url
                }),
            );
            send_obj.insert("normalUrl".into(), json!(normal_url));
            send_obj.insert(
                "hdSize".into(),
                json!(if is_original_hd { total_size } else { 0 }),
            );
            send_obj.insert("isHD".into(), json!(if is_original_hd { 1 } else { 0 }));
            send_obj.insert("hd".into(), json!(if is_original_hd { 1 } else { 0 }));
            send_obj.insert(
                "isOriginal".into(),
                json!(if is_original_hd { 1 } else { 0 }),
            );
            send_obj.insert("zsource".into(), json!(-1));
            send_obj.insert("jcp".into(), json!("{\"convertible\":true}"));
            send_obj.insert("imei".into(), json!(s.imei));
            if is_group {
                send_obj.insert("grid".into(), json!(clean_target));
                send_obj.insert("visibility".into(), json!(0));
            } else {
                send_obj.insert("toid".into(), json!(clean_target));
            }
            let params = s.encoded_params(&Value::Object(send_obj))?;
            let url = format!("{file_host}{send_ep}?{}", s.common_params());
            let r = s.post_form(&url, format!("params={params}")).await?;
            s.maybe_decrypt(r, s.session_key_bytes()).await
        } else {
            // Fallback: single-shot `/api/message/photo` or `/api/group/sendpt`
            let single_ep = if is_group {
                "/api/group/sendpt"
            } else {
                "/api/message/photo"
            };
            let mut body_obj = serde_json::Map::new();
            body_obj.insert("clientId".into(), json!(client_id));
            body_obj.insert("desc".into(), json!(caption));
            body_obj.insert("width".into(), json!(width));
            body_obj.insert("height".into(), json!(height));
            body_obj.insert("imei".into(), json!(s.imei));
            if is_group {
                body_obj.insert("grid".into(), json!(clean_target));
                body_obj.insert("visibility".into(), json!(0));
            } else {
                body_obj.insert("toid".into(), json!(clean_target));
            }
            let params = s.encoded_params(&Value::Object(body_obj))?;
            let url = format!(
                "{file_host}{single_ep}?{}&params={params}&type={target_type}",
                s.common_params()
            );
            let r = s
                .post_multipart(&url, "fileContent", &file_name, mime, &file_bytes)
                .await?;
            s.maybe_decrypt(r, s.session_key_bytes()).await
        };

        if let Some(code) = send_resp.get("error_code").and_then(Value::as_i64) {
            if code != 0 {
                let msg = send_resp
                    .get("error_message")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown");
                return Err(crate::session::SessionError::new(code, msg).into());
            }
        }

        let final_thumb = if !thumb_url.is_empty() {
            thumb_url
        } else {
            format!("local://{}", file_path.display())
        };
        let final_url = if is_original_hd && !ori_url.is_empty() {
            ori_url
        } else if !hd_url.is_empty() {
            hd_url
        } else if !normal_url.is_empty() {
            normal_url
        } else {
            file_path.display().to_string()
        };
        // Pre-populate `stickers_cache/thumb_<md5>.png` with the local image bytes so the GUI
        // renders the sent photo immediately without an extra network round-trip.
        let _ = std::fs::create_dir_all("stickers_cache");
        let cache_key = format!("thumb_{}", crate::crypto::md5_hex(&final_thumb));
        let _ = std::fs::write(format!("stickers_cache/{cache_key}.png"), &file_bytes);

        let msg_id = send_resp
            .get("data")
            .and_then(|d| pick_str(d, &["msgId"]))
            .unwrap_or_else(|| format!("out_{client_id}"));
        let now_str = crate::ui::clock();
        let display_text = if !caption.trim().is_empty() {
            caption.trim().to_string()
        } else {
            "[Hình ảnh]".to_string()
        };
        let clean_title = if file_name.starts_with("clipboard_")
            || file_name.starts_with("msedge_")
            || file_name.starts_with("ctdozlite_")
        {
            format!(
                "Hình ảnh ({width}×{height} · {})",
                format_file_size(total_size)
            )
        } else {
            format!(
                "{file_name} ({width}×{height} · {})",
                format_file_size(total_size)
            )
        };
        let cm = ChatMessage {
            id: msg_id,
            client_message_id: client_id.to_string(),
            sender: "Tôi".into(),
            text: display_text.clone(),
            time: now_str,
            is_outgoing: true,
            ts: client_id as i64,
            link_url: Some(final_url),
            link_title: Some(clean_title),
            link_desc: if caption.trim().is_empty() {
                None
            } else {
                Some(caption.trim().to_string())
            },
            link_thumb: Some(final_thumb),
            link_src: Some("Hình ảnh Zalo".into()),
            ..Default::default()
        };
        return Ok((send_resp, cm));
    }

    // Video / Audio / Document / Archive upload (`asyncfile/upload` -> `asyncfile/msg`)
    // Zalo server processes the uploaded file asynchronously and emits a WebSocket notification
    // (`cmd = 601`, `subCmd = 0`, `act_type = "file_done"`) containing the CDN fileUrl.
    // That fileUrl is mandatory for `/api/message/asyncfile/msg` (or `/api/group/asyncfile/msg`).
    let mime = match ext.as_str() {
        "mp4" => "video/mp4",
        "mov" => "video/quicktime",
        "mkv" => "video/x-matroska",
        "webm" => "video/webm",
        "mp3" => "audio/mpeg",
        "wav" => "audio/wav",
        "pdf" => "application/pdf",
        "zip" => "application/zip",
        "txt" | "md" | "log" => "text/plain",
        _ => "application/octet-stream",
    };
    let checksum = md5_hex_bytes(&file_bytes);
    let upload_ep = if is_group {
        "/api/group/asyncfile/upload"
    } else {
        "/api/message/asyncfile/upload"
    };

    let mut last_up_dec = Value::Null;
    for (idx, chunk) in file_bytes.chunks(chunk_size).enumerate() {
        let chunk_id = idx + 1;
        let mut m = serde_json::Map::new();
        m.insert("totalChunk".into(), json!(total_chunks));
        m.insert("fileName".into(), json!(file_name));
        m.insert("clientId".into(), json!(client_id));
        m.insert("totalSize".into(), json!(total_size));
        m.insert("imei".into(), json!(s.imei));
        m.insert("isE2EE".into(), json!(0));
        m.insert("chunkId".into(), json!(chunk_id));
        if is_group {
            m.insert("grid".into(), json!(clean_target));
        } else {
            m.insert("toid".into(), json!(clean_target));
        }
        let params = s.encoded_params(&Value::Object(m))?;
        let url = format!(
            "{file_host}{upload_ep}?{}&params={params}&type={target_type}",
            s.common_params()
        );
        let up_raw = s
            .post_multipart(&url, "chunkContent", &file_name, mime, chunk)
            .await
            .map_err(|e| anyhow::anyhow!("tải lên phần {chunk_id}/{total_chunks} thất bại: {e}"))?;
        last_up_dec = s.maybe_decrypt(up_raw, s.session_key_bytes()).await;
        if let Some(code) = last_up_dec.get("error_code").and_then(Value::as_i64) {
            if code != 0 {
                let msg = last_up_dec
                    .get("error_message")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown");
                return Err(crate::session::SessionError::new(code, msg).into());
            }
        }
    }

    let data_node = last_up_dec.get("data").unwrap_or(&last_up_dec);
    let file_id = pick_str(data_node, &["fileId", "clientFileId"]).filter(|id| id != "-1");
    let client_file_id = data_node
        .get("clientFileId")
        .cloned()
        .unwrap_or_else(|| json!(client_id));
    let mut file_url = pick_str(data_node, &["fileUrl", "url", "normalUrl"]).unwrap_or_default();
    if file_url.is_empty() && FILE_DONE_LISTENER_ACTIVE.load(std::sync::atomic::Ordering::Relaxed) {
        if let Some(fid) = file_id.as_deref() {
            match wait_for_file_done(fid, FILE_DONE_TIMEOUT).await {
                Some(url) => file_url = url,
                None => crate::logger::log_error(
                    "chat::send_attachment",
                    &format!("no file_done event for fileId {fid} within {FILE_DONE_TIMEOUT:?}"),
                ),
            }
        }
    }

    let send_resp = if let Some(fid) = file_id {
        let msg_ep = if is_group {
            "/api/group/asyncfile/msg"
        } else {
            "/api/message/asyncfile/msg"
        };
        let mut msg_obj = serde_json::Map::new();
        msg_obj.insert("fileId".into(), json!(fid));
        msg_obj.insert("checksum".into(), json!(checksum));
        msg_obj.insert("checksumSha".into(), json!(""));
        msg_obj.insert("extention".into(), json!(ext));
        msg_obj.insert("extension".into(), json!(ext));
        msg_obj.insert("totalSize".into(), json!(total_size));
        msg_obj.insert("fileName".into(), json!(file_name));
        msg_obj.insert("clientId".into(), client_file_id);
        msg_obj.insert("fType".into(), json!(if is_video { 2 } else { 1 }));
        msg_obj.insert("fileCount".into(), json!(0));
        msg_obj.insert("fdata".into(), json!("{}"));
        if is_group {
            msg_obj.insert("grid".into(), json!(clean_target));
        } else {
            msg_obj.insert("toid".into(), json!(clean_target));
        }
        msg_obj.insert("fileUrl".into(), json!(file_url));
        msg_obj.insert("zsource".into(), json!(-1));
        msg_obj.insert("ttl".into(), json!(0));

        let params = s.encoded_params(&Value::Object(msg_obj))?;
        let url = format!("{file_host}{msg_ep}?{}", s.common_params());
        let r = s.post_form(&url, format!("params={params}")).await?;
        s.maybe_decrypt(r, s.session_key_bytes()).await
    } else {
        json!({ "error_code": -1, "error_message": "Không nhận được fileId từ máy chủ Zalo" })
    };

    if send_resp
        .get("error_code")
        .and_then(Value::as_i64)
        .unwrap_or(-1)
        != 0
    {
        eprintln!("[send_attachment] asyncfile/msg failed: {send_resp:?}");
    }

    if let Some(code) = send_resp.get("error_code").and_then(Value::as_i64) {
        if code != 0 {
            let msg = send_resp
                .get("error_message")
                .and_then(Value::as_str)
                .unwrap_or("unknown");
            return Err(crate::session::SessionError::new(code, msg).into());
        }
    }

    let final_url = if !file_url.is_empty() {
        file_url
    } else {
        send_resp
            .get("data")
            .and_then(|d| pick_str(d, &["fileUrl", "url", "href"]))
            .unwrap_or_default()
    };
    let msg_id = send_resp
        .get("data")
        .and_then(|d| pick_str(d, &["msgId"]))
        .unwrap_or_else(|| format!("out_{client_id}"));
    let kind_label = if is_video { "Video" } else { "Tập tin" };
    let display_text = if !caption.trim().is_empty() {
        caption.trim().to_string()
    } else {
        format!("[{kind_label}] {file_name}")
    };
    let cm = ChatMessage {
        id: msg_id,
        client_message_id: client_id.to_string(),
        sender: "Tôi".into(),
        text: display_text,
        time: crate::ui::clock(),
        is_outgoing: true,
        ts: client_id as i64,
        link_url: if final_url.is_empty() {
            None
        } else {
            Some(final_url)
        },
        link_title: Some(format!("{file_name} ({})", format_file_size(total_size))),
        link_desc: if caption.trim().is_empty() {
            None
        } else {
            Some(caption.trim().to_string())
        },
        link_src: Some(format!("{kind_label} Zalo")),
        ..Default::default()
    };
    Ok((send_resp, cm))
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct LinkMetadata {
    pub url: String,
    pub title: String,
    pub desc: String,
    pub thumb: String,
    pub src: String,
}

/// Extract the first `https://` or `http://` URL from a message text string.
pub fn extract_first_url(text: &str) -> Option<String> {
    let trimmed = text.trim();
    let pos = trimmed
        .find("https://")
        .or_else(|| trimmed.find("http://"))?;
    let after = &trimmed[pos..];
    let end = after.find(char::is_whitespace).unwrap_or(after.len());
    let url = after[..end].trim_end_matches(|c: char| matches!(c, ')' | ']' | ',' | ';'));
    if url.len() >= 10 {
        Some(url.to_string())
    } else {
        None
    }
}

fn unescape_html_entities(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&#x27;", "'")
        .replace("&apos;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&nbsp;", " ")
        .trim()
        .to_string()
}

fn extract_meta_attr(tag: &str, attr_name: &str) -> Option<String> {
    let lower = tag.to_ascii_lowercase();
    for pat in [
        format!("{attr_name}=\""),
        format!("{attr_name}='"),
        format!("{attr_name} = \""),
    ] {
        if let Some(idx) = lower.find(&pat) {
            let quote_char = pat.chars().last().unwrap_or('"');
            let rest = &tag[idx + pat.len()..];
            if let Some(end) = rest.find(quote_char) {
                let val = unescape_html_entities(&rest[..end]);
                if !val.is_empty() {
                    return Some(val);
                }
            }
        }
    }
    None
}

/// Scrape OpenGraph / Twitter Card / HTML `<meta>` tags directly from a webpage URL.
pub async fn scrape_opengraph_metadata(url: &str) -> Option<LinkMetadata> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_millis(3800))
        .redirect(reqwest::redirect::Policy::limited(5))
        .build()
        .ok()?;
    let resp = client
        .get(url)
        .header(
            "User-Agent",
            "Mozilla/5.0 (compatible; Discordbot/2.0; +https://discordapp.com)",
        )
        .header("Accept-Language", "vi-VN,vi;q=0.9,en-US;q=0.8")
        .send()
        .await
        .ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let bytes = resp.bytes().await.ok()?;
    let take_len = bytes.len().min(131_072);
    let html = String::from_utf8_lossy(&bytes[..take_len]);

    let mut og_title = String::new();
    let mut og_desc = String::new();
    let mut og_thumb = String::new();
    let mut og_site = String::new();

    let mut cursor = html.as_ref();
    while let Some(start) = cursor.find('<') {
        let after = &cursor[start..];
        let Some(end) = after.find('>') else {
            break;
        };
        let tag = &after[..=end];
        cursor = &after[end + 1..];

        if tag
            .get(1..5)
            .map_or(false, |s| s.eq_ignore_ascii_case("meta"))
        {
            let key = extract_meta_attr(tag, "property")
                .or_else(|| extract_meta_attr(tag, "name"))
                .unwrap_or_default()
                .to_ascii_lowercase();
            let Some(content) = extract_meta_attr(tag, "content") else {
                continue;
            };
            match key.as_str() {
                "og:title" | "twitter:title" | "lark:url:video_title" if og_title.is_empty() => {
                    og_title = content;
                }
                "og:description" | "twitter:description" | "description" if og_desc.is_empty() => {
                    og_desc = content;
                }
                "og:image" | "twitter:image" | "lark:url:video_cover_image_url"
                    if og_thumb.is_empty() && content.starts_with("http") =>
                {
                    og_thumb = content;
                }
                "og:site_name" if og_site.is_empty() => {
                    og_site = content;
                }
                _ => {}
            }
        } else if og_title.is_empty()
            && tag
                .get(1..6)
                .map_or(false, |s| s.eq_ignore_ascii_case("title"))
        {
            if let Some(close) = cursor.find("</") {
                let raw_t = unescape_html_entities(&cursor[..close]);
                if !raw_t.is_empty() {
                    og_title = raw_t;
                }
            }
        }
    }

    let domain = url
        .split("://")
        .nth(1)
        .and_then(|r| r.split('/').next())
        .unwrap_or("Liên kết")
        .to_string();
    Some(LinkMetadata {
        url: url.to_string(),
        title: og_title,
        desc: og_desc,
        thumb: og_thumb,
        src: if !og_site.is_empty() { og_site } else { domain },
    })
}

fn is_placeholder_zalo_thumb(thumb: &str) -> bool {
    thumb.is_empty()
        || thumb.contains("feed_thumb_link")
        || thumb.contains("im_parselink")
        || thumb.contains("aHR0cHM6Ly9yZXMtemFsby56YWRuLnZuL3VwbG9hZC9tZWRpYS8yMDE5LzEwLzE1L2ZlZWRfdGh1bWJfbGlua19fMV9fMTU3MTEzMzEyMjc3OF85Mzc4MC5wbmc=")
}

/// Parse rich link metadata (`title`, `desc`, `thumb`, `src`) via Zalo's `/api/message/parselink`
/// (`cmd 11285`) combined with direct OpenGraph HTML scraping fallback.
pub async fn parse_link_metadata(s: &Session, raw_url: &str) -> Result<LinkMetadata> {
    let url = raw_url.trim().to_string();
    let domain = url
        .split("://")
        .nth(1)
        .and_then(|r| r.split('/').next())
        .unwrap_or("Liên kết")
        .to_string();
    let mut meta = LinkMetadata {
        url: url.clone(),
        title: String::new(),
        desc: String::new(),
        thumb: String::new(),
        src: domain.clone(),
    };

    if s.login.is_some() {
        let file_host = if s.domains.file.is_empty() {
            "https://tt-files-wpa.chat.zalo.me"
        } else {
            s.domains.file.trim_end_matches('/')
        };
        let body = obj(&[
            ("link", json!(url)),
            ("version", json!(1)),
            ("imei", json!(s.imei)),
        ]);
        if let Ok(params) = s.encoded_params(&body) {
            let parse_url = format!(
                "{file_host}/api/message/parselink?{}&params={params}",
                s.common_params()
            );
            if let Ok(resp) = s.get(&parse_url).await {
                let dec = s.maybe_decrypt(resp, s.session_key_bytes()).await;
                let d = dec
                    .get("data")
                    .and_then(|x| x.get("data").or(Some(x)))
                    .unwrap_or(&dec);
                if let Some(t) = d.get("title").and_then(Value::as_str).map(str::trim) {
                    if !t.is_empty() && t != url {
                        meta.title = t.to_string();
                    }
                }
                if let Some(ds) = d.get("desc").and_then(Value::as_str).map(str::trim) {
                    if !ds.is_empty() && ds != url {
                        meta.desc = ds.to_string();
                    }
                }
                if let Some(th) = d.get("thumb").and_then(Value::as_str).map(str::trim) {
                    if !is_placeholder_zalo_thumb(th) {
                        meta.thumb = th.to_string();
                    }
                }
                if let Some(sr) = d.get("src").and_then(Value::as_str).map(str::trim) {
                    if !sr.is_empty() {
                        meta.src = sr.to_string();
                    }
                }
            }
        }
    }

    if meta.title.is_empty() || meta.thumb.is_empty() || meta.desc.is_empty() {
        if let Some(og) = scrape_opengraph_metadata(&url).await {
            if meta.title.is_empty() && !og.title.is_empty() {
                meta.title = og.title;
            }
            if meta.desc.is_empty() && !og.desc.is_empty() {
                meta.desc = og.desc;
            }
            if meta.thumb.is_empty() && !og.thumb.is_empty() {
                meta.thumb = og.thumb;
            }
            if (meta.src.is_empty() || meta.src == domain) && !og.src.is_empty() {
                meta.src = og.src;
            }
        }
    }

    if meta.title.is_empty() {
        meta.title = domain;
    }
    Ok(meta)
}

/// Send a rich link message (`sendLinkMessage` @ `compact-app-pc.js:14319196`):
/// - 1-1 (`cmd 11286`): `POST {domains.chat}/api/message/link?{common}`
/// - Group (`cmd 11193`): `POST {domains.group}/api/group/sendlink?{common}`
pub async fn send_link_message(
    s: &Session,
    to_id: &str,
    text: &str,
    meta: &LinkMetadata,
    is_group: bool,
) -> Result<Value> {
    send_link_message_with_client_id(s, to_id, text, meta, is_group, next_client_message_id()).await
}

/// Preserve the request client ID for reconciliation with a later server echo.
pub(crate) async fn send_link_message_with_client_id(
    s: &Session,
    to_id: &str,
    text: &str,
    meta: &LinkMetadata,
    is_group: bool,
    now: u64,
) -> Result<Value> {
    if s.login.is_none() {
        anyhow::bail!("chưa đăng nhập");
    }
    let msg_str = if text.trim().is_empty() {
        meta.url.as_str()
    } else {
        text.trim()
    };
    let title_str = if meta.title.trim().is_empty() {
        meta.url.as_str()
    } else {
        meta.title.trim()
    };
    let media_json = json!({
        "type": 0,
        "count": 0,
        "mediaTitle": "",
        "artist": "",
        "streamUrl": "",
        "stream_icon": ""
    })
    .to_string();

    let res: Result<Value> = async {
        if is_group {
            let clean_grid = to_id
                .trim_start_matches("group_")
                .trim_start_matches('g')
                .trim_start_matches('G');
            let body = obj(&[
                ("grid", json!(clean_grid)),
                ("msg", json!(msg_str)),
                ("href", json!(meta.url)),
                ("src", json!(meta.src)),
                ("title", json!(title_str)),
                ("desc", json!(meta.desc)),
                ("thumb", json!(meta.thumb)),
                ("type", json!(0)),
                ("media", json!(media_json)),
                ("clientId", json!(now)),
                ("mentionInfo", json!("[]")),
                ("language", json!("vi")),
                ("ttl", json!(0)),
                ("imei", json!(s.imei)),
            ]);
            let params = s.encoded_params(&body)?;
            let url = format!(
                "{}/api/group/sendlink?{}",
                s.domains.group,
                s.common_params()
            );
            let resp = s.post_form(&url, format!("params={params}")).await?;
            let dec = s.maybe_decrypt(resp, s.session_key_bytes()).await;
            if let Some(code) = dec.get("error_code").and_then(Value::as_i64) {
                if code != 0 {
                    anyhow::bail!("sendlink group error {code}");
                }
            }
            Ok(dec)
        } else {
            let host = if s.domains.chat.contains("-msg-chat.")
                || s.domains.chat.contains("tt-chat2-wpa")
                || s.domains.chat.is_empty()
            {
                "https://tt-chat3-wpa.api.zaloapp.com"
            } else {
                &s.domains.chat
            };
            let body = obj(&[
                ("toId", json!(to_id)),
                ("msg", json!(msg_str)),
                ("href", json!(meta.url)),
                ("src", json!(meta.src)),
                ("title", json!(title_str)),
                ("desc", json!(meta.desc)),
                ("thumb", json!(meta.thumb)),
                ("type", json!(0)),
                ("media", json!(media_json)),
                ("clientId", json!(now)),
                ("mentionInfo", json!("")),
                ("language", json!("vi")),
                ("ttl", json!(0)),
                ("imei", json!(s.imei)),
            ]);
            let params = s.encoded_params(&body)?;
            let url = format!("{host}/api/message/link?{}", s.common_params());
            let resp = s.post_form(&url, format!("params={params}")).await?;
            let dec = s.maybe_decrypt(resp, s.session_key_bytes()).await;
            if let Some(code) = dec.get("error_code").and_then(Value::as_i64) {
                if code != 0 {
                    anyhow::bail!("sendlink error {code}");
                }
            }
            Ok(dec)
        }
    }
    .await;

    match res {
        Ok(v) => Ok(v),
        Err(_) => {
            if is_group {
                send_group_message_with_client_id(s, to_id, msg_str, now).await
            } else {
                send_message_with_client_id(s, to_id, msg_str, now).await
            }
        }
    }
}

/// Send a sticker (`sendSticker` in `compact-app-pc.b7bec208c4c8f73e09f8.js:14325410`):
/// - 1-1 (`cmd 11154`): `POST {domains.chat}/api/message/sticker?{common}`
/// - Group (`cmd 11165`): `POST {domains.group}/api/group/sticker?{common}`
pub async fn send_sticker(
    s: &Session,
    to_id: &str,
    sticker_id: u64,
    cate_id: u64,
    sticker_type: u32,
    is_group: bool,
) -> Result<Value> {
    send_sticker_with_client_id(
        s,
        to_id,
        sticker_id,
        cate_id,
        sticker_type,
        is_group,
        next_client_message_id(),
    )
    .await
}

/// Preserve the request client ID for reconciliation with a later server echo.
pub(crate) async fn send_sticker_with_client_id(
    s: &Session,
    to_id: &str,
    sticker_id: u64,
    cate_id: u64,
    sticker_type: u32,
    is_group: bool,
    now: u64,
) -> Result<Value> {
    if s.login.is_none() {
        anyhow::bail!("chưa đăng nhập");
    }

    let url = if is_group {
        let clean_grid = to_id
            .trim_start_matches("group_")
            .trim_start_matches('g')
            .trim_start_matches('G');
        let body = obj(&[
            ("grid", json!(clean_grid)),
            ("stickerId", json!(sticker_id)),
            ("cateId", json!(cate_id)),
            ("type", json!(sticker_type)),
            ("clientId", json!(now)),
            ("imei", json!(s.imei)),
            ("zsource", json!(101)),
        ]);
        let params = s.encoded_params(&body)?;
        let url = format!(
            "{}/api/group/sticker?{}",
            s.domains.group,
            s.common_params()
        );
        let form = format!("params={params}");
        let resp = s.post_form(&url, form).await?;
        let resp = s.maybe_decrypt(resp, s.session_key_bytes()).await;
        if let Some(code) = resp.get("error_code").and_then(Value::as_i64) {
            if code != 0 {
                let msg = resp
                    .get("error_message")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown");
                return Err(crate::session::SessionError::new(code, msg).into());
            }
        }
        return Ok(resp);
    } else {
        let host = if s.domains.chat.contains("-msg-chat.")
            || s.domains.chat.contains("tt-chat2-wpa")
            || s.domains.chat.is_empty()
        {
            "https://tt-chat3-wpa.api.zaloapp.com"
        } else {
            &s.domains.chat
        };
        format!("{host}/api/message/sticker?{}", s.common_params())
    };

    let body = obj(&[
        ("toid", json!(to_id)),
        ("stickerId", json!(sticker_id)),
        ("cateId", json!(cate_id)),
        ("type", json!(sticker_type)),
        ("clientId", json!(now)),
        ("imei", json!(s.imei)),
        ("zsource", json!(101)),
    ]);
    let params = s.encoded_params(&body)?;
    let form = format!("params={params}");
    let resp = s.post_form(&url, form).await?;
    let resp = s.maybe_decrypt(resp, s.session_key_bytes()).await;
    if let Some(code) = resp.get("error_code").and_then(Value::as_i64) {
        if code != 0 {
            let msg = resp
                .get("error_message")
                .and_then(Value::as_str)
                .unwrap_or("unknown");
            return Err(crate::session::SessionError::new(code, msg).into());
        }
    }
    Ok(resp)
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StickerPackInfo {
    pub id: u64,
    pub name: String,
    pub total_image: u32,
    pub icon_url: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StickerItem {
    pub sticker_id: u64,
    pub cate_id: u64,
    pub sticker_type: u32,
    pub label: String,
}

/// Fetch the complete Zalo Sticker Store catalog (`getStickerCatsV2` -> `getAllCate` in `compact-app-pc.js:14288962`).
///
/// Returns ~390 official Zalo sticker packs sorted by newest/featured first and caches them to
/// `stickers_cache/store_packs.json`.
pub async fn fetch_sticker_store_packs(s: &Session) -> Result<Vec<StickerPackInfo>> {
    let cache_dir = std::path::Path::new("stickers_cache");
    let _ = std::fs::create_dir_all(cache_dir);
    let cache_path = cache_dir.join("store_packs.json");
    if let Ok(raw_cache) = std::fs::read_to_string(&cache_path) {
        if let Ok(cached) = serde_json::from_str::<Vec<StickerPackInfo>>(&raw_cache) {
            if cached.len() >= 20 {
                return Ok(cached);
            }
        }
    }

    if s.login.is_none() {
        anyhow::bail!("chưa đăng nhập");
    }
    let sticker_host = if s.domains.sticker.is_empty() {
        "https://tt-sticker-wpa.chat.zalo.me"
    } else {
        s.domains.sticker.trim_end_matches('/')
    };
    let key = s.session_key_bytes();
    if key.is_empty() {
        anyhow::bail!("thiếu session key để tải kho nhãn dán");
    }
    let enc_empty = crate::crypto::encode_aes(key, "")?;
    let mut encoded_param = String::with_capacity(enc_empty.len() + 16);
    for b in enc_empty.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded_param.push(b as char);
            }
            _ => {
                use std::fmt::Write;
                let _ = write!(encoded_param, "%{b:02X}");
            }
        }
    }

    let list_url = format!(
        "{sticker_host}/api/message/sticker/category/list/v2?{}&params={encoded_param}",
        s.common_params()
    );
    let resp = s.get(&list_url).await?;
    let dec = s.maybe_decrypt(resp, key).await;
    let data = dec.get("data").unwrap_or(&dec);

    let ex_ids: std::collections::HashSet<u64> = data
        .get("exCateIds")
        .and_then(Value::as_array)
        .map(|arr| arr.iter().filter_map(Value::as_u64).collect())
        .unwrap_or_default();

    let data_url = data
        .get("dataUrl")
        .and_then(Value::as_str)
        .unwrap_or("https://zalo-api.zadn.vn/api/emoticon/pcweb/getAllCate")
        .replace("http://", "https://");

    let cdn_resp = s.http.get(&data_url).send().await?;
    let raw_bytes = cdn_resp.bytes().await?;
    let json_text = if raw_bytes.len() >= 2 && raw_bytes[0] == 0x1f && raw_bytes[1] == 0x8b {
        use std::io::Read;
        let mut gz = flate2::read::GzDecoder::new(&raw_bytes[..]);
        let mut out = String::new();
        gz.read_to_string(&mut out)?;
        out
    } else {
        String::from_utf8_lossy(&raw_bytes).into_owned()
    };

    let arr: Vec<Value> = serde_json::from_str(&json_text)?;
    let mut packs = Vec::with_capacity(arr.len());
    for item in arr {
        let Some(id) = item.get("id").and_then(Value::as_u64) else {
            continue;
        };
        if id == 0 || ex_ids.contains(&id) {
            continue;
        }
        let name = item
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim()
            .to_string();
        if name.is_empty() || name.to_lowercase().contains("test") {
            continue;
        }
        let total_image = item.get("totalImage").and_then(Value::as_u64).unwrap_or(16) as u32;
        let icon_url = item
            .get("iconUrl")
            .or_else(|| item.get("thumbImg"))
            .and_then(Value::as_str)
            .unwrap_or("")
            .replace("http://", "https://");
        packs.push(StickerPackInfo {
            id,
            name,
            total_image,
            icon_url,
        });
    }

    // Sort newest packs first (`id` descending) so modern Zalo packs appear at the top of the store
    packs.sort_by(|a, b| b.id.cmp(&a.id));
    if let Ok(txt) = serde_json::to_string(&packs) {
        let _ = std::fs::write(&cache_path, txt);
    }
    Ok(packs)
}

/// Fetch all stickers belonging to a specific category (`getStickerCategoryDetail` — `cmd 11284`).
pub async fn fetch_sticker_pack_items(
    s: &Session,
    cate_id: u64,
    pack_name: &str,
) -> Result<Vec<StickerItem>> {
    if cate_id == 0 {
        return Ok((1..=30)
            .map(|id| StickerItem {
                sticker_id: id,
                cate_id: 0,
                sticker_type: 7,
                label: format!("Củ hành #{id}"),
            })
            .collect());
    }

    let cache_dir = std::path::Path::new("stickers_cache");
    let _ = std::fs::create_dir_all(cache_dir);
    let cache_path = cache_dir.join(format!("pack_{cate_id}.json"));
    if let Ok(raw_cache) = std::fs::read_to_string(&cache_path) {
        if let Ok(cached) = serde_json::from_str::<Vec<StickerItem>>(&raw_cache) {
            if !cached.is_empty() {
                return Ok(cached);
            }
        }
    }

    if s.login.is_none() {
        anyhow::bail!("chưa đăng nhập");
    }
    let sticker_host = if s.domains.sticker.is_empty() {
        "https://tt-sticker-wpa.chat.zalo.me"
    } else {
        s.domains.sticker.trim_end_matches('/')
    };
    let params = s.encoded_params(&json!({ "cid": cate_id.to_string() }))?;
    let url = format!(
        "{sticker_host}/api/message/sticker/category/sticker_detail?{}&params={params}",
        s.common_params()
    );
    let resp = s.get(&url).await?;
    let dec = s.maybe_decrypt(resp, s.session_key_bytes()).await;
    let arr = dec
        .get("data")
        .and_then(Value::as_array)
        .or_else(|| dec.as_array())
        .cloned()
        .unwrap_or_default();

    let mut items = Vec::with_capacity(arr.len());
    for (idx, it) in arr.iter().enumerate() {
        let Some(sid) = it
            .get("id")
            .or_else(|| it.get("stickerId"))
            .and_then(Value::as_u64)
        else {
            continue;
        };
        let cid = it.get("cateId").and_then(Value::as_u64).unwrap_or(cate_id);
        let stype = it.get("type").and_then(Value::as_u64).unwrap_or(7) as u32;
        items.push(StickerItem {
            sticker_id: sid,
            cate_id: cid,
            sticker_type: stype,
            label: format!("{pack_name} #{}", idx + 1),
        });
    }

    if !items.is_empty() {
        if let Ok(txt) = serde_json::to_string(&items) {
            let _ = std::fs::write(&cache_path, txt);
        }
    }
    Ok(items)
}

/// Normalize a sticker keyword by converting non-breaking spaces (`\u{a0}`), collapsing
/// consecutive whitespace to a single ASCII space, and converting to lowercase.
pub fn normalize_sticker_keyword(s: &str) -> String {
    s.replace('\u{a0}', " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

/// Strip Vietnamese diacritics to lowercase ASCII (`simpleStripVietnamese` in `compact-app-pc.js:12955017`).
pub fn strip_vn_for_sticker_kw(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for raw in s.chars() {
        for c in raw.to_lowercase() {
            let mapped = match c {
                'à' | 'á' | 'ạ' | 'ả' | 'ã' | 'â' | 'ầ' | 'ấ' | 'ậ' | 'ẩ' | 'ẫ' | 'ă' | 'ằ'
                | 'ắ' | 'ặ' | 'ẳ' | 'ẵ' => 'a',
                'è' | 'é' | 'ẹ' | 'ẻ' | 'ẽ' | 'ê' | 'ề' | 'ế' | 'ệ' | 'ể' | 'ễ' => {
                    'e'
                }
                'ì' | 'í' | 'ị' | 'ỉ' | 'ĩ' => 'i',
                'ò' | 'ó' | 'ọ' | 'ỏ' | 'õ' | 'ô' | 'ồ' | 'ố' | 'ộ' | 'ổ' | 'ỗ' | 'ơ' | 'ờ'
                | 'ớ' | 'ợ' | 'ở' | 'ỡ' => 'o',
                'ù' | 'ú' | 'ụ' | 'ủ' | 'ũ' | 'ư' | 'ừ' | 'ứ' | 'ự' | 'ử' | 'ữ' => {
                    'u'
                }
                'ỳ' | 'ý' | 'ỵ' | 'ỷ' | 'ỹ' => 'y',
                'đ' => 'd',
                '\u{a0}' => ' ',
                other => other,
            };
            out.push(mapped);
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Map common unaccented Vietnamese keywords, English terms, or chat slang to the accented
/// Vietnamese form indexed by Zalo's `/api/message/sticker/suggest/stickers` (`cmd 11741`) and
/// `/api/message/sticker/search` (`cmd 11747`) endpoints.
pub fn canonical_accented_vn_for_sticker(stripped: &str) -> Option<&'static str> {
    match stripped {
        "nho" | "nho em" | "nho anh" | "nho qua" | "miss" | "miss u" | "miss you" => Some("nhớ"),
        "khoc" | "huhu" | "huhuhu" | "hic" | "cry" => Some("khóc"),
        "buon" | "buon qua" | "sad" | "tui than" => Some("buồn"),
        "vui" | "vui ve" | "vui qua" | "happy" => Some("vui"),
        "cuoi" | "mac cuoi" | "buon cuoi" => Some("cười"),
        "yeu" | "iu" | "yeu em" | "yeu anh" | "iu em" | "iu anh" | "love" | "love u"
        | "love you" | "tha tim" | "tim" => Some("yêu"),
        "thuong" | "thuong em" | "thuong anh" | "thuong qua" => Some("thương"),
        "gian" | "gian doi" | "doi hon" | "tuc" | "buc" | "buc minh" | "cay" | "angry" => {
            Some("giận")
        }
        "met" | "met qua" | "met moi" | "duoi" | "duoi qua" | "tired" => Some("mệt"),
        "doi" | "doi bung" | "doi qua" | "hungry" => Some("đói"),
        "an" | "an com" | "di an" => Some("ăn cơm"),
        "ca phe" | "cf" | "cafe" | "coffee" => Some("cà phê"),
        "tra sua" | "trasua" => Some("trà sữa"),
        "nhau" | "di nhau" | "bia" | "dzoo" | "dzo" => Some("nhậu"),
        "ngu" | "di ngu" | "buon ngu" | "ngu ngon" | "ngu ngon nhe" | "ngu ngon nha"
        | "good night" | "goodnight" | "gn" | "g9" => Some("ngủ"),
        "chao" | "xin chao" | "chao ban" | "chao anh" | "chao chi" | "chao em" | "he lo"
        | "helo" => Some("xin chào"),
        "cam on" | "cam on ban" | "cam on anh" | "cam on chi" | "cam on em" | "cam on nhieu"
        | "thank" | "thanks" | "thank you" | "tks" | "tkss" | "thank kiu" => Some("cảm ơn"),
        "xin loi" | "xin loi nha" | "xin loi nhe" | "sorry" | "sr" | "so ry" => Some("xin lỗi"),
        "tam biet" | "tam biet nhe" | "bye" | "byebye" | "good bye" | "goodbye" | "bibi" | "pp" => {
            Some("tạm biệt")
        }
        "chuc mung" | "happy birthday" | "hpbd" | "sinh nhat" | "cmsn" => Some("chúc mừng"),
        "tet" | "chuc mung nam moi" | "nam moi" => Some("tết"),
        "co len" | "co gang" | "fighting" => Some("cố lên"),
        "tuyet" | "tuyet voi" | "xuat sac" | "dinh" | "dinh qua" | "chuan" | "duoc" => {
            Some("tuyệt vời")
        }
        "ngai" | "ngai qua" | "ngai nhi" | "xau ho" => Some("ngại"),
        "so" | "so qua" | "scared" => Some("sợ"),
        "lanh" | "lanh qua" | "lanh khong" | "ret" | "ret qua" => Some("lạnh"),
        "nong" | "nong qua" | "nong buc" => Some("nóng"),
        "tien" | "luong" => Some("tiền"),
        "het tien" | "ngheo" | "ngheo qua" | "vi rong" => Some("hết tiền"),
        "quy" | "lay" | "xin" | "van xin" => Some("lạy"),
        "ban" | "ban qua" | "busy" => Some("bận"),
        "chan" | "chan qua" | "chan doi" | "boring" => Some("chán"),
        "cho" | "cho chut" | "cho xiu" | "doi chut" | "doi xiu" | "wait" => Some("chờ chút"),
        "di choi" | "choi" | "dao pho" => Some("đi chơi"),
        "di lam" | "lam viec" | "deadline" | "code" => Some("làm"),
        "hong" | "hong chuyen" | "ngac nhien" => Some("ngạc nhiên"),
        _ => None,
    }
}

/// Build the deduplicated list of query variants (accented + unaccented + canonical synonym)
/// to maximize recall across both Zalo Sticker Suggest (`cmd 11741`) and Search (`cmd 11747`).
pub fn canonical_vn_sticker_queries(keyword: &str) -> Vec<String> {
    let norm = normalize_sticker_keyword(keyword);
    if norm.is_empty() {
        return Vec::new();
    }
    let stripped = strip_vn_for_sticker_kw(&norm);
    let mut queries = vec![norm.clone()];
    if !stripped.is_empty() && stripped != norm && !queries.contains(&stripped) {
        queries.push(stripped.clone());
    }
    if let Some(accented) = canonical_accented_vn_for_sticker(&stripped) {
        let acc_s = accented.to_string();
        if !queries.contains(&acc_s) {
            queries.push(acc_s.clone());
        }
        let acc_stripped = strip_vn_for_sticker_kw(&acc_s);
        if !acc_stripped.is_empty() && !queries.contains(&acc_stripped) {
            queries.push(acc_stripped);
        }
    }
    queries.truncate(4);
    queries
}

/// Fetch official Zalo contextual sticker trigger keywords (`getStickerListKeyWords` — `cmd 11740`).
/// Includes both normalized accented/raw forms and ASCII-folded forms so typing accented or
/// unaccented Vietnamese triggers suggestions reliably.
pub async fn fetch_sticker_suggest_keywords(s: &Session) -> Result<Vec<String>> {
    let cache_dir = std::path::Path::new("stickers_cache");
    let _ = std::fs::create_dir_all(cache_dir);
    let cache_path = cache_dir.join("suggest_keywords_v2.json");
    if let Ok(raw_cache) = std::fs::read_to_string(&cache_path) {
        if let Ok(cached) = serde_json::from_str::<Vec<String>>(&raw_cache) {
            if cached.len() >= 100 {
                return Ok(cached);
            }
        }
    }
    if s.login.is_none() {
        anyhow::bail!("chưa đăng nhập");
    }
    let sticker_host = if s.domains.sticker.is_empty() {
        "https://tt-sticker-wpa.chat.zalo.me"
    } else {
        s.domains.sticker.trim_end_matches('/')
    };
    let params = s.encoded_params(&json!({ "imei": s.imei }))?;
    let url = format!(
        "{sticker_host}/api/message/sticker/suggest/keywords?{}&params={params}",
        s.common_params()
    );
    let resp = match s.get(&url).await {
        Ok(r) => r,
        Err(_) => {
            let fallback_url = format!(
                "https://tt-sticker-wpa.zaloapp.com/api/message/sticker/suggest/keywords?{}&params={params}",
                s.common_params()
            );
            s.get(&fallback_url).await?
        }
    };
    let dec = s.maybe_decrypt(resp, s.session_key_bytes()).await;
    let mut seen = std::collections::HashSet::new();
    let mut kws: Vec<String> = Vec::new();
    if let Some(arr) = dec
        .get("data")
        .or(Some(&dec))
        .and_then(|d| d.get("keywords"))
        .and_then(Value::as_array)
    {
        for raw_v in arr {
            let Some(raw_str) = raw_v.as_str() else {
                continue;
            };
            let norm = normalize_sticker_keyword(raw_str);
            if norm.is_empty() || norm.starts_with("zkwd_") {
                continue;
            }
            if seen.insert(norm.clone()) {
                kws.push(norm.clone());
            }
            let stripped = strip_vn_for_sticker_kw(&norm);
            if !stripped.is_empty() && seen.insert(stripped.clone()) {
                kws.push(stripped);
            }
        }
    }
    if !kws.is_empty() {
        if let Ok(txt) = serde_json::to_string(&kws) {
            let _ = std::fs::write(&cache_path, txt);
        }
    }
    Ok(kws)
}

async fn query_sticker_endpoints_once(
    s: &Session,
    sticker_host: &str,
    queries: &[String],
) -> Vec<(u64, u64, u32, u8)> {
    let mut futs = Vec::with_capacity(queries.len() * 2);
    for (q_idx, q) in queries.iter().enumerate() {
        // 1) Contextual suggest endpoint (`getStickerSuggestion` — `cmd 11741`)
        let sugg_body = obj(&[
            ("keyword", json!(q)),
            ("gif", json!(1)),
            ("guggy", json!(0)),
            ("imei", json!(s.imei)),
        ]);
        if let Ok(params) = s.encoded_params(&sugg_body) {
            let url = format!(
                "{sticker_host}/api/message/sticker/suggest/stickers?{}&params={params}",
                s.common_params()
            );
            let is_primary = q_idx == 0;
            futs.push(async move {
                let Ok(resp) = s.get(&url).await else {
                    return Vec::new();
                };
                let dec = s.maybe_decrypt(resp, s.session_key_bytes()).await;
                let arr = dec
                    .get("data")
                    .or(Some(&dec))
                    .and_then(|d| d.get("sugg_sticker"))
                    .and_then(Value::as_array)
                    .cloned()
                    .unwrap_or_default();
                let mut items = Vec::with_capacity(arr.len());
                for it in arr {
                    let Some(sid) = it
                        .get("sticker_id")
                        .or_else(|| it.get("id"))
                        .and_then(Value::as_u64)
                    else {
                        continue;
                    };
                    let cid = it
                        .get("cate_id")
                        .or_else(|| it.get("cateId"))
                        .and_then(Value::as_u64)
                        .unwrap_or(0);
                    let stype = it.get("type").and_then(Value::as_u64).unwrap_or(7) as u32;
                    items.push((cid, sid, stype, if is_primary { 3u8 } else { 2u8 }));
                }
                items
            });
        }
    }

    let mut search_futs = Vec::with_capacity(queries.len());
    for (q_idx, q) in queries.iter().enumerate() {
        // 2) Full sticker tag search endpoint (`searchSticker` — `cmd 11747`)
        let search_body = obj(&[
            ("keyword", json!(q)),
            ("limit", json!(30)),
            ("srcType", json!(0)),
            ("imei", json!(s.imei)),
        ]);
        if let Ok(params) = s.encoded_params(&search_body) {
            let url = format!(
                "{sticker_host}/api/message/sticker/search?{}&params={params}",
                s.common_params()
            );
            let is_primary = q_idx == 0;
            search_futs.push(async move {
                let Ok(resp) = s.get(&url).await else {
                    return Vec::new();
                };
                let dec = s.maybe_decrypt(resp, s.session_key_bytes()).await;
                let arr = dec
                    .get("data")
                    .and_then(Value::as_array)
                    .or_else(|| dec.as_array())
                    .cloned()
                    .unwrap_or_default();
                let mut items = Vec::with_capacity(arr.len());
                for it in arr {
                    let Some(sid) = it
                        .get("sticker_id")
                        .or_else(|| it.get("id"))
                        .and_then(Value::as_u64)
                    else {
                        continue;
                    };
                    let cid = it
                        .get("cate_id")
                        .or_else(|| it.get("cateId"))
                        .and_then(Value::as_u64)
                        .unwrap_or(0);
                    let stype = it.get("type").and_then(Value::as_u64).unwrap_or(7) as u32;
                    items.push((cid, sid, stype, if is_primary { 3u8 } else { 2u8 }));
                }
                items
            });
        }
    }

    let (sugg_batches, search_batches) = tokio::join!(
        futures_util::future::join_all(futs),
        futures_util::future::join_all(search_futs)
    );

    // Merge and boost stickers that matched across multiple endpoints/queries
    let mut score_map: std::collections::HashMap<u64, (u64, u32, u8)> =
        std::collections::HashMap::new();
    for batch in sugg_batches.into_iter().chain(search_batches.into_iter()) {
        for (cid, sid, stype, weight) in batch {
            score_map
                .entry(sid)
                .and_modify(|e| {
                    e.2 = e.2.saturating_add(weight);
                    if e.0 == 0 && cid > 0 {
                        e.0 = cid;
                    }
                })
                .or_insert((cid, stype, weight));
        }
    }

    score_map
        .into_iter()
        .map(|(sid, (cid, stype, score))| (cid, sid, stype, score))
        .collect()
}

/// Search Zalo stickers by Vietnamese, English, or emoji keyword by querying both
/// `/api/message/sticker/suggest/stickers` (`cmd 11741`) and `/api/message/sticker/search` (`cmd 11747`)
/// concurrently across accented and unaccented Vietnamese variants.
pub async fn search_stickers_by_keyword(s: &Session, keyword: &str) -> Result<Vec<StickerItem>> {
    let kw = normalize_sticker_keyword(keyword);
    if kw.is_empty() {
        return Ok(Vec::new());
    }
    let cache_dir = std::path::Path::new("stickers_cache");
    let _ = std::fs::create_dir_all(cache_dir);
    let cache_path = cache_dir.join(format!("suggest_v2_{}.json", crate::crypto::md5_hex(&kw)));
    if let Ok(raw_cache) = std::fs::read_to_string(&cache_path) {
        if let Ok(cached) = serde_json::from_str::<Vec<StickerItem>>(&raw_cache) {
            if !cached.is_empty() {
                return Ok(cached);
            }
        }
    }
    if s.login.is_none() {
        anyhow::bail!("chưa đăng nhập");
    }
    let sticker_host = if s.domains.sticker.is_empty() {
        "https://tt-sticker-wpa.chat.zalo.me"
    } else {
        s.domains.sticker.trim_end_matches('/')
    };

    let queries = canonical_vn_sticker_queries(&kw);
    let mut raw_items = query_sticker_endpoints_once(s, sticker_host, &queries).await;

    // If a multi-word sentence/phrase (e.g. "anh ơi đi chơi", "hôm nay mệt quá") had no direct match,
    // automatically fallback to its trailing 2-word phrase (`tail2`) or trailing word (`last_word`).
    if raw_items.is_empty() {
        let words: Vec<&str> = kw.split_whitespace().collect();
        if words.len() >= 2 && words.len() <= 5 {
            let mut fallback_phrases = Vec::new();
            if words.len() >= 3 {
                fallback_phrases.push(format!(
                    "{} {}",
                    words[words.len() - 2],
                    words[words.len() - 1]
                ));
            }
            let last_w = words[words.len() - 1];
            if last_w.chars().count() >= 2 {
                fallback_phrases.push(last_w.to_string());
            }
            for fb in fallback_phrases {
                let fb_queries = canonical_vn_sticker_queries(&fb);
                raw_items = query_sticker_endpoints_once(s, sticker_host, &fb_queries).await;
                if !raw_items.is_empty() {
                    break;
                }
            }
        }
    }

    // Prioritize modern illustrated Zalo character packs (`cate_id` descending, matching Zalo PC's
    // `pointCate` ranking) and exclude legacy round-emoji/photo-meme packs when character packs exist.
    const LEGACY_NON_CHAR_PACKS: &[u64] = &[
        0, 50, 10788, 11190, 11984, 11998, 12002, 12003, 12024, 12658,
    ];
    let has_modern_char = raw_items
        .iter()
        .any(|(cid, _, _, _)| *cid >= 10000 && !LEGACY_NON_CHAR_PACKS.contains(cid));
    if has_modern_char {
        raw_items.retain(|(cid, _, _, _)| *cid >= 10000 && !LEGACY_NON_CHAR_PACKS.contains(cid));
    }
    // Sort by relevance score first (matched in both search + suggest), then newest pack (`cate_id` desc)
    raw_items.sort_by(|a, b| b.3.cmp(&a.3).then_with(|| b.0.cmp(&a.0)));

    // Keep at most 2 stickers from the same pack in the first pass so suggestions show diverse characters,
    // then fill up to 24 items if needed for Sticker Picker search grids.
    let mut pack_counts: std::collections::HashMap<u64, usize> = std::collections::HashMap::new();
    let mut out = Vec::with_capacity(24);
    let mut deferred = Vec::new();
    for &(cid, sid, stype, _) in &raw_items {
        let cnt = pack_counts.entry(cid).or_insert(0);
        if *cnt >= 2 {
            deferred.push((cid, sid, stype));
            continue;
        }
        *cnt += 1;
        let idx = out.len() + 1;
        out.push(StickerItem {
            sticker_id: sid,
            cate_id: cid,
            sticker_type: stype,
            label: format!("{kw} #{idx}"),
        });
        if out.len() >= 24 {
            break;
        }
    }
    for (cid, sid, stype) in deferred {
        if out.len() >= 24 {
            break;
        }
        let idx = out.len() + 1;
        out.push(StickerItem {
            sticker_id: sid,
            cate_id: cid,
            sticker_type: stype,
            label: format!("{kw} #{idx}"),
        });
    }

    if !out.is_empty() {
        if let Ok(txt) = serde_json::to_string(&out) {
            let _ = std::fs::write(&cache_path, txt);
        }
    }
    Ok(out)
}

/// Send a quote/reply message (`sendQuoteMessage` / `sendGroupQuoteMessage` in `compact-app-pc.b7bec208c4c8f73e09f8.js:14317494`):
/// - 1-1 (`cmd 11771`): `POST {domains.chat}/api/message/quote?{common}`
/// - Group (`cmd 11197`): `POST {domains.group}/api/group/quote?{common}`
/// Automatically falls back to `send_message` / `send_group_message` if the quoted message lacks a server ID.
pub async fn send_quote_message(
    s: &Session,
    to_id: &str,
    text: &str,
    quoted: &ChatMessage,
    is_group: bool,
) -> Result<Value> {
    send_quote_message_with_client_id(s, to_id, text, quoted, is_group, next_client_message_id())
        .await
}

/// Preserve the request client ID for reconciliation with a later server echo.
pub(crate) async fn send_quote_message_with_client_id(
    s: &Session,
    to_id: &str,
    text: &str,
    quoted: &ChatMessage,
    is_group: bool,
    now: u64,
) -> Result<Value> {
    if s.login.is_none() {
        anyhow::bail!("chưa đăng nhập");
    }

    let my_uid = s
        .login
        .as_ref()
        .map(|l| l.uin.clone())
        .unwrap_or_else(|| "0".into());

    let qmsg_owner = if quoted.is_outgoing {
        my_uid.as_str()
    } else if !quoted.sender.is_empty() && quoted.sender.chars().all(|c| c.is_ascii_digit()) {
        quoted.sender.as_str()
    } else if !is_group {
        to_id
    } else {
        "0"
    };

    let qmsg_id_num = quoted.id.parse::<u64>().unwrap_or(now.saturating_sub(1000));
    let qmsg_ts = if quoted.ts > 0 {
        quoted.ts as u64
    } else {
        now.saturating_sub(1000)
    };
    let qmsg_cli_id = qmsg_ts;

    let quote_res: Result<Value> = async {
        if quoted.id.starts_with("out_") || quoted.id.starts_with("local_") || quoted.id.is_empty()
        {
            anyhow::bail!("local quote id");
        }
        if is_group {
            let clean_grid = to_id
                .trim_start_matches("group_")
                .trim_start_matches('g')
                .trim_start_matches('G');
            let (clean_text, text_props_opt) = extract_zalo_rtf_text_properties(text);
            let mut body = obj(&[
                ("grid", json!(clean_grid)),
                ("message", json!(clean_text)),
                ("clientId", json!(now)),
                ("language", json!("vi")),
                ("qmsgOwner", json!(qmsg_owner)),
                ("qmsgId", json!(qmsg_id_num)),
                ("qmsgCliId", json!(qmsg_cli_id)),
                ("qmsgType", json!(1)),
                ("qmsg", json!(quoted.text)),
                ("qmsgTs", json!(qmsg_ts)),
                ("visibility", json!(0)),
                ("ttl", json!(0)),
                ("imei", json!(s.imei)),
                ("zsource", json!(101)),
            ]);
            if let (Some(tp), Some(map)) = (text_props_opt, body.as_object_mut()) {
                map.insert("textProperties".into(), json!(tp));
            }
            let params = s.encoded_params(&body)?;
            let url = format!("{}/api/group/quote?{}", s.domains.group, s.common_params());
            let resp = s.post_form(&url, format!("params={params}")).await?;
            let resp = s.maybe_decrypt(resp, s.session_key_bytes()).await;
            if let Some(code) = resp.get("error_code").and_then(Value::as_i64) {
                if code != 0 {
                    anyhow::bail!("quote error {code}");
                }
            }
            Ok(resp)
        } else {
            let host = if s.domains.chat.contains("-msg-chat.")
                || s.domains.chat.contains("tt-chat2-wpa")
                || s.domains.chat.is_empty()
            {
                "https://tt-chat3-wpa.api.zaloapp.com"
            } else {
                &s.domains.chat
            };
            let (clean_text, text_props_opt) = extract_zalo_rtf_text_properties(text);
            let mut body = obj(&[
                ("toid", json!(to_id)),
                ("message", json!(clean_text)),
                ("clientId", json!(now)),
                ("qmsgOwner", json!(qmsg_owner)),
                ("qmsgId", json!(qmsg_id_num)),
                ("qmsgCliId", json!(qmsg_cli_id)),
                ("qmsgType", json!(1)),
                ("qmsgTs", json!(qmsg_ts)),
                ("qmsg", json!(quoted.text)),
                ("imei", json!(s.imei)),
                ("ttl", json!(0)),
                ("zsource", json!(101)),
            ]);
            if let (Some(tp), Some(map)) = (text_props_opt, body.as_object_mut()) {
                map.insert("textProperties".into(), json!(tp));
            }
            let params = s.encoded_params(&body)?;
            let url = format!("{host}/api/message/quote?{}", s.common_params());
            let resp = s.post_form(&url, format!("params={params}")).await?;
            let resp = s.maybe_decrypt(resp, s.session_key_bytes()).await;
            if let Some(code) = resp.get("error_code").and_then(Value::as_i64) {
                if code != 0 {
                    anyhow::bail!("quote error {code}");
                }
            }
            Ok(resp)
        }
    }
    .await;

    match quote_res {
        Ok(r) => Ok(r),
        Err(_) => {
            if is_group {
                send_group_message_with_client_id(s, to_id, text, now).await
            } else {
                send_message_with_client_id(s, to_id, text, now).await
            }
        }
    }
}

/// Forward a message or sticker to another conversation (`forwardMultiMessage` @ `compact-app-pc.js:14323900`).
pub async fn forward_message(
    s: &Session,
    target_id: &str,
    target_is_group: bool,
    msg: &ChatMessage,
) -> Result<Value> {
    forward_message_with_client_id(s, target_id, target_is_group, msg, next_client_message_id())
        .await
}

/// Preserve the request client ID for reconciliation with a later server echo.
pub(crate) async fn forward_message_with_client_id(
    s: &Session,
    target_id: &str,
    target_is_group: bool,
    msg: &ChatMessage,
    now: u64,
) -> Result<Value> {
    if let Some(sid) = msg.sticker_id {
        if sid > 0 {
            let cid = msg.sticker_cate_id.unwrap_or(0);
            let stype = msg.sticker_type.unwrap_or(7);
            return send_sticker_with_client_id(
                s,
                target_id,
                sid,
                cid,
                stype,
                target_is_group,
                now,
            )
            .await;
        }
    }
    // Clean text if message was previously marked as recalled
    let clean_text = if let Some(stripped) = msg.text.strip_prefix("[Đã thu hồi] ") {
        stripped
    } else {
        &msg.text
    };

    // Forward photo if message contains a photo CDN URL
    if let Some(photo_url) = msg
        .link_url
        .as_deref()
        .filter(|u| is_zalo_photo_cdn_url(u) && is_safe_external_image_url(u))
    {
        if let Ok(resp) = s.http.get(photo_url).send().await {
            if let Ok(bytes) = resp.bytes().await {
                if !bytes.is_empty() {
                    let temp_path = std::env::temp_dir().join(format!("zalo_fwd_{now}.jpg"));
                    if std::fs::write(&temp_path, &bytes).is_ok() {
                        let caption = if clean_text == "[Hình ảnh]" {
                            ""
                        } else {
                            clean_text
                        };
                        let res =
                            send_attachment(s, target_id, target_is_group, &temp_path, caption)
                                .await;
                        let _ = std::fs::remove_file(&temp_path);
                        if let Ok((resp_v, _)) = res {
                            return Ok(resp_v);
                        }
                    }
                }
            }
        }
    }

    if target_is_group {
        send_group_message_with_client_id(s, target_id, clean_text, now).await
    } else {
        send_message_with_client_id(s, target_id, clean_text, now).await
    }
}

/// Convert Zalo reaction icon code to visual emoji.
pub fn icon_to_emoji(icon: &str) -> &'static str {
    match icon {
        "/-strong" => "👍",
        "/-heart" => "❤️",
        ":>" => "😆",
        ":o" => "😮",
        ":-((" | ":-(( " => "😢",
        ":-h" => "😠",
        _ => "❤️",
    }
}

/// Map a visual reaction emoji or Zalo reaction code to `(rIcon, rType)` (`compact-app-pc.js:10848900`).
pub fn reaction_code_and_type(emoji_or_code: &str) -> (&'static str, i32) {
    match emoji_or_code {
        "👍" | "/-strong" => ("/-strong", 3),
        "❤️" | "❤" | "/-heart" => ("/-heart", 5),
        "😆" | ":>" => (":>", 0),
        "😮" | ":o" => (":o", 32),
        "😢" | ":-(( " | ":-((" => (":-((", 2),
        "😠" | ":-h" => (":-h", 20),
        _ => ("/-heart", 5),
    }
}

/// Send a message reaction (`sendReactionMessage` in `compact-app-pc.b7bec208c4c8f73e09f8.js:14317097`, cmd `12410` / `12411`).
pub async fn send_reaction(
    s: &Session,
    to_id: &str,
    is_group: bool,
    msg: &ChatMessage,
    reaction: &str,
) -> Result<Value> {
    if s.login.is_none() {
        anyhow::bail!("chưa đăng nhập");
    }
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);

    let (r_icon, r_type) = reaction_code_and_type(reaction);
    let is_local_id =
        msg.id.is_empty() || msg.id.starts_with("out_") || msg.id.starts_with("local_");
    let g_msg_id_val = if is_local_id {
        json!("")
    } else {
        json!(msg.id)
    };
    let c_msg_id = if msg.ts > 0 {
        msg.ts as u64
    } else {
        msg.id.parse::<u64>().unwrap_or(now)
    };

    let inner_msg = json!({
        "rMsg": [{
            "gMsgID": g_msg_id_val,
            "cMsgID": c_msg_id,
            "msgType": if msg.sticker_id.is_some() { 5 } else { 1 }
        }],
        "rIcon": r_icon,
        "rType": r_type,
        "source": 6
    })
    .to_string();

    let react_list = json!([{
        "message": inner_msg,
        "clientId": now
    }]);

    let (url, body) = if is_group {
        let clean_grid = to_id
            .trim_start_matches("group_")
            .trim_start_matches('g')
            .trim_start_matches('G');
        (
            format!(
                "https://reaction.api.zaloapp.com/api/group/reaction?{}",
                s.common_params()
            ),
            obj(&[
                ("react_list", react_list),
                ("grid", json!(clean_grid)),
                ("imei", json!(s.imei)),
            ]),
        )
    } else {
        (
            format!(
                "https://reaction.api.zaloapp.com/api/message/reaction?{}",
                s.common_params()
            ),
            obj(&[("react_list", react_list), ("toid", json!(to_id))]),
        )
    };

    let params = s.encoded_params(&body)?;
    let resp = s.post_form(&url, format!("params={params}")).await?;
    Ok(s.maybe_decrypt(resp, s.session_key_bytes()).await)
}

/// Delete a message for me (`deleteOneOneMessageV2` / `deleteGroupMessageV2` @ `compact-app-pc.js:14373319`, cmd `11779` / `11834`).
pub async fn delete_message_for_me(
    s: &Session,
    to_id: &str,
    is_group: bool,
    msg: &ChatMessage,
) -> Result<Value> {
    if s.login.is_none() {
        anyhow::bail!("chưa đăng nhập");
    }
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let my_uid = s
        .login
        .as_ref()
        .map(|l| l.uin.clone())
        .unwrap_or_else(|| "0".into());
    let owner_id = if msg.is_outgoing {
        my_uid.as_str()
    } else if !is_group {
        to_id
    } else {
        &msg.sender
    };
    let cli_id = if msg.ts > 0 {
        msg.ts.to_string()
    } else {
        now.to_string()
    };

    let msgs_arr = json!([{
        "cliMsgId": cli_id,
        "globalMsgId": msg.id,
        "ownerId": owner_id,
        "destId": to_id
    }]);

    if is_group {
        let clean_grid = to_id
            .trim_start_matches("group_")
            .trim_start_matches('g')
            .trim_start_matches('G');
        let body = obj(&[
            ("grid", json!(clean_grid)),
            ("cliMsgId", json!(now)),
            ("msgs", msgs_arr),
            ("onlyMe", json!(1)),
        ]);
        let params = s.encoded_params(&body)?;
        let url = format!(
            "{}/api/group/deletemsg?{}",
            s.domains.group,
            s.common_params()
        );
        let resp = s.post_form(&url, format!("params={params}")).await?;
        Ok(s.maybe_decrypt(resp, s.session_key_bytes()).await)
    } else {
        let host = if s.domains.chat.contains("-msg-chat.")
            || s.domains.chat.contains("tt-chat2-wpa")
            || s.domains.chat.is_empty()
        {
            "https://tt-chat3-wpa.api.zaloapp.com"
        } else {
            &s.domains.chat
        };
        let body = obj(&[
            ("toid", json!(to_id)),
            ("cliMsgId", json!(now)),
            ("msgs", msgs_arr),
            ("onlyMe", json!(1)),
            ("imei", json!(s.imei)),
        ]);
        let params = s.encoded_params(&body)?;
        let url = format!("{host}/api/message/delete?{}", s.common_params());
        let resp = s.post_form(&url, format!("params={params}")).await?;
        Ok(s.maybe_decrypt(resp, s.session_key_bytes()).await)
    }
}

/// Recall / undo an outgoing message for everyone (`undoMessage` @ `compact-app-pc.js:14376599`, cmd `11281` / `11192`).
pub async fn recall_message(
    s: &Session,
    to_id: &str,
    is_group: bool,
    msg: &ChatMessage,
) -> Result<Value> {
    if s.login.is_none() {
        anyhow::bail!("chưa đăng nhập");
    }
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let cli_id = if msg.ts > 0 {
        msg.ts.to_string()
    } else {
        now.to_string()
    };

    if is_group {
        let clean_grid = to_id
            .trim_start_matches("group_")
            .trim_start_matches('g')
            .trim_start_matches('G');
        let body = obj(&[
            ("msgId", json!(msg.id)),
            ("grid", json!(clean_grid)),
            ("clientId", json!(now)),
            ("visibility", json!(0)),
            ("cliMsgIdUndo", json!(cli_id)),
            ("imei", json!(s.imei)),
        ]);
        let params = s.encoded_params(&body)?;
        let url = format!(
            "{}/api/group/undomsg?{}",
            s.domains.group,
            s.common_params()
        );
        let resp = s.post_form(&url, format!("params={params}")).await?;
        Ok(s.maybe_decrypt(resp, s.session_key_bytes()).await)
    } else {
        let host = if s.domains.chat.contains("-msg-chat.")
            || s.domains.chat.contains("tt-chat2-wpa")
            || s.domains.chat.is_empty()
        {
            "https://tt-chat3-wpa.api.zaloapp.com"
        } else {
            &s.domains.chat
        };
        let body = obj(&[
            ("msgId", json!(msg.id)),
            ("toid", json!(to_id)),
            ("clientId", json!(now)),
            ("cliMsgIdUndo", json!(cli_id)),
        ]);
        let params = s.encoded_params(&body)?;
        let url = format!("{host}/api/message/undo?{}", s.common_params());
        let resp = s.post_form(&url, format!("params={params}")).await?;
        Ok(s.maybe_decrypt(resp, s.session_key_bytes()).await)
    }
}

/// `GET /api/preloadconvers/get-last-msgs` (cmd 10053).
///
/// [O] request: GET `{convers_api}/api/preloadconvers/get-last-msgs?{common}&params={encodeAES(json)}`
/// where payload is `{ threadIdLocalMsgId: map_object, imei }`.
pub async fn get_last_msgs(s: &Session, thread_ids: &[String]) -> Result<Value> {
    if s.login.is_none() {
        anyhow::bail!("chưa đăng nhập");
    }
    let mut map = serde_json::Map::new();
    // [O] `getLastMessagesForPreview(e)` in `compact-app-pc.js:14385933` passes `{}` on preload
    // to fetch all recent conversation previews without overflowing URL length.
    if thread_ids.len() <= 25 {
        for id in thread_ids {
            let clean = id.trim_start_matches("group_").trim_start_matches('g');
            let key = if id.starts_with("group_") || id.starts_with('g') {
                format!("{clean}_1")
            } else {
                format!("{clean}_0")
            };
            map.insert(key, json!("1"));
        }
    }
    let map_str = serde_json::to_string(&map).unwrap_or_else(|_| "{}".into());
    let body = obj(&[
        ("threadIdLocalMsgId", json!(map_str)),
        ("imei", json!(s.imei)),
    ]);
    let params = match s.encoded_params(&body) {
        Ok(p) => format!("params={p}"),
        Err(e) => {
            crate::logger::log_error("chat::get_last_msgs::encoded_params", &e.to_string());
            eprintln!("! get_last_msgs: no session key for encoded_params ({e}), falling back to plain_params");
            Session::plain_params(&body)
        }
    };

    let url = format!(
        "{}/api/preloadconvers/get-last-msgs?{}&{params}",
        s.domains.convers_api,
        s.common_params()
    );
    let resp = s.get(&url).await?;
    let resp = s.maybe_decrypt(resp, s.session_key_bytes()).await;
    Ok(resp)
}

/// `GET /api/pinconvers/list` (cmd 12120) — pinned threads.
pub async fn get_pinned(s: &Session) -> Result<Value> {
    if s.login.is_none() {
        anyhow::bail!("chưa đăng nhập");
    }
    let path = format!("/api/pinconvers/list?{}", s.common_params());
    let resp = match s.get_endpoint("conversation", &path).await {
        Ok(v) => v,
        Err(_) => {
            let url = format!(
                "{}/api/pinconvers/list?{}",
                s.domains.conversation,
                s.common_params()
            );
            s.get(&url).await?
        }
    };
    Ok(s.maybe_decrypt(resp, s.session_key_bytes()).await)
}

fn extract_pinned_id_from_value(item: &Value) -> Option<String> {
    if let Some(s) = item.as_str() {
        let trimmed = s.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    }
    if let Some(n) = item.as_i64() {
        if n > 0 {
            return Some(n.to_string());
        }
    }
    if let Some(obj) = item.as_object() {
        for key in [
            "convId",
            "id",
            "conversId",
            "threadId",
            "grid",
            "uid",
            "targetId",
        ] {
            if let Some(v) = obj.get(key) {
                if let Some(s) = v.as_str() {
                    let trimmed = s.trim();
                    if !trimmed.is_empty() {
                        return Some(trimmed.to_string());
                    }
                }
                if let Some(n) = v.as_i64() {
                    if n > 0 {
                        return Some(n.to_string());
                    }
                }
            }
        }
    }
    None
}

/// Parse list of pinned conversation IDs from `/api/pinconvers/list` response (#87).
pub fn parse_pinned_conversations(resp: &Value) -> Vec<String> {
    let mut out = Vec::new();
    let data_node = if let Some(s) = resp.get("data").and_then(Value::as_str) {
        serde_json::from_str::<Value>(s.trim())
            .unwrap_or_else(|_| resp.get("data").cloned().unwrap_or_else(|| resp.clone()))
    } else {
        resp.get("data").cloned().unwrap_or_else(|| resp.clone())
    };

    let mut check_array = |arr: &[Value]| {
        for item in arr {
            if let Some(id) = extract_pinned_id_from_value(item) {
                if !out.contains(&id) {
                    out.push(id);
                }
            }
        }
    };

    if let Some(arr) = data_node.as_array() {
        check_array(arr);
    } else if let Some(obj) = data_node.as_object() {
        for key in [
            "conversations",
            "pins",
            "pinned",
            "convs",
            "data",
            "list",
            "items",
        ] {
            if let Some(arr) = obj.get(key).and_then(Value::as_array) {
                check_array(arr);
            }
        }
        if out.is_empty() {
            for (_, val) in obj {
                if let Some(id) = extract_pinned_id_from_value(val) {
                    if !out.contains(&id) {
                        out.push(id);
                    }
                }
            }
        }
    }
    out
}

/// Determine whether `thread_id` matches any pinned conversation identifier in `pinned_ids`.
///
/// Handles Zalo's prefix conventions:
/// - Group threads (`is_group: true`) match `g{id}`, `group_{id}`, or unadorned `{id}` (when not starting with `u`).
/// - 1-1 User threads (`is_group: false`) match `u{id}` or unadorned `{id}` (when not starting with `g`).
pub fn is_pinned_in_list(thread_id: &str, is_group: bool, pinned_ids: &[String]) -> bool {
    let clean = thread_id
        .trim_start_matches("group_")
        .trim_start_matches('g')
        .trim_start_matches('G')
        .trim_start_matches('u')
        .trim_start_matches('U')
        .trim();
    if clean.is_empty() {
        return false;
    }
    pinned_ids.iter().any(|pid| {
        let p_trimmed = pid.trim();
        if p_trimmed == thread_id {
            return true;
        }
        let p_clean = p_trimmed
            .trim_start_matches("group_")
            .trim_start_matches('g')
            .trim_start_matches('G')
            .trim_start_matches('u')
            .trim_start_matches('U')
            .trim();
        if p_clean == clean {
            if is_group {
                !p_trimmed.starts_with('u') && !p_trimmed.starts_with('U')
            } else {
                !p_trimmed.starts_with('g')
                    && !p_trimmed.starts_with('G')
                    && !p_trimmed.starts_with("group_")
            }
        } else {
            false
        }
    })
}

/// Apply server pinned status to threads.
pub fn apply_pinned_threads(threads: &mut [Thread], pinned_ids: &[String]) {
    for t in threads.iter_mut() {
        if is_pinned_in_list(&t.id, t.is_group, pinned_ids) {
            t.presentation.pinned = true;
        }
    }
}

/// `GET /api/group/history` (cmd 11240) — group message history.
pub async fn get_group_history(s: &Session, group_id: &str, count: u32) -> Result<Value> {
    if s.login.is_none() {
        anyhow::bail!("chưa đăng nhập");
    }
    let grid = group_id
        .trim_start_matches("group_")
        .trim_start_matches('g')
        .trim_start_matches('G');
    let body = obj(&[
        ("grid", json!(grid)),
        ("count", json!(count)),
        ("imei", json!(s.imei)),
    ]);
    let params = match s.encoded_params(&body) {
        Ok(p) => format!("params={p}"),
        Err(e) => {
            crate::logger::log_error("chat::get_group_history::encoded_params", &e.to_string());
            Session::plain_params(&body)
        }
    };
    let url = format!(
        "{}/api/group/history?{}&{params}",
        s.domains.group,
        s.common_params()
    );
    match s.get(&url).await {
        Ok(resp) => Ok(s.maybe_decrypt(resp, s.session_key_bytes()).await),
        Err(_) => get_group_history_raw(s, grid, count).await,
    }
}

/// Byte-exact replica of the shipped bundle's `getHistoryMessage(e, t)`.
///
/// [O] `compact-app-pc:14364564`:
/// `n = {grid: e, count: t}`
/// `getGroupDomain() + "/api/group/history?" + _getCommonParams() + "&params=" + encodeURIComponent(encodeAES(JSON.stringify(n)))`
/// Signing tag: **11240**.
///
/// Note: the bundle sends **only** `grid` and `count` — no `imei`, no `signkey` on this
/// path. Kept as a separate raw probe so the guessed `get_group_history` above can be
/// compared against it without breaking existing callers.
pub async fn get_group_history_raw(s: &Session, grid: &str, count: u32) -> Result<Value> {
    if s.login.is_none() {
        anyhow::bail!("chưa đăng nhập");
    }
    // [O] getGroupIdFromConversationId: strip the group-id prefix if present.
    let grid = grid
        .trim_start_matches("group_")
        .trim_start_matches('g')
        .trim_start_matches('G');
    let body = obj(&[("grid", json!(grid)), ("count", json!(count))]);
    let params = match s.encoded_params(&body) {
        Ok(p) => format!("params={p}"),
        Err(e) => {
            crate::logger::log_error(
                "chat::get_group_history_raw::encoded_params",
                &e.to_string(),
            );
            Session::plain_params(&body)
        }
    };
    let url = format!(
        "{}/api/group/history?{}&{params}",
        s.domains.group,
        s.common_params()
    );
    let resp = s.get(&url).await?;
    let resp = s.maybe_decrypt(resp, s.session_key_bytes()).await;
    Ok(resp)
}

/// Fetch group metadata (`name`, `avt`/`fullAvt`, `totalMember`, `removedsGroup`) from
/// `POST /api/group/getmg-v2` by sending `gridVerMap: {"<grid>": 0}` so the server always
/// returns `gridInfoMap` instead of putting groups into `unchangedsGroup`.
pub async fn fetch_group_profiles(
    s: &Session,
    group_ids: &[String],
) -> Result<(
    serde_json::Map<String, Value>,
    std::collections::HashSet<String>,
)> {
    if s.login.is_none() || group_ids.is_empty() {
        return Ok((serde_json::Map::new(), std::collections::HashSet::new()));
    }
    let mut gv_zero = serde_json::Map::new();
    for gid in group_ids {
        let clean = gid
            .trim_start_matches("group_")
            .trim_start_matches('g')
            .trim_start_matches('G')
            .trim();
        if !clean.is_empty() {
            gv_zero.insert(clean.to_string(), json!(0));
        }
    }
    let gv_str = serde_json::to_string(&gv_zero)?;
    let body = obj(&[("gridVerMap", json!(gv_str)), ("imei", json!(s.imei))]);
    let params = s.encoded_params(&body)?;
    let mg_url = format!(
        "{}/api/group/getmg-v2?{}",
        s.domains.group,
        s.common_params()
    );
    let mg_resp = s.post_form(&mg_url, format!("params={params}")).await?;
    let mut mg_dec = s.maybe_decrypt(mg_resp, s.session_key_bytes()).await;
    if let Some(d_str) = mg_dec.get("data").and_then(Value::as_str) {
        if let Ok(parsed) = serde_json::from_str::<Value>(d_str.trim()) {
            mg_dec["data"] = parsed;
        }
    }
    let data_node = mg_dec
        .pointer("/data/data")
        .or_else(|| mg_dec.get("data"))
        .unwrap_or(&mg_dec);
    let info_map = data_node
        .get("gridInfoMap")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let removed: std::collections::HashSet<String> = data_node
        .get("removedsGroup")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    Ok((info_map, removed))
}

/// Extract up to 4 sorted member UIDs (`creatorId` first, then `adminIds`, then `memVerList`)
/// and total member count from a `getmg-v2` group info object.
fn extract_group_member_ids(info: &Value) -> (Vec<String>, usize) {
    let mut raw_ids: Vec<String> = Vec::new();
    if let Some(arr) = info.get("memVerList").and_then(Value::as_array) {
        for item in arr {
            if let Some(s) = item.as_str() {
                let uid = s.split('_').next().unwrap_or("").trim();
                if !uid.is_empty() && !raw_ids.iter().any(|x| x == uid) {
                    raw_ids.push(uid.to_string());
                }
            }
        }
    }
    let mut sorted: Vec<String> = Vec::new();
    if let Some(creator) = pick_str(info, &["creatorId"]) {
        if raw_ids.iter().any(|x| x == &creator) {
            sorted.push(creator);
        }
    }
    if let Some(admins) = info.get("adminIds").and_then(Value::as_array) {
        for a in admins {
            let aid = a
                .as_str()
                .map(str::to_string)
                .or_else(|| a.as_u64().map(|n| n.to_string()))
                .unwrap_or_default();
            if !aid.is_empty()
                && raw_ids.iter().any(|x| x == &aid)
                && !sorted.iter().any(|x| x == &aid)
            {
                sorted.push(aid);
            }
        }
    }
    for uid in &raw_ids {
        if !sorted.iter().any(|x| x == uid) {
            sorted.push(uid.clone());
        }
    }
    let total = info
        .get("totalMember")
        .and_then(Value::as_u64)
        .map(|n| n as usize)
        .unwrap_or_else(|| sorted.len())
        .max(sorted.len());
    sorted.truncate(4);
    (sorted, total)
}

/// Format a `multi:<total>;<name1>|<url1>;...` composite group avatar string.
fn format_multi_group_avatar(
    group_name: &str,
    mem_ids: &[String],
    total_members: usize,
    profiles: &serde_json::Map<String, Value>,
) -> Option<String> {
    let fallback_names: Vec<&str> = group_name
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();
    let mut slots: Vec<String> = Vec::new();
    for (idx, uid) in mem_ids.iter().take(4).enumerate() {
        let mut m_name = fallback_names
            .get(idx)
            .copied()
            .unwrap_or("Thành viên")
            .to_string();
        let mut m_ava = String::new();
        if let Some(p) = profiles.get(uid) {
            if let Some(n) = pick_str(p, &["zaloName", "displayName", "name", "display_name"]) {
                if !n.is_empty() {
                    m_name = n;
                }
            }
            if let Some(a) = pick_str(p, &["avatar", "avt", "profile_avatar"]) {
                if !a.is_empty() && !a.ends_with("/default") {
                    m_ava = a;
                }
            }
        }
        let clean_name = m_name.replace(';', " ").replace('|', " ");
        let clean_ava = m_ava.replace(';', "").replace('|', "");
        slots.push(format!("{clean_name}|{clean_ava}"));
    }
    if slots.is_empty() {
        return None;
    }
    // If the group name is a comma-separated list of member names (e.g. "Đỗ Thành, Đtc, Hoang"),
    // order the slots to match `fallback_names` so API-resolved and name-derived slots never swap positions.
    if fallback_names.len() >= 2 {
        slots.sort_by_key(|slot| {
            let s_name = slot.split('|').next().unwrap_or("").trim();
            let s_fold = strip_vn_for_sticker_kw(s_name);
            fallback_names
                .iter()
                .position(|cand| {
                    let c_fold = strip_vn_for_sticker_kw(cand);
                    !c_fold.is_empty()
                        && (c_fold == s_fold
                            || s_fold.contains(&c_fold)
                            || c_fold.contains(&s_fold))
                })
                .unwrap_or(99)
        });
    }
    let tot = total_members.max(slots.len());
    Some(format!("multi:{tot};{}", slots.join(";")))
}

fn group_posting_restricted(info: &Value, uid: &str) -> Option<bool> {
    let locked = info.pointer("/setting/lockSendMsg")?.as_i64()? != 0;
    if !locked {
        return Some(false);
    }
    let creator = info.get("creatorId")?.as_str()?;
    let admins = info.get("adminIds")?.as_array()?;
    if uid.is_empty() {
        return None;
    }
    Some(creator != uid && !admins.iter().any(|admin| admin.as_str() == Some(uid)))
}

/// `GET /api/group/getlg/v4` (cmd 12027) — list of all groups user belongs to.
pub async fn get_groups(s: &Session) -> Result<Vec<Thread>> {
    if s.login.is_none() {
        return Ok(Vec::new());
    }
    let url = format!(
        "{}/api/group/getlg/v4?{}",
        s.domains.group,
        s.common_params()
    );
    let resp = s.get(&url).await?;
    let mut resp = s.maybe_decrypt(resp, s.session_key_bytes()).await;
    if let Some(d_str) = resp.get("data").and_then(Value::as_str) {
        if let Ok(parsed) = serde_json::from_str::<Value>(d_str.trim()) {
            resp["data"] = parsed;
        }
    }
    let data_node = resp
        .pointer("/data/data")
        .or_else(|| resp.get("data"))
        .unwrap_or(&resp);

    let mut groups = Vec::new();
    if let Some(map) = data_node.get("gridVerMap").and_then(Value::as_object) {
        for (grid, _ver) in map {
            groups.push(Thread {
                presentation: Default::default(),
                unread: Default::default(),
                id: grid.clone(),
                name: format!("Nhóm {}", grid),
                preview: String::new(),
                time: String::new(),
                avatar: None,
                is_group: true,
                messages: Vec::new(),
            });
        }
        if !groups.is_empty() {
            let gids: Vec<String> = groups.iter().map(|g| g.id.clone()).collect();
            if let Ok((info_map, rem_ids)) = fetch_group_profiles(s, &gids).await {
                let mut need_mem_uids: Vec<String> = Vec::new();
                for g in &mut groups {
                    if let Some(info) = info_map.get(&g.id) {
                        let uid = s
                            .login
                            .as_ref()
                            .map(|login| login.uin.as_str())
                            .unwrap_or("");
                        g.presentation.posting_restricted = group_posting_restricted(info, uid);
                        if let Some(name) = pick_str(info, &["name", "groupName", "displayName"]) {
                            g.name = name;
                        }
                        if let Some(avt) = pick_str(info, &["fullAvt", "avt", "avatar"])
                            .filter(|u| !u.is_empty() && !u.ends_with("/default"))
                        {
                            g.avatar = Some(avt);
                        } else {
                            let (mids, _) = extract_group_member_ids(info);
                            for uid in mids {
                                if !need_mem_uids.iter().any(|x| x == &uid) {
                                    need_mem_uids.push(uid);
                                }
                            }
                        }
                        if g.preview.is_empty() {
                            if let Some(n_mem) = info.get("totalMember").and_then(Value::as_u64) {
                                if n_mem > 0 {
                                    g.preview = format!("Nhóm · {n_mem} thành viên");
                                }
                            }
                        }
                    }
                }
                if !need_mem_uids.is_empty() {
                    let mut all_profiles = serde_json::Map::new();
                    for chunk in need_mem_uids.chunks(50) {
                        if let Ok(mp) = get_mini_profiles(s, chunk).await {
                            for (k, v) in mp {
                                all_profiles.insert(k, v);
                            }
                        }
                    }
                    for g in &mut groups {
                        if g.avatar.is_none() {
                            if let Some(info) = info_map.get(&g.id) {
                                let (mids, tot) = extract_group_member_ids(info);
                                g.avatar =
                                    format_multi_group_avatar(&g.name, &mids, tot, &all_profiles);
                            }
                        }
                    }
                }
                if !rem_ids.is_empty() {
                    groups.retain(|g| !rem_ids.contains(&g.id));
                }
            }
        }
    }
    Ok(groups)
}

/// `POST /api/social/group/members` (cmd 12030) — fetch member display names and avatars for user IDs.
///
/// Zalo's `/api/social/friend/getminiprofiles` returns `error_code = 114` for raw UID lists,
/// whereas `/api/social/group/members` with `friend_pversion_map: ["<uid>_0", ...]` returns
/// `data.profiles.<uid>.{displayName, zaloName, avatar}` with `error_code = 0`.
pub async fn get_mini_profiles(
    s: &Session,
    user_ids: &[String],
) -> Result<serde_json::Map<String, Value>> {
    if s.login.is_none() || user_ids.is_empty() {
        return Ok(serde_json::Map::new());
    }
    let versioned_ids: Vec<String> = user_ids
        .iter()
        .map(|uid| {
            let clean = uid.trim();
            if clean.contains('_') {
                clean.to_string()
            } else {
                format!("{clean}_0")
            }
        })
        .collect();
    let body = obj(&[("friend_pversion_map", json!(versioned_ids))]);
    let params = s.encoded_params(&body)?;
    let url = format!(
        "{}/api/social/group/members?{}",
        s.domains.profile,
        s.common_params()
    );
    let form = format!("params={params}");
    let resp = s.post_form(&url, form).await?;
    let mut resp = s.maybe_decrypt(resp, s.session_key_bytes()).await;
    if let Some(d_str) = resp.get("data").and_then(Value::as_str) {
        if let Ok(parsed) = serde_json::from_str::<Value>(d_str.trim()) {
            resp["data"] = parsed;
        }
    }
    if let Some(profiles) = resp
        .pointer("/data/profiles")
        .or_else(|| resp.pointer("/data/data/profiles"))
        .or_else(|| resp.get("data"))
        .and_then(Value::as_object)
    {
        return Ok(profiles.clone());
    }
    Ok(serde_json::Map::new())
}

/// Automatically resolve real display names and avatars for 1-1 and group threads that only have placeholder names.
pub async fn resolve_thread_names(s: &Session, threads: &mut [Thread]) {
    let unknown_ids: Vec<String> = threads
        .iter()
        .filter(|t| {
            !t.is_group
                && !t.name.contains("Cloud của tôi")
                && (t.name.starts_with("Người dùng ")
                    || t.name.is_empty()
                    || t.avatar.as_deref().unwrap_or("").is_empty())
        })
        .map(|t| t.id.clone())
        .collect();

    if !unknown_ids.is_empty() {
        for chunk in unknown_ids.chunks(50) {
            if let Ok(profiles) = get_mini_profiles(s, chunk).await {
                for t in threads.iter_mut() {
                    if let Some(p) = profiles.get(&t.id) {
                        if t.name.starts_with("Người dùng ") || t.name.is_empty() {
                            if let Some(name) =
                                pick_str(p, &["zaloName", "displayName", "name", "display_name"])
                            {
                                if !name.is_empty() {
                                    t.name = name;
                                }
                            }
                        }
                        if let Some(ava) = pick_str(p, &["avatar", "avt", "profile_avatar"]) {
                            if !ava.is_empty() {
                                t.avatar = Some(ava);
                            }
                        }
                    }
                }
            }
        }
    }

    let unknown_group_ids: Vec<String> = threads
        .iter()
        .filter(|t| {
            t.is_group
                && (is_generic_thread_name(&t.name)
                    || t.preview.is_empty()
                    || t.avatar.as_deref().unwrap_or("").is_empty())
        })
        .map(|t| t.id.clone())
        .collect();

    if !unknown_group_ids.is_empty() {
        for chunk in unknown_group_ids.chunks(50) {
            if let Ok((info_map, _)) = fetch_group_profiles(s, chunk).await {
                let mut need_mem_uids: Vec<String> = Vec::new();
                for t in threads.iter_mut() {
                    let clean_id =
                        t.id.trim_start_matches("group_")
                            .trim_start_matches('g')
                            .trim_start_matches('G');
                    if let Some(info) = info_map.get(&t.id).or_else(|| info_map.get(clean_id)) {
                        if let Some(name) = pick_str(info, &["name", "groupName", "displayName"]) {
                            if !name.is_empty() {
                                t.name = name;
                            }
                        }
                        if let Some(avt) = pick_str(info, &["fullAvt", "avt", "avatar"])
                            .filter(|u| !u.is_empty() && !u.ends_with("/default"))
                        {
                            t.avatar = Some(avt);
                        } else {
                            let (mids, _) = extract_group_member_ids(info);
                            for uid in mids {
                                if !need_mem_uids.iter().any(|x| x == &uid) {
                                    need_mem_uids.push(uid);
                                }
                            }
                        }
                        if t.preview.is_empty() {
                            if let Some(n_mem) = info.get("totalMember").and_then(Value::as_u64) {
                                if n_mem > 0 {
                                    t.preview = format!("Nhóm · {n_mem} thành viên");
                                }
                            }
                        }
                    }
                }
                if !need_mem_uids.is_empty() {
                    let mut all_profiles = serde_json::Map::new();
                    for mchunk in need_mem_uids.chunks(50) {
                        if let Ok(mp) = get_mini_profiles(s, mchunk).await {
                            for (k, v) in mp {
                                all_profiles.insert(k, v);
                            }
                        }
                    }
                    for t in threads.iter_mut() {
                        if !t.is_group {
                            continue;
                        }
                        let clean_id =
                            t.id.trim_start_matches("group_")
                                .trim_start_matches('g')
                                .trim_start_matches('G');
                        if let Some(info) = info_map.get(&t.id).or_else(|| info_map.get(clean_id)) {
                            let cur = t.avatar.as_deref().unwrap_or("");
                            if cur.is_empty() || cur.starts_with("multi:") {
                                let (mids, tot) = extract_group_member_ids(info);
                                if let Some(multi_s) =
                                    format_multi_group_avatar(&t.name, &mids, tot, &all_profiles)
                                {
                                    if should_replace_avatar(t.avatar.as_deref(), Some(&multi_s)) {
                                        t.avatar = Some(multi_s);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Offline / local contact fallback for composite group avatars: match comma-separated member names
    // against known 1-1 contact threads so group avatars display real member photos even without network.
    let known_contacts: Vec<(String, String)> = threads
        .iter()
        .filter(|t| {
            !t.is_group
                && !t.name.contains("Cloud của tôi")
                && !t.avatar.as_deref().unwrap_or("").is_empty()
                && !t.avatar.as_deref().unwrap_or("").ends_with("/default")
        })
        .map(|t| (t.name.clone(), t.avatar.clone().unwrap_or_default()))
        .collect();

    for t in threads.iter_mut() {
        if !t.is_group {
            continue;
        }
        let cur_ava = t.avatar.as_deref().unwrap_or("");
        if avatar_http_url_count(Some(cur_ava)) > 0 {
            continue;
        }
        let parts: Vec<&str> = t
            .name
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .collect();
        if parts.len() >= 2 {
            let mut slots: Vec<String> = Vec::new();
            for p in parts.iter().take(4) {
                let p_fold = strip_vn_for_sticker_kw(p);
                let matched_url = known_contacts
                    .iter()
                    .find(|(cname, _)| {
                        let c_fold = strip_vn_for_sticker_kw(cname);
                        !p_fold.is_empty() && c_fold == p_fold
                    })
                    .map(|(_, url)| url.as_str())
                    .unwrap_or("");
                slots.push(format!(
                    "{}|{}",
                    p.replace(';', " ").replace('|', " "),
                    matched_url
                ));
            }
            let new_multi = format!("multi:{};{}", parts.len(), slots.join(";"));
            if should_replace_avatar(t.avatar.as_deref(), Some(&new_multi)) {
                t.avatar = Some(new_multi);
            }
        }
    }
}

/// Per-queue cursor for `getChatFromPolling` (`MultiQueueId` in module `1iR4`, `sync-v2-sub-worker.js:1620000`).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct QueueCursor {
    #[serde(rename = "lastId")]
    pub last_id: String,
    pub ids: Vec<String>,
}

/// State machine for Zalo's native HTTP long-polling message receiver (`getChatFromPolling`, cmd `99100`).
///
/// [O] Observed in `sync-v2-sub-worker.b7bec208c4c8f73e09f8.js` (offsets `1620000`, `3498103`, `14557981`):
/// - `R = new y({ type: "MSG", qosId: 91021, queueIds: [1, 2, 3], ... })`
/// - `getLastActionIds()` builds:
///   `"{lastActionId},{lastActionIdOther}," + encodeURIComponent(btoa(JSON.stringify(queues)) + ",1")`
/// - `getChatFromPolling(params)`:
///   `GET {zpw_chts_new[0]}/?zpw_ver=694&zpw_type=30&params={params}&ts={unix_sec}`
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PollState {
    pub last_action_id: String,
    pub last_action_id_other: String,
    pub queues: std::collections::BTreeMap<String, QueueCursor>,
}

impl Default for PollState {
    fn default() -> Self {
        let mut queues = std::collections::BTreeMap::new();
        for qid in ["1", "2", "3"] {
            queues.insert(
                qid.to_string(),
                QueueCursor {
                    last_id: "1".into(),
                    ids: Vec::new(),
                },
            );
        }
        Self {
            last_action_id: "1".into(),
            last_action_id_other: "1".into(),
            queues,
        }
    }
}

fn max_action_id(a: &str, b: &str) -> String {
    let na = a.parse::<u128>().unwrap_or(0);
    let nb = b.parse::<u128>().unwrap_or(0);
    if nb >= na && nb > 0 {
        b.to_string()
    } else if na > 0 {
        a.to_string()
    } else {
        "1".into()
    }
}

impl PollState {
    pub fn encode_params(&self) -> String {
        use base64::Engine;
        let q_json = serde_json::to_string(&self.queues).unwrap_or_else(|_| "{}".into());
        let b64 = base64::engine::general_purpose::STANDARD.encode(q_json.as_bytes());
        let tail = crate::session::urlencode(&format!("{b64},1"));
        format!(
            "{},{},{}",
            self.last_action_id, self.last_action_id_other, tail
        )
    }

    pub fn update_from_response(&mut self, resp: &Value) {
        let mut data_node = resp.get("data").unwrap_or(resp);
        if let Some(inner) = data_node.get("data").filter(|v| v.is_object()) {
            data_node = inner;
        }
        if let Some(reset_id) = pick_str(data_node, &["resetLastActionId"]) {
            if reset_id != "0" && reset_id.parse::<u128>().unwrap_or(0) > 0 {
                self.last_action_id = reset_id.clone();
                self.last_action_id_other = "1".into();
                for q in self.queues.values_mut() {
                    q.last_id = reset_id.clone();
                    q.ids.clear();
                }
                return;
            }
        }
        if let Some(lai) = pick_str(data_node, &["lastActionId"]) {
            if lai != "0" {
                self.last_action_id = max_action_id(&self.last_action_id, &lai);
            }
        }
        if let Some(laio) = pick_str(data_node, &["lastActionIdOther"]) {
            if laio != "0" {
                self.last_action_id_other = max_action_id(&self.last_action_id_other, &laio);
            }
        }
        if let Some(qs) = data_node.get("queueStatus").and_then(Value::as_object) {
            for (qid, qval) in qs {
                // Only update configured HTTP long-polling MSG queues ("1", "2", "3").
                // Adding socket-only queues ("15", "16", "7", "8") causes lp_chat to reject subsequent requests.
                let Some(entry) = self.queues.get_mut(qid) else {
                    continue;
                };
                if let Some(lid) = pick_str(qval, &["lastId"]) {
                    if lid != "0" {
                        entry.last_id = max_action_id(&entry.last_id, &lid);
                    }
                }
                if let Some(ids_arr) = qval.get("ids").and_then(Value::as_array) {
                    let max_lid = entry.last_id.parse::<u128>().unwrap_or(u128::MAX);
                    let mut new_ids: Vec<String> = ids_arr
                        .iter()
                        .filter_map(|v| match v {
                            Value::String(s) if !s.is_empty() => Some(s.clone()),
                            Value::Number(n) => Some(n.to_string()),
                            _ => None,
                        })
                        .filter(|s| s.parse::<u128>().map(|n| n <= max_lid).unwrap_or(true))
                        .collect();
                    if !new_ids.is_empty() {
                        // [O] `QueueControl.getLastActionIdsArray` (`sync-v2-sub-worker.js:1606950`):
                        // sort descending, take at most 10 <= lastId, then reverse so `ids` is ascending.
                        new_ids.sort_by_key(|s| std::cmp::Reverse(s.parse::<u128>().unwrap_or(0)));
                        new_ids.dedup();
                        new_ids.truncate(10);
                        new_ids.reverse();
                        entry.ids = new_ids;
                    }
                }
            }
        }
    }
}

/// Fetch cloud message history (`getCM` in `sync-v2-sub-worker.b7bec208c4c8f73e09f8.js:14595433`, cmd `12506` / `12507`).
///
/// [O] Observed in module `1EWQ` (`sync-v2-sub-worker.js:1545933`):
/// Cloud message history (`/api/cm/getrecentv2` and `/api/cm/getoldv2` on `group_cloud_message`)
/// is enabled for Group conversations (`g<groupId>`) and My Cloud (`sendToMeId`).
pub async fn get_cloud_messages(
    s: &Session,
    conv_id: &str,
    global_msg_id: &str,
    is_old: bool,
    count: u32,
) -> Result<Value> {
    if s.login.is_none() {
        anyhow::bail!("chưa đăng nhập");
    }
    let clean_id = conv_id
        .trim_start_matches("group_")
        .trim_start_matches('g')
        .trim_start_matches('G');
    let is_send2me = s
        .login
        .as_ref()
        .map(|l| !l.send2me_id.is_empty() && l.send2me_id == clean_id)
        .unwrap_or(false);
    let mut map = serde_json::Map::new();
    map.insert("groupId".into(), json!(clean_id));
    map.insert(
        "globalMsgId".into(),
        json!(if global_msg_id.is_empty() || global_msg_id == "0" {
            "9223372036854775808"
        } else {
            global_msg_id
        }),
    );
    map.insert("count".into(), json!(count));
    map.insert("msgIds".into(), json!([]));
    map.insert("imei".into(), json!(s.imei));
    if is_send2me {
        map.insert("isOA".into(), json!(1));
    }
    map.insert("src".into(), json!(if is_old { 3 } else { 1 }));

    let body = Value::Object(map);
    let params = s.encoded_params(&body)?;
    let ep = if is_old {
        "/api/cm/getoldv2"
    } else {
        "/api/cm/getrecentv2"
    };
    let host = if s.domains.cloud_msg.is_empty() {
        "https://tt-group-cm.api.zaloapp.com"
    } else {
        &s.domains.cloud_msg
    };
    let url = format!(
        "{}{ep}?{}&params={params}&nretry=0",
        host.trim_end_matches('/'),
        s.common_params()
    );
    let resp = s.get(&url).await?;
    let mut dec = s.maybe_decrypt(resp, s.session_key_bytes()).await;
    if let Some(d_str) = dec.get("data").and_then(Value::as_str) {
        let trimmed = d_str.trim();
        if (trimmed.starts_with('{') && trimmed.ends_with('}'))
            || (trimmed.starts_with('[') && trimmed.ends_with(']'))
        {
            if let Ok(parsed_data) = serde_json::from_str::<Value>(trimmed) {
                dec["data"] = parsed_data;
            }
        }
    }
    Ok(dec)
}

/// Call Zalo's native HTTP long-polling message endpoint (`getChatFromPolling`, cmd `99100`).
///
/// Uses a 12-second timeout matching `lp_chat` long-polling windows (`sync-v2-sub-worker.js:3498030`).
pub async fn get_chat_from_polling(s: &Session, state: &mut PollState) -> Result<Value> {
    if s.login.is_none() {
        anyhow::bail!("chưa đăng nhập");
    }
    let host = if s.domains.lp_chat.is_empty() {
        "https://p4-msg-chat.zaloapp.com"
    } else {
        &s.domains.lp_chat
    };
    let ts = crate::login::unix_now();
    let url = format!(
        "{}/?{}&params={}&ts={ts}",
        host.trim_end_matches('/'),
        s.common_params(),
        state.encode_params()
    );
    let fut = s.get(&url);
    match tokio::time::timeout(std::time::Duration::from_millis(12000), fut).await {
        Ok(Ok(resp)) => {
            let dec = s.maybe_decrypt(resp, s.session_key_bytes()).await;
            if let Some(code) = dec.get("error_code").and_then(Value::as_i64) {
                if code != 0 {
                    let msg = dec
                        .get("error_message")
                        .and_then(Value::as_str)
                        .unwrap_or("unknown");
                    return Err(crate::session::SessionError::new(code, msg).into());
                }
            }
            state.update_from_response(&dec);
            Ok(dec)
        }
        Ok(Err(e)) => Err(e),
        Err(_) => Ok(json!({"error_code": 0, "data": {}})),
    }
}

/// Poll both `getChatFromPolling` (`lp_chat`) and `get-last-msgs` (`convers_api`),
/// merge all incoming/outgoing/group messages into `threads`, resolve unknown names,
/// and persist to `zalo_history.db` and `threads_cache.json`.
pub async fn poll_new_messages(
    s: &Session,
    state: &mut PollState,
    threads: &mut Vec<Thread>,
) -> Result<()> {
    if s.login.is_none() {
        anyhow::bail!("chưa đăng nhập");
    }
    // 1. Poll live/offline message queues via `getChatFromPolling` (follow `more == 1` / `resetLastActionId` up to 3 batches)
    for _ in 0..3 {
        let Ok(lp_resp) = get_chat_from_polling(s, state).await else {
            break;
        };
        merge_previews(threads, &lp_resp);
        let data_node = lp_resp.get("data").unwrap_or(&lp_resp);
        let more = data_node.get("more").and_then(Value::as_i64).unwrap_or(0);
        let has_reset = pick_str(data_node, &["resetLastActionId"])
            .map(|r| r != "0" && !r.is_empty())
            .unwrap_or(false);
        if more == 0 && !has_reset {
            break;
        }
    }
    // 2. Poll conversation previews via `get-last-msgs` (both empty map and populated known thread IDs)
    if let Ok(preview_resp) = get_last_msgs(s, &[]).await {
        merge_previews(threads, &preview_resp);
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
    for chunk in known_ids.chunks(25) {
        if let Ok(preview_resp) = get_last_msgs(s, chunk).await {
            merge_previews(threads, &preview_resp);
        }
    }
    // 2b. Sync Cloud của tôi (`send2me_id`) and group conversations via `/api/cm/getrecentv2`
    let send2me_id = s
        .login
        .as_ref()
        .map(|l| l.send2me_id.clone())
        .unwrap_or_default();
    if !send2me_id.is_empty() {
        if let Ok(cm_resp) = get_cloud_messages(s, &send2me_id, "0", false, 50).await {
            if let Some(t) = threads.iter_mut().find(|t| t.id == send2me_id) {
                merge_group_history(t, &cm_resp);
            } else {
                merge_previews(threads, &cm_resp);
            }
        }
    }
    let group_ids: Vec<String> = threads
        .iter()
        .filter(|t| t.is_group && !t.id.is_empty())
        .map(|t| t.id.clone())
        .collect();
    for gid in group_ids {
        if let Ok(cm_resp) = get_cloud_messages(s, &gid, "0", false, 50).await {
            if let Some(t) = threads.iter_mut().find(|t| t.id == gid) {
                merge_group_history(t, &cm_resp);
            }
        }
    }
    // 3. Resolve display names for any newly discovered threads
    resolve_thread_names(s, threads).await;
    threads.retain(|t| !t.messages.is_empty() || !t.name.starts_with("Người dùng "));
    deduplicate_threads(threads, None);
    if let Ok(mut store) = crate::store::MessageStore::open("zalo_history.db") {
        let _ = crate::store::persist_threads(&mut store, threads);
    }
    save_threads_cache(threads);
    Ok(())
}

fn extract_content_text(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => {
            let trimmed = s.trim();
            if trimmed == "sendBubbleMessage" {
                return None;
            }
            if trimmed.starts_with('{') && trimmed.ends_with('}') {
                if let Ok(parsed) = serde_json::from_str::<Value>(trimmed) {
                    if let Some(inner) = extract_content_text(&parsed) {
                        if !inner.is_empty() {
                            return Some(inner);
                        }
                    }
                    // It is a JSON object with no text/caption fields (like sticker {catId, id} or photo {href}).
                    // Return None so parse_chat_message can assign a clean descriptive label like [Nhãn dán] or [Hình ảnh]
                    // instead of displaying the raw JSON string in chat.
                    return None;
                }
            }
            Some(s.clone())
        }
        Value::Object(o) => {
            let href = o
                .get("href")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|h| {
                    !h.is_empty() && (h.starts_with("http://") || h.starts_with("https://"))
                });
            if o.get("action").and_then(Value::as_str) == Some("rtf") {
                if let Some(title) = o.get("title").and_then(Value::as_str) {
                    if let Some(params_s) = o.get("params").and_then(Value::as_str) {
                        return Some(format_incoming_zalo_rtf(title, params_s));
                    }
                    return Some(title.to_string());
                }
            }
            for k in ["description", "caption", "text", "msg", "title"] {
                if let Some(Value::String(s)) = o.get(k) {
                    let t = s.trim();
                    if !t.is_empty() && t != "sendBubbleMessage" {
                        return Some(s.clone());
                    }
                }
            }
            if let Some(Value::String(s)) = o.get("fileName").or_else(|| o.get("name")) {
                if !s.trim().is_empty() {
                    return Some(format!("[Tập tin] {s}"));
                }
            }
            if let Some(link) = href {
                return Some(link.to_string());
            }
            None
        }
        _ => None,
    }
}

fn normalize_msg_type(it: &Value) -> String {
    let raw = it
        .get("msgType")
        .or_else(|| it.get("messageType"))
        .or_else(|| it.get("type"));
    match raw {
        Some(Value::String(s)) => s.to_ascii_lowercase(),
        Some(Value::Number(n)) => match n.as_i64() {
            Some(1) => "chat.text".into(),
            Some(2) => "chat.photo".into(),
            Some(3) => "chat.voice".into(),
            Some(4) => "chat.video".into(),
            Some(5) => "chat.sticker".into(),
            Some(19) => "chat.file".into(),
            Some(24) => "chat.location".into(),
            Some(31) => "chat.undo".into(),
            _ => n.to_string(),
        },
        _ => String::new(),
    }
}

fn extract_sticker_info(v: &Value) -> Option<(u64, u64, u32)> {
    match v {
        Value::String(s) => {
            let trimmed = s.trim();
            if trimmed.starts_with('{') && trimmed.ends_with('}') {
                if let Ok(parsed) = serde_json::from_str::<Value>(trimmed) {
                    return extract_sticker_info(&parsed);
                }
            }
            None
        }
        Value::Object(o) => {
            let parse_u64 = |val: &Value| -> Option<u64> {
                match val {
                    Value::Number(n) => n.as_u64(),
                    Value::String(s) => s.parse::<u64>().ok(),
                    _ => None,
                }
            };
            let cat_id = o
                .get("catId")
                .or_else(|| o.get("cateId"))
                .and_then(parse_u64);
            let stk_id = o
                .get("stickerId")
                .or_else(|| o.get("id"))
                .and_then(parse_u64);
            let stk_type = o
                .get("type")
                .and_then(parse_u64)
                .map(|t| t as u32)
                .unwrap_or(7);
            if let Some(sid) = stk_id {
                if cat_id.is_some()
                    || o.contains_key("stickerId")
                    || o.contains_key("catId")
                    || o.contains_key("cateId")
                {
                    return Some((sid, cat_id.unwrap_or(0), stk_type));
                }
            }
            None
        }
        _ => None,
    }
}

/// Check whether `url` points to an image asset (Zalo photo CDN, local photo, or direct image URL, #72).
pub fn is_zalo_photo_cdn_url(url: &str) -> bool {
    let u = url.trim().to_ascii_lowercase();
    if (!u.starts_with("http://") && !u.starts_with("https://") && !u.starts_with("local://"))
        || u.contains("/renewmedia")
    {
        return false;
    }
    let path_part = u.split('?').next().unwrap_or(&u);
    if [
        ".doc", ".docx", ".pdf", ".xls", ".xlsx", ".ppt", ".pptx", ".zip", ".rar", ".7z", ".tar",
        ".gz", ".txt", ".csv", ".mp4", ".mov", ".mkv", ".avi", ".mp3", ".m4a", ".wav", ".exe",
        ".dmg", ".apk",
    ]
    .iter()
    .any(|ext| path_part.ends_with(ext))
    {
        return false;
    }
    // Direct image file extensions anywhere
    if [
        ".jpg", ".jpeg", ".png", ".webp", ".gif", ".bmp", ".ico", ".jfif", ".tiff",
    ]
    .iter()
    .any(|ext| path_part.ends_with(ext))
    {
        return true;
    }
    // Zalo specific photo CDNs and subpaths
    u.contains("photo-stal")
        || u.contains("photo-zmp")
        || u.contains("photo-aka")
        || u.contains("photo-link-talk")
        || u.contains("res-zalo")
        || ((u.contains("zdn.vn") || u.contains("zadn.vn"))
            && (path_part.contains("/jpg/")
                || path_part.contains("/png/")
                || path_part.contains("/webp/")
                || path_part.contains("/gif/")
                || path_part.contains("/gr/")
                || path_part.contains("/no/")
                || u.contains("zpcloud")
                || u.contains("zpg-r")
                || u.contains("zfcloud")))
}

/// Check that a remote URL does not target loopback, private RFC1918, or link-local destinations.
pub fn is_safe_external_image_url(url: &str) -> bool {
    let lower = url.trim().to_ascii_lowercase();
    let without_scheme = if let Some(rest) = lower.strip_prefix("https://") {
        rest
    } else if let Some(rest) = lower.strip_prefix("http://") {
        rest
    } else {
        return false;
    };
    let host = without_scheme
        .split(&['/', '?', ':', '#'][..])
        .next()
        .unwrap_or("")
        .trim();
    if host.is_empty()
        || host == "localhost"
        || host.ends_with(".localhost")
        || host.ends_with(".local")
        || host == "127.0.0.1"
        || host == "[::1]"
        || host == "::1"
        || host.starts_with("127.")
        || host.starts_with("10.")
        || host.starts_with("192.168.")
        || host.starts_with("169.254.")
        || (host.starts_with("172.") && {
            let parts: Vec<&str> = host.split('.').collect();
            parts
                .get(1)
                .and_then(|p| p.parse::<u8>().ok())
                .map(|b| (16..=31).contains(&b))
                .unwrap_or(false)
        })
    {
        return false;
    }
    true
}

/// Sanitize and resolve the original sender filename for a Zalo file attachment (#74),
/// ensuring `/renewMedia` URLs never overwrite the sender's real filename.
pub fn resolve_attachment_file_name(title: Option<&str>, text: &str, url: &str) -> String {
    let is_usable_name = |s: &str| -> bool {
        let t = s.trim();
        !t.is_empty()
            && !t.starts_with("http://")
            && !t.starts_with("https://")
            && !t.eq_ignore_ascii_case("renewmedia")
            && !t.starts_with("renewMedia.")
            && !t.starts_with("renewmedia.")
            && !matches!(
                t,
                "[Tập tin]" | "[Chia sẻ]" | "[Hình ảnh]" | "[Video]" | "[Tin nhắn]"
            )
    };

    fn strip_prefix_tag(s: &str) -> &str {
        let t = s.trim();
        t.strip_prefix("[Tập tin]")
            .or_else(|| t.strip_prefix("[Chia sẻ]"))
            .map(str::trim)
            .unwrap_or(t)
    }

    let raw_candidate = title
        .map(strip_prefix_tag)
        .filter(|s| is_usable_name(s))
        .or_else(|| {
            let cleaned = strip_prefix_tag(text);
            is_usable_name(cleaned).then_some(cleaned)
        })
        .unwrap_or_else(|| {
            let path_seg = url
                .split('?')
                .next()
                .unwrap_or(url)
                .rsplit('/')
                .next()
                .unwrap_or("tap_tin_zalo");
            if path_seg.is_empty() || path_seg.eq_ignore_ascii_case("renewmedia") {
                "tap_tin_zalo"
            } else {
                path_seg
            }
        });

    let sanitized: String = raw_candidate
        .chars()
        .map(|c| {
            if matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') || c.is_control() {
                '_'
            } else {
                c
            }
        })
        .collect();
    let trimmed = sanitized.trim().trim_matches('.');
    if trimmed.is_empty() {
        "tap_tin_zalo".to_string()
    } else {
        trimmed.to_string()
    }
}

/// Extract rich link/media card metadata (`link_url`, `link_title`, `link_desc`, `link_thumb`, `link_src`)
/// from a Zalo `chat.recommended` (`recommened.link`) or rich-object `content`.
fn extract_link_info(
    v: &Value,
) -> (
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
) {
    match v {
        Value::String(s) => {
            let trimmed = s.trim();
            if trimmed.starts_with('{') && trimmed.ends_with('}') {
                if let Ok(parsed) = serde_json::from_str::<Value>(trimmed) {
                    return extract_link_info(&parsed);
                }
            }
            (None, None, None, None, None)
        }
        Value::Object(o) => {
            let href = o
                .get("href")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|h| {
                    !h.is_empty() && (h.starts_with("http://") || h.starts_with("https://"))
                })
                .map(str::to_string);
            let Some(url) = href else {
                return (None, None, None, None, None);
            };
            let thumb = o
                .get("thumb")
                .or_else(|| o.get("hd"))
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|t| {
                    !t.is_empty() && (t.starts_with("http://") || t.starts_with("https://"))
                })
                .map(str::to_string);
            let mut title = o
                .get("fileName")
                .or_else(|| o.get("name"))
                .or_else(|| o.get("title"))
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|t| !t.is_empty())
                .map(str::to_string);
            let desc = o
                .get("description")
                .or_else(|| o.get("caption"))
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|d| !d.is_empty())
                .map(str::to_string);
            let mut src = None;
            let mut file_ext: Option<String> = o
                .get("fileExt")
                .and_then(Value::as_str)
                .map(|s| s.trim().trim_start_matches('.').to_string())
                .filter(|s| !s.is_empty());
            let mut has_file_params =
                o.get("fileName").is_some() || url.to_ascii_lowercase().contains("/renewmedia");

            if let Some(params_str) = o.get("params").and_then(Value::as_str) {
                if let Ok(p_json) = serde_json::from_str::<Value>(params_str.trim()) {
                    if title.is_none() {
                        title = p_json
                            .get("fileName")
                            .or_else(|| p_json.get("mediaTitle"))
                            .and_then(Value::as_str)
                            .map(str::trim)
                            .filter(|s| !s.is_empty())
                            .map(str::to_string);
                    }
                    if file_ext.is_none() {
                        file_ext = p_json
                            .get("fileExt")
                            .and_then(Value::as_str)
                            .map(|s| s.trim().trim_start_matches('.').to_string())
                            .filter(|s| !s.is_empty());
                    }
                    if p_json.get("fileExt").is_some()
                        || p_json.get("fileSize").is_some()
                        || p_json.get("checksum").is_some()
                    {
                        has_file_params = true;
                    }
                    src = p_json
                        .get("src")
                        .and_then(Value::as_str)
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                        .map(str::to_string);
                }
            }
            if let (Some(ref mut t), Some(ref ext)) = (&mut title, &file_ext) {
                let suffix = format!(".{ext}");
                if !t
                    .to_ascii_lowercase()
                    .ends_with(&suffix.to_ascii_lowercase())
                    && !t.starts_with("http://")
                    && !t.starts_with("https://")
                {
                    t.push_str(&suffix);
                }
            }
            if src.is_none() && has_file_params {
                src = Some("Tập tin Zalo".to_string());
            }
            if src.is_none() && is_zalo_photo_cdn_url(&url) {
                src = Some("Hình ảnh Zalo".to_string());
            }
            if src.is_none() {
                src = url
                    .split("://")
                    .nth(1)
                    .and_then(|rest| rest.split('/').next())
                    .filter(|h| !h.is_empty())
                    .map(str::to_string);
            }
            (Some(url), title, desc, thumb, src)
        }
        _ => (None, None, None, None, None),
    }
}
fn parse_chat_message(it: &Value) -> Option<ChatMessage> {
    let msg_type = normalize_msg_type(it);

    let raw_content_val = it
        .get("content")
        .or_else(|| {
            it.get("message")
                .and_then(|m| m.get("content"))
                .or_else(|| it.get("message"))
        })
        .or_else(|| it.get("msg"));

    let mut sticker_meta = raw_content_val.and_then(extract_sticker_info);
    if sticker_meta.is_none()
        && (msg_type == "chat.sticker" || msg_type == "sticker" || msg_type == "5")
    {
        sticker_meta = extract_sticker_info(it);
    }
    let (link_url, mut link_title, link_desc, mut link_thumb, mut link_src) = raw_content_val
        .map(extract_link_info)
        .unwrap_or((None, None, None, None, None));

    let mut content = raw_content_val
        .and_then(extract_content_text)
        .unwrap_or_default();

    let is_photo_msg = matches!(msg_type.as_str(), "chat.photo" | "photo" | "2")
        || link_src.as_deref() == Some("Hình ảnh Zalo")
        || link_url.as_deref().is_some_and(is_zalo_photo_cdn_url)
        || link_thumb.as_deref().is_some_and(is_zalo_photo_cdn_url);
    let is_file_msg = matches!(
        msg_type.as_str(),
        "chat.file" | "file" | "19" | "share.file"
    ) || link_src.as_deref() == Some("Tập tin Zalo")
        || link_url
            .as_deref()
            .is_some_and(|u| u.to_ascii_lowercase().contains("/renewmedia"));

    if is_photo_msg && !is_file_msg && sticker_meta.is_none() {
        link_src = Some("Hình ảnh Zalo".to_string());
        if link_thumb.is_none() {
            link_thumb = link_url.clone();
        }
        let trimmed_c = content.trim();
        let is_raw_url_content =
            trimmed_c.starts_with("http://") || trimmed_c.starts_with("https://");
        if trimmed_c.is_empty() || is_raw_url_content {
            if let Some(cap) = link_desc.as_deref().map(str::trim).filter(|d| {
                !d.is_empty() && !d.starts_with("http://") && !d.starts_with("https://")
            }) {
                content = format!("[Hình ảnh] {cap}");
            } else {
                content = "[Hình ảnh]".to_string();
            }
        }
    } else if is_file_msg && sticker_meta.is_none() {
        link_src = Some("Tập tin Zalo".to_string());
        if link_url.is_some() || link_title.is_some() {
            let resolved_name = resolve_attachment_file_name(
                link_title.as_deref(),
                &content,
                link_url.as_deref().unwrap_or_default(),
            );
            if link_title.is_none()
                || link_title
                    .as_deref()
                    .is_some_and(|t| t.eq_ignore_ascii_case("renewmedia"))
            {
                link_title = Some(resolved_name.clone());
            }
            let final_title = link_title.as_deref().unwrap_or(&resolved_name);
            let trimmed_c = content.trim();
            if !trimmed_c.starts_with("[Tập tin]") {
                content = format!("[Tập tin] {final_title}");
            }
        }
    }
    let is_undo_type = matches!(msg_type.as_str(), "chat.undo" | "undo" | "31");
    if is_undo_type {
        content = "[Tin nhắn đã thu hồi]".to_string();
        sticker_meta = None;
    } else if sticker_meta.is_some() && (content.is_empty() || content.starts_with('{')) {
        content = "[Nhãn dán]".to_string();
    }
    if sticker_meta.is_none() {
        let parse_ami = |s: &str| -> Option<(u64, u64, u32)> {
            let pos = s.find("ami_fat_cat_")?;
            let num_str = s.get(pos + 12..pos + 14)?;
            let idx = num_str.parse::<u64>().ok()?;
            if (1..=15).contains(&idx) {
                Some((999000 + idx, 99901, 7))
            } else {
                None
            }
        };
        if let Some(m) = parse_ami(&content)
            .or_else(|| link_desc.as_deref().and_then(parse_ami))
            .or_else(|| link_title.as_deref().and_then(parse_ami))
            .or_else(|| link_url.as_deref().and_then(parse_ami))
        {
            sticker_meta = Some(m);
            content = "[Nhãn dán]".to_string();
        }
    }

    if content.is_empty() {
        content = match msg_type.as_str() {
            "chat.sticker" | "sticker" | "5" => "[Nhãn dán]".to_string(),
            "chat.photo" | "photo" | "2" => "[Hình ảnh]".to_string(),
            "chat.voice" | "voice" | "3" => "[Tin nhắn thoại]".to_string(),
            "chat.video" | "video" | "4" => "[Video]".to_string(),
            "chat.file" | "file" | "19" => "[Tập tin]".to_string(),
            "chat.location" | "location" | "24" => "[Vị trí]".to_string(),
            "chat.contact" | "contact" => "[Danh thiếp]".to_string(),
            "chat.undo" | "undo" | "31" => "[Tin nhắn đã thu hồi]".to_string(),
            "chat.recommended" | "share.file" => "[Chia sẻ]".to_string(),
            _ => {
                let obj = it.get("content").or_else(|| it.get("message"));
                if let Some(o) = obj {
                    if o.get("catId").is_some()
                        || o.get("stickerId").is_some()
                        || o.get("id").is_some()
                    {
                        "[Nhãn dán]".to_string()
                    } else if o.get("href").is_some() || o.get("thumb").is_some() {
                        "[Hình ảnh]".to_string()
                    } else if o.is_object() {
                        "[Tin nhắn]".to_string()
                    } else {
                        String::new()
                    }
                } else {
                    String::new()
                }
            }
        };
    }

    let undo_target_id = if is_undo_type {
        raw_content_val.and_then(|rv| match rv {
            Value::Object(_) => pick_str(rv, &["globalMsgId", "msgId", "cliMsgId"]),
            Value::String(s) if s.trim().starts_with('{') => {
                serde_json::from_str::<Value>(s.trim())
                    .ok()
                    .and_then(|parsed| pick_str(&parsed, &["globalMsgId", "msgId", "cliMsgId"]))
            }
            _ => None,
        })
    } else {
        None
    };

    // Preserve the persisted canonical ID contract until a schema-aware migration
    // can replace legacy cliMsgId keys without duplicating cached messages.
    let msg_id = undo_target_id
        .or_else(|| {
            ["msgId", "cliMsgId", "globalMsgId", "lastMsgId", "id"]
                .iter()
                .find_map(|key| wire_identity(it.get(*key)))
        })
        .unwrap_or_default();

    if content.is_empty() && msg_id.is_empty() {
        return None;
    }

    let uid_from = it
        .get("uidFrom")
        .or_else(|| it.get("fromUid"))
        .or_else(|| it.get("fromId"))
        .or_else(|| it.get("senderId"))
        .or_else(|| it.get("sender"))
        .and_then(|v| match v {
            Value::String(s) => Some(s.clone()),
            Value::Number(n) => Some(n.to_string()),
            _ => None,
        })
        .unwrap_or_default();

    let display_sender = it
        .get("dName")
        .or_else(|| it.get("displayName"))
        .or_else(|| it.get("fromName"))
        .or_else(|| it.get("senderName"))
        .and_then(Value::as_str)
        .filter(|s| !s.trim().is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| uid_from.clone());

    let is_outgoing = uid_from == "0"
        || uid_from.is_empty()
        || it
            .get("isOutgoing")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        || it
            .get("is_outgoing")
            .and_then(Value::as_i64)
            .map(|n| n == 1)
            .unwrap_or(false);

    let ts = it
        .get("ts")
        .or_else(|| it.get("cliMsgId"))
        .or_else(|| it.get("sendTime"))
        .or_else(|| it.get("serverTime"))
        .or_else(|| it.get("time"))
        .and_then(|v| match v {
            Value::Number(n) => n.as_i64(),
            Value::String(s) => s.parse::<i64>().ok(),
            _ => None,
        })
        .unwrap_or(0);

    let time = if ts > 0 {
        crate::clock::message_time(ts)
    } else {
        crate::ui::clock()
    };

    let (reply_to_id, reply_to_sender, reply_to_text) =
        if let Some(q) = it.get("quote").filter(|v| v.is_object()) {
            let q_id = pick_str(q, &["cliMsgId", "globalMsgId", "msgId"]);
            let q_sender = pick_str(q, &["fromD", "dName", "ownerId", "fromUid", "uidFrom"]);
            let q_text = pick_str(q, &["msg", "message", "content"]).or_else(|| {
                q.get("msg")
                    .or_else(|| q.get("content"))
                    .and_then(extract_content_text)
            });
            (q_id, q_sender, q_text)
        } else {
            (None, None, None)
        };

    let (reactions, reaction_summaries) = {
        let mut summaries = crate::reactions::parse_reactions(it);
        if summaries.is_empty() {
            if let Some(c) = raw_content_val {
                summaries = crate::reactions::parse_reactions(c);
            }
        }
        let reacts = if !summaries.is_empty() {
            crate::reactions::legacy_emojis(&summaries)
        } else {
            let legacy = extract_reactions(it);
            if legacy.is_empty() {
                if let Some(c) = raw_content_val {
                    extract_reactions(c)
                } else {
                    Vec::new()
                }
            } else {
                legacy
            }
        };
        if summaries.is_empty() && !reacts.is_empty() {
            summaries = reacts
                .iter()
                .map(|e| crate::reactions::ReactionSummary {
                    emoji: e.clone(),
                    count: 1,
                    ..Default::default()
                })
                .collect();
        }
        (reacts, summaries)
    };

    Some(ChatMessage {
        id: msg_id,
        client_message_id: ["cliMsgId", "clientId", "client_message_id"]
            .iter()
            .find_map(|key| wire_identity(it.get(*key)))
            .unwrap_or_default(),
        wire_ref: crate::message_model::WireMessageRef::from_raw(it),
        sender: display_sender,
        text: content,
        time,
        is_outgoing,
        ts,
        sticker_id: sticker_meta.map(|(sid, _, _)| sid),
        sticker_cate_id: sticker_meta.map(|(_, cid, _)| cid),
        sticker_type: sticker_meta.map(|(_, _, st)| st),
        reply_to_id,
        reply_to_sender,
        reply_to_text,
        reactions,
        reaction_summaries,
        is_pinned: false,
        is_starred: false,
        link_url,
        link_title,
        link_desc,
        link_thumb,
        link_src,

        ..Default::default()
    })
}

fn extract_reactions(it: &Value) -> Vec<String> {
    let mut res = Vec::new();
    if let Some(r) = it.get("reacts").or_else(|| it.get("reactions")) {
        let arr_opt = r
            .get("reactions")
            .and_then(Value::as_array)
            .or_else(|| r.as_array());
        if let Some(arr) = arr_opt {
            for item in arr {
                let icon = item
                    .as_str()
                    .or_else(|| item.get("rIcon").and_then(Value::as_str));
                if let Some(icon) = icon {
                    let em = icon_to_emoji(icon);
                    if !res.contains(&em.to_string()) {
                        res.push(em.to_string());
                    }
                }
            }
        }
    }
    res
}

fn is_group_flag(v: &Value) -> bool {
    v.get("isGroup")
        .map(|g| g.as_bool() == Some(true) || g.as_i64() == Some(1))
        .unwrap_or(false)
}

fn extract_message_thread_id_ext(
    entry: &Value,
    m_val: &Value,
    in_group_bucket: bool,
    cloud_thread_id: Option<&str>,
) -> Option<(String, bool)> {
    let is_group = in_group_bucket || is_group_flag(m_val) || is_group_flag(entry);

    // 1. Explicit `grid` field or `groupMsgs` bucket (`e.msgKey = GROUPID_PREFIX + e.idTo` in `sync-v2-sub-worker.js:3500340`)
    for obj in [m_val, entry] {
        if let Some(grid) = pick_str(obj, &["grid", "groupId"]) {
            if !grid.is_empty() && grid != "0" {
                let clean = grid.trim_start_matches("group_").to_string();
                return Some((clean, true));
            }
        }
    }
    if is_group {
        if let Some(gid) = pick_str(m_val, &["idTo", "toid", "toId", "threadId", "conversId"])
            .or_else(|| pick_str(entry, &["idTo", "toid", "toId", "threadId", "conversId"]))
        {
            if !gid.is_empty() && gid != "0" {
                let clean = gid.trim_start_matches("group_").to_string();
                return Some((clean, true));
            }
        }
    }

    // 2. 1-on-1 or self ("Cloud của tôi") message routing
    // [O] `sync-v2-sub-worker.js:3500260`:
    //   `"0" == e.idTo && "0" == e.uidFrom && (e.idTo = "" + this.userId)`
    // [O] `sync-v2-sub-worker.js:829500` (`_filterDuplicatedMessages`):
    //   `e.isGroup ? e.msgKey : "0" == e.idTo ? e.uidFrom : e.idTo`
    let uid_from = pick_str(m_val, &["uidFrom", "fromUid", "fromId", "senderId"])
        .or_else(|| pick_str(entry, &["uidFrom", "fromUid", "fromId", "senderId"]))
        .unwrap_or_default();
    let id_to = pick_str(m_val, &["idTo", "toid", "toId"])
        .or_else(|| pick_str(entry, &["idTo", "toid", "toId"]))
        .unwrap_or_default();

    if (id_to.is_empty() || id_to == "0") && (uid_from.is_empty() || uid_from == "0") {
        if let Some(cid) = cloud_thread_id {
            if !cid.is_empty() {
                return Some((cid.to_string(), false));
            }
        }
    }

    if !uid_from.is_empty()
        && uid_from != "0"
        && (id_to.is_empty() || id_to == "0" || Some(id_to.as_str()) == cloud_thread_id)
    {
        // Incoming 1-1 message: peer sent it to us -> thread ID is `uid_from`
        Some((uid_from, false))
    } else if !id_to.is_empty() && id_to != "0" {
        // Outgoing 1-1 message or conversation wrapper with explicit `idTo` -> thread ID is `id_to`
        Some((id_to, false))
    } else if !uid_from.is_empty() && uid_from != "0" {
        Some((uid_from, false))
    } else {
        pick_str(
            m_val,
            &["threadId", "conversId", "userId", "friendId", "id"],
        )
        .or_else(|| {
            pick_str(
                entry,
                &["threadId", "conversId", "userId", "friendId", "id"],
            )
        })
        .filter(|s| s != "0" && !s.is_empty())
        .map(|s| (s, is_group))
    }
}

/// Fold a `get-last-msgs` or `getChatFromPolling` response into thread previews and conversation messages.
pub fn merge_previews(threads: &mut Vec<Thread>, resp: &Value) {
    let mut entries: Vec<(&Value, bool)> = Vec::new();
    let parsed_str_data: Option<Value> = resp
        .get("data")
        .and_then(Value::as_str)
        .and_then(|s| serde_json::from_str::<Value>(s.trim()).ok());
    let mut data_node = parsed_str_data
        .as_ref()
        .unwrap_or_else(|| resp.get("data").unwrap_or(resp));
    if let Some(inner_data) = data_node
        .get("data")
        .filter(|v| v.is_object() || v.is_array())
    {
        data_node = inner_data;
    }
    let cloud_tid: Option<String> = threads
        .iter()
        .find(|t| t.name.contains("Cloud của tôi"))
        .map(|t| t.id.clone());

    if let Some(arr) = data_node.as_array() {
        for item in arr {
            entries.push((item, is_group_flag(item)));
        }
    } else if let Some(obj) = data_node.as_object() {
        let mut found_list = false;
        for (key, is_grp) in [
            ("msgs", false),
            ("groupMsgs", true),
            ("pageMsgs", false),
            ("conversations", false),
            ("threads", false),
            ("allmsgs", false),
        ] {
            if let Some(arr) = obj.get(key).and_then(Value::as_array) {
                for item in arr {
                    entries.push((item, is_grp || is_group_flag(item)));
                }
                found_list = true;
            }
        }
        if let Some(cu_arr) = obj.get("clearUnreads").and_then(Value::as_array) {
            for cu in cu_arr {
                let Some(tid) = pick_str(cu, &["idTo", "threadId", "userId"]) else {
                    continue;
                };
                if tid.is_empty() || tid == "0" {
                    continue;
                }
                let is_grp = is_group_flag(cu);
                // Only auto-create 1-1 conversation threads from `clearUnreads`; disbanded groups
                // may linger in `clearUnreads` after removal, whereas active groups come from `get_groups`.
                if !is_grp && !threads.iter().any(|t| t.id == tid) {
                    threads.push(Thread {
                        presentation: Default::default(),
                        unread: Default::default(),
                        id: tid.clone(),
                        name: format!("Người dùng {tid}"),
                        preview: String::new(),
                        time: String::new(),
                        avatar: None,
                        is_group: false,
                        messages: Vec::new(),
                    });
                }
            }
            found_list = true;
        }
        if !found_list {
            for (_, val) in obj {
                if val.is_object() {
                    entries.push((val, is_group_flag(val)));
                }
            }
        }
    }

    let mut message_indices: Vec<MessageIdentityIndex> = threads
        .iter()
        .map(|thread| {
            let mut index = MessageIdentityIndex::default();
            for (row, message) in thread.messages.iter().enumerate() {
                index.insert(message, row);
            }
            index
        })
        .collect();

    for (entry, is_grp_bucket) in entries {
        let mut raw_msgs: Vec<(&Value, bool)> = Vec::new();
        if let Some(arr) = entry.get("messages").and_then(Value::as_array) {
            for m in arr {
                raw_msgs.push((m, is_grp_bucket || is_group_flag(m)));
            }
        }
        if let Some(arr) = entry.get("msgs").and_then(Value::as_array) {
            for m in arr {
                raw_msgs.push((m, is_grp_bucket || is_group_flag(m)));
            }
        }
        if let Some(arr) = entry.get("groupMsgs").and_then(Value::as_array) {
            for m in arr {
                raw_msgs.push((m, true));
            }
        }
        if let Some(obj) = entry.get("lastMsg").filter(|v| v.is_object()) {
            raw_msgs.push((obj, is_grp_bucket || is_group_flag(obj)));
        }
        if let Some(obj) = entry.get("message").filter(|v| v.is_object()) {
            raw_msgs.push((obj, is_grp_bucket || is_group_flag(obj)));
        }
        if raw_msgs.is_empty()
            && (entry.get("content").is_some()
                || entry.get("msg").is_some()
                || entry.get("msgId").is_some())
        {
            raw_msgs.push((entry, is_grp_bucket));
        }

        for (m_val, is_grp) in raw_msgs {
            if m_val.get("msgType").and_then(Value::as_str) == Some("chat.reaction") {
                let events = crate::reactions::parse_reaction_events(m_val);
                for ev in events {
                    for t in threads.iter_mut() {
                        for m in t.messages.iter_mut() {
                            if (!ev.message_id.is_empty() && m.id == ev.message_id)
                                || (!ev.client_message_id.is_empty()
                                    && m.client_message_id == ev.client_message_id)
                            {
                                m.apply_reaction(&ev);
                            }
                        }
                    }
                }
                continue;
            }

            let Some(parsed) = parse_chat_message(m_val) else {
                continue;
            };

            let Some((tid, thread_is_group)) =
                extract_message_thread_id_ext(entry, m_val, is_grp, cloud_tid.as_deref())
            else {
                continue;
            };

            if let Some(thread_index) = threads.iter().position(|thread| thread.id == tid) {
                let t = &mut threads[thread_index];
                let identities = &mut message_indices[thread_index];
                if thread_is_group {
                    t.is_group = true;
                }
                let is_incoming = !parsed.is_outgoing;
                let parsed_ts = parsed.ts;
                if let Some(row) = identities.find(&t.messages, &parsed) {
                    let existing = &mut t.messages[row];
                    identities.remove(existing, row);
                    existing.merge_reaction_metadata(&parsed);
                    if existing.id.starts_with("out_")
                        && !parsed.id.is_empty()
                        && !parsed.id.starts_with("out_")
                    {
                        existing.id = parsed.id;
                    }
                    if parsed.ts > 0 && existing.ts != parsed.ts {
                        existing.ts = parsed.ts;
                        existing.time = parsed.time;
                    }
                    if parsed.text.trim() == "[Tin nhắn đã thu hồi]" {
                        if !existing.text.starts_with("[Đã thu hồi]") {
                            if existing.text.trim().is_empty() {
                                existing.text = "[Tin nhắn đã thu hồi]".to_string();
                            } else {
                                existing.text = format!("[Đã thu hồi] {}", existing.text);
                            }
                        }
                    }
                    if existing.sticker_id.is_none() && parsed.sticker_id.is_some() {
                        existing.sticker_id = parsed.sticker_id;
                        existing.sticker_cate_id = parsed.sticker_cate_id;
                        existing.sticker_type = parsed.sticker_type;
                    }
                    if existing.reply_to_text.is_none() && parsed.reply_to_text.is_some() {
                        existing.reply_to_id = parsed.reply_to_id;
                        existing.reply_to_sender = parsed.reply_to_sender;
                        existing.reply_to_text = parsed.reply_to_text;
                    }
                    if existing.link_url.is_none() && parsed.link_url.is_some() {
                        existing.link_url = parsed.link_url;
                        existing.link_title = parsed.link_title;
                        existing.link_desc = parsed.link_desc;
                        existing.link_thumb = parsed.link_thumb;
                        existing.link_src = parsed.link_src;
                        if !parsed.text.is_empty() {
                            existing.text = parsed.text;
                        }
                    }
                    if !parsed.reactions.is_empty() {
                        existing.reactions = parsed.reactions;
                    }
                    if !parsed.reaction_summaries.is_empty() {
                        crate::reactions::trace_count_change(
                            "merge_previews:overwrite_summaries",
                            &existing.id,
                            crate::reactions::total_count(&existing.reaction_summaries),
                            crate::reactions::total_count(&parsed.reaction_summaries),
                        );
                        existing.reaction_summaries = parsed.reaction_summaries;
                    }
                    existing.is_pinned = existing.is_pinned || parsed.is_pinned;
                    existing.is_starred = existing.is_starred || parsed.is_starred;
                    identities.insert(existing, row);
                } else {
                    identities.insert(&parsed, t.messages.len());
                    t.messages.push(parsed);
                }
                if is_incoming {
                    for em in t.messages.iter_mut() {
                        if em.is_outgoing && (em.ts <= parsed_ts || parsed_ts == 0) {
                            em.delivery = crate::message_model::DeliveryState::Read;
                        }
                    }
                }
            } else {
                // New thread detected from incoming message!
                let default_name = if thread_is_group {
                    format!("Nhóm {}", tid)
                } else {
                    format!("Người dùng {}", tid)
                };
                let mut identities = MessageIdentityIndex::default();
                identities.insert(&parsed, 0);
                message_indices.push(identities);
                threads.push(Thread {
                    presentation: Default::default(),
                    unread: Default::default(),
                    id: tid,
                    name: default_name,
                    preview: parsed.text.clone(),
                    time: parsed.time.clone(),
                    avatar: None,
                    is_group: thread_is_group,
                    messages: vec![parsed],
                });
            }
        }
    }

    for t in threads.iter_mut() {
        if !t.messages.is_empty() {
            t.messages.sort_by_key(|m| m.ts);
            if let Some(newest) = t.messages.last() {
                if !newest.text.is_empty() {
                    t.preview = newest.text.replace('\n', " ");
                    t.time = newest.time.clone();
                }
            }
        }
    }
}

/// Merge group history response into a thread's messages, sorted by timestamp and deduplicated by msg_id.
pub fn merge_group_history(thread: &mut Thread, resp: &Value) {
    let mut raw_msgs: Vec<&Value> = Vec::new();
    let parsed_str_data: Option<Value> = resp
        .get("data")
        .and_then(Value::as_str)
        .and_then(|s| serde_json::from_str::<Value>(s.trim()).ok());
    let mut data_node = parsed_str_data
        .as_ref()
        .unwrap_or_else(|| resp.get("data").unwrap_or(resp));
    if let Some(inner_data) = data_node
        .get("data")
        .filter(|v| v.is_object() || v.is_array())
    {
        data_node = inner_data;
    }

    if let Some(arr) = data_node.as_array() {
        raw_msgs.extend(arr);
    } else if let Some(obj) = data_node.as_object() {
        for key in ["msgs", "groupMsgs", "messages"] {
            if let Some(arr) = obj.get(key).and_then(Value::as_array) {
                raw_msgs.extend(arr);
            }
        }
    }

    let clean_thread_id = thread
        .id
        .trim_start_matches("group_")
        .trim_start_matches('g')
        .trim_start_matches('G')
        .to_string();

    let mut identities = MessageIdentityIndex::default();
    for (row, message) in thread.messages.iter().enumerate() {
        identities.insert(message, row);
    }

    for it in raw_msgs {
        if !clean_thread_id.is_empty() {
            if let Some(id_to) = pick_str(it, &["idTo", "toId", "grid", "groupId"]) {
                let clean_to = id_to
                    .trim_start_matches("group_")
                    .trim_start_matches('g')
                    .trim_start_matches('G');
                if !clean_to.is_empty() && clean_to != "0" && clean_to != clean_thread_id {
                    continue;
                }
            }
        }
        if let Some(m) = parse_chat_message(it) {
            if let Some(row) = identities.find(&thread.messages, &m) {
                let existing = &mut thread.messages[row];
                identities.remove(existing, row);
                existing.merge_reaction_metadata(&m);
                if existing.id.starts_with("out_") && !m.id.is_empty() && !m.id.starts_with("out_")
                {
                    existing.id = m.id.clone();
                }
                if existing.link_url.is_none() && m.link_url.is_some() {
                    existing.link_url = m.link_url;
                    existing.link_title = m.link_title;
                    existing.link_desc = m.link_desc;
                    existing.link_thumb = m.link_thumb;
                    existing.link_src = m.link_src;
                    if !m.text.is_empty() {
                        existing.text = m.text;
                    }
                }
                identities.insert(existing, row);
            } else {
                identities.insert(&m, thread.messages.len());
                thread.messages.push(m);
            }
        }
    }

    thread.messages.sort_by_key(|m| m.ts);

    if let Some(newest) = thread.messages.last() {
        if !newest.text.is_empty() {
            thread.preview = newest.text.replace('\n', " ");
            thread.time = newest.time.clone();
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum FriendshipStatus {
    #[default]
    Unknown,
    Friend,
    OutgoingPending,
    IncomingPending,
    Stranger,
}

/// Parse `/api/friend/reqstatus` (`cmd 11430`) JSON into `FriendshipStatus`
/// (`compact-app-pc.js:8351970`: `is_friend`, `is_requested` = I requested you, `is_requesting` = you requested me).
pub fn parse_friendship_status_response(resp: &Value) -> FriendshipStatus {
    let d = resp
        .get("data")
        .and_then(|x| x.get("data").or(Some(x)))
        .unwrap_or(resp);
    let is_friend = d
        .get("is_friend")
        .or_else(|| d.get("isFriend"))
        .and_then(|v| {
            v.as_i64()
                .or_else(|| v.as_bool().map(|b| if b { 1 } else { 0 }))
        })
        .unwrap_or(0);
    if is_friend == 1 {
        return FriendshipStatus::Friend;
    }
    let is_requesting = d
        .get("is_requesting")
        .or_else(|| d.get("isRequesting"))
        .and_then(|v| {
            v.as_i64()
                .or_else(|| v.as_bool().map(|b| if b { 1 } else { 0 }))
        })
        .unwrap_or(0);
    if is_requesting == 1 {
        return FriendshipStatus::IncomingPending;
    }
    let is_requested = d
        .get("is_requested")
        .or_else(|| d.get("isRequested"))
        .and_then(|v| {
            v.as_i64()
                .or_else(|| v.as_bool().map(|b| if b { 1 } else { 0 }))
        })
        .unwrap_or(0);
    if is_requested == 1 {
        return FriendshipStatus::OutgoingPending;
    }
    FriendshipStatus::Stranger
}

/// Query `/api/friend/reqstatus` (`cmd 11430`) and return the typed `FriendshipStatus`.
pub async fn check_friendship_status(s: &Session, fid: &str) -> Result<FriendshipStatus> {
    let resp = get_friend_request_status(s, fid).await?;
    Ok(parse_friendship_status_response(&resp))
}
/// Check friend request status with a target user (`/api/friend/reqstatus`, cmd `11430`).
pub async fn get_friend_request_status(s: &Session, fid: &str) -> Result<Value> {
    if s.login.is_none() {
        anyhow::bail!("chưa đăng nhập");
    }
    let body = obj(&[("fid", json!(fid)), ("imei", json!(s.imei))]);
    let enc = s.encoded_params(&body)?;
    let host = if s.domains.friend.is_empty() {
        "https://tt-friend-wpa.chat.zalo.me"
    } else {
        &s.domains.friend
    };
    let url = format!(
        "{}/api/friend/reqstatus?{}&params={enc}",
        host.trim_end_matches('/'),
        s.common_params()
    );
    let resp = s.get(&url).await?;
    Ok(s.maybe_decrypt(resp, s.session_key_bytes()).await)
}

/// Query `/api/social/profile/lastOnline` (`cmd 12090`) and return `(is_online, label)`
/// matching Zalo PC's `getLastActiveText` (`user_active_time = 5 minutes`).
pub async fn get_user_online_status(s: &Session, uid: &str) -> Result<(bool, String)> {
    if s.login.is_none() || uid.trim().is_empty() {
        return Ok((false, String::new()));
    }
    let body = obj(&[
        ("uid", json!(uid.trim())),
        ("conv_type", json!(1)),
        ("imei", json!(s.imei)),
    ]);
    let enc = s.encoded_params(&body)?;
    let host = if s.domains.profile.is_empty() {
        "https://tt-profile-wpa.chat.zalo.me"
    } else {
        &s.domains.profile
    };
    let url = format!(
        "{}/api/social/profile/lastOnline?{}&params={enc}",
        host.trim_end_matches('/'),
        s.common_params()
    );
    let resp = s.get(&url).await?;
    let mut dec = s.maybe_decrypt(resp, s.session_key_bytes()).await;
    if let Some(d_str) = dec.get("data").and_then(Value::as_str) {
        if let Ok(parsed) = serde_json::from_str::<Value>(d_str.trim()) {
            dec["data"] = parsed;
        }
    }
    let d = dec
        .pointer("/data/data")
        .or_else(|| dec.get("data"))
        .unwrap_or(&dec);
    let show_online = d
        .pointer("/settings/show_online_status")
        .and_then(|v| v.as_bool().or_else(|| v.as_i64().map(|n| n != 0)))
        .unwrap_or(true);
    if !show_online {
        return Ok((false, String::new()));
    }
    let last_onl_raw = d
        .get("lastOnline")
        .and_then(|v| {
            v.as_i64()
                .or_else(|| v.as_str().and_then(|s| s.trim().parse::<i64>().ok()))
        })
        .unwrap_or(0);
    if last_onl_raw <= 0 {
        return Ok((false, String::new()));
    }
    let last_sec = if last_onl_raw > 10_000_000_000 {
        last_onl_raw / 1000
    } else {
        last_onl_raw
    };
    let now_sec = crate::login::unix_now() as i64;
    let diff_sec = (now_sec - last_sec).max(0);
    if diff_sec <= 300 {
        return Ok((true, "Vừa mới truy cập".into()));
    }
    let mins = diff_sec / 60;
    if mins < 60 {
        return Ok((false, format!("Truy cập {mins} phút trước")));
    }
    let hours = diff_sec / 3600;
    if hours < 24 {
        return Ok((false, format!("Truy cập {hours} giờ trước")));
    }
    let days = (diff_sec / 86400).min(7);
    Ok((false, format!("Truy cập {days} ngày trước")))
}

/// Send a friend request to a target user (`/api/friend/sendreq`, cmd `11424`).
pub async fn send_friend_request(s: &Session, to_id: &str, msg: &str) -> Result<Value> {
    if s.login.is_none() {
        anyhow::bail!("chưa đăng nhập");
    }
    let body = obj(&[
        ("toid", json!(to_id)),
        ("msg", json!(msg)),
        ("reqsrc", json!(30)),
        ("imei", json!(s.imei)),
        ("language", json!("vi")),
    ]);
    let enc = s.encoded_params(&body)?;
    let host = if s.domains.friend.is_empty() {
        "https://tt-friend-wpa.chat.zalo.me"
    } else {
        &s.domains.friend
    };
    let url = format!(
        "{}/api/friend/sendreq?{}",
        host.trim_end_matches('/'),
        s.common_params()
    );
    let resp = s.post_form(&url, format!("params={enc}")).await?;
    let dec = s.maybe_decrypt(resp, s.session_key_bytes()).await;
    if let Some(code) = dec.get("error_code").and_then(Value::as_i64) {
        if code != 0 {
            let err_msg = dec
                .get("error_message")
                .and_then(Value::as_str)
                .unwrap_or("unknown");
            anyhow::bail!("sendreq error {code}: {err_msg}");
        }
    }
    Ok(dec)
}

/// Accept a pending incoming friend request (`/api/friend/accept`, cmd `11427`).
pub async fn accept_friend_request(s: &Session, fid: &str) -> Result<Value> {
    if s.login.is_none() {
        anyhow::bail!("chưa đăng nhập");
    }
    let body = obj(&[("fid", json!(fid)), ("language", json!("vi"))]);
    let enc = s.encoded_params(&body)?;
    let host = if s.domains.friend.is_empty() {
        "https://tt-friend-wpa.chat.zalo.me"
    } else {
        &s.domains.friend
    };
    let url = format!(
        "{}/api/friend/accept?{}",
        host.trim_end_matches('/'),
        s.common_params()
    );
    let resp = s.post_form(&url, format!("params={enc}")).await?;
    let dec = s.maybe_decrypt(resp, s.session_key_bytes()).await;
    if let Some(code) = dec.get("error_code").and_then(Value::as_i64) {
        if code != 0 {
            let err_msg = dec
                .get("error_message")
                .and_then(Value::as_str)
                .unwrap_or("unknown");
            anyhow::bail!("accept error {code}: {err_msg}");
        }
    }
    Ok(dec)
}

/// Cancel / undo a pending outgoing friend request (`/api/friend/undo`, cmd `11441`).
pub async fn undo_friend_request(s: &Session, fid: &str) -> Result<Value> {
    if s.login.is_none() {
        anyhow::bail!("chưa đăng nhập");
    }
    let body = obj(&[("fid", json!(fid)), ("imei", json!(s.imei))]);
    let enc = s.encoded_params(&body)?;
    let host = if s.domains.friend.is_empty() {
        "https://tt-friend-wpa.chat.zalo.me"
    } else {
        &s.domains.friend
    };
    let url = format!(
        "{}/api/friend/undo?{}",
        host.trim_end_matches('/'),
        s.common_params()
    );
    let resp = s.post_form(&url, format!("params={enc}")).await?;
    Ok(s.maybe_decrypt(resp, s.session_key_bytes()).await)
}

/// Reject a pending incoming friend request (`/api/friend/reject`, cmd `11428`).
pub async fn reject_friend_request(s: &Session, fid: &str) -> Result<Value> {
    if s.login.is_none() {
        anyhow::bail!("chưa đăng nhập");
    }
    let body = obj(&[("fid", json!(fid)), ("imei", json!(s.imei))]);
    let enc = s.encoded_params(&body)?;
    let host = if s.domains.friend.is_empty() {
        "https://tt-friend-wpa.chat.zalo.me"
    } else {
        &s.domains.friend
    };
    let url = format!(
        "{}/api/friend/reject?{}",
        host.trim_end_matches('/'),
        s.common_params()
    );
    let resp = s.post_form(&url, format!("params={enc}")).await?;
    Ok(s.maybe_decrypt(resp, s.session_key_bytes()).await)
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PhoneLookupResult {
    pub user_id: String,
    pub display_name: String,
    pub zalo_name: String,
    pub phone: String,
    pub avatar: String,
    pub status: String,
    #[serde(default)]
    pub friendship: FriendshipStatus,
}

/// Normalize a Vietnamese or international phone string for Zalo's `/api/friend/profile/get`.
///
/// Converts leading `0` to `84` (e.g. `0912345678` -> `84912345678`), strips `+`, spaces, dots, and dashes.
pub fn normalize_phone_for_lookup(raw: &str) -> String {
    let digits: String = raw.chars().filter(|c| c.is_ascii_digit()).collect();
    if let Some(rest) = digits.strip_prefix('0') {
        format!("84{rest}")
    } else {
        digits
    }
}

/// Look up a Zalo user profile by phone number (`getUserByPhone` in `compact-app-pc.js:14306443`, cmd `11420`).
///
/// Endpoint: `GET {domains.friend}/api/friend/profile/get?{common}&params={encodeAES({ phone, avatar_size: 240, language: "vi", imei, reqSrc: 40 })}`
pub async fn find_user_by_phone(s: &Session, raw_phone: &str) -> Result<PhoneLookupResult> {
    if s.login.is_none() {
        anyhow::bail!("chưa đăng nhập");
    }
    let normalized = normalize_phone_for_lookup(raw_phone);
    if normalized.len() < 8 {
        anyhow::bail!("Số điện thoại không hợp lệ (cần ít nhất 9 chữ số)");
    }
    let body = obj(&[
        ("phone", json!(normalized)),
        ("avatar_size", json!(240)),
        ("language", json!("vi")),
        ("imei", json!(s.imei)),
        ("reqSrc", json!(40)),
    ]);
    let enc = s.encoded_params(&body)?;
    let host = if s.domains.friend.is_empty() {
        "https://tt-friend-wpa.chat.zalo.me"
    } else {
        s.domains.friend.trim_end_matches('/')
    };
    let url = format!(
        "{host}/api/friend/profile/get?{}&params={enc}",
        s.common_params()
    );
    let resp = s.get(&url).await?;
    let dec = s.maybe_decrypt(resp, s.session_key_bytes()).await;
    if let Some(code) = dec.get("error_code").and_then(Value::as_i64) {
        if code != 0 {
            if code == 216 {
                anyhow::bail!(
                    "Số điện thoại chưa đăng ký Zalo hoặc người dùng đã chặn tìm kiếm qua SĐT (mã 216)"
                );
            }
            let err_msg = dec
                .get("error_message")
                .and_then(Value::as_str)
                .unwrap_or("Không tìm thấy tài khoản");
            anyhow::bail!("{err_msg} (mã {code})");
        }
    }

    let data = dec.get("data").unwrap_or(&dec);
    let Some(uid) = pick_str(data, &["uid", "userId", "id", "globalId"]) else {
        anyhow::bail!("Không tìm thấy tài khoản Zalo ứng với số điện thoại này");
    };
    let display_name = pick_str(
        data,
        &["display_name", "displayName", "zalo_name", "zaloName"],
    )
    .unwrap_or_else(|| raw_phone.trim().to_string());
    let zalo_name = pick_str(
        data,
        &["zalo_name", "zaloName", "display_name", "displayName"],
    )
    .unwrap_or_else(|| display_name.clone());
    let avatar = pick_str(data, &["avatar", "avt"]).unwrap_or_default();
    let status = pick_str(data, &["status"]).unwrap_or_default();

    let friendship = check_friendship_status(s, &uid)
        .await
        .unwrap_or(FriendshipStatus::Unknown);

    Ok(PhoneLookupResult {
        user_id: uid,
        display_name,
        zalo_name,
        phone: raw_phone.trim().to_string(),
        avatar,
        status,
        friendship,
    })
}
/// Board topic type for a pinned chat message (`MSG_TOPIC` = 2 in Zalo's topic enum:
/// `TEXT_TOPIC=0`, `LINK_TOPIC=1`, `MSG_TOPIC=2`, `POLL_TOPIC=3`, `REMINDER_TOPIC=4`).
pub const MSG_TOPIC: i32 = 2;

/// Group id as the board API expects it: no `g`/`group_` prefix.
fn board_grid(thread_id: &str) -> String {
    thread_id
        .trim_start_matches("group_")
        .trim_start_matches('g')
        .trim_start_matches('G')
        .trim()
        .to_string()
}

/// Zalo answers HTTP 200 with `error_code` in the body; non-zero means the board call was refused
/// (permissions, stale grid, unsupported thread) and nothing changed server-side.
fn board_reject_nonzero(resp: Value) -> Result<Value> {
    let code = resp.get("error_code").and_then(Value::as_i64).unwrap_or(0);
    if code != 0 {
        let msg = resp
            .get("error_message")
            .and_then(Value::as_str)
            .filter(|m| !m.is_empty())
            .unwrap_or("board request rejected");
        anyhow::bail!("mã lỗi {code}: {msg}");
    }
    Ok(resp)
}

/// Topic `params` object for a pinned message, using the field names Zalo reads back
/// (`params.global_msg_id`, `params.client_msg_id`, `params.senderUid`, `params.msg_type`,
/// `params.title` — `getTopicDataFromMessage` in `compact-app-pc.js`).
fn message_topic_params(msg: &ChatMessage, thread_id: &str) -> Value {
    let title = if msg.sticker_id.is_some() {
        "[Nhãn dán]".to_string()
    } else {
        let trimmed = msg.text.trim();
        let mut s: String = trimmed.chars().take(120).collect();
        if trimmed.chars().count() > 120 {
            s.push('…');
        }
        if s.is_empty() {
            "[Tin nhắn]".to_string()
        } else {
            s
        }
    };
    let sender = if msg.is_outgoing {
        "0".to_string()
    } else {
        sidebar_sender_id(msg).unwrap_or_else(|| thread_id.to_string())
    };
    json!({
        "global_msg_id": msg.id,
        "client_msg_id": msg.client_message_id,
        "senderUid": sender,
        "title": title,
        "createTime": msg.ts,
    })
}

fn board_topic_body(grid: &str, thread_id: &str, msg: &ChatMessage, imei: &str) -> Value {
    json!({
        "grid": grid,
        "type": MSG_TOPIC,
        "color": -16777216,
        "emoji": "",
        "startTime": -1,
        "duration": -1,
        "params": message_topic_params(msg, thread_id).to_string(),
        "needPin": true,
        "pinAct": 1,
        "repeat": 0,
        "src": 0,
        "imei": imei,
    })
}

/// Pin one chat message.
///
/// Zalo stores a pinned message as a *board topic* of type `MSG_TOPIC` and pins it in the same
/// call. Groups: POST `groupboard-wpa` `/api/board/topic/createv2` (cmd `12007`). 1-1 chats:
/// GET `friendboard-wpa` `/api/friendboard/create` (cmd `12067`, confirmed by a live web capture).
pub async fn pin_message(
    s: &Session,
    thread_id: &str,
    is_group: bool,
    msg: &ChatMessage,
) -> Result<Value> {
    if s.login.is_none() {
        anyhow::bail!("chưa đăng nhập");
    }
    let grid = board_grid(thread_id);
    let body = if is_group {
        board_topic_body(&grid, thread_id, msg, &s.imei)
    } else {
        // 1-1 uses the friend board: same topic fields, keyed by the peer uid.
        let mut v = board_topic_body(&grid, thread_id, msg, &s.imei);
        if let Some(obj) = v.as_object_mut() {
            obj.remove("grid");
            obj.insert("uid".into(), json!(grid));
        }
        v
    };
    let params = s.encoded_params(&body)?;
    if is_group {
        let url = format!(
            "{}/api/board/topic/createv2?{}",
            crate::endpoints::GROUP_BOARD,
            s.common_params()
        );
        let resp = s.post_form(&url, format!("params={params}")).await?;
        board_reject_nonzero(s.maybe_decrypt(resp, s.session_key_bytes()).await)
    } else {
        let url = format!(
            "{}/api/friendboard/create?{}&params={params}",
            crate::endpoints::FRIEND_BOARD,
            s.common_params()
        );
        let resp = s.get(&url).await?;
        board_reject_nonzero(s.maybe_decrypt(resp, s.session_key_bytes()).await)
    }
}

/// Unpin a chat message previously pinned with [`pin_message`].
pub async fn unpin_message(
    s: &Session,
    thread_id: &str,
    is_group: bool,
    topic_id: &str,
) -> Result<Value> {
    if s.login.is_none() {
        anyhow::bail!("chưa đăng nhập");
    }
    if topic_id.trim().is_empty() {
        // Without the board topic id the server has nothing to address; say so instead of letting
        // it answer with the opaque 114 "invalid parameter".
        anyhow::bail!("chưa có mã bảng tin (topic id) của tin nhắn này");
    }
    let grid = board_grid(thread_id);
    // Zalo unpins topics through the multi_unpin GET endpoint. Its `topics` array carries
    // `{topicId, topicType}` objects (module `L()` in `compact-app-pc.js`), not bare id strings —
    // sending strings is rejected with error 114 "invalid parameter".
    let topic_entry = json!({ "topicId": topic_id, "topicType": MSG_TOPIC });
    let body = if is_group {
        json!({ "grid": grid, "topics": [topic_entry], "boardVersion": 0 })
    } else {
        json!({ "uid": grid, "topics": [topic_entry], "boardVersion": 0 })
    };
    let params = s.encoded_params(&body)?;
    let url = if is_group {
        format!(
            "{}/api/board/multi_unpin?{}&params={params}",
            crate::endpoints::GROUP_BOARD,
            s.common_params()
        )
    } else {
        format!(
            "{}/api/friendboard/multi_unpin?{}&params={params}",
            crate::endpoints::FRIEND_BOARD,
            s.common_params()
        )
    };
    let resp = s.get(&url).await?;
    board_reject_nonzero(s.maybe_decrypt(resp, s.session_key_bytes()).await)
}

/// One pinned board item as the server reports it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BoardTopic {
    pub id: String,
    /// Message ids from `topic.params` (`global_msg_id` / `client_msg_id`), used to match the
    /// pinned message back to a loaded bubble.
    pub msg_ids: Vec<String>,
}

fn collect_msg_ids(params: &Value, out: &mut Vec<String>) {
    for key in ["global_msg_id", "client_msg_id", "msgId", "cliMsgId"] {
        if let Some(v) = params.get(key) {
            let s = match v {
                Value::String(s) => s.clone(),
                Value::Number(n) => n.to_string(),
                _ => String::new(),
            };
            if !s.is_empty() && s != "0" && !out.contains(&s) {
                out.push(s);
            }
        }
    }
}

/// Parse a board list response into topics. `data` is normally a JSON object, but some endpoints
/// hand it back as a JSON string.
pub fn parse_board_topics(resp: &Value) -> Vec<BoardTopic> {
    let mut data = resp.get("data").cloned().unwrap_or(Value::Null);
    if let Value::String(txt) = &data {
        data = serde_json::from_str::<Value>(txt).unwrap_or(Value::Null);
    }
    let mut raw: Vec<Value> = Vec::new();
    match &data {
        Value::Array(items) => raw.extend(items.iter().cloned()),
        Value::Object(map) => {
            for key in ["topics", "topic", "items", "data", "boards"] {
                match map.get(key) {
                    Some(Value::Array(items)) => raw.extend(items.iter().cloned()),
                    Some(v @ Value::Object(_)) => raw.push(v.clone()),
                    _ => {}
                }
            }
        }
        _ => {}
    }

    let mut out: Vec<BoardTopic> = Vec::new();
    for item in raw {
        let id = ["id", "topicId"]
            .iter()
            .find_map(|k| item.get(*k))
            .map(|v| match v {
                Value::String(s) => s.clone(),
                Value::Number(n) => n.to_string(),
                _ => String::new(),
            })
            .unwrap_or_default();
        if id.is_empty() {
            continue;
        }
        let mut msg_ids: Vec<String> = Vec::new();
        if let Some(params) = item.get("params") {
            let params = match params {
                Value::String(txt) => {
                    serde_json::from_str::<Value>(txt).unwrap_or(Value::Null)
                }
                other => other.clone(),
            };
            collect_msg_ids(&params, &mut msg_ids);
        }
        out.push(BoardTopic { id, msg_ids });
    }
    out
}

fn board_trace(label: &str, detail: &str) {
    // Opt-in diagnostics: `CTDOZLITE_BOARD_TRACE=1` appends the board calls to `board_trace.log`
    // (gitignored). Off by default: payloads carry conversation content.
    if std::env::var("CTDOZLITE_BOARD_TRACE").ok().as_deref() != Some("1") {
        return;
    }
    use std::io::Write;
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open("board_trace.log")
    {
        let _ = writeln!(f, "{label} | {detail}");
    }
}

async fn board_get_json(s: &Session, url: &str) -> Result<Value> {
    let resp = s.get(url).await?;
    let dec = s.maybe_decrypt(resp, s.session_key_bytes()).await;
    board_reject_nonzero(dec)
}

#[allow(dead_code)]
async fn board_post_json(s: &Session, url: &str, body: &Value) -> Result<Value> {
    let params = s.encoded_params(body)?;
    let resp = s.post_form(url, format!("params={params}")).await?;
    let dec = s.maybe_decrypt(resp, s.session_key_bytes()).await;
    board_reject_nonzero(dec)
}

/// Fetch the pinned board items of a conversation, so pins made elsewhere show up here too.
///
/// Zalo has two generations of this API and which one answers depends on the account/session, so
/// both are tried in order and the first accepted reply wins:
/// - 1-1: `GET /api/friendboard/list` (cmd `12065`), then `POST /api/board/oneone/list` (cmd `12432`).
/// - group: `GET /api/board/pin/list` (cmd `12001`), then `GET /api/board/list` (cmd `12140`).
pub async fn list_board_topics(
    s: &Session,
    thread_id: &str,
    is_group: bool,
) -> Result<Vec<BoardTopic>> {
    if s.login.is_none() {
        anyhow::bail!("chưa đăng nhập");
    }
    let grid = board_grid(thread_id);
    let mut errors: Vec<String> = Vec::new();

    if is_group {
        let body = json!({ "groupId": grid, "boardVersion": 0, "imei": s.imei });
        if let Ok(params) = s.encoded_params(&body) {
            let url = format!(
                "{}/api/board/pin/list?{}&params={params}",
                crate::endpoints::GROUP_BOARD,
                s.common_params()
            );
            match board_get_json(s, &url).await {
                Ok(v) => {
                    let topics = parse_board_topics(&v);
                    board_trace("pin/list", &format!("ok topics={}", topics.len()));
                    return Ok(topics);
                }
                Err(e) => {
                    board_trace("pin/list", &format!("err {e}"));
                    errors.push(format!("pin/list: {e}"));
                }
            }
        }
        let body = json!({
            "group_id": grid, "board_type": 0, "page": 0, "count": 50,
            "last_id": 0, "last_type": 0, "imei": s.imei,
        });
        if let Ok(params) = s.encoded_params(&body) {
            let url = format!(
                "{}/api/board/list?{}&params={params}",
                crate::endpoints::GROUP_BOARD,
                s.common_params()
            );
            match board_get_json(s, &url).await {
                Ok(v) => {
                    let topics = parse_board_topics(&v);
                    board_trace("board/list", &format!("ok topics={}", topics.len()));
                    return Ok(topics);
                }
                Err(e) => {
                    board_trace("board/list", &format!("err {e}"));
                    errors.push(format!("board/list: {e}"));
                }
            }
        }
    } else {
        let body = json!({
            "uid": grid, "page": 0, "count": 50, "boardVersion": 0, "imei": s.imei,
        });
        if let Ok(params) = s.encoded_params(&body) {
            let url = format!(
                "{}/api/friendboard/list?{}&params={params}",
                crate::endpoints::FRIEND_BOARD,
                s.common_params()
            );
            match board_get_json(s, &url).await {
                Ok(v) => {
                    let topics = parse_board_topics(&v);
                    board_trace("friendboard/list", &format!("ok topics={}", topics.len()));
                    return Ok(topics);
                }
                Err(e) => {
                    board_trace("friendboard/list", &format!("err {e}"));
                    errors.push(format!("friendboard/list: {e}"));
                }
            }
        }
        // `getListBoardOneOne`: GET, and everything except `objectData` sits inside that JSON
        // string — sending it as a POST body is what returned a generic -1.
        let object = json!({
            "uid": grid, "board_type": 0, "page": 0, "count": 50,
            "last_id": 0, "last_type": 0,
        });
        let body = json!({ "objectData": object.to_string() });
        if let Ok(params) = s.encoded_params(&body) {
            let url = format!(
                "{}/api/board/oneone/list?{}&params={params}",
                crate::endpoints::GROUP_BOARD,
                s.common_params()
            );
            match board_get_json(s, &url).await {
                Ok(v) => {
                    let topics = parse_board_topics(&v);
                    board_trace("oneone/list", &format!("ok topics={}", topics.len()));
                    return Ok(topics);
                }
                Err(e) => {
                    board_trace("oneone/list", &format!("err {e}"));
                    errors.push(format!("oneone/list: {e}"));
                }
            }
        }
    }

    anyhow::bail!("{}", errors.join(" · "))
}



/// Pin or unpin a conversation (`updatePinnedConversationsV2` @ `compact-app-pc.js:14354215`, cmd `12122`).
pub async fn update_pinned_conversation(
    s: &Session,
    thread_id: &str,
    is_group: bool,
    pin: bool,
) -> Result<Value> {
    if s.login.is_none() {
        anyhow::bail!("chưa đăng nhập");
    }
    let clean = thread_id
        .trim_start_matches("group_")
        .trim_start_matches('g')
        .trim_start_matches('G')
        .trim_start_matches('u')
        .trim();
    let prefixed = if is_group {
        format!("g{clean}")
    } else {
        format!("u{clean}")
    };
    let body = json!({
        "actionType": if pin { 1 } else { 2 },
        "conversations": [prefixed],
        "tab": 0
    });
    let params = s.encoded_params(&body)?;
    let url = format!(
        "{}/api/pinconvers/updatev2?{}",
        s.domains.conversation,
        s.common_params()
    );
    let resp = s.post_form(&url, format!("params={params}")).await?;
    Ok(s.maybe_decrypt(resp, s.session_key_bytes()).await)
}

/// Mark a conversation as unread or read (`addMarkAsUnread` / `removeMarkAsUnread` @ `compact-app-pc.js:14299958`, cmd `12013` / `12014`).
pub async fn set_unread_mark(
    s: &Session,
    thread_id: &str,
    is_group: bool,
    unread: bool,
) -> Result<Value> {
    if s.login.is_none() {
        anyhow::bail!("chưa đăng nhập");
    }
    let clean = thread_id
        .trim_start_matches("group_")
        .trim_start_matches('g')
        .trim_start_matches('G')
        .trim();
    let (convs_group, convs_user) = if is_group {
        (vec![clean], Vec::<&str>::new())
    } else {
        (Vec::<&str>::new(), vec![clean])
    };
    let (path, inner) = if unread {
        (
            "/api/conv/addUnreadMark",
            json!({
                "convsGroup": convs_group,
                "convsUser": convs_user,
                "imei": s.imei
            }),
        )
    } else {
        (
            "/api/conv/removeUnreadMark",
            json!({
                "convsGroup": convs_group,
                "convsUser": convs_user,
                "convsGroupData": [],
                "convsUserData": []
            }),
        )
    };
    let outer = json!({ "param": inner.to_string() });
    let params = s.encoded_params(&outer)?;
    let url = format!("{}{path}?{}", s.domains.conversation, s.common_params());
    let resp = s.post_form(&url, format!("params={params}")).await?;
    Ok(s.maybe_decrypt(resp, s.session_key_bytes()).await)
}

/// Mute or unmute notifications for a conversation (`setMuteConversation` @ `compact-app-pc.js:14304409`, cmd `11288`).
/// `duration_sec`: `3600` (1h), `14400` (4h), seconds until 8:00 AM, or `-1` (until reopened).
/// `mute`: `true` -> `action = 1`, `false` -> `action = 2`.
pub async fn set_mute_conversation(
    s: &Session,
    thread_id: &str,
    is_group: bool,
    duration_sec: i64,
    mute: bool,
) -> Result<Value> {
    if s.login.is_none() {
        anyhow::bail!("chưa đăng nhập");
    }
    let clean = thread_id
        .trim_start_matches("group_")
        .trim_start_matches('g')
        .trim_start_matches('G')
        .trim();
    let body = json!({
        "toid": clean,
        "duration": duration_sec,
        "action": if mute { 1 } else { 2 },
        "startTime": crate::login::unix_now(),
        "muteType": if is_group { 2 } else { 1 },
        "imei": s.imei
    });
    let params = s.encoded_params(&body)?;
    let path = format!("/api/social/profile/setmute?{}", s.common_params());
    let resp = s
        .post_endpoint_form("profile", &path, format!("params={params}"))
        .await?;
    Ok(s.maybe_decrypt(resp, s.session_key_bytes()).await)
}

/// Fetch the full mute snapshot (`chatEntries` + `groupChatEntries`) from Official Zalo PC
/// (`getMuteConversations` @ `default-login-main-startup-shared-worker.js`, cmd `11280`, #67).
pub async fn get_mute_conversations(s: &Session) -> Result<Value> {
    if s.login.is_none() {
        anyhow::bail!("chưa đăng nhập");
    }
    let body = json!({
        "imei": s.imei
    });
    let params = s.encoded_params(&body)?;
    let path = format!(
        "/api/social/profile/getmute?{}&params={params}",
        s.common_params()
    );
    let resp = s.get_endpoint("profile", &path).await?;
    Ok(s.maybe_decrypt(resp, s.session_key_bytes()).await)
}

/// Real-time mute/unmute control notification (`act_type: "mute"`, `act: "set" | "unset"`, #67).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MuteControlEvent {
    /// Canonical thread ID (`g<id>` for groups, `<id>` for 1-1 peers).
    pub thread_id: String,
    pub is_group: bool,
    pub muted: bool,
    /// Expiry Unix timestamp (`Some(ts)`), or `None` when indefinite (`duration == -1`) or unmuted.
    pub muted_until: Option<i64>,
    /// Value for `AppSettings::muted_threads` (`Some(-1)` for indefinite, `Some(until)` for timed, `None` when unmuted).
    pub settings_until: Option<i64>,
}

/// Parse `act_type: "mute"` items inside WebSocket `cmd 601`/`602`/`603` or `lp_control`
/// payloads (`_processMuteConversations` in Zalo PC `app.asar`, #67).
pub fn extract_mute_controls(data_node: &Value, now_unix: i64) -> Vec<MuteControlEvent> {
    let mut out = Vec::new();
    let mut candidates: Vec<&Value> = Vec::new();
    if let Some(arr) = data_node.get("controls").and_then(Value::as_array) {
        candidates.extend(arr.iter());
    } else if let Some(arr) = data_node.as_array() {
        for item in arr {
            if let Some(sub) = item.get("controls").and_then(Value::as_array) {
                candidates.extend(sub.iter());
            } else {
                candidates.push(item);
            }
        }
    } else {
        candidates.push(data_node);
    }

    for item in candidates {
        let c = item.get("content").unwrap_or(item);
        let act_type = c
            .get("act_type")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if act_type != "mute" {
            continue;
        }
        let act = c.get("act").and_then(Value::as_str).unwrap_or_default();
        let raw_data = c.get("data").unwrap_or(c);
        let parsed_data: Value;
        let d = if let Some(s) = raw_data.as_str() {
            if let Ok(v) = serde_json::from_str::<Value>(s) {
                parsed_data = v;
                &parsed_data
            } else {
                continue;
            }
        } else {
            raw_data
        };

        let raw_id = d
            .get("toId")
            .or_else(|| d.get("toid"))
            .or_else(|| d.get("id"))
            .and_then(|v| {
                v.as_str()
                    .map(str::to_owned)
                    .or_else(|| v.as_u64().map(|n| n.to_string()))
                    .or_else(|| v.as_i64().map(|n| n.to_string()))
            })
            .unwrap_or_default();
        let clean = raw_id
            .trim()
            .trim_start_matches("group_")
            .trim_start_matches('g')
            .trim_start_matches('G');
        if clean.is_empty() || clean == "0" {
            continue;
        }

        let mute_type = d
            .get("muteType")
            .and_then(|v| v.as_i64().or_else(|| v.as_str()?.parse().ok()))
            .unwrap_or(1);
        let is_group = mute_type == 2 || raw_id.starts_with('g') || raw_id.starts_with("group_");
        let thread_id = if is_group {
            format!("g{clean}")
        } else {
            clean.to_string()
        };

        let duration = d
            .get("duration")
            .and_then(|v| v.as_i64().or_else(|| v.as_str()?.parse().ok()))
            .unwrap_or(-1);
        let mute_mode = d
            .get("muteMode")
            .and_then(|v| v.as_i64().or_else(|| v.as_str()?.parse().ok()))
            .unwrap_or(1);
        let start_time = d
            .get("startTime")
            .or_else(|| d.get("systemTime"))
            .and_then(|v| v.as_i64().or_else(|| v.as_str()?.parse().ok()))
            .unwrap_or(now_unix);

        let is_set = match act {
            "set" => mute_mode == 1 && duration != 0,
            "unset" => false,
            _ => {
                let action_code = d.get("action").and_then(Value::as_i64).unwrap_or(1);
                action_code == 1 && mute_mode == 1 && duration != 0
            }
        };

        if !is_set {
            out.push(MuteControlEvent {
                thread_id,
                is_group,
                muted: false,
                muted_until: None,
                settings_until: None,
            });
            continue;
        }

        if duration < 0 {
            out.push(MuteControlEvent {
                thread_id,
                is_group,
                muted: true,
                muted_until: None,
                settings_until: Some(-1),
            });
        } else {
            let until = start_time.saturating_add(duration);
            if until > now_unix {
                out.push(MuteControlEvent {
                    thread_id,
                    is_group,
                    muted: true,
                    muted_until: Some(until),
                    settings_until: Some(until),
                });
            } else {
                out.push(MuteControlEvent {
                    thread_id,
                    is_group,
                    muted: false,
                    muted_until: None,
                    settings_until: None,
                });
            }
        }
    }
    out
}

/// Real-time typing notification event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypingEvent {
    pub thread_id: String,
    pub uid: String,
    pub is_typing: bool,
}

/// Real-time message read receipt / seen event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeenEvent {
    pub thread_id: String,
    pub uid: String,
    pub seen_ts: i64,
}

/// Parse `act_type: "typing"` and `act_type: "seen"` controls from WebSocket `ev.payload` or `lp_control`.
pub fn extract_typing_and_seen_controls(data_node: &Value) -> (Vec<TypingEvent>, Vec<SeenEvent>) {
    let mut typing_events = Vec::new();
    let mut seen_events = Vec::new();
    let mut candidates: Vec<&Value> = Vec::new();

    if let Some(arr) = data_node.get("controls").and_then(Value::as_array) {
        candidates.extend(arr.iter());
    } else if let Some(arr) = data_node.as_array() {
        for item in arr {
            if let Some(sub) = item.get("controls").and_then(Value::as_array) {
                candidates.extend(sub.iter());
            } else {
                candidates.push(item);
            }
        }
    } else {
        candidates.push(data_node);
    }

    for item in candidates {
        let c = item.get("content").unwrap_or(item);
        let act_type = c
            .get("act_type")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let act = c.get("act").and_then(Value::as_str).unwrap_or_default();

        let raw_data = c.get("data").unwrap_or(c);
        let parsed_data: Value;
        let d = if let Some(s) = raw_data.as_str() {
            if let Ok(v) = serde_json::from_str::<Value>(s) {
                parsed_data = v;
                &parsed_data
            } else {
                continue;
            }
        } else {
            raw_data
        };

        if act_type == "typing" || act == "typing" {
            let from_uid = d
                .get("fromUid")
                .or_else(|| d.get("uid"))
                .or_else(|| d.get("from"))
                .and_then(|v| {
                    v.as_str()
                        .map(str::to_owned)
                        .or_else(|| v.as_i64().map(|n| n.to_string()))
                })
                .unwrap_or_default();

            let grid = d.get("grid").and_then(|v| {
                v.as_str()
                    .map(str::to_owned)
                    .or_else(|| v.as_i64().map(|n| n.to_string()))
            });

            let to_id = d.get("toid").or_else(|| d.get("toId")).and_then(|v| {
                v.as_str()
                    .map(str::to_owned)
                    .or_else(|| v.as_i64().map(|n| n.to_string()))
            });

            let thread_id = if let Some(g) = grid {
                if g.starts_with('g') || g.starts_with('G') {
                    g
                } else {
                    format!("g{g}")
                }
            } else if !from_uid.is_empty() {
                from_uid.clone()
            } else if let Some(t) = to_id {
                t
            } else {
                continue;
            };

            let is_typing = d
                .get("status")
                .and_then(Value::as_i64)
                .map(|s| s != 0)
                .unwrap_or(true);

            typing_events.push(TypingEvent {
                thread_id,
                uid: from_uid,
                is_typing,
            });
        } else if act_type == "seen" || act_type == "read" || act == "seen" {
            let from_uid = d
                .get("fromUid")
                .or_else(|| d.get("uid"))
                .or_else(|| d.get("from"))
                .and_then(|v| {
                    v.as_str()
                        .map(str::to_owned)
                        .or_else(|| v.as_i64().map(|n| n.to_string()))
                })
                .unwrap_or_default();

            let grid = d.get("grid").and_then(|v| {
                v.as_str()
                    .map(str::to_owned)
                    .or_else(|| v.as_i64().map(|n| n.to_string()))
            });

            let to_id = d.get("toid").or_else(|| d.get("toId")).and_then(|v| {
                v.as_str()
                    .map(str::to_owned)
                    .or_else(|| v.as_i64().map(|n| n.to_string()))
            });

            let thread_id = if let Some(g) = grid {
                if g.starts_with('g') || g.starts_with('G') {
                    g
                } else {
                    format!("g{g}")
                }
            } else if !from_uid.is_empty() {
                from_uid.clone()
            } else if let Some(t) = to_id {
                t
            } else {
                continue;
            };

            let seen_ts = d
                .get("seenTime")
                .or_else(|| d.get("ts"))
                .and_then(|v| v.as_i64().or_else(|| v.as_str()?.parse().ok()))
                .unwrap_or(0);

            seen_events.push(SeenEvent {
                thread_id,
                uid: from_uid,
                seen_ts,
            });
        }
    }

    (typing_events, seen_events)
}

/// Send typing indicator signal to recipient or group (`/api/message/typing` or `/api/group/typing`).
pub async fn send_typing(s: &Session, to_id: &str, is_group: bool) -> Result<Value> {
    if s.login.is_none() {
        anyhow::bail!("chưa đăng nhập");
    }
    let host = if s.domains.conversation.is_empty() {
        "https://tt-conversation-wpa.zaloapp.com"
    } else {
        s.domains.conversation.trim_end_matches('/')
    };
    let clean_target = if is_group {
        to_id
            .trim_start_matches("group_")
            .trim_start_matches('g')
            .trim_start_matches('G')
    } else {
        to_id
    };
    let ep = if is_group {
        "/api/group/typing"
    } else {
        "/api/message/typing"
    };
    let mut m = serde_json::Map::new();
    m.insert("status".into(), json!(1));
    m.insert("imei".into(), json!(s.imei));
    if is_group {
        m.insert("grid".into(), json!(clean_target));
    } else {
        m.insert("toid".into(), json!(clean_target));
    }
    let params = s.encoded_params(&Value::Object(m))?;
    let url = format!("{host}{ep}?{}", s.common_params());
    let r = s.post_form(&url, format!("params={params}")).await?;
    Ok(s.maybe_decrypt(r, s.session_key_bytes()).await)
}

/// Send a real-time "is typing" indicator to a 1-1 contact (`cmd 11144`, `/api/message/typing`)
/// or a group (`cmd 11196`, `/api/group/typing`), unless blocked by `block_typing`.
pub async fn send_typing_indicator(s: &Session, thread_id: &str, is_group: bool) -> Result<Value> {
    if s.login.is_none() {
        anyhow::bail!("chưa đăng nhập");
    }
    if crate::session::privacy_blocker_state().0 {
        return Ok(json!({"error_code": 0, "data": {}}));
    }
    let clean = thread_id
        .trim_start_matches("group_")
        .trim_start_matches('g')
        .trim_start_matches('G')
        .trim_start_matches('u')
        .trim();
    if clean.is_empty() {
        return Ok(json!({"error_code": 0, "data": {}}));
    }
    let (endpoint, path_and_q, body) = if is_group {
        (
            "group",
            format!("/api/group/typing?{}", s.common_params()),
            json!({
                "grid": clean,
                "imei": s.imei,
            }),
        )
    } else {
        (
            "chat",
            format!("/api/message/typing?{}", s.common_params()),
            json!({
                "toid": clean,
                "destType": 0,
                "imei": s.imei,
            }),
        )
    };
    let params = s.encoded_params(&body)?;
    let resp = s
        .post_endpoint_form(endpoint, &path_and_q, format!("params={params}"))
        .await?;
    Ok(s.maybe_decrypt(resp, s.session_key_bytes()).await)
}

/// Send a "seen" receipt (`cmd 12118` `/api/message/seenv2` or `cmd 12124` `/api/group/seenv2`),
/// unless blocked by `block_seen`.
pub async fn send_seen_v2(
    s: &Session,
    thread_id: &str,
    is_group: bool,
    last_msg_id: &str,
) -> Result<Value> {
    if s.login.is_none() {
        anyhow::bail!("chưa đăng nhập");
    }
    if crate::session::privacy_blocker_state().2 {
        return Ok(json!({"error_code": 0, "data": {}}));
    }
    let clean = thread_id
        .trim_start_matches("group_")
        .trim_start_matches('g')
        .trim_start_matches('G')
        .trim_start_matches('u')
        .trim();
    if clean.is_empty() || last_msg_id.trim().is_empty() {
        return Ok(json!({"error_code": 0, "data": {}}));
    }
    let (endpoint, path_and_q, body) = if is_group {
        (
            "group",
            format!("/api/group/seenv2?{}", s.common_params()),
            json!({
                "msgInfos": json!({
                    "data": [{ "gmi": last_msg_id, "cmi": last_msg_id }],
                    "grid": clean,
                }).to_string(),
                "imei": s.imei,
            }),
        )
    } else {
        (
            "chat",
            format!("/api/message/seenv2?{}", s.common_params()),
            json!({
                "msgInfos": json!({
                    "data": [{ "gmi": last_msg_id, "cmi": last_msg_id }],
                    "senderId": clean,
                }).to_string(),
                "imei": s.imei,
            }),
        )
    };
    let params = s.encoded_params(&body)?;
    let resp = s
        .post_endpoint_form(endpoint, &path_and_q, format!("params={params}"))
        .await?;
    Ok(s.maybe_decrypt(resp, s.session_key_bytes()).await)
}

/// Request mobile message sync (`pullMobileMsg` @ `app.asar`, cmd `12000`, `file` endpoint, #68, #73).
/// Sends a wake-up/sync signal with a 2048-bit RSA SPKI DER public key to the user's phone Zalo client.
pub async fn request_phone_sync(s: &Session, public_key: &str) -> Result<Value> {
    let temp_key = crate::history_sync::latest_mobile_temp_key();
    request_phone_sync_with_temp_key(s, public_key, &temp_key).await
}

/// Request mobile message sync (`pullMobileMsg`, cmd `12000`) with an explicit `temp_key`
/// (received from `transfer_after_login` control event or empty for manual request).
pub async fn request_phone_sync_with_temp_key(
    s: &Session,
    public_key: &str,
    temp_key: &str,
) -> Result<Value> {
    if s.login.is_none() {
        anyhow::bail!("chưa đăng nhập");
    }
    let body = json!({
        "pc_name": s.computer_name,
        "public_key": public_key,
        "from_seq_id": 0,
        "is_retry": 0,
        "min_seq_id": 0,
        "temp_key": temp_key,
        "imei": s.imei
    });
    let params = s.encoded_params(&body)?;
    let path = format!(
        "/api/message/pull_mobile_msg?{}&params={params}",
        s.common_params()
    );
    if let Ok(resp) = s.get_endpoint("file", &path).await {
        let dec = s.maybe_decrypt(resp, s.session_key_bytes()).await;
        if dec.get("error_code").and_then(Value::as_i64) == Some(0) {
            return Ok(dec);
        }
    }
    let post_url = format!(
        "{}/api/message/pull_mobile_msg?{}",
        s.domains.file,
        s.common_params()
    );
    let resp = s.post_form(&post_url, format!("params={params}")).await?;
    Ok(s.maybe_decrypt(resp, s.session_key_bytes()).await)
}

/// Request transfer-sync v2 từ điện thoại. Lệnh thật đi qua WebSocket
/// `cmd 592` (wake-up) + `cmd 590` (SYNC_MESSAGE.REQUEST) — KHÔNG có HTTP REST
/// `/api/transfer-sync-v2/request-sync` (404 mọi host, #87); cmd 12888 trong
/// bundle Zalo chỉ là command-id, dispatch qua WS runner.
pub async fn request_phone_sync_v2(s: &Session, _public_key: &str) -> Result<Value> {
    if s.login.is_none() {
        anyhow::bail!("chưa đăng nhập");
    }
    let sync_id = crate::param_cipher::random_hex(32);
    let temp_key = crate::history_sync::latest_mobile_temp_key();
    // Sync V2 BẮT BUỘC có keypair thật từ WASM libzproto (host bằng wasmtime,
    // không cần Node.js). Fail rõ ở đây thay vì gửi request rác.
    let keys = crate::history_sync::generate_sync_v2_keys().map_err(|e| {
        anyhow::anyhow!(
            "Không sinh được khóa Sync V2: {e}. \
             Cài Zalo PC hoặc đặt CTDOZLITE_ZPROTO_WASM trỏ tới file WASM"
        )
    })?;
    let (ik_pub, ek_pub) = (keys.ik_pub, keys.ek_pub);
    // 1. Wake up mobile master device over WebSocket (`cmd 592`)
    let _ = crate::socket::send_mobile_wakeup(&sync_id);

    // 2. Build SyncFull v2 request payload matching `shared-worker.js:282247` & `314736`
    let sync_data = json!({
        "syncId": sync_id,
        "syncType": 0, // No.d.SyncFull = 0
        "ek": ek_pub,
        "ik": ik_pub,
        "toDevice": 0, // No.c.Mobile = 0
        "tempKey": temp_key,
        "deviceName": s.computer_name,
        "req": {
            "type": "message",
            "priority": 1,
            "queries": [
                {
                    "partition": "",
                    "from": 0,
                    "to": 9_007_199_254_740_991_i64,
                    "limit": 2000
                }
            ],
            "batchSize": 2000
        },
        "ver": 1,
        "ussidx": 0
    });
    let sent_ws = crate::socket::send_sync_message_ws_request(&sync_data);
    Ok(json!({
        "error_code": 0,
        "data": {
            "syncId": sync_id,
            "ws_sent": sent_ws,
        }
    }))
}

/// Cancel an active mobile message sync request (`cancelTransferMessage` @ `app.asar:14372356`, cmd `12700`, #68).
pub async fn cancel_phone_sync(s: &Session, public_key: &str) -> Result<Value> {
    if s.login.is_none() {
        anyhow::bail!("chưa đăng nhập");
    }
    let body = json!({
        "pc_name": s.computer_name,
        "public_key": public_key,
        "imei": s.imei
    });
    let params = s.encoded_params(&body)?;
    let path = format!(
        "/api/message/cancel_pull_mobile_msg?{}&params={params}",
        s.common_params()
    );
    let resp = s.get_endpoint("file", &path).await?;
    Ok(s.maybe_decrypt(resp, s.session_key_bytes()).await)
}

/// Notify Zalo server and mobile device that sync snapshot download is completed
/// (`cleanSync` @ `app.asar`, cmd `12003`), dismissing the mobile sync spinner.
pub async fn clean_sync(s: &Session, public_key: &str) -> Result<Value> {
    if s.login.is_none() {
        anyhow::bail!("chưa đăng nhập");
    }
    let body = json!({
        "public_key": public_key,
        "imei": s.imei
    });
    let params = s.encoded_params(&body)?;
    let path = format!(
        "/api/message/delete_snapshot_mobile_msg?{}&params={params}",
        s.common_params()
    );
    let resp = s.get_endpoint("file", &path).await?;
    Ok(s.maybe_decrypt(resp, s.session_key_bytes()).await)
}

/// Mobile sync control event received via WebSocket (`cmd 601`/`602`/`603`) or `lp_control`
/// (`act_type: "transfer_msg" | "syncmsgmb" | "transfer_sync2"`, `main-startup.js:647494`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MobileSyncControlEvent {
    /// `act == "transfer_after_login"` — carries `temp_key` issued right after QR scan.
    TransferAfterLogin { temp_key: String },
    /// `act == "transfer_status"` (`status == 3`) — phone displayed confirmation prompt and is waiting for user tap!
    WaitingConfirmOnPhone { sync_id: String },
    /// `act == "user_confirm"` (`user_action == 1 | 3`) or `transfer_status` (`status == 4 | 5`) — user tapped "Đồng bộ" on their phone!
    UserConfirmed,
    /// `act == "user_confirm"` (`user_action == 0`) or `transfer_status` (`status == 6 | 7`) — user declined sync on their phone.
    UserRejected,
    /// `act == "user_confirm"` (`user_action == 2`) or `transfer_status` (`status == 8`) — phone is busy backing up/restoring (`BUSY_RESTORING`).
    MobileBusyRestoring,
    /// `act == "syncmsg_info"` or `act == "upload_batch"` — phone finished uploading backup package (`url`, `file_name`, `encrypted_key`).
    BackupReady {
        url: String,
        file_name: String,
        encrypted_key: String,
        checksum: String,
    },
    /// `act == "upload_batch"` (`act_type == "transfer_sync2"`) — Sync v2 chunk (used by iOS).
    SyncV2Batch {
        url: String,
        sync_id: String,
        batch_type: u8,
        is_last: bool,
    },
    /// `act == "transfer_error"` with `error_code != 0`.
    SyncError { message: String },
}

/// Extract `transfer_msg` / `syncmsgmb` / `transfer_sync2` events from a control payload.
pub fn extract_mobile_sync_controls(data_node: &Value) -> Vec<MobileSyncControlEvent> {
    let mut out = Vec::new();
    let mut candidates: Vec<&Value> = Vec::new();
    if let Some(arr) = data_node.get("controls").and_then(Value::as_array) {
        candidates.extend(arr.iter());
    } else if let Some(arr) = data_node.as_array() {
        for item in arr {
            if let Some(sub) = item.get("controls").and_then(Value::as_array) {
                candidates.extend(sub.iter());
            } else {
                candidates.push(item);
            }
        }
    } else {
        candidates.push(data_node);
    }

    for item in candidates {
        let c = item.get("content").unwrap_or(item);
        let act_type = c
            .get("act_type")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if !matches!(act_type, "transfer_msg" | "syncmsgmb" | "transfer_sync2") {
            continue;
        }
        let act = c.get("act").and_then(Value::as_str).unwrap_or_default();
        let parsed_data: Value = match c.get("data") {
            Some(Value::String(s)) => serde_json::from_str(s).unwrap_or(Value::Null),
            Some(v) => v.clone(),
            None => Value::Null,
        };
        match act {
            "transfer_after_login" => {
                if let Some(tk) = parsed_data
                    .get("temp_key")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                {
                    out.push(MobileSyncControlEvent::TransferAfterLogin {
                        temp_key: tk.to_string(),
                    });
                }
            }
            "user_confirm" => {
                let ua = parsed_data
                    .get("user_action")
                    .and_then(|v| v.as_i64().or_else(|| v.as_str()?.parse().ok()))
                    .unwrap_or(-1);
                if ua == 1 || ua == 3 {
                    out.push(MobileSyncControlEvent::UserConfirmed);
                } else if ua == 2 {
                    out.push(MobileSyncControlEvent::MobileBusyRestoring);
                } else if ua == 0 {
                    out.push(MobileSyncControlEvent::UserRejected);
                }
            }
            "transfer_status" => {
                let st = parsed_data
                    .get("status")
                    .and_then(|v| v.as_i64().or_else(|| v.as_str()?.parse().ok()))
                    .unwrap_or(-1);
                let sid = parsed_data
                    .get("syncId")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                match st {
                    3 => out.push(MobileSyncControlEvent::WaitingConfirmOnPhone { sync_id: sid }),
                    4 | 5 => out.push(MobileSyncControlEvent::UserConfirmed),
                    6 | 7 => out.push(MobileSyncControlEvent::UserRejected),
                    8 => out.push(MobileSyncControlEvent::MobileBusyRestoring),
                    _ => {}
                }
            }
            "upload_batch" => {
                let url = parsed_data
                    .get("msgUrl")
                    .or_else(|| parsed_data.get("url"))
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                let sid = parsed_data
                    .get("syncId")
                    .and_then(Value::as_str)
                    .unwrap_or("sync_v2_batch")
                    .to_string();
                let batch_type = parsed_data
                    .get("batchType")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0) as u8;
                let is_last = parsed_data
                    .get("isLast")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0)
                    == 1;
                if !url.is_empty() {
                    out.push(MobileSyncControlEvent::SyncV2Batch {
                        url,
                        sync_id: sid,
                        batch_type,
                        is_last,
                    });
                } else {
                    out.push(MobileSyncControlEvent::MobileBusyRestoring);
                }
            }
            "syncmsg_info" => {
                let url = parsed_data
                    .get("url")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                let file_name = parsed_data
                    .get("file_name")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                let encrypted_key = parsed_data
                    .get("encrypted_key")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                let checksum = parsed_data
                    .get("checksum_code")
                    .or_else(|| parsed_data.get("checksum"))
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                if !url.is_empty() {
                    out.push(MobileSyncControlEvent::BackupReady {
                        url,
                        file_name,
                        encrypted_key,
                        checksum,
                    });
                }
            }
            "transfer_error" => {
                let err_code = parsed_data
                    .get("error_code")
                    .and_then(|v| v.as_i64().or_else(|| v.as_str()?.parse().ok()))
                    .unwrap_or(0);
                let err_msg = parsed_data
                    .get("error_msg")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                if err_code != 0 {
                    let msg = if !err_msg.is_empty() {
                        err_msg.to_string()
                    } else {
                        format!("Mã lỗi #{err_code}")
                    };
                    out.push(MobileSyncControlEvent::SyncError { message: msg });
                }
            }
            _ => {}
        }
    }
    out
}

/// Hide or unhide a conversation (`updateItemHiddenChat` @ `compact-app-pc.js:14384423`, cmd `10049`).
pub async fn set_hidden_conversation(
    s: &Session,
    thread_id: &str,
    is_group: bool,
    hide: bool,
) -> Result<Value> {
    if s.login.is_none() {
        anyhow::bail!("chưa đăng nhập");
    }
    let clean = thread_id
        .trim_start_matches("group_")
        .trim_start_matches('g')
        .trim_start_matches('G')
        .trim();
    let entry = json!([{
        "thread_id": clean,
        "is_group": if is_group { 1 } else { 0 }
    }]);
    let (add_str, del_str) = if hide {
        (entry.to_string(), "[]".to_string())
    } else {
        ("[]".to_string(), entry.to_string())
    };
    let body = json!({
        "del_threads": del_str,
        "add_threads": add_str,
        "imei": s.imei
    });
    let params = s.encoded_params(&body)?;
    let url = format!(
        "{}/api/hiddenconvers/add-remove?{}",
        s.domains.conversation,
        s.common_params()
    );
    let resp = s.post_form(&url, format!("params={params}")).await?;
    Ok(s.maybe_decrypt(resp, s.session_key_bytes()).await)
}

/// Delete conversation history for me (`deleteOneOneConversationV2` / `deleteGroupConversationV2` @ `compact-app-pc.js:14374881`, cmd `11778` / `11835`).
pub async fn delete_conversation(
    s: &Session,
    thread_id: &str,
    is_group: bool,
    last_cli_msg_id: &str,
) -> Result<Value> {
    if s.login.is_none() {
        anyhow::bail!("chưa đăng nhập");
    }
    let clean = thread_id
        .trim_start_matches("group_")
        .trim_start_matches('g')
        .trim_start_matches('G')
        .trim();
    let cli_id = if last_cli_msg_id.is_empty() {
        format!("{}", crate::login::unix_now() * 1000)
    } else {
        last_cli_msg_id.to_string()
    };
    if is_group {
        let body = json!({
            "grid": clean,
            "cliMsgId": cli_id,
            "conver": {},
            "onlyMe": 1,
            "imei": s.imei
        });
        let params = s.encoded_params(&body)?;
        let url = format!(
            "{}/api/group/deleteconver?{}",
            s.domains.group,
            s.common_params()
        );
        let resp = s.post_form(&url, format!("params={params}")).await?;
        Ok(s.maybe_decrypt(resp, s.session_key_bytes()).await)
    } else {
        let body = json!({
            "toid": clean,
            "cliMsgId": cli_id,
            "conver": {},
            "onlyMe": 1,
            "imei": s.imei
        });
        let params = s.encoded_params(&body)?;
        let url = format!(
            "{}/api/message/deleteconver?{}",
            s.domains.chat,
            s.common_params()
        );
        let resp = s.post_form(&url, format!("params={params}")).await?;
        Ok(s.maybe_decrypt(resp, s.session_key_bytes()).await)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn group_posting_restriction_respects_member_roles() {
        let info = serde_json::json!({
            "setting": {"lockSendMsg": 1},
            "creatorId": "owner-1",
            "adminIds": ["admin-1"]
        });
        assert_eq!(group_posting_restricted(&info, "member-1"), Some(true));
        assert_eq!(group_posting_restricted(&info, "owner-1"), Some(false));
        assert_eq!(group_posting_restricted(&info, "admin-1"), Some(false));
        assert_eq!(
            group_posting_restricted(
                &serde_json::json!({"setting": {"lockSendMsg": 0}}),
                "member-1"
            ),
            Some(false)
        );
        assert_eq!(
            group_posting_restricted(&serde_json::json!({}), "member-1"),
            None
        );
    }

    #[test]
    fn local_time_when_parsing_millisecond_timestamps() {
        use chrono::TimeZone;
        let expected = chrono::Local
            .timestamp_millis_opt(1793467800000)
            .single()
            .unwrap()
            .format("%H:%M")
            .to_string();
        let thread = parse_thread(&serde_json::json!({
            "userId": "user-1", "lastActionTime": 1793467800000_i64
        }))
        .unwrap();
        assert_eq!(thread.time, expected);
        let message = parse_chat_message(&serde_json::json!({
            "msgId": "msg-0001", "content": "hello", "ts": 1793467800000_i64
        }))
        .unwrap();
        assert_eq!(message.time, expected);
        assert_eq!(message.ts, 1793467800000_i64);
    }

    #[test]
    fn anti_recall_merging_retains_original_text_and_media() {
        let mut target = Thread {
            id: "t1".into(),
            name: "Test".into(),
            is_group: false,
            preview: "Ảnh đẹp nè".into(),
            time: "10:00".into(),
            messages: vec![ChatMessage {
                id: "m1".into(),
                client_message_id: "c1".into(),
                sender: "Friend".into(),
                text: "Ảnh đẹp nè".into(),
                time: "10:00".into(),
                is_outgoing: false,
                ts: 1000,
                link_url: Some("https://res-zalo.zadn.vn/photo.jpg".into()),
                link_thumb: Some("https://res-zalo.zadn.vn/thumb.jpg".into()),
                sticker_id: Some(123),
                ..Default::default()
            }],
            ..Default::default()
        };
        let incoming = Thread {
            id: "t1".into(),
            name: "Test".into(),
            is_group: false,
            preview: "[Tin nhắn đã thu hồi]".into(),
            time: "10:05".into(),
            messages: vec![ChatMessage {
                id: "m1".into(),
                client_message_id: "c1".into(),
                sender: "Friend".into(),
                text: "[Tin nhắn đã thu hồi]".into(),
                time: "10:05".into(),
                is_outgoing: false,
                ts: 1005,
                ..Default::default()
            }],
            ..Default::default()
        };
        merge_thread_into(&mut target, incoming, true);
        let m = &target.messages[0];
        assert_eq!(m.text, "[Đã thu hồi] Ảnh đẹp nè");
        assert_eq!(
            m.link_url.as_deref(),
            Some("https://res-zalo.zadn.vn/photo.jpg")
        );
        assert_eq!(
            m.link_thumb.as_deref(),
            Some("https://res-zalo.zadn.vn/thumb.jpg")
        );
        assert_eq!(m.sticker_id, Some(123));
    }

    #[test]
    fn extract_typing_and_seen_controls_parses_events() {
        let payload = json!({
            "controls": [
                {
                    "content": {
                        "act_type": "typing",
                        "act": "typing",
                        "data": {
                            "fromUid": "u123",
                            "grid": "g456",
                            "status": 1
                        }
                    }
                },
                {
                    "content": {
                        "act_type": "seen",
                        "act": "seen",
                        "data": {
                            "fromUid": "u789",
                            "toid": "u789",
                            "seenTime": 1700000000
                        }
                    }
                }
            ]
        });
        let (typing, seen) = extract_typing_and_seen_controls(&payload);
        assert_eq!(typing.len(), 1);
        assert_eq!(typing[0].thread_id, "g456");
        assert_eq!(typing[0].uid, "u123");
        assert!(typing[0].is_typing);

        assert_eq!(seen.len(), 1);
        assert_eq!(seen[0].thread_id, "u789");
        assert_eq!(seen[0].seen_ts, 1700000000);
    }

    #[test]
    fn local_time_when_parsing_second_timestamps_and_invalid_values() {
        let expected = crate::clock::message_time(1793467800000);
        for ts in [
            serde_json::json!(1793467800),
            serde_json::json!("1793467800"),
        ] {
            let thread = parse_thread(&serde_json::json!({
                "userId": "user-1", "lastActionTime": ts
            }))
            .unwrap();
            let message = parse_chat_message(&serde_json::json!({
                "msgId": "msg-0001", "content": "hello", "ts": ts
            }))
            .unwrap();
            assert_eq!(thread.time, expected);
            assert_eq!(message.time, expected);
            assert_eq!(message.ts, 1793467800);
        }
        for ts in [
            serde_json::json!(-1),
            serde_json::json!(0),
            serde_json::json!(i64::MAX),
            serde_json::json!("invalid"),
        ] {
            let thread = parse_thread(&serde_json::json!({
                "userId": "user-1", "lastActionTime": ts
            }))
            .unwrap();
            assert_eq!(thread.time, "");
        }
        let message = parse_chat_message(&serde_json::json!({
            "msgId": "msg-0001", "content": "hello", "ts": i64::MAX
        }))
        .unwrap();
        assert_eq!(message.time, "");
    }

    #[tokio::test]
    async fn unauthenticated_calls_fail_cleanly() {
        let s = Session::new(false, false, false).unwrap();
        let err1 = get_friends(&s).await.unwrap_err();
        assert_eq!(err1.to_string(), "chưa đăng nhập");

        let err2 = get_last_msgs(&s, &["test_tid".into()]).await.unwrap_err();
        assert_eq!(err2.to_string(), "chưa đăng nhập");

        let err3 = send_message(&s, "test_to", "hello").await.unwrap_err();
        assert_eq!(err3.to_string(), "chưa đăng nhập");

        let err4 = get_pinned(&s).await.unwrap_err();
        assert_eq!(err4.to_string(), "chưa đăng nhập");

        let err5 = get_group_history(&s, "test_gid", 20).await.unwrap_err();
        assert_eq!(err5.to_string(), "chưa đăng nhập");
    }

    fn sample_thread(id: &str) -> Thread {
        Thread {
            presentation: Default::default(),
            unread: Default::default(),
            id: id.into(),
            name: format!("Name {id}"),
            preview: String::new(),
            time: String::new(),
            avatar: None,
            is_group: false,
            messages: Vec::new(),
        }
    }

    #[test]
    fn merge_thread_into_matches_exact_client_reference() {
        let m = |id: &str, text: &str, ts: i64, out: bool| ChatMessage {
            id: id.into(),
            client_message_id: if id == "out_9" || id == "77" {
                "client-9".into()
            } else {
                String::new()
            },
            text: text.into(),
            ts,
            time: format!("{}", ts / 60_000),
            is_outgoing: out,
            ..Default::default()
        };
        let mut target = Thread {
            presentation: Default::default(),
            unread: Default::default(),
            id: "t".into(),
            name: "A".into(),
            preview: String::new(),
            time: String::new(),
            avatar: None,
            is_group: false,
            messages: vec![
                m("1", "chào", 1_000_000, false),
                m("out_9", "ok", 2_000_000, true),
            ],
        };
        let incoming = Thread {
            messages: vec![
                m("1", "chào", 1_000_000, false),   // same id -> no duplicate
                m("77", "ok", 2_002_000, true),     // exact client reference -> takes server ID
                m("78", "ok", 9_000_000_000, true), // same text, other day -> new message
                m("79", "mới", 3_000_000, false),   // new
            ],
            ..target.clone()
        };
        merge_thread_into(&mut target, incoming, false);
        let ids: Vec<&str> = target.messages.iter().map(|x| x.id.as_str()).collect();
        assert_eq!(ids, vec!["1", "77", "79", "78"]);
    }

    #[test]
    fn test_merge_previews_single_message_object() {
        let mut threads = vec![sample_thread("u1")];
        let resp = json!({
            "error_code": 0,
            "data": [
                {
                    "idTo": "u1",
                    "message": {
                        "msgId": "m_single",
                        "content": "Single message text",
                        "ts": 1729501000000_i64,
                        "uidFrom": "u1"
                    }
                }
            ]
        });

        merge_previews(&mut threads, &resp);
        assert_eq!(threads[0].messages.len(), 1);
        assert_eq!(threads[0].messages[0].id, "m_single");
        assert_eq!(threads[0].messages[0].text, "Single message text");
        assert_eq!(threads[0].preview, "Single message text");
    }

    #[test]
    fn test_merge_previews_last_msg_object() {
        let mut threads = vec![sample_thread("u2")];
        let resp = json!({
            "error_code": 0,
            "data": [
                {
                    "idTo": "u2",
                    "lastMsg": {
                        "msgId": "m_last",
                        "content": "LastMsg text",
                        "ts": 1729502000000_i64,
                        "uidFrom": "u2"
                    }
                }
            ]
        });

        merge_previews(&mut threads, &resp);
        assert_eq!(threads[0].messages.len(), 1);
        assert_eq!(threads[0].messages[0].id, "m_last");
        assert_eq!(threads[0].preview, "LastMsg text");
    }

    #[test]
    fn test_merge_previews_messages_array_three_items() {
        let mut threads = vec![sample_thread("u3")];
        // Provide items out of order to verify timestamp ascending sort
        let resp = json!({
            "error_code": 0,
            "data": [
                {
                    "idTo": "u3",
                    "messages": [
                        {
                            "msgId": "m_item1",
                            "content": "First message",
                            "ts": 1000_i64,
                            "uidFrom": "u3"
                        },
                        {
                            "msgId": "m_item3",
                            "content": "Third message",
                            "ts": 3000_i64,
                            "uidFrom": "u3"
                        },
                        {
                            "msgId": "m_item2",
                            "content": "Second message",
                            "ts": 2000_i64,
                            "uidFrom": "0"
                        }
                    ]
                }
            ]
        });

        merge_previews(&mut threads, &resp);
        assert_eq!(threads[0].messages.len(), 3);
        assert_eq!(threads[0].messages[0].id, "m_item1");
        assert_eq!(threads[0].messages[0].text, "First message");
        assert_eq!(threads[0].messages[1].id, "m_item2");
        assert_eq!(threads[0].messages[1].text, "Second message");
        assert!(threads[0].messages[1].is_outgoing);
        assert_eq!(threads[0].messages[2].id, "m_item3");
        assert_eq!(threads[0].messages[2].text, "Third message");
        // Preview must be from the newest message (m_item3)
        assert_eq!(threads[0].preview, "Third message");
    }

    #[test]
    fn test_merge_previews_entry_with_no_message() {
        let mut threads = vec![sample_thread("u4")];
        threads[0].preview = "Existing preview".into();
        let resp = json!({
            "error_code": 0,
            "data": [
                {
                    "idTo": "u4"
                },
                {
                    "idTo": "u4",
                    "messages": []
                }
            ]
        });

        merge_previews(&mut threads, &resp);
        assert!(threads[0].messages.is_empty());
        assert_eq!(threads[0].preview, "Existing preview");
    }

    #[test]
    fn test_merge_group_history_fixture() {
        let mut thread = sample_thread("grp_123");
        thread.is_group = true;

        let resp = json!({
            "error_code": 0,
            "data": {
                "groupMsgs": [
                    {
                        "msgId": "gm_2",
                        "content": "Second group message",
                        "ts": 2000_i64,
                        "uidFrom": "member_2"
                    },
                    {
                        "msgId": "gm_1",
                        "content": "First group message",
                        "ts": 1000_i64,
                        "uidFrom": "member_1"
                    },
                    {
                        "msgId": "gm_2",
                        "content": "Duplicate second message",
                        "ts": 2000_i64,
                        "uidFrom": "member_2"
                    }
                ]
            }
        });

        merge_group_history(&mut thread, &resp);
        assert_eq!(thread.messages.len(), 2);
        assert_eq!(thread.messages[0].id, "gm_1");
        assert_eq!(thread.messages[1].id, "gm_2");
        assert_eq!(thread.preview, "Second group message");
    }

    #[test]
    fn test_merge_previews_incoming_1_1_and_group_and_cloud() {
        let mut cloud = sample_thread("9000000000000000001");
        cloud.name = "Cloud của tôi (Lưu trữ)".into();
        let friend = sample_thread("1000000000000000001");
        let mut group = sample_thread("888999000");
        group.is_group = true;
        let mut threads = vec![cloud, friend, group];

        let resp = json!({
            "error_code": 0,
            "data": {
                "msgs": [
                    {
                        "msgId": "in_1",
                        "content": "Tin nhan den tu ban be",
                        "idTo": "0",
                        "uidFrom": "1000000000000000001",
                        "ts": "1729505000000"
                    },
                    {
                        "msgId": "cloud_1",
                        "content": "Tin nhan tu gui vao Cloud",
                        "idTo": "0",
                        "uidFrom": "0",
                        "ts": "1729506000000"
                    }
                ],
                "groupMsgs": [
                    {
                        "msgId": "grp_msg_1",
                        "content": "Tin nhan trong nhom",
                        "idTo": "888999000",
                        "uidFrom": "111222333",
                        "dName": "Nguyen Van B",
                        "ts": "1729507000000"
                    }
                ]
            }
        });

        merge_previews(&mut threads, &resp);
        // Cloud message routed to `9000000000000000001`
        assert_eq!(threads[0].messages.len(), 1);
        assert_eq!(threads[0].messages[0].text, "Tin nhan tu gui vao Cloud");
        assert!(threads[0].messages[0].is_outgoing);

        // Incoming 1-1 routed to `1000000000000000001`
        assert_eq!(threads[1].messages.len(), 1);
        assert_eq!(threads[1].messages[0].text, "Tin nhan den tu ban be");
        assert!(!threads[1].messages[0].is_outgoing);

        // Group message routed to `888999000` (not `111222333`)
        assert_eq!(threads[2].messages.len(), 1);
        assert_eq!(threads[2].messages[0].text, "Tin nhan trong nhom");
        assert_eq!(threads[2].messages[0].sender, "Nguyen Van B");
        assert!(!threads[2].messages[0].is_outgoing);
    }

    #[test]
    fn test_poll_state_encode_and_update() {
        let mut ps = PollState::default();
        let encoded = ps.encode_params();
        assert!(encoded.starts_with("1,1,"));
        assert!(encoded.ends_with("%2C1"));

        let resp = json!({
            "data": {
                "lastActionId": "8399086511016",
                "lastActionIdOther": "8399036593265",
                "queueStatus": {
                    "1": {
                        "lastId": "8399086511016",
                        "ids": ["8399086511016"]
                    },
                    "15": {
                        "lastId": "8399036593265",
                        "ids": ["8399036593265"]
                    }
                }
            }
        });
        ps.update_from_response(&resp);
        assert_eq!(ps.last_action_id, "8399086511016");
        assert_eq!(ps.last_action_id_other, "8399036593265");
        assert_eq!(ps.queues.get("1").unwrap().last_id, "8399086511016");
        assert_eq!(
            ps.queues.get("1").unwrap().ids,
            vec!["8399086511016".to_string()]
        );
        // Socket-only queue "15" must NOT be added to HTTP polling queues
        assert!(!ps.queues.contains_key("15"));
    }

    #[test]
    fn test_parse_chat_message_sticker_json_string() {
        let msg_val = json!({
            "msgId": "stk_1",
            "msgType": "chat.sticker",
            "content": "{\"catId\":1002,\"id\":2005,\"type\":1}",
            "uidFrom": "user_a",
            "ts": 1729508000000_i64
        });
        let parsed = parse_chat_message(&msg_val).expect("must parse sticker");
        assert_eq!(parsed.text, "[Nhãn dán]");
        assert_eq!(parsed.id, "stk_1");
        assert_eq!(parsed.sticker_id, Some(2005));
        assert_eq!(parsed.sticker_cate_id, Some(1002));
        assert_eq!(parsed.sticker_type, Some(1));
        assert_eq!(
            parsed.sticker_image_url().as_deref(),
            Some("https://api.zaloapp.com/api/emoticon/sticker/webpc?eid=2005&size=130")
        );
    }

    #[test]
    fn test_parse_chat_message_integer_msg_types() {
        // Integer msgType = 5 (sticker) with empty content
        let stk = json!({
            "msgId": "stk_int",
            "msgType": 5,
            "content": "",
            "uidFrom": "user_b",
            "ts": 1000_i64
        });
        assert_eq!(parse_chat_message(&stk).unwrap().text, "[Nhãn dán]");

        // Integer msgType = 2 (photo)
        let photo = json!({
            "msgId": "photo_int",
            "msgType": 2,
            "content": "",
            "uidFrom": "user_b",
            "ts": 2000_i64
        });
        assert_eq!(parse_chat_message(&photo).unwrap().text, "[Hình ảnh]");

        // Integer msgType = 3 (voice)
        let voice = json!({
            "msgId": "voice_int",
            "msgType": 3,
            "content": "",
            "uidFrom": "user_b",
            "ts": 3000_i64
        });
        assert_eq!(parse_chat_message(&voice).unwrap().text, "[Tin nhắn thoại]");

        // Integer msgType = 19 (file)
        let file = json!({
            "msgId": "file_int",
            "msgType": 19,
            "content": "",
            "uidFrom": "user_b",
            "ts": 4000_i64
        });
        assert_eq!(parse_chat_message(&file).unwrap().text, "[Tập tin]");

        // Integer msgType = 31 (undo)
        let undo = json!({
            "msgId": "undo_int",
            "msgType": 31,
            "content": "",
            "uidFrom": "user_b",
            "ts": 5000_i64
        });
        assert_eq!(
            parse_chat_message(&undo).unwrap().text,
            "[Tin nhắn đã thu hồi]"
        );
    }

    #[test]
    fn test_parse_chat_message_with_plain_message_string() {
        let msg = json!({
            "msgId": "msg_str",
            "message": "Xin chao buoi sang",
            "uidFrom": "user_c",
            "dName": "Charlie",
            "ts": 1000_i64
        });
        let parsed = parse_chat_message(&msg).unwrap();
        assert_eq!(parsed.text, "Xin chao buoi sang");
        assert_eq!(parsed.sender, "Charlie");
    }

    #[test]
    fn test_parse_thread_boolean_is_group_and_string_time() {
        let val = json!({
            "userId": "grp_test",
            "displayName": "Nhom Chem Gio",
            "isGroup": true,
            "lastActionTime": "1729505000000"
        });
        let thread = parse_thread(&val).unwrap();
        assert!(thread.is_group);
        assert_eq!(thread.name, "Nhom Chem Gio");
        assert!(!thread.time.is_empty());
    }

    #[test]
    fn test_deduplicate_threads_migrates_session_scoped_ids() {
        let old_dtc = Thread {
            presentation: Default::default(),
            unread: Default::default(),
            id: "1000000000000000005".into(),
            name: "Đtc".into(),
            preview: "Hi".into(),
            time: "10:21".into(),
            avatar: Some("https://s120-26-ava-talk.zadn.vn/5/00000000000000000000000000000001.jpg?key=abc&time=1797763127".into()),
            is_group: false,
            messages: vec![ChatMessage {
                id: "m1".into(),
                sender: "Đtc".into(),
                text: "Hi".into(),
                time: "10:21".into(),
                is_outgoing: false,
                ts: 1792578088952,
                ..Default::default()
            }],
        };
        let new_dtc = Thread {
            presentation: Default::default(),
            unread: Default::default(),
            id: "1000000000000000006".into(),
            name: "Đtc".into(),
            preview: "".into(),
            time: "16:48".into(),
            avatar: Some("https://s160-26-ava-talk.zadn.vn/5/00000000000000000000000000000001.jpg?key=xyz&time=1797876325".into()),
            is_group: false,
            messages: Vec::new(),
        };
        let mut threads = vec![old_dtc, new_dtc];
        deduplicate_threads(&mut threads, None);
        assert_eq!(threads.len(), 1);
        assert_eq!(threads[0].id, "1000000000000000006");
        assert_eq!(threads[0].messages.len(), 1);
        assert_eq!(threads[0].preview, "Hi");
    }

    #[test]
    fn test_merge_group_history_parses_stringified_cloud_data() {
        let mut cloud_thread = Thread {
            presentation: Default::default(),
            unread: Default::default(),
            id: "9000000000000000002".into(),
            name: "Cloud của tôi (Lưu trữ)".into(),
            preview: String::new(),
            time: String::new(),
            avatar: None,
            is_group: false,
            messages: Vec::new(),
        };
        let raw_cloud_resp = json!({
            "error_code": 0,
            "data": r#"{"error":0,"groupMsgs":[{"msgId":"1000000000001","cliMsgId":"1700000000001","msgType":"chat.recommended","uidFrom":"0","idTo":"9000000000000000002","dName":"User Test","ts":"1700000000001","content":{"title":"","description":"Ghi chú đám mây","href":"https://zalo.me/test","action":"recommened.link"}}]}"#
        });
        merge_group_history(&mut cloud_thread, &raw_cloud_resp);
        assert_eq!(cloud_thread.messages.len(), 1);
        assert!(cloud_thread.messages[0].text.contains("Ghi chú đám mây"));
        assert_eq!(
            cloud_thread.messages[0].link_url.as_deref(),
            Some("https://zalo.me/test")
        );
    }

    #[test]
    fn test_strip_vn_for_sticker_kw() {
        assert_eq!(strip_vn_for_sticker_kw("Xin chào"), "xin chao");
        assert_eq!(strip_vn_for_sticker_kw("Cảm ơn nhiều"), "cam on nhieu");
        assert_eq!(strip_vn_for_sticker_kw("Ngủ ngon nhé"), "ngu ngon nhe");
        assert_eq!(strip_vn_for_sticker_kw("Nguyễn Văn An"), "nguyen van an");
        assert_eq!(
            normalize_sticker_keyword("  iu\u{a0}that\u{a0}  "),
            "iu that"
        );

        let q1 = canonical_vn_sticker_queries("Mệt quá");
        assert!(q1.contains(&"mệt quá".to_string()));
        assert!(q1.contains(&"met qua".to_string()));

        let q2 = canonical_vn_sticker_queries("khoc");
        assert!(q2.contains(&"khoc".to_string()));
        assert!(q2.contains(&"khóc".to_string()));
    }

    #[test]
    fn test_parse_friendship_status_response() {
        let r_friend = json!({"error_code": 0, "data": {"is_friend": 1, "is_requested": 0, "is_requesting": 0}});
        assert_eq!(
            parse_friendship_status_response(&r_friend),
            FriendshipStatus::Friend
        );

        let r_inc = json!({"error_code": 0, "data": {"is_friend": 0, "is_requested": 0, "is_requesting": 1}});
        assert_eq!(
            parse_friendship_status_response(&r_inc),
            FriendshipStatus::IncomingPending
        );

        let r_out = json!({"error_code": 0, "data": {"is_friend": 0, "is_requested": 1, "is_requesting": 0}});
        assert_eq!(
            parse_friendship_status_response(&r_out),
            FriendshipStatus::OutgoingPending
        );

        let r_stranger = json!({"error_code": 0, "data": {"is_friend": 0, "is_requested": 0, "is_requesting": 0}});
        assert_eq!(
            parse_friendship_status_response(&r_stranger),
            FriendshipStatus::Stranger
        );
    }

    #[test]
    fn test_prevent_cloud_message_pollution_across_threads() {
        let mut contact = Thread {
            presentation: Default::default(),
            unread: Default::default(),
            id: "1000000000000000009".into(),
            name: "Người dùng Test".into(),
            preview: String::new(),
            time: String::new(),
            avatar: None,
            is_group: false,
            messages: Vec::new(),
        };
        let raw_cloud_resp = json!({
            "error_code": 0,
            "data": {
                "groupMsgs": [{
                    "msgId": "1000000000002",
                    "msgType": "webchat",
                    "uidFrom": "0",
                    "idTo": "9000000000000000001",
                    "dName": "User Test",
                    "ts": "1700000000002",
                    "content": "Ghi chú Cloud của tôi"
                }]
            }
        });
        merge_group_history(&mut contact, &raw_cloud_resp);
        assert!(contact.messages.is_empty());

        let cloud = Thread {
            presentation: Default::default(),
            unread: Default::default(),
            id: "9000000000000000001".into(),
            name: "Cloud của tôi (Lưu trữ)".into(),
            preview: "Ghi chú Cloud của tôi".into(),
            time: "12:00".into(),
            avatar: None,
            is_group: false,
            messages: vec![ChatMessage {
                id: "1000000000002".into(),
                sender: "Tôi".into(),
                text: "Ghi chú Cloud của tôi".into(),
                time: "12:00".into(),
                is_outgoing: true,
                ts: 1700000000002,
                ..Default::default()
            }],
        };
        let polluted_contact = Thread {
            presentation: Default::default(),
            unread: Default::default(),
            id: "1000000000000000009".into(),
            name: "Người dùng Test".into(),
            preview: "Ghi chú Cloud của tôi".into(),
            time: "12:00".into(),
            avatar: None,
            is_group: false,
            messages: vec![ChatMessage {
                id: "1000000000002".into(),
                sender: "Tôi".into(),
                text: "Ghi chú Cloud của tôi".into(),
                time: "12:00".into(),
                is_outgoing: true,
                ts: 1700000000002,
                ..Default::default()
            }],
        };
        let mut list = vec![cloud, polluted_contact];
        deduplicate_threads(&mut list, None);
        assert_eq!(list[0].messages.len(), 1);
        assert!(list[1].messages.is_empty());
        assert!(list[1].preview.is_empty());
    }

    #[test]
    fn test_duplicate_display_names_with_different_ids_stay_distinct() {
        let contact_a = Thread {
            presentation: Default::default(),
            unread: Default::default(),
            id: "1000000000000000111".into(),
            name: "Nguyễn Văn A".into(),
            preview: "Xin chào từ người 1".into(),
            time: "09:00".into(),
            avatar: Some(
                "https://s120-ava-talk.zadn.vn/1/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa.jpg".into(),
            ),
            is_group: false,
            messages: vec![ChatMessage {
                id: "m_a".into(),
                sender: "Nguyễn Văn A".into(),
                text: "Xin chào từ người 1".into(),
                time: "09:00".into(),
                is_outgoing: false,
                ts: 1700000001000,
                ..Default::default()
            }],
        };
        // Second contact has the exact same Zalo display name and no avatar URL yet (e.g., from incoming poll)
        let contact_b = Thread {
            presentation: Default::default(),
            unread: Default::default(),
            id: "1000000000000000222".into(),
            name: "Nguyễn Văn A".into(),
            preview: "Tin nhắn mới từ người 2".into(),
            time: "10:30".into(),
            avatar: None,
            is_group: false,
            messages: vec![ChatMessage {
                id: "m_b".into(),
                sender: "Nguyễn Văn A".into(),
                text: "Tin nhắn mới từ người 2".into(),
                time: "10:30".into(),
                is_outgoing: false,
                ts: 1700000002000,
                ..Default::default()
            }],
        };
        assert!(!threads_represent_same_contact(&contact_a, &contact_b));
        let mut threads = vec![contact_a, contact_b];
        deduplicate_threads(&mut threads, None);
        assert_eq!(threads.len(), 2);
        assert_eq!(threads[0].id, "1000000000000000111");
        assert_eq!(threads[1].id, "1000000000000000222");
        assert_eq!(threads[0].messages[0].text, "Xin chào từ người 1");
        assert_eq!(threads[1].messages[0].text, "Tin nhắn mới từ người 2");
    }
}

#[cfg(test)]
mod message_state_defaults_tests {
    use super::*;

    #[test]
    fn legacy_message_when_metadata_is_absent_uses_conservative_defaults() {
        let message: ChatMessage = serde_json::from_value(json!({
            "id": "msg-0001", "sender": "user-1", "text": "Hello",
            "time": "", "is_outgoing": false, "ts": 1
        }))
        .unwrap();
        assert_eq!(message.kind, crate::message_model::MessageKind::Unknown);
        assert_eq!(
            message.delivery,
            crate::message_model::DeliveryState::default()
        );
        assert!(message.wire_ref.fields.is_empty());
        assert!(message.reaction_summaries.is_empty());
        assert!(message.resolved_mention_uids.is_empty());
    }

    #[test]
    fn legacy_thread_when_sidebar_and_unread_are_absent_roundtrips() {
        let thread: Thread = serde_json::from_value(json!({
            "id": "thread-1", "name": "Example", "preview": "", "time": "",
            "avatar": null, "is_group": false, "messages": []
        }))
        .unwrap();
        assert_eq!(thread.presentation, SidebarPresentation::default());
        assert_eq!(thread.unread, crate::unread::UnreadState::default());
        let restored: Thread =
            serde_json::from_value(serde_json::to_value(&thread).unwrap()).unwrap();
        assert_eq!(restored.presentation, thread.presentation);
        assert_eq!(restored.unread, thread.unread);
    }
}

#[cfg(test)]
mod send_identity_tests {
    use super::*;

    #[test]
    fn request_ids_are_unique_when_sends_share_a_clock_tick() {
        let ids: Vec<_> = (0..10_000).map(|_| next_client_message_id()).collect();
        assert!(ids.windows(2).all(|ids| ids[0] < ids[1]));
    }

    #[tokio::test]
    async fn request_client_id_survives_send_variants_and_fallbacks() {
        use std::io::{Read, Write};
        use std::net::TcpListener;
        use std::time::Duration;

        // Only loopback fixture traffic is used; all keys and identifiers are synthetic.
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let host = format!("http://{}", listener.local_addr().unwrap());
        let fixture = std::thread::spawn(move || {
            let mut requests = Vec::new();
            for _ in 0..9 {
                let (mut stream, _) = listener.accept().unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(3)))
                    .unwrap();
                let mut headers = Vec::new();
                while !headers.ends_with(b"\r\n\r\n") {
                    let mut byte = [0];
                    stream.read_exact(&mut byte).unwrap();
                    headers.push(byte[0]);
                    assert!(headers.len() < 16_384);
                }
                let headers = String::from_utf8(headers).unwrap();
                let size: usize = headers
                    .lines()
                    .find_map(|line| {
                        let (name, value) = line.split_once(':')?;
                        name.eq_ignore_ascii_case("content-length")
                            .then(|| value.trim().parse().unwrap())
                    })
                    .unwrap();
                assert!(size < 16_384);
                let mut body = vec![0; size];
                stream.read_exact(&mut body).unwrap();
                let query = reqwest::Url::parse(&format!(
                    "http://fixture.invalid/?{}",
                    String::from_utf8(body).unwrap()
                ))
                .unwrap();
                let encrypted = query
                    .query_pairs()
                    .find(|(key, _)| key == "params")
                    .unwrap()
                    .1
                    .into_owned();
                let body: Value =
                    serde_json::from_str(&crate::crypto::decode_aes(&[7; 32], &encrypted).unwrap())
                        .unwrap();
                let path = headers
                    .lines()
                    .next()
                    .unwrap()
                    .split_whitespace()
                    .nth(1)
                    .unwrap()
                    .to_string();
                let fail = path.contains("/link?") || path.contains("/quote?");
                requests.push((path, body));
                let response = if fail {
                    r#"{"error_code":114}"#
                } else {
                    r#"{"error_code":0,"data":{"msgId":"server-a"}}"#
                };
                write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{}", response.len(), response).unwrap();
            }
            requests
        });
        let mut session = Session::new(false, false, false).unwrap();
        session.login = Some(crate::session::LoginInfo::default());
        session.secret_key = Some(vec![7; 32]);
        session.domains.chat = host.clone();
        session.domains.group = host;
        let client_id = 1_793_425_680_123;
        send_message_with_client_id(&session, "contact-1", "Text", client_id)
            .await
            .unwrap();
        send_group_message_with_client_id(&session, "group-1", "Text", client_id)
            .await
            .unwrap();
        send_link_message_with_client_id(
            &session,
            "contact-1",
            "Text",
            &LinkMetadata {
                url: "https://example.invalid/link".into(),
                title: "Title".into(),
                desc: String::new(),
                thumb: String::new(),
                src: String::new(),
            },
            false,
            client_id,
        )
        .await
        .unwrap();
        send_sticker_with_client_id(&session, "contact-1", 1, 2, 7, false, client_id)
            .await
            .unwrap();
        send_quote_message_with_client_id(
            &session,
            "contact-1",
            "Text",
            &ChatMessage {
                id: "server-quote".into(),
                ..Default::default()
            },
            false,
            client_id,
        )
        .await
        .unwrap();
        forward_message_with_client_id(
            &session,
            "contact-1",
            false,
            &ChatMessage {
                text: "Forward".into(),
                ..Default::default()
            },
            client_id,
        )
        .await
        .unwrap();
        forward_message_with_client_id(
            &session,
            "contact-1",
            false,
            &ChatMessage {
                sticker_id: Some(1),
                ..Default::default()
            },
            client_id,
        )
        .await
        .unwrap();
        let requests = fixture.join().unwrap();
        assert_eq!(requests.len(), 9);
        for (_, body) in requests {
            assert_eq!(body["clientId"], json!(client_id));
        }
    }

    #[test]
    fn extract_mute_controls_parses_set_and_unset_events_for_peers_and_groups() {
        let now = 1700000000_i64;
        let payload = json!({
            "controls": [
                {
                    "content": {
                        "act_type": "mute",
                        "act": "set",
                        "data": {
                            "toId": "123456789",
                            "muteType": 1,
                            "duration": -1,
                            "startTime": now,
                            "muteMode": 1
                        }
                    }
                },
                {
                    "content": {
                        "act_type": "mute",
                        "act": "set",
                        "data": "{\"toId\":\"987654321\",\"muteType\":2,\"duration\":3600,\"startTime\":1700000000,\"muteMode\":1}"
                    }
                },
                {
                    "content": {
                        "act_type": "mute",
                        "act": "unset",
                        "data": {
                            "toId": "123456789",
                            "muteType": 1,
                            "duration": 0,
                            "muteMode": 0
                        }
                    }
                }
            ]
        });

        let events = extract_mute_controls(&payload, now);
        assert_eq!(events.len(), 3);
        assert_eq!(
            events[0],
            MuteControlEvent {
                thread_id: "123456789".into(),
                is_group: false,
                muted: true,
                muted_until: None,
                settings_until: Some(-1),
            }
        );
        assert_eq!(
            events[1],
            MuteControlEvent {
                thread_id: "g987654321".into(),
                is_group: true,
                muted: true,
                muted_until: Some(now + 3600),
                settings_until: Some(now + 3600),
            }
        );
        assert_eq!(
            events[2],
            MuteControlEvent {
                thread_id: "123456789".into(),
                is_group: false,
                muted: false,
                muted_until: None,
                settings_until: None,
            }
        );
    }

    #[test]
    fn parse_photo_message_hides_raw_cdn_url_and_sets_photo_source() {
        let raw_photo = json!({
            "msgId": "photo_72",
            "msgType": "chat.photo",
            "uidFrom": "1000001",
            "ts": 1700000000000_i64,
            "content": {
                "title": "",
                "description": "",
                "href": "https://photo-stal-zmp.zdn.vn/jpg/55022364c4202d7e7431/999888777.jpg",
                "thumb": "https://photo-stal-zmp.zdn.vn/jpg/55022364c4202d7e7431/999888777_thumb.jpg",
                "params": "{\"width\":1280,\"height\":960}"
            }
        });
        let msg = parse_chat_message(&raw_photo).expect("must parse photo message");
        assert_eq!(msg.link_src.as_deref(), Some("Hình ảnh Zalo"));
        assert_eq!(msg.text, "[Hình ảnh]");
        assert!(is_zalo_photo_cdn_url(msg.link_url.as_deref().unwrap()));
    }

    #[test]
    fn parse_share_file_preserves_sender_doc_filename_instead_of_renew_media() {
        let raw_file = json!({
            "msgId": "file_74",
            "msgType": "share.file",
            "uidFrom": "1000002",
            "ts": 1700000001000_i64,
            "content": {
                "title": "Bao_cao_do_an.doc",
                "description": "",
                "href": "https://f16-zpc.zdn.vn/renewMedia?p=abc123xyz",
                "thumb": "",
                "params": "{\"fileSize\":\"54272\",\"checksum\":\"deadbeef\",\"fileExt\":\"doc\"}"
            }
        });
        let msg = parse_chat_message(&raw_file).expect("must parse file message");
        assert_eq!(msg.link_src.as_deref(), Some("Tập tin Zalo"));
        assert_eq!(msg.link_title.as_deref(), Some("Bao_cao_do_an.doc"));
        assert_eq!(msg.text, "[Tập tin] Bao_cao_do_an.doc");
        let resolved = resolve_attachment_file_name(
            msg.link_title.as_deref(),
            &msg.text,
            msg.link_url.as_deref().unwrap_or_default(),
        );
        assert_eq!(resolved, "Bao_cao_do_an.doc");

        // Also verify when `title` omits `.doc` but `params.fileExt` has `"doc"`
        let raw_file_no_ext = json!({
            "msgId": "file_74_b",
            "msgType": "share.file",
            "uidFrom": "1000002",
            "ts": 1700000002000_i64,
            "content": {
                "title": "De_cuong_mon_hoc",
                "description": "",
                "href": "https://f16-zpc.zdn.vn/renewMedia?p=def456",
                "params": "{\"fileSize\":\"12345\",\"fileExt\":\"doc\"}"
            }
        });
        let msg_b = parse_chat_message(&raw_file_no_ext).expect("must parse file message");
        assert_eq!(msg_b.link_src.as_deref(), Some("Tập tin Zalo"));
        assert_eq!(msg_b.link_title.as_deref(), Some("De_cuong_mon_hoc.doc"));
    }

    #[test]
    fn test_is_zalo_photo_cdn_url_comprehensive() {
        // Direct images and Zalo CDNs
        assert!(is_zalo_photo_cdn_url("https://photo-stal-6.zdn.vn/gr/jpg/beab7091ea123f4c6603/1WUpuktR2ex7c7iAAbWSyDORFc86tI59Rxwuy31ukJWzM6h8D9hN5HA66gJ2eFCzUBU.jpg"));
        assert!(is_zalo_photo_cdn_url(
            "https://f16-zpcloud.zdn.vn/4755838023238443440/415f16b28c720c2c5563.jpg"
        ));
        assert!(is_zalo_photo_cdn_url(
            "https://b-f65-zpg-r.zdn.vn/1913892508530765023/24acd63939f9b9a7e0e8.jpg"
        ));
        assert!(is_zalo_photo_cdn_url(
            "https://res-zalo.zadn.vn/upload/media/2020/2/7/2x_ecardsn_1581058405234_716426.png"
        ));
        assert!(is_zalo_photo_cdn_url(
            "https://example.com/banner.webp?w=500"
        ));
        assert!(is_zalo_photo_cdn_url("local:///Users/mac/test.png"));

        // Non-images
        assert!(!is_zalo_photo_cdn_url(
            "https://file-stal-10.dlfl.vn/gr/1d5605655a9c8fc2d68d/document.pdf"
        ));
        assert!(!is_zalo_photo_cdn_url(
            "https://www.youtube.com/watch?v=F1c8-nNze3U"
        ));
        assert!(!is_zalo_photo_cdn_url(
            "https://docs.google.com/spreadsheets/d/123/edit"
        ));
        assert!(!is_zalo_photo_cdn_url(
            "https://f16-zpc.zdn.vn/renewMedia?p=abc"
        ));
    }

    #[test]
    fn test_is_safe_external_image_url() {
        assert!(is_safe_external_image_url(
            "https://photo-stal-6.zdn.vn/gr/jpg/test.jpg"
        ));
        assert!(is_safe_external_image_url(
            "https://example.com/banner.webp"
        ));
        assert!(!is_safe_external_image_url(
            "http://localhost:8080/test.png"
        ));
        assert!(!is_safe_external_image_url("http://127.0.0.1/admin.png"));
        assert!(!is_safe_external_image_url("https://10.0.0.1/secret.jpg"));
        assert!(!is_safe_external_image_url("http://192.168.1.1/router.png"));
        assert!(!is_safe_external_image_url(
            "http://169.254.169.254/latest/meta-data.png"
        ));
        assert!(!is_safe_external_image_url("local:///Users/mac/test.png"));
    }

    #[test]
    fn test_parse_pinned_conversations_various_formats() {
        // Format 1: Direct array of string IDs
        let v1 = json!({
            "error_code": 0,
            "data": ["u10001", "g20002", "30003"]
        });
        assert_eq!(
            parse_pinned_conversations(&v1),
            vec![
                "u10001".to_string(),
                "g20002".to_string(),
                "30003".to_string()
            ]
        );

        // Format 2: Object with "conversations" or "pins" key
        let v2 = json!({
            "error_code": 0,
            "data": {
                "conversations": ["u10001", "g20002"]
            }
        });
        assert_eq!(
            parse_pinned_conversations(&v2),
            vec!["u10001".to_string(), "g20002".to_string()]
        );

        // Format 3: Array of objects with convId / id
        let v3 = json!({
            "error_code": 0,
            "data": [
                {"convId": "u10001", "pinned": true},
                {"id": "g20002", "pinned": true}
            ]
        });
        assert_eq!(
            parse_pinned_conversations(&v3),
            vec!["u10001".to_string(), "g20002".to_string()]
        );

        // Format 4: Stringified JSON in data
        let v4 = json!({
            "error_code": 0,
            "data": "[\"u999\", \"g888\"]"
        });
        assert_eq!(
            parse_pinned_conversations(&v4),
            vec!["u999".to_string(), "g888".to_string()]
        );
    }

    #[test]
    fn parse_board_topics_reads_ids_and_message_keys_from_object_and_string_data() {
        // `data` as an object with a `topics` array; `params` as a JSON string (server shape).
        let v1 = json!({
            "error_code": 0,
            "data": { "topics": [
                { "id": 111, "type": 2, "params": "{\"global_msg_id\":\"m-1\",\"client_msg_id\":\"c-1\"}" }
            ]}
        });
        let got = parse_board_topics(&v1);
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].id, "111");
        assert_eq!(got[0].msg_ids, vec!["m-1".to_string(), "c-1".to_string()]);

        // `data` as a JSON string, single topic object, numeric ids.
        let v2 = json!({
            "error_code": 0,
            "data": "{\"topic\":{\"topicId\":\"222\",\"params\":{\"global_msg_id\":777}}}"
        });
        let got = parse_board_topics(&v2);
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].id, "222");
        assert_eq!(got[0].msg_ids, vec!["777".to_string()]);

        // Empty board.
        assert!(parse_board_topics(&json!({ "error_code": 0, "data": {} })).is_empty());
    }

    #[test]
    fn test_is_pinned_in_list_handles_prefixes_and_types() {
        let pinned = vec![
            "u10001".to_string(),
            "g20002".to_string(),
            "30003".to_string(),
        ];

        // 1-1 user thread (is_group = false)
        assert!(is_pinned_in_list("10001", false, &pinned));
        assert!(is_pinned_in_list("u10001", false, &pinned));
        assert!(!is_pinned_in_list("20002", false, &pinned)); // g20002 should not match user 20002

        // Group thread (is_group = true)
        assert!(is_pinned_in_list("20002", true, &pinned));
        assert!(is_pinned_in_list("g20002", true, &pinned));
        assert!(is_pinned_in_list("group_20002", true, &pinned));
        assert!(!is_pinned_in_list("10001", true, &pinned)); // u10001 should not match group 10001

        // Plain ID matches both if unadorned
        assert!(is_pinned_in_list("30003", false, &pinned));
        assert!(is_pinned_in_list("30003", true, &pinned));
    }

    #[test]
    fn test_apply_pinned_threads_and_merge() {
        let t1 = Thread {
            id: "10001".into(),
            is_group: false,
            ..Default::default()
        };
        let t2 = Thread {
            id: "20002".into(),
            is_group: true,
            ..Default::default()
        };
        let t3 = Thread {
            id: "40004".into(),
            is_group: false,
            ..Default::default()
        };
        let mut threads = vec![t1, t2, t3];
        let pinned = vec!["u10001".to_string(), "g20002".to_string()];

        apply_pinned_threads(&mut threads, &pinned);
        assert!(threads[0].presentation.pinned);
        assert!(threads[1].presentation.pinned);
        assert!(!threads[2].presentation.pinned);

        // Verify merge_thread_into preserves pinned status
        let incoming_pinned = Thread {
            id: "40004".into(),
            presentation: SidebarPresentation {
                pinned: true,
                ..Default::default()
            },
            ..Default::default()
        };
        merge_thread_into(&mut threads[2], incoming_pinned, true);
        assert!(threads[2].presentation.pinned);
    }

    #[test]
    fn extract_mobile_sync_controls_parses_transfer_confirm_and_backup_ready() {
        let payload = json!({
            "controls": [
                {
                    "content": {
                        "act_type": "transfer_msg",
                        "act": "transfer_after_login",
                        "data": "{\"temp_key\":\"tk_login_999\"}"
                    }
                },
                {
                    "content": {
                        "act_type": "syncmsgmb",
                        "act": "user_confirm",
                        "data": { "user_action": 1, "pc_name": "DESKTOP" }
                    }
                },
                {
                    "content": {
                        "act_type": "syncmsgmb",
                        "act": "syncmsg_info",
                        "data": {
                            "url": "https://f1-zpc.zdn.vn/backup.zdb",
                            "file_name": "backup.zdb",
                            "encrypted_key": "enc_key_b64",
                            "checksum_code": "md5_123"
                        }
                    }
                }
            ]
        });
        let evs = extract_mobile_sync_controls(&payload);
        assert_eq!(evs.len(), 3);
        assert_eq!(
            evs[0],
            MobileSyncControlEvent::TransferAfterLogin {
                temp_key: "tk_login_999".into()
            }
        );
        assert_eq!(evs[1], MobileSyncControlEvent::UserConfirmed);
        assert_eq!(
            evs[2],
            MobileSyncControlEvent::BackupReady {
                url: "https://f1-zpc.zdn.vn/backup.zdb".into(),
                file_name: "backup.zdb".into(),
                encrypted_key: "enc_key_b64".into(),
                checksum: "md5_123".into(),
            }
        );
    }
}
