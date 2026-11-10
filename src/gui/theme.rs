use super::*;

// ---------------------------------------------------------------------------
// Semantic Design System Tokens (UI/UX Pro Max — Minimalism & Swiss Style)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy)]
pub(super) struct ThemePalette {
    pub(super) is_dark: bool,
    pub(super) bg_app: Color32,
    pub(super) bg_sidebar: Color32,
    pub(super) bg_chat_canvas: Color32,
    pub(super) bg_elevated: Color32,
    pub(super) bg_hover: Color32,
    pub(super) bg_selected: Color32,
    pub(super) bg_input: Color32,
    pub(super) border: Color32,
    pub(super) text_primary: Color32,
    pub(super) text_secondary: Color32,
    pub(super) text_muted: Color32,
    pub(super) primary: Color32,
    pub(super) primary_hover: Color32,
    pub(super) accent_online: Color32,
    pub(super) danger: Color32,
    pub(super) warning: Color32,
    pub(super) bubble_out_bg: Color32,
    pub(super) bubble_out_text: Color32,
    pub(super) bubble_out_meta: Color32,
    pub(super) bubble_in_bg: Color32,
    pub(super) bubble_in_border: Color32,
    pub(super) bubble_in_text: Color32,
    pub(super) bubble_in_meta: Color32,
}

impl ThemePalette {
    pub(super) fn dark() -> Self {
        // Official Atom One Dark (`one-dark-ui` + `one-dark-syntax`) color system
        Self {
            is_dark: true,
            bg_app: Color32::from_rgb(24, 26, 31), // #181A1F Activity Bar & Title Bar
            bg_sidebar: Color32::from_rgb(33, 37, 43), // #21252B Tree-View / Sidebar / Header
            bg_chat_canvas: Color32::from_rgb(40, 44, 52), // #282C34 Atom One Dark Editor Canvas
            bg_elevated: Color32::from_rgb(44, 49, 58), // #2C313A Elevated Card / Modal / Popover
            bg_hover: Color32::from_rgb(50, 56, 66), // #323842 List Hover Surface
            bg_selected: Color32::from_rgb(58, 63, 75), // #3A3F4B Active Selection Row
            bg_input: Color32::from_rgb(29, 31, 35), // #1D1F23 Input / Search Well
            border: Color32::from_rgb(52, 57, 66), // #343942 Subtle Panel Divider
            text_primary: Color32::from_rgb(215, 218, 224), // #D7DAE0 Bright Foreground
            text_secondary: Color32::from_rgb(171, 178, 191), // #ABB2BF Atom @mono-1
            text_muted: Color32::from_rgb(118, 126, 142), // #767E8E Atom @mono-3 (readable muted)
            primary: Color32::from_rgb(82, 139, 255), // #528BFF Atom UI Accent Blue
            primary_hover: Color32::from_rgb(97, 175, 239), // #61AFEF Atom @hue-2 Cyan-Blue
            accent_online: Color32::from_rgb(152, 195, 121), // #98C379 Atom @hue-4 Green
            danger: Color32::from_rgb(224, 108, 117), // #E06C75 Atom @hue-5 Coral Red
            warning: Color32::from_rgb(229, 192, 123), // #E5C07B Atom @hue-62 Warm Gold
            bubble_out_bg: Color32::from_rgb(37, 55, 77), // #25374D Zalo Dark Slate-Navy Outgoing Bubble
            bubble_out_text: Color32::from_rgb(235, 240, 246), // #EBF0F6
            bubble_out_meta: Color32::from_rgb(154, 171, 192), // #9AABC0
            bubble_in_bg: Color32::from_rgb(50, 56, 66),  // #323842 Atom Elevated Incoming Bubble
            bubble_in_border: Color32::from_rgb(62, 68, 81), // #3E4451 Atom Gutter Border
            bubble_in_text: Color32::from_rgb(215, 218, 224), // #D7DAE0
            bubble_in_meta: Color32::from_rgb(142, 150, 165), // #8E96A5
        }
    }

    pub(super) fn light() -> Self {
        Self {
            is_dark: false,
            bg_app: Color32::from_rgb(255, 255, 255), // #FFFFFF
            bg_sidebar: Color32::from_rgb(255, 255, 255), // #FFFFFF
            bg_chat_canvas: Color32::from_rgb(235, 240, 247), // Authentic Zalo PC cool blue-gray canvas
            bg_elevated: Color32::from_rgb(255, 255, 255),    // #FFFFFF
            bg_hover: Color32::from_rgb(241, 245, 249),       // #F1F5F9
            bg_selected: Color32::from_rgb(229, 241, 255),    // #E5F1FF Zalo active row
            bg_input: Color32::from_rgb(248, 250, 252),       // #F8FAFC
            border: Color32::from_rgb(226, 232, 240),         // #E2E8F0
            text_primary: Color32::from_rgb(15, 23, 42),      // #0F172A (16.1:1 contrast)
            text_secondary: Color32::from_rgb(71, 85, 105),   // #475569 (7.2:1 contrast)
            text_muted: Color32::from_rgb(100, 116, 139),     // #64748B (4.6:1 contrast)
            primary: Color32::from_rgb(0, 104, 255),          // #0068FF Zalo Blue
            primary_hover: Color32::from_rgb(37, 99, 235),    // #2563EB
            accent_online: Color32::from_rgb(5, 150, 105),    // #059669 Emerald
            danger: Color32::from_rgb(220, 38, 38),           // #DC2626
            warning: Color32::from_rgb(217, 119, 6),          // #D97706
            bubble_out_bg: Color32::from_rgb(229, 239, 255), // Zalo Desktop Light Outgoing (#E5EFFF)
            bubble_out_text: Color32::from_rgb(15, 23, 42),  // Crisp dark text on light blue (14:1)
            bubble_out_meta: Color32::from_rgb(71, 92, 122),
            bubble_in_bg: Color32::WHITE,
            bubble_in_border: Color32::from_rgb(216, 224, 236),
            bubble_in_text: Color32::from_rgb(15, 23, 42),
            bubble_in_meta: Color32::from_rgb(100, 116, 139),
        }
    }

