//! Native full-color inline emoji renderer for Linux/egui.
//!
//! `egui`/`epaint` uses `ab_glyph`, which only rasterizes vector `glyf`/`CFF` outlines and skips
//! color bitmap tables (`CBDT`/`CBLC` in `NotoColorEmoji.ttf`) while letting older text fonts like
//! `DejaVuSans` hijack emojis as monochrome outlines (e.g. turning `💗` into `🤍` and hiding `🫶`).
//!
//! This module extracts the embedded RGBA PNG bitmaps directly from the system's `NotoColorEmoji.ttf`
//! `CBDT`/`CBLC` tables on startup and renders emojis inline as cached `TextureHandle` images.

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::OnceLock;

use eframe::egui::{self, Color32, FontId, Pos2, RichText, TextureHandle, TextureOptions, Vec2};

struct ColorEmojiFont {
    data: Vec<u8>,
    /// Unicode codepoint -> `(cbdt_png_offset, cbdt_png_len)`
    glyph_pngs: HashMap<u32, (usize, usize)>,
}

static COLOR_EMOJI_FONT: OnceLock<Option<ColorEmojiFont>> = OnceLock::new();

thread_local! {
    static EMOJI_TEX_CACHE: RefCell<HashMap<char, Option<TextureHandle>>> = RefCell::new(HashMap::new());
}

fn read_u16(b: &[u8], off: usize) -> Option<u16> {
    let s = b.get(off..off + 2)?;
    Some(u16::from_be_bytes([s[0], s[1]]))
}

fn read_u32(b: &[u8], off: usize) -> Option<u32> {
    let s = b.get(off..off + 4)?;
    Some(u32::from_be_bytes([s[0], s[1], s[2], s[3]]))
}

impl ColorEmojiFont {
    fn load_system() -> Option<Self> {
        let candidates = [
            "/usr/share/fonts/truetype/noto/NotoColorEmoji.ttf",
            "/usr/share/fonts/google-noto-color-emoji/NotoColorEmoji.ttf",
            "/usr/share/fonts/noto-color-emoji/NotoColorEmoji.ttf",
            "/usr/share/fonts/noto/NotoColorEmoji.ttf",
            "/usr/share/fonts/TTF/NotoColorEmoji.ttf",
        ];
        for path in candidates {
            if let Ok(data) = std::fs::read(path) {
                if let Some(parsed) = Self::parse(data) {
                    return Some(parsed);
                }
            }
        }
        None
    }