    pub(super) fn apply_to_ctx(&self, ctx: &egui::Context) {
        let mut style = (*ctx.style()).clone();
        let mut visuals = if self.is_dark {
            egui::Visuals::dark()
        } else {
            egui::Visuals::light()
        };

        visuals.panel_fill = self.bg_app;
        visuals.window_fill = self.bg_elevated;
        visuals.extreme_bg_color = self.bg_input;
        visuals.faint_bg_color = self.bg_hover;
        visuals.override_text_color = Some(self.text_primary);
        visuals.selection.bg_fill =
            self.primary
                .linear_multiply(if self.is_dark { 0.45 } else { 0.22 });
        visuals.selection.stroke = Stroke::new(1.5_f32, self.primary);

        let r = Rounding::same(8.0);
        visuals.widgets.noninteractive.bg_fill = self.bg_app;
        visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, self.border);
        visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, self.text_primary);
        visuals.widgets.noninteractive.rounding = r;

        visuals.widgets.inactive.bg_fill = self.bg_elevated;
        visuals.widgets.inactive.weak_bg_fill = self.bg_elevated;
        visuals.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, self.border);
        visuals.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, self.text_primary);
        visuals.widgets.inactive.rounding = r;

        visuals.widgets.hovered.bg_fill = self.bg_hover;
        visuals.widgets.hovered.weak_bg_fill = self.bg_hover;
        visuals.widgets.hovered.bg_stroke = Stroke::new(1.2_f32, self.primary);
        visuals.widgets.hovered.fg_stroke = Stroke::new(1.2_f32, self.text_primary);
        visuals.widgets.hovered.rounding = r;

        visuals.widgets.active.bg_fill = self.primary;
        visuals.widgets.active.weak_bg_fill = self.primary;
        visuals.widgets.active.bg_stroke = Stroke::new(1.5_f32, self.primary);
        visuals.widgets.active.fg_stroke = Stroke::new(1.5_f32, Color32::WHITE);
        visuals.widgets.active.rounding = r;

        style.spacing.item_spacing = Vec2::new(8.0, 6.0);
        style.spacing.button_padding = Vec2::new(10.0, 6.0);
        style.spacing.scroll = egui::style::ScrollStyle::solid();
        style.spacing.scroll.bar_width = 6.0;
        style.visuals = visuals;
        ctx.set_style(style);
    }

    /// Load either the built-in dark/light palette or apply custom overrides from `theme.json` (#7).
    pub(super) fn load_with_custom_override(dark_mode: bool) -> Self {
        let mut base = if dark_mode {
            Self::dark()
        } else {
            Self::light()
        };
        if let Ok(raw) = std::fs::read_to_string("theme.json") {
            base.apply_json_overrides(&raw);
        }
        base
    }

    /// Apply any `#RRGGBB` color overrides from a JSON string (`theme.json`) (#7).
    pub(super) fn apply_json_overrides(&mut self, json_text: &str) {
        let Ok(val) = serde_json::from_str::<serde_json::Value>(json_text) else {
            return;
        };
        let sub = if self.is_dark {
            val.get("dark").unwrap_or(&val)
        } else {
            val.get("light").unwrap_or(&val)
        };
        for (key, target) in [
            ("bg_app", &mut self.bg_app),
            ("bg_sidebar", &mut self.bg_sidebar),
            ("bg_chat_canvas", &mut self.bg_chat_canvas),
            ("bg_elevated", &mut self.bg_elevated),
            ("bg_hover", &mut self.bg_hover),
            ("bg_selected", &mut self.bg_selected),
            ("bg_input", &mut self.bg_input),
            ("border", &mut self.border),
            ("text_primary", &mut self.text_primary),
            ("text_secondary", &mut self.text_secondary),
            ("text_muted", &mut self.text_muted),
            ("primary", &mut self.primary),
            ("primary_hover", &mut self.primary_hover),
            ("accent_online", &mut self.accent_online),
            ("danger", &mut self.danger),
            ("warning", &mut self.warning),
            ("bubble_out_bg", &mut self.bubble_out_bg),
            ("bubble_out_text", &mut self.bubble_out_text),
            ("bubble_out_meta", &mut self.bubble_out_meta),
            ("bubble_in_bg", &mut self.bubble_in_bg),
            ("bubble_in_border", &mut self.bubble_in_border),
            ("bubble_in_text", &mut self.bubble_in_text),
            ("bubble_in_meta", &mut self.bubble_in_meta),
        ] {
            if let Some(hex) = sub.get(key).and_then(|v| v.as_str()) {
                if let Some(c) = parse_hex_color(hex) {
                    *target = c;
                }
            }
        }
    }

    /// Export a sample `theme.json` containing both `dark` and `light` token maps (#7).
    pub(super) fn export_template(path: &str) -> std::io::Result<()> {
        let d = Self::dark();
        let l = Self::light();
        let to_map = |p: &Self| {
            serde_json::json!({
                "bg_app": format_hex_color(p.bg_app),
                "bg_sidebar": format_hex_color(p.bg_sidebar),
                "bg_chat_canvas": format_hex_color(p.bg_chat_canvas),
                "bg_elevated": format_hex_color(p.bg_elevated),
                "bg_hover": format_hex_color(p.bg_hover),
                "bg_selected": format_hex_color(p.bg_selected),
                "bg_input": format_hex_color(p.bg_input),
                "border": format_hex_color(p.border),
                "text_primary": format_hex_color(p.text_primary),
                "text_secondary": format_hex_color(p.text_secondary),
                "text_muted": format_hex_color(p.text_muted),
                "primary": format_hex_color(p.primary),
                "primary_hover": format_hex_color(p.primary_hover),
                "accent_online": format_hex_color(p.accent_online),
                "danger": format_hex_color(p.danger),
                "warning": format_hex_color(p.warning),
                "bubble_out_bg": format_hex_color(p.bubble_out_bg),
                "bubble_out_text": format_hex_color(p.bubble_out_text),
                "bubble_out_meta": format_hex_color(p.bubble_out_meta),
                "bubble_in_bg": format_hex_color(p.bubble_in_bg),
                "bubble_in_border": format_hex_color(p.bubble_in_border),
                "bubble_in_text": format_hex_color(p.bubble_in_text),
                "bubble_in_meta": format_hex_color(p.bubble_in_meta),
            })
        };
        let doc = serde_json::json!({
            "_comment": "ctdozlite custom theme file (#7). Edit hex colors (#RRGGBB) under dark/light and reload in Settings.",
            "dark": to_map(&d),
            "light": to_map(&l),
        });
        let text = serde_json::to_string_pretty(&doc).unwrap_or_default();
        std::fs::write(path, text)
    }
}

pub(super) fn parse_hex_color(s: &str) -> Option<Color32> {
    let hex = s.trim().trim_start_matches('#');
    if hex.len() == 6 {
        let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
        let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
        let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
        Some(Color32::from_rgb(r, g, b))
    } else {
        None
    }
}

pub(super) fn format_hex_color(c: Color32) -> String {
    format!("#{:02X}{:02X}{:02X}", c.r(), c.g(), c.b())
}

// ---------------------------------------------------------------------------
// Zero-allocation Vector Icon Painter Primitives (Replaces Emoji Icons)
// ---------------------------------------------------------------------------

pub(super) fn avatar_color_for(name: &str, is_group: bool) -> Color32 {
    const PALETTE: &[Color32] = &[
        Color32::from_rgb(64, 120, 192), // Atom Blue (#4078C0)
        Color32::from_rgb(56, 148, 160), // Atom Cyan (#3894A0)
        Color32::from_rgb(152, 88, 178), // Atom Purple (#9858B2)
        Color32::from_rgb(190, 80, 70),  // Atom Red (#BE5046)
        Color32::from_rgb(184, 118, 62), // Atom Orange (#B8763E)
        Color32::from_rgb(98, 148, 72),  // Atom Green (#629448)
        Color32::from_rgb(82, 116, 196), // Atom Indigo
        Color32::from_rgb(72, 138, 182), // Atom Cerulean
    ];
    let mut hash: usize = if is_group { 3 } else { 0 };
    for b in name.bytes() {
        hash = hash.wrapping_mul(31).wrapping_add(b as usize);
    }
    PALETTE[hash % PALETTE.len()]
}

/// Strip invisible Unicode Variation Selectors (`U+FE0F` / `U+FE0E`) that cause broken square
/// missing-glyph boxes on Linux desktop font engines when rendering iPhone emojis.
pub fn clean_emoji_presentation(s: &str) -> std::borrow::Cow<'_, str> {
    if s.contains('\u{FE0F}') || s.contains('\u{FE0E}') {
        std::borrow::Cow::Owned(s.replace(['\u{FE0F}', '\u{FE0E}'], ""))
    } else {
        std::borrow::Cow::Borrowed(s)
    }
}

pub(super) fn truncate_chars<'a>(s: &'a str, max_chars: usize) -> std::borrow::Cow<'a, str> {
    let clean = clean_emoji_presentation(s);
    if let Some((byte_idx, _)) = clean.char_indices().nth(max_chars) {
        std::borrow::Cow::Owned(format!("{}…", &clean[..byte_idx]))
    } else {
        clean
    }
}

/// Extract up to 2 uppercase alphanumeric initials from a display name (e.g. `"Nguyễn An"` -> `"NA"`, `"Hoang"` -> `"H"`).
pub(super) fn avatar_initials(name: &str) -> String {
    let clean_words: Vec<&str> = name
        .split_whitespace()
        .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()))
        .filter(|w| !w.is_empty())
        .collect();
    if clean_words.len() >= 2 {
        let c1 = clean_words[0].chars().next().unwrap_or('Z').to_uppercase();
        let c2 = clean_words[clean_words.len() - 1]
            .chars()
            .next()
            .unwrap_or('U')
            .to_uppercase();
        format!("{c1}{c2}")
    } else if let Some(first) = clean_words.first() {
        first
            .chars()
            .next()
            .unwrap_or('Z')
            .to_uppercase()
            .to_string()
    } else {
        "Z".to_string()
    }
}

/// Fold Vietnamese text to lowercase ASCII for diacritic-insensitive search
/// (e.g. `"Nguyễn Văn An"` -> `"nguyen van an"`).
pub(super) fn fold_vn_search(s: &str) -> String {
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
                other => other,
            };
            out.push(mapped);
        }
    }
    out
}

/// Check whether `haystack` matches `query_lower` (or `query_folded` without Vietnamese accents).
pub(super) fn matches_vn_query(haystack: &str, query_lower: &str, query_folded: &str) -> bool {
    if query_lower.is_empty() {
        return true;
    }
    // Fast path: most matches are plain ASCII. This avoids allocating a
    // lowercased copy of `haystack` (and a folded copy on failure) for every
    // message on every keystroke while in-chat search is active.
    if contains_ignore_ascii_case(haystack, query_lower) {
        return true;
    }
    !query_folded.is_empty() && fold_vn_search(haystack).contains(query_folded)
}

/// ASCII case-insensitive substring check without allocating.
fn contains_ignore_ascii_case(haystack: &str, needle: &str) -> bool {
    let needle = needle.as_bytes();
    let haystack = haystack.as_bytes();
    if needle.is_empty() {
        return true;
    }
    haystack
        .windows(needle.len())
        .any(|window| window.eq_ignore_ascii_case(needle))
}

fn paint_circular_image(painter: &egui::Painter, center: Pos2, radius: f32, tex: &TextureHandle) {
    let [tw, th] = tex.size();
    let (uv_min, uv_max) = if tw == th || tw == 0 || th == 0 {
        (Pos2::ZERO, Pos2::new(1.0, 1.0))
    } else if tw > th {
        let crop = (tw - th) as f32 / (2.0 * tw as f32);
        (Pos2::new(crop, 0.0), Pos2::new(1.0 - crop, 1.0))
    } else {
        let crop = (th - tw) as f32 / (2.0 * th as f32);
        (Pos2::new(0.0, crop), Pos2::new(1.0, 1.0 - crop))
    };

    let mut mesh = egui::Mesh::with_texture(tex.id());
    let n_pts = 32;
    mesh.vertices.push(egui::epaint::Vertex {
        pos: center,
        uv: Pos2::new((uv_min.x + uv_max.x) * 0.5, (uv_min.y + uv_max.y) * 0.5),
        color: Color32::WHITE,
    });
    for i in 0..=n_pts {
        let angle = (i as f32) / (n_pts as f32) * std::f32::consts::TAU;
        let (sin_a, cos_a) = angle.sin_cos();
        let pos = Pos2::new(center.x + radius * cos_a, center.y + radius * sin_a);
        let u = uv_min.x + (cos_a * 0.5 + 0.5) * (uv_max.x - uv_min.x);
        let v = uv_min.y + (sin_a * 0.5 + 0.5) * (uv_max.y - uv_min.y);
        mesh.vertices.push(egui::epaint::Vertex {
            pos,
            uv: Pos2::new(u, v),
            color: Color32::WHITE,
        });
        if i > 0 {
            mesh.indices.push(0);
            mesh.indices.push(i as u32);
            mesh.indices.push((i + 1) as u32);
        }
    }
    painter.add(egui::Shape::mesh(mesh));
}