    fn parse(data: Vec<u8>) -> Option<Self> {
        let num_tables = read_u16(&data, 4)? as usize;
        let mut cmap_off = None;
        let mut cblc_off = None;
        let mut cbdt_off = None;

        for i in 0..num_tables {
            let off = 12 + i * 16;
            let tag = data.get(off..off + 4)?;
            let t_off = read_u32(&data, off + 8)? as usize;
            match tag {
                b"cmap" => cmap_off = Some(t_off),
                b"CBLC" => cblc_off = Some(t_off),
                b"CBDT" => cbdt_off = Some(t_off),
                _ => {}
            }
        }

        let (cmap_off, cblc_off, cbdt_off) = (cmap_off?, cblc_off?, cbdt_off?);

        // 1. Parse cmap format 12 (or format 4) to map Unicode scalar -> glyph_id
        let num_sub = read_u16(&data, cmap_off + 2)? as usize;
        let mut cp_to_glyph: HashMap<u32, u16> = HashMap::new();
        for i in 0..num_sub {
            let sub_rec = cmap_off + 4 + i * 8;
            let rel_off = read_u32(&data, sub_rec + 4)? as usize;
            let sub_off = cmap_off + rel_off;
            let fmt = read_u16(&data, sub_off)?;
            if fmt == 12 {
                let n_groups = read_u32(&data, sub_off + 12)? as usize;
                for g in 0..n_groups {
                    let go = sub_off + 16 + g * 12;
                    let start_c = read_u32(&data, go)?;
                    let end_c = read_u32(&data, go + 4)?;
                    let start_g = read_u32(&data, go + 8)?;
                    for c in start_c..=end_c {
                        let gid = start_g + (c - start_c);
                        if gid <= u16::MAX as u32 {
                            cp_to_glyph.insert(c, gid as u16);
                        }
                    }
                }
                break;
            }
        }

        if cp_to_glyph.is_empty() {
            return None;
        }

        // 2. Parse CBLC index subtables to locate PNG blobs inside CBDT
        let idx_sub_arr_off = read_u32(&data, cblc_off + 8)? as usize;
        let num_idx_sub = read_u32(&data, cblc_off + 16)? as usize;
        let mut gid_to_slice: HashMap<u16, (usize, usize)> = HashMap::new();

        for s in 0..num_idx_sub {
            let rec = cblc_off + idx_sub_arr_off + s * 8;
            let first_g = read_u16(&data, rec)?;
            let last_g = read_u16(&data, rec + 2)?;
            let add_off = read_u32(&data, rec + 4)? as usize;
            if last_g < first_g {
                continue;
            }
            let sub = cblc_off + idx_sub_arr_off + add_off;
            let idx_fmt = read_u16(&data, sub)?;
            let img_fmt = read_u16(&data, sub + 2)?;
            let img_data_off = read_u32(&data, sub + 4)? as usize;
            if !matches!(img_fmt, 17..=19) {
                continue;
            }

            let count = (last_g - first_g) as usize + 1;
            if idx_fmt == 1 {
                for k in 0..count {
                    let o1 = read_u32(&data, sub + 8 + k * 4)? as usize;
                    let o2 = read_u32(&data, sub + 12 + k * 4)? as usize;
                    if o2 > o1 {
                        let raw_start = cbdt_off + img_data_off + o1;
                        let raw_end = cbdt_off + img_data_off + o2;
                        if let Some(raw) = data.get(raw_start..raw_end) {
                            if let Some(png_rel) = raw.windows(4).position(|w| w == b"\x89PNG") {
                                gid_to_slice.insert(
                                    first_g + k as u16,
                                    (raw_start + png_rel, raw.len() - png_rel),
                                );
                            }
                        }
                    }
                }
            } else if idx_fmt == 3 {
                for k in 0..count {
                    let o1 = read_u16(&data, sub + 8 + k * 2)? as usize;
                    let o2 = read_u16(&data, sub + 10 + k * 2)? as usize;
                    if o2 > o1 {
                        let raw_start = cbdt_off + img_data_off + o1;
                        let raw_end = cbdt_off + img_data_off + o2;
                        if let Some(raw) = data.get(raw_start..raw_end) {
                            if let Some(png_rel) = raw.windows(4).position(|w| w == b"\x89PNG") {
                                gid_to_slice.insert(
                                    first_g + k as u16,
                                    (raw_start + png_rel, raw.len() - png_rel),
                                );
                            }
                        }
                    }
                }
            }
        }

        let mut glyph_pngs = HashMap::with_capacity(cp_to_glyph.len());
        for (cp, gid) in cp_to_glyph {
            if let Some(&slice) = gid_to_slice.get(&gid) {
                glyph_pngs.insert(cp, slice);
            }
        }

        if glyph_pngs.is_empty() {
            None
        } else {
            Some(Self { data, glyph_pngs })
        }
    }

    fn png_bytes(&self, ch: char) -> Option<&[u8]> {
        let &(off, len) = self.glyph_pngs.get(&(ch as u32))?;
        self.data.get(off..off + len)
    }
}

fn system_emoji_font() -> Option<&'static ColorEmojiFont> {
    COLOR_EMOJI_FONT
        .get_or_init(ColorEmojiFont::load_system)
        .as_ref()
}

/// Returns `true` if the character is a renderable color emoji (excluding ASCII, Latin,
/// Lucide private-use glyphs, CJK, and common monochrome UI punctuation like `✓` or `•`).
pub(super) fn is_color_emoji(ch: char) -> bool {
    let cp = ch as u32;
    if cp < 0x2000 {
        return false;
    }
    // Exclude Lucide PUA, CJK, Hangul, and common UI symbols
    if (0xE000..=0xF8FF).contains(&cp)
        || (0x4E00..=0x9FFF).contains(&cp)
        || (0x3040..=0x30FF).contains(&cp)
        || (0xAC00..=0xD7AF).contains(&cp)
        || matches!(
            ch,
            '✓' | '✔'
                | '✕'
                | '✖'
                | '✗'
                | '•'
                | '·'
                | '…'
                | '—'
                | '–'
                | '→'
                | '←'
                | '↑'
                | '↓'
                | '©'
                | '®'
                | '™'
        )
    {
        return false;
    }
    system_emoji_font().is_some_and(|f| f.png_bytes(ch).is_some())
}