fn paint_single_circle_avatar(
    painter: &egui::Painter,
    center: Pos2,
    radius: f32,
    name: &str,
    is_group: bool,
    avatar_url: Option<&str>,
    textures: &HashMap<String, TextureHandle>,
    loading: &mut HashSet<String>,
) {
    if let Some(url) = avatar_url.map(str::trim).filter(|u| {
        !u.is_empty()
            && !u.ends_with("/default")
            && (u.starts_with("http://") || u.starts_with("https://"))
    }) {
        let file_or_base = crate::chat::avatar_file_key(Some(url))
            .unwrap_or_else(|| url.split('?').next().unwrap_or(url).to_string());
        let key = format!("ava_{}", crate::crypto::md5_hex(&file_or_base));
        if let Some(tex) = textures.get(&key) {
            paint_circular_image(painter, center, radius, tex);
            painter.circle_stroke(
                center,
                radius,
                Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(255, 255, 255, 34)),
            );
            return;
        }
        if loading.insert(key.clone()) {
            if let Some(tx) = WORKER_TX.get() {
                let _ = tx.send(WorkerCmd::FetchStickerImage {
                    key,
                    url: url.to_string(),
                });
            }
        }
    }

    let av_col = avatar_color_for(name, is_group);
    painter.circle_filled(center, radius, av_col);
    let initial = if radius >= 15.5 {
        avatar_initials(name)
    } else {
        name.trim_matches(|c: char| !c.is_alphanumeric())
            .chars()
            .next()
            .unwrap_or('Z')
            .to_uppercase()
            .to_string()
    };
    painter.text(
        center,
        egui::Align2::CENTER_CENTER,
        initial,
        FontId::proportional((radius * 0.76).max(8.5)),
        Color32::WHITE,
    );
}

/// Paint a circular user or group avatar:
/// - For 1-1 contacts or groups with a custom uploaded group photo (`http(s)://...`), renders the circular image.
/// - For groups without a single custom photo (`multi:...` or `None`), renders Zalo PC's `zavatar-multi`
///   composite layout (3-member triangle or 2x2 member grid with optional member count badge).
pub(super) fn paint_user_avatar(
    painter: &egui::Painter,
    center: Pos2,
    radius: f32,
    name: &str,
    is_group: bool,
    avatar_url: Option<&str>,
    textures: &HashMap<String, TextureHandle>,
    loading: &mut HashSet<String>,
) {
    let raw_ava = avatar_url.map(str::trim).unwrap_or("");
    let has_single_custom_photo = !raw_ava.is_empty()
        && !raw_ava.starts_with("multi:")
        && !raw_ava.ends_with("/default")
        && (raw_ava.starts_with("http://") || raw_ava.starts_with("https://"));

    if !is_group || has_single_custom_photo {
        paint_single_circle_avatar(
            painter, center, radius, name, is_group, avatar_url, textures, loading,
        );
        return;
    }

    // Parse composite group member slots (`multi:<total>;<name1>|<url1>;<name2>|<url2>;...`)
    let mut slots: Vec<(String, String)> = Vec::new();
    let mut total_members: usize = 0;
    if let Some(rest) = raw_ava.strip_prefix("multi:") {
        for (idx, part) in rest.split(';').enumerate() {
            if idx == 0 && !part.contains('|') {
                total_members = part.trim().parse::<usize>().unwrap_or(0);
                continue;
            }
            if let Some((m_name, m_url)) = part.split_once('|') {
                slots.push((m_name.trim().to_string(), m_url.trim().to_string()));
            } else if !part.trim().is_empty() {
                slots.push((part.trim().to_string(), String::new()));
            }
        }
    }

    // Nếu không có slots hoặc tất cả các slot đều không có ảnh URL:
    // Vẽ avatar nhóm chuẩn đơn với màu sắc và tên viết tắt đẹp mắt, không cắt ghép chữ li ti
    let has_any_photo = slots.iter().any(|(_, u)| !u.is_empty());
    if !has_any_photo {
        paint_single_circle_avatar(painter, center, radius, name, true, None, textures, loading);
        return;
    }

    if total_members < slots.len() {
        total_members = slots.len();
    }

    let border_col = Color32::from_rgb(26, 30, 38);
    // Vẽ vòng tròn nền lớn chuẩn bán kính R để toàn bộ avatar nhóm nằm gọn trong hình tròn
    painter.circle_filled(center, radius, border_col);
    painter.circle_stroke(
        center,
        radius,
        Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(255, 255, 255, 24)),
    );

    let n_slots = slots.len().min(4);
    if n_slots == 2 {
        let sub_r = radius * 0.46;
        let delta = radius * 0.25;
        let p0 = Pos2::new(center.x - delta, center.y + delta);
        let p1 = Pos2::new(center.x + delta, center.y - delta);
        for &(pos, idx) in &[(p1, 1usize), (p0, 0usize)] {
            if let Some((ref m_name, ref m_url)) = slots.get(idx) {
                painter.circle_filled(pos, sub_r + 1.2, border_col);
                paint_single_circle_avatar(
                    painter,
                    pos,
                    sub_r,
                    m_name,
                    false,
                    if m_url.is_empty() {
                        None
                    } else {
                        Some(m_url.as_str())
                    },
                    textures,
                    loading,
                );
            }
        }
    } else if n_slots == 3 && total_members <= 3 {
        // Zalo PC `zavatar-multi.zavatar-3`: 1 top-center, 1 bottom-left, 1 bottom-right
        let sub_r = radius * 0.42;
        let p_top = Pos2::new(center.x, center.y - radius * 0.40);
        let p_bl = Pos2::new(center.x - radius * 0.38, center.y + radius * 0.34);
        let p_br = Pos2::new(center.x + radius * 0.38, center.y + radius * 0.34);
        for &(pos, idx) in &[(p_br, 2usize), (p_bl, 1usize), (p_top, 0usize)] {
            if let Some((ref m_name, ref m_url)) = slots.get(idx) {
                painter.circle_filled(pos, sub_r + 1.2, border_col);
                paint_single_circle_avatar(
                    painter,
                    pos,
                    sub_r,
                    m_name,
                    false,
                    if m_url.is_empty() {
                        None
                    } else {
                        Some(m_url.as_str())
                    },
                    textures,
                    loading,
                );
            }
        }
    } else {
        // Zalo PC `zavatar-multi.zavatar-4`: 2x2 grid (với ô đếm số thành viên còn lại ở góc dưới-phải nếu > 4)
        let sub_r = radius * 0.38;
        let delta = radius * 0.38;
        let p_tl = Pos2::new(center.x - delta, center.y - delta);
        let p_tr = Pos2::new(center.x + delta, center.y - delta);
        let p_bl = Pos2::new(center.x - delta, center.y + delta);
        let p_br = Pos2::new(center.x + delta, center.y + delta);

        let show_counter = total_members > 4;
        if show_counter {
            painter.circle_filled(p_br, sub_r + 1.2, border_col);
            painter.circle_filled(p_br, sub_r, Color32::from_rgb(52, 60, 76));
            let cnt_txt = if total_members > 99 {
                "99+".to_string()
            } else {
                format!("+{}", total_members.saturating_sub(3))
            };
            painter.text(
                p_br,
                egui::Align2::CENTER_CENTER,
                cnt_txt,
                FontId::proportional((sub_r * 0.76).max(8.0)),
                Color32::from_rgb(220, 228, 242),
            );
        } else if let Some((ref m_name, ref m_url)) = slots.get(3) {
            painter.circle_filled(p_br, sub_r + 1.2, border_col);
            paint_single_circle_avatar(
                painter,
                p_br,
                sub_r,
                m_name,
                false,
                if m_url.is_empty() {
                    None
                } else {
                    Some(m_url.as_str())
                },
                textures,
                loading,
            );
        }

        for &(pos, idx) in &[(p_bl, 2usize), (p_tr, 1usize), (p_tl, 0usize)] {
            if let Some((ref m_name, ref m_url)) = slots.get(idx) {
                painter.circle_filled(pos, sub_r + 1.2, border_col);
                paint_single_circle_avatar(
                    painter,
                    pos,
                    sub_r,
                    m_name,
                    false,
                    if m_url.is_empty() {
                        None
                    } else {
                        Some(m_url.as_str())
                    },
                    textures,
                    loading,
                );
            }
        }
    }
}