/// Returns `true` when `text` contains any color emoji or iPhone variation selector.
pub(super) fn contains_color_emoji(text: &str) -> bool {
    if text.is_ascii() {
        return false;
    }
    text.chars()
        .any(|c| is_color_emoji(c) || matches!(c, '\u{FE0F}' | '\u{FE0E}' | '\u{200D}'))
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum EmojiToken<'a> {
    Text(&'a str),
    Emoji(char),
}

fn is_emoji_modifier_or_joiner(c: char) -> bool {
    let cp = c as u32;
    matches!(c, '\u{FE0F}' | '\u{FE0E}' | '\u{200D}') || (0x1F3FB..=0x1F3FF).contains(&cp)
}

/// Split `text` into plain text slices and color emoji characters while stripping
/// invisible variation selectors, ZWJ, and standalone skin-tone modifiers so no tofu boxes appear.
pub(super) fn tokenize_emoji_runs(text: &str) -> Vec<EmojiToken<'_>> {
    let mut out = Vec::new();
    let mut text_start = 0usize;

    let mut iter = text.char_indices().peekable();
    while let Some((idx, ch)) = iter.next() {
        if is_color_emoji(ch) {
            if idx > text_start {
                out.push(EmojiToken::Text(&text[text_start..idx]));
            }
            out.push(EmojiToken::Emoji(ch));
            while let Some(&(_, next_ch)) = iter.peek() {
                if is_emoji_modifier_or_joiner(next_ch) {
                    iter.next();
                } else {
                    break;
                }
            }
            text_start = iter.peek().map_or(text.len(), |&(next_i, _)| next_i);
        } else if is_emoji_modifier_or_joiner(ch) {
            if idx > text_start {
                out.push(EmojiToken::Text(&text[text_start..idx]));
            }
            text_start = idx + ch.len_utf8();
        }
    }

    if text_start < text.len() {
        out.push(EmojiToken::Text(&text[text_start..]));
    }
    out
}

/// Decode and cache a `TextureHandle` for `ch` from the system `NotoColorEmoji.ttf`.
pub(super) fn emoji_texture(ctx: &egui::Context, ch: char) -> Option<TextureHandle> {
    EMOJI_TEX_CACHE.with(|cache| {
        if let Some(hit) = cache.borrow().get(&ch) {
            return hit.clone();
        }
        let tex = (|| {
            let font = system_emoji_font()?;
            let png = font.png_bytes(ch)?;
            let dyn_img = image::load_from_memory(png).ok()?;
            // Downscale 128x128 CBDT PNG once to crisp 40x40 RGBA so UI mipmaps stay sharp at 14-24px
            let resized = if dyn_img.width() > 40 || dyn_img.height() > 40 {
                dyn_img.resize_exact(40, 40, image::imageops::FilterType::Lanczos3)
            } else {
                dyn_img
            };
            let rgba = resized.to_rgba8();
            let size = [rgba.width() as usize, rgba.height() as usize];
            let color_img =
                egui::ColorImage::from_rgba_unmultiplied(size, rgba.as_flat_samples().as_slice());
            Some(ctx.load_texture(
                format!("emoji_{:x}", ch as u32),
                color_img,
                TextureOptions::LINEAR,
            ))
        })();
        cache.borrow_mut().insert(ch, tex.clone());
        tex
    })
}

/// Paint single-line text with full-color inline emojis directly onto `ui.painter()`.
/// Ideal for custom-painted sidebar rows where widgets are avoided for 60fps performance.
pub(super) fn paint_inline_emoji_text(
    ui: &egui::Ui,
    pos: Pos2,
    text: &str,
    font_size: f32,
    color: Color32,
) {
    if !contains_color_emoji(text) {
        let clean = super::theme::clean_emoji_presentation(text);
        ui.painter().text(
            pos,
            egui::Align2::LEFT_TOP,
            clean.as_ref(),
            FontId::proportional(font_size),
            color,
        );
        return;
    }

    let font_id = FontId::proportional(font_size);
    let emoji_sz = (font_size * 1.15).round();
    let mut cursor_x = pos.x;
    for tok in tokenize_emoji_runs(text) {
        match tok {
            EmojiToken::Text(seg) => {
                let galley =
                    ui.fonts(|f| f.layout_no_wrap(seg.to_string(), font_id.clone(), color));
                let w = galley.size().x;
                ui.painter()
                    .galley(Pos2::new(cursor_x, pos.y), galley, color);
                cursor_x += w;
            }
            EmojiToken::Emoji(ch) => {
                if let Some(tex) = emoji_texture(ui.ctx(), ch) {
                    let y_off = (font_size - emoji_sz) * 0.35 + 1.0;
                    let rect = egui::Rect::from_min_size(
                        Pos2::new(cursor_x + 1.0, pos.y + y_off),
                        Vec2::splat(emoji_sz),
                    );
                    ui.painter().image(
                        tex.id(),
                        rect,
                        egui::Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                        Color32::WHITE,
                    );
                    cursor_x += emoji_sz + 2.0;
                } else {
                    let s = ch.to_string();
                    let galley = ui.fonts(|f| f.layout_no_wrap(s, font_id.clone(), color));
                    let w = galley.size().x;
                    ui.painter()
                        .galley(Pos2::new(cursor_x, pos.y), galley, color);
                    cursor_x += w;
                }
            }
        }
    }
}