pub mod lucide {
    pub const SEARCH: &str = "\u{e151}";
    pub const REFRESH: &str = "\u{e145}";
    pub const SMILE: &str = "\u{e164}";
    pub const SEND: &str = "\u{e4f2}";
    pub const SUN: &str = "\u{e178}";
    pub const MOON: &str = "\u{e11e}";
    pub const PHONE: &str = "\u{e133}";
    pub const PHONE_CALL: &str = "\u{e134}";
    pub const PHONE_OFF: &str = "\u{e138}";
    pub const VIDEO: &str = "\u{e1a5}";
    pub const VIDEO_OFF: &str = "\u{e1a6}";
    pub const CAMERA: &str = "\u{e062}";
    pub const IMAGE: &str = "\u{e0f6}";
    pub const PAPERCLIP: &str = "\u{e12d}";
    pub const CHECK: &str = "\u{e06c}";
    pub const CHECK_CHECK: &str = "\u{e38e}";
    pub const MESSAGE_SQUARE: &str = "\u{e117}";
    pub const USERS: &str = "\u{e1a4}";
    pub const USER_PLUS: &str = "\u{e1a2}";
    pub const CLOUD: &str = "\u{e088}";
    pub const SETTINGS: &str = "\u{e154}";
    pub const PANEL_RIGHT: &str = "\u{e431}";
    pub const PIN: &str = "\u{e259}";
    pub const COPY: &str = "\u{e09e}";
    pub const STAR: &str = "\u{e176}";
    pub const GLOBE: &str = "\u{e0e8}";
    pub const LINK: &str = "\u{e102}";
    pub const EXTERNAL_LINK: &str = "\u{e0b9}";
    pub const X: &str = "\u{e1b2}";
    pub const PLUS: &str = "\u{e13d}";
    pub const MINUS: &str = "\u{e11c}";
    pub const STORE: &str = "\u{e3e4}";
    pub const FLAME: &str = "\u{e0d2}";
    pub const SPARKLES: &str = "\u{e412}";
    pub const BAN: &str = "\u{e051}";
    pub const MIC: &str = "\u{e118}";
    pub const MIC_OFF: &str = "\u{e119}";
    pub const LOG_OUT: &str = "\u{e10e}";
    pub const FILE_TEXT: &str = "\u{e0cc}";
    pub const FILM: &str = "\u{e0d0}";
    pub const SHIELD_CHECK: &str = "\u{e15c}";
    pub const LOCK: &str = "\u{e10b}";
    pub const BELL: &str = "\u{e059}";
    pub const PAINTBRUSH: &str = "\u{e2e7}";
    pub const SLIDERS: &str = "\u{e29a}";
    pub const FOLDER_OPEN: &str = "\u{e247}";
    pub const CHEVRON_RIGHT: &str = "\u{e06f}";
    pub const CHEVRON_DOWN: &str = "\u{e06d}";
    pub const USER: &str = "\u{e19f}";
    pub const NEWSPAPER: &str = "\u{e124}";
    pub const QUOTE: &str = "\u{e239}";
    pub const FORWARD: &str = "\u{e229}";
    pub const SHARE_2: &str = "\u{e156}";
    pub const TRASH_2: &str = "\u{e18e}";
    pub const INFO: &str = "\u{e0f9}";
}

pub(super) fn draw_lucide_icon(
    painter: &egui::Painter,
    center: Pos2,
    icon: &str,
    size: f32,
    color: Color32,
) {
    painter.text(
        center,
        egui::Align2::CENTER_CENTER,
        icon,
        FontId::proportional(size),
        color,
    );
}

pub(super) fn draw_icon_search(painter: &egui::Painter, center: Pos2, size: f32, color: Color32) {
    draw_lucide_icon(painter, center, lucide::SEARCH, size, color);
}

pub(super) fn draw_icon_refresh(painter: &egui::Painter, center: Pos2, size: f32, color: Color32) {
    draw_lucide_icon(painter, center, lucide::REFRESH, size, color);
}

pub(super) fn draw_icon_sticker(painter: &egui::Painter, center: Pos2, size: f32, color: Color32) {
    draw_lucide_icon(painter, center, lucide::SMILE, size, color);
}

pub(super) fn draw_icon_send(painter: &egui::Painter, center: Pos2, size: f32, color: Color32) {
    draw_lucide_icon(painter, center, lucide::SEND, size, color);
}

pub(super) fn draw_icon_phone(painter: &egui::Painter, center: Pos2, size: f32, color: Color32) {
    draw_lucide_icon(painter, center, lucide::PHONE, size, color);
}

pub(super) fn draw_icon_video(painter: &egui::Painter, center: Pos2, size: f32, color: Color32) {
    draw_lucide_icon(painter, center, lucide::VIDEO, size, color);
}

pub(super) fn draw_icon_image(painter: &egui::Painter, center: Pos2, size: f32, color: Color32) {
    draw_lucide_icon(painter, center, lucide::IMAGE, size, color);
}

pub(super) fn draw_icon_camera(painter: &egui::Painter, center: Pos2, size: f32, color: Color32) {
    draw_lucide_icon(painter, center, lucide::CAMERA, size, color);
}

pub(super) fn draw_icon_attachment(
    painter: &egui::Painter,
    center: Pos2,
    size: f32,
    color: Color32,
) {
    draw_lucide_icon(painter, center, lucide::PAPERCLIP, size, color);
}

pub(super) fn draw_icon_chat_nav(painter: &egui::Painter, center: Pos2, size: f32, color: Color32) {
    draw_lucide_icon(painter, center, lucide::MESSAGE_SQUARE, size, color);
}

pub(super) fn draw_icon_contacts(painter: &egui::Painter, center: Pos2, size: f32, color: Color32) {
    draw_lucide_icon(painter, center, lucide::USERS, size, color);
}

pub(super) fn draw_icon_cloud(painter: &egui::Painter, center: Pos2, size: f32, color: Color32) {
    draw_lucide_icon(painter, center, lucide::CLOUD, size, color);
}

pub(super) fn draw_icon_settings(painter: &egui::Painter, center: Pos2, size: f32, color: Color32) {
    draw_lucide_icon(painter, center, lucide::SETTINGS, size, color);
}
pub(super) fn draw_icon_feed(painter: &egui::Painter, center: Pos2, size: f32, color: Color32) {
    draw_lucide_icon(painter, center, lucide::NEWSPAPER, size, color);
}

pub(super) fn draw_icon_drawer(painter: &egui::Painter, center: Pos2, size: f32, color: Color32) {
    draw_lucide_icon(painter, center, lucide::PANEL_RIGHT, size, color);
}

pub(super) fn draw_icon_pin(painter: &egui::Painter, center: Pos2, size: f32, color: Color32) {
    draw_lucide_icon(painter, center, lucide::PIN, size, color);
}

pub(super) fn draw_icon_copy(painter: &egui::Painter, center: Pos2, size: f32, color: Color32) {
    draw_lucide_icon(painter, center, lucide::COPY, size, color);
}

pub(super) fn draw_icon_user_plus(
    painter: &egui::Painter,
    center: Pos2,
    size: f32,
    color: Color32,
) {
    draw_lucide_icon(painter, center, lucide::USER_PLUS, size, color);
}

pub const REACTION_LIST: [(&str, &str, &str); 6] = [
    ("👍", "like", "Thích"),
    ("❤️", "heart", "Yêu thích"),
    ("😆", "smile", "Haha"),
    ("😮", "surprised", "Wow"),
    ("😢", "cry", "Buồn"),
    ("😠", "rage", "Phẫn nộ"),
];

pub fn emoji_to_tex_key(emoji: &str) -> &'static str {
    match emoji {
        "👍" | "/-strong" => "like",
        "❤️" | "❤" | "/-heart" => "heart",
        "😆" | ":>" => "smile",
        "😮" | ":o" => "surprised",
        "😢" | ":-(( " | ":-((" => "cry",
        "😠" | ":-h" => "rage",
        _ => "like",
    }
}

pub(super) const APP_ICON_PNG: &[u8] = include_bytes!("../assets/app_icon.png");

/// Decode the embedded 256x256 RGBA application icon for the OS window/taskbar (`ViewportBuilder::with_icon`).
pub fn load_app_icon() -> Option<egui::IconData> {
    let img = image::load_from_memory(APP_ICON_PNG).ok()?.into_rgba8();
    let (width, height) = img.dimensions();
    Some(egui::IconData {
        rgba: img.into_raw(),
        width,
        height,
    })
}

/// Render a dynamic 64x64 RGBA window/taskbar icon with a crisp anti-aliased red unread badge
/// (`1`..`99+`) using a 3x5 bitmap pixel font renderer.
pub fn load_app_icon_with_badge(unread_count: usize) -> Option<egui::IconData> {
    if unread_count == 0 {
        return load_app_icon();
    }
    let base = image::load_from_memory(APP_ICON_PNG).ok()?;
    let size: u32 = 64;
    let mut rgba = image::imageops::resize(
        &base.to_rgba8(),
        size,
        size,
        image::imageops::FilterType::Lanczos3,
    )
    .into_raw();

    let label = if unread_count > 99 {
        "99+".to_string()
    } else {
        unread_count.to_string()
    };
    let chars: Vec<char> = label.chars().collect();

    let blend_pixel = |buf: &mut [u8], x: i32, y: i32, r: u8, g: u8, b: u8, alpha: u8| {
        if x < 0 || y < 0 || x >= size as i32 || y >= size as i32 || alpha == 0 {
            return;
        }
        let idx = ((y as u32 * size + x as u32) * 4) as usize;
        let src_a = alpha as f32 / 255.0;
        let dst_a = buf[idx + 3] as f32 / 255.0;
        let out_a = src_a + dst_a * (1.0 - src_a);
        if out_a <= 0.0 {
            return;
        }
        buf[idx] = (((r as f32) * src_a + (buf[idx] as f32) * dst_a * (1.0 - src_a)) / out_a)
            .round()
            .clamp(0.0, 255.0) as u8;
        buf[idx + 1] = (((g as f32) * src_a + (buf[idx + 1] as f32) * dst_a * (1.0 - src_a))
            / out_a)
            .round()
            .clamp(0.0, 255.0) as u8;
        buf[idx + 2] = (((b as f32) * src_a + (buf[idx + 2] as f32) * dst_a * (1.0 - src_a))
            / out_a)
            .round()
            .clamp(0.0, 255.0) as u8;
        buf[idx + 3] = (out_a * 255.0).round().clamp(0.0, 255.0) as u8;
    };

    let scale: f32 = 2.0;
    let badge_radius: f32 = 6.5 * scale;
    let half_span: f32 = if chars.len() >= 3 {
        4.0 * scale
    } else if chars.len() == 2 {
        2.0 * scale
    } else {
        0.0
    };
    let border_w: f32 = 1.2 * scale;
    let cx: f32 = (size as f32) - badge_radius - half_span - border_w.ceil();
    let cy: f32 = badge_radius + border_w.floor();

    let min_y = ((cy - badge_radius - border_w - 1.0).floor() as i32).max(0);
    let max_y = ((cy + badge_radius + border_w + 2.0).ceil() as i32).min(size as i32);
    let min_x = ((cx - half_span - badge_radius - border_w - 1.0).floor() as i32).max(0);
    let max_x = ((cx + half_span + badge_radius + border_w + 2.0).ceil() as i32).min(size as i32);

    for y in min_y..max_y {
        for x in min_x..max_x {
            let fx = x as f32;
            let fy = y as f32;
            let clamped_x = fx.clamp(cx - half_span, cx + half_span);
            let dist = (fx - clamped_x).hypot(fy - cy);
            if dist <= badge_radius + border_w + 0.6 {
                if dist <= badge_radius - 0.5 {
                    blend_pixel(&mut rgba, x, y, 239, 56, 56, 255);
                } else if dist <= badge_radius + 0.5 {
                    let t = dist - (badge_radius - 0.5);
                    let r = (239.0 * (1.0 - t) + 255.0 * t).round() as u8;
                    let g = (56.0 * (1.0 - t) + 255.0 * t).round() as u8;
                    let b = (56.0 * (1.0 - t) + 255.0 * t).round() as u8;
                    blend_pixel(&mut rgba, x, y, r, g, b, 255);
                } else if dist <= badge_radius + border_w - 0.3 {
                    blend_pixel(&mut rgba, x, y, 255, 255, 255, 255);
                } else {
                    let a = ((badge_radius + border_w + 0.6 - dist) * 255.0)
                        .round()
                        .clamp(0.0, 255.0) as u8;
                    blend_pixel(&mut rgba, x, y, 255, 255, 255, a);
                }
            }
        }
    }

    let glyph_rows = |ch: char| -> [u8; 5] {
        match ch {
            '0' => [0b111, 0b101, 0b101, 0b101, 0b111],
            '1' => [0b010, 0b110, 0b010, 0b010, 0b111],
            '2' => [0b111, 0b001, 0b111, 0b100, 0b111],
            '3' => [0b111, 0b001, 0b111, 0b001, 0b111],
            '4' => [0b101, 0b101, 0b111, 0b001, 0b001],
            '5' => [0b111, 0b100, 0b111, 0b001, 0b111],
            '6' => [0b111, 0b100, 0b111, 0b101, 0b111],
            '7' => [0b111, 0b001, 0b010, 0b010, 0b010],
            '8' => [0b111, 0b101, 0b111, 0b101, 0b111],
            '9' => [0b111, 0b101, 0b111, 0b001, 0b111],
            '+' => [0b000, 0b010, 0b111, 0b010, 0b000],
            _ => [0; 5],
        }
    };

    let px_scale: i32 = 2;
    let glyph_w: i32 = 3 * px_scale;
    let glyph_h: i32 = 5 * px_scale;
    let gap: i32 = px_scale;
    let total_w: i32 = (chars.len() as i32) * glyph_w + ((chars.len() as i32 - 1).max(0)) * gap;
    let start_x: i32 = (cx - (total_w as f32) * 0.5).round() as i32;
    let start_y: i32 = (cy - (glyph_h as f32) * 0.5).round() as i32;

    for (idx, ch) in chars.iter().enumerate() {
        let rows = glyph_rows(*ch);
        let gx = start_x + (idx as i32) * (glyph_w + gap);
        for (r, mask) in rows.iter().enumerate() {
            for c in 0..3_i32 {
                if (mask >> (2 - c)) & 1 == 1 {
                    for dy in 0..px_scale {
                        for dx in 0..px_scale {
                            blend_pixel(
                                &mut rgba,
                                gx + c * px_scale + dx,
                                start_y + (r as i32) * px_scale + dy,
                                255,
                                255,
                                255,
                                255,
                            );
                        }
                    }
                }
            }
        }
    }

    Some(egui::IconData {
        rgba,
        width: size,
        height: size,
    })
}