/// Render a single line of text with full-color inline emojis (e.g. thread names, headers).
pub(super) fn render_inline_emoji_line(
    ui: &mut egui::Ui,
    text: &str,
    font_size: f32,
    color: Color32,
    strong: bool,
) {
    if !contains_color_emoji(text) {
        let clean = super::theme::clean_emoji_presentation(text);
        let mut rt = RichText::new(clean.as_ref()).size(font_size).color(color);
        if strong {
            rt = rt.strong();
        }
        ui.add(egui::Label::new(rt).truncate().selectable(false));
        return;
    }

    let tokens = tokenize_emoji_runs(text);
    let emoji_sz = (font_size * 1.15).round();
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 1.0;
        for tok in tokens {
            match tok {
                EmojiToken::Text(seg) => {
                    let mut rt = RichText::new(seg).size(font_size).color(color);
                    if strong {
                        rt = rt.strong();
                    }
                    ui.add(egui::Label::new(rt).selectable(false));
                }
                EmojiToken::Emoji(ch) => {
                    if let Some(tex) = emoji_texture(ui.ctx(), ch) {
                        ui.add(egui::Image::new(&tex).fit_to_exact_size(Vec2::splat(emoji_sz)));
                    } else {
                        let mut rt = RichText::new(ch.to_string()).size(font_size).color(color);
                        if strong {
                            rt = rt.strong();
                        }
                        ui.add(egui::Label::new(rt).selectable(false));
                    }
                }
            }
        }
    });
}

/// Render wrapped text with full-color inline emojis (used inside chat bubbles).
pub(super) fn render_wrapped_emoji_text(
    ui: &mut egui::Ui,
    text: &str,
    font_size: f32,
    color: Color32,
) {
    if !contains_color_emoji(text) {
        let clean = super::theme::clean_emoji_presentation(text);
        ui.label(RichText::new(clean.as_ref()).size(font_size).color(color));
        return;
    }

    let emoji_sz = (font_size * 1.18).round();
    for line in text.split('\n') {
        if line.is_empty() {
            ui.add_space(font_size * 0.6);
            continue;
        }
        let tokens = tokenize_emoji_runs(line);
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = Vec2::new(2.0, 2.0);
            for tok in tokens {
                match tok {
                    EmojiToken::Text(seg) => {
                        for word in seg.split_inclusive(' ') {
                            ui.label(RichText::new(word).size(font_size).color(color));
                        }
                    }
                    EmojiToken::Emoji(ch) => {
                        if let Some(tex) = emoji_texture(ui.ctx(), ch) {
                            ui.add(egui::Image::new(&tex).fit_to_exact_size(Vec2::splat(emoji_sz)));
                        } else {
                            ui.label(RichText::new(ch.to_string()).size(font_size).color(color));
                        }
                    }
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenizes_mixed_text_and_ios_emojis_without_modifiers() {
        let tokens = tokenize_emoji_runs("Lemmm 💗💗🫶🫶");
        if system_emoji_font().is_some() {
            assert_eq!(
                tokens,
                vec![
                    EmojiToken::Text("Lemmm "),
                    EmojiToken::Emoji('💗'),
                    EmojiToken::Emoji('💗'),
                    EmojiToken::Emoji('🫶'),
                    EmojiToken::Emoji('🫶'),
                ]
            );
        }
    }

    #[test]
    fn system_cbdt_emoji_font_extracts_valid_png_for_modern_emojis() {
        if let Some(font) = system_emoji_font() {
            for ch in ['💗', '🫶', '❤', '👍'] {
                let png = font.png_bytes(ch).expect("emoji PNG must exist");
                assert!(png.starts_with(b"\x89PNG"));
                assert!(image::load_from_memory(png).is_ok());
            }
        }
    }
}