pub(super) fn load_app_logo_texture(ctx: &egui::Context) -> Option<TextureHandle> {
    let img = image::load_from_memory(APP_ICON_PNG).ok()?.into_rgba8();
    let (w, h) = img.dimensions();
    let color_img =
        egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], img.as_raw());
    Some(ctx.load_texture("zalo_app_logo", color_img, TextureOptions::LINEAR))
}

pub(super) fn load_default_emoji_textures(ctx: &egui::Context) -> HashMap<String, TextureHandle> {
    let raw: [(&str, &[u8]); 6] = [
        ("like", include_bytes!("../assets/emojis/like.png")),
        ("heart", include_bytes!("../assets/emojis/heart.png")),
        ("smile", include_bytes!("../assets/emojis/smile.png")),
        (
            "surprised",
            include_bytes!("../assets/emojis/surprised.png"),
        ),
        ("cry", include_bytes!("../assets/emojis/cry.png")),
        ("rage", include_bytes!("../assets/emojis/rage.png")),
    ];
    let mut map = HashMap::new();
    for (name, bytes) in raw {
        if let Ok(img) = image::load_from_memory(bytes) {
            let rgba = img.to_rgba8();
            let (w, h) = rgba.dimensions();
            let color_img =
                egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], rgba.as_raw());
            let tex = ctx.load_texture(
                format!("zalo_emoji_{name}"),
                color_img,
                TextureOptions::LINEAR,
            );
            map.insert(name.to_string(), tex);
        }
    }
    map
}

/// Render an iOS/Zalo PC style rounded Toggle Switch in pure egui.
pub(super) fn render_toggle_switch(
    ui: &mut egui::Ui,
    value: &mut bool,
    pal: &ThemePalette,
) -> egui::Response {
    let (rect, mut resp) = ui.allocate_exact_size(Vec2::new(38.0, 20.0), Sense::click());
    if resp.clicked() {
        *value = !*value;
        resp.mark_changed();
    }
    resp = resp.on_hover_cursor(CursorIcon::PointingHand);

    let bg = if *value {
        pal.primary
    } else if pal.is_dark {
        Color32::from_rgb(58, 63, 75)
    } else {
        Color32::from_rgb(203, 213, 225)
    };
    ui.painter().rect_filled(rect, Rounding::same(10.0), bg);

    let knob_radius = 7.5_f32;
    let knob_x = if *value {
        rect.right() - 10.0
    } else {
        rect.left() + 10.0
    };
    ui.painter().circle_filled(
        Pos2::new(knob_x, rect.center().y),
        knob_radius,
        Color32::WHITE,
    );
    resp
}

static CJK_LOADED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub(crate) fn is_cjk(c: char) -> bool {
    matches!(c as u32,
        0x1100..=0x11FF | 0x3040..=0x30FF | 0x3130..=0x318F | 0x3400..=0x4DBF
        | 0x4E00..=0x9FFF | 0xAC00..=0xD7AF | 0xF900..=0xFAFF)
}

/// Load the CJK fallback the first time Han / Kana / Hangul text shows up. msyh.ttc alone is
/// ~20 MB held in memory for the whole run, which a Vietnamese-only user never needs.
pub(super) fn ensure_cjk_font<'a>(ctx: &egui::Context, mut texts: impl Iterator<Item = &'a str>) {
    use std::sync::atomic::Ordering;
    if CJK_LOADED.load(Ordering::Relaxed) || !texts.any(|t| t.chars().any(is_cjk)) {
        return;
    }
    CJK_LOADED.store(true, Ordering::Relaxed);
    build_fonts(ctx, true);
}

pub(super) fn setup_custom_fonts(ctx: &egui::Context) {
    build_fonts(ctx, false);
}

fn build_fonts(ctx: &egui::Context, with_cjk: bool) {
    let mut fonts = egui::FontDefinitions::default();

    // 1. Embed official Lucide Icon Font (vector glyphs for all UI icons)
    const LUCIDE_BYTES: &[u8] = include_bytes!("../assets/fonts/lucide.ttf");
    fonts.font_data.insert(
        "lucide_font".to_owned(),
        egui::FontData::from_static(LUCIDE_BYTES),
    );

    // 2. Bundled Vietnamese sans-serif so diacritics render identically on every OS.
    //    System fonts below are only an optional aesthetic override, never a requirement.
    const BUNDLED_SANS: &[u8] = include_bytes!("../assets/fonts/BeVietnamPro-Regular.ttf");
    fonts.font_data.insert(
        "bundled_sans".to_owned(),
        egui::FontData::from_static(BUNDLED_SANS),
    );

    let font_candidates = [
        "C:\\Windows\\Fonts\\segoeui.ttf",
        "C:\\Windows\\Fonts\\arial.ttf",
        "C:\\Windows\\Fonts\\tahoma.ttf",
        "/System/Library/Fonts/Supplemental/Arial.ttf",
        "/System/Library/Fonts/Supplemental/Tahoma.ttf",
        "/Library/Fonts/Arial.ttf",
        "/usr/share/fonts/dejavu-sans-fonts/DejaVuSans.ttf",
        "/usr/share/fonts/google-noto/NotoSans-Regular.ttf",
        "/usr/share/fonts/truetype/noto/NotoSans-Regular.ttf",
        "/usr/share/fonts/noto/NotoSans-Regular.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        "/usr/share/fonts/TTF/DejaVuSans.ttf",
        "/usr/share/fonts/truetype/ubuntu/Ubuntu-R.ttf",
        "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
    ];

    for path in font_candidates {
        if let Ok(font_data) = std::fs::read(path) {
            fonts.font_data.insert(
                "vietnamese_font".to_owned(),
                egui::FontData::from_owned(font_data),
            );
            break;
        }
    }

    // 3. Optional CJK (Chinese / Japanese / Korean) system font fallback (#5, #6)
    let cjk_candidates = [
        "C:\\Windows\\Fonts\\msyh.ttc",
        "C:\\Windows\\Fonts\\simsun.ttc",
        "C:\\Windows\\Fonts\\malgun.ttf",
        "C:\\Windows\\Fonts\\YuGothR.ttc",
        "/System/Library/Fonts/PingFang.ttc",
        "/System/Library/Fonts/Hiragino Sans GB.ttc",
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/google-noto-cjk/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/wqy-zenhei/wqy-zenhei.ttc",
    ];
    for path in cjk_candidates.into_iter().filter(|_| with_cjk) {
        if let Ok(font_data) = std::fs::read(path) {
            fonts
                .font_data
                .insert("cjk_font".to_owned(), egui::FontData::from_owned(font_data));
            break;
        }
    }

    // 4. Optional vector/native emoji fonts (CBDT bitmap fonts like NotoColorEmoji.ttf are handled
    // natively in full color by `crate::gui::emoji` because `ab_glyph` cannot rasterize CBDT).
    let emoji_candidates = [
        "/usr/share/fonts/truetype/ancient-scripts/Symbola_hint.ttf",
        "/usr/share/fonts/truetype/ancient-scripts/Symbola.ttf",
        "C:\\Windows\\Fonts\\seguiemj.ttf",
        "/System/Library/Fonts/Apple Color Emoji.ttc",
    ];
    for path in emoji_candidates {
        if let Ok(font_data) = std::fs::read(path) {
            fonts.font_data.insert(
                "emoji_font".to_owned(),
                egui::FontData::from_owned(font_data),
            );
            break;
        }
    }

    // Fallback order: bundled sans guarantees Vietnamese coverage, CJK supplies Han/Kana/Hangul,
    // emoji_font supplies modern color glyphs, and Lucide supplies vector UI icons.
    let prop = fonts
        .families
        .entry(egui::FontFamily::Proportional)
        .or_default();
    prop.insert(0, "bundled_sans".to_owned());
    if fonts.font_data.contains_key("vietnamese_font") {
        prop.insert(1, "vietnamese_font".to_owned());
    }
    if fonts.font_data.contains_key("cjk_font") {
        prop.push("cjk_font".to_owned());
    }
    if fonts.font_data.contains_key("emoji_font") {
        prop.push("emoji_font".to_owned());
    }
    prop.push("lucide_font".to_owned());

    let mono = fonts
        .families
        .entry(egui::FontFamily::Monospace)
        .or_default();
    mono.push("bundled_sans".to_owned());
    if fonts.font_data.contains_key("vietnamese_font") {
        mono.push("vietnamese_font".to_owned());
    }
    if fonts.font_data.contains_key("cjk_font") {
        mono.push("cjk_font".to_owned());
    }
    if fonts.font_data.contains_key("emoji_font") {
        mono.push("emoji_font".to_owned());
    }
    mono.push("lucide_font".to_owned());

    ctx.set_fonts(fonts);
}

#[cfg(test)]
mod search_match_tests {
    use super::*;

    fn matches(haystack: &str, query: &str) -> bool {
        let lower = query.to_lowercase();
        let folded = fold_vn_search(&lower);
        matches_vn_query(haystack, &lower, &folded)
    }

    #[test]
    fn ascii_match_is_case_insensitive() {
        assert!(matches("Hello World", "hello"));
        assert!(matches("Hello World", "WORLD"));
        assert!(!matches("Hello World", "goodbye"));
    }

    #[test]
    fn vietnamese_query_matches_accented_and_unaccented_text() {
        assert!(matches("Nguyễn Văn An", "nguyen"));
        assert!(matches("Nguyễn Văn An", "nguyễn"));
        assert!(matches("Đã kết bạn", "da ket"));
    }

    #[test]
    fn empty_query_matches_everything() {
        assert!(matches("anything", ""));
    }

    #[test]
    fn test_clean_emoji_presentation_strips_variation_selectors() {
        assert_eq!(clean_emoji_presentation("Lemmm 💗💗🫶🫶"), "Lemmm 💗💗🫶🫶");
        assert_eq!(clean_emoji_presentation("❤️"), "❤");
        assert_eq!(clean_emoji_presentation("Text\u{FE0F}\u{FE0E}"), "Text");
    }
}
