use super::*;

impl ZaloGuiApp {
    /// Start loading the full-resolution texture the image viewer shows. Message bubbles and
    /// the media grid keep a 480 px copy; the `hd_` key keeps this one (and its disk cache) apart.
    pub(super) fn request_hd_texture(&mut self, full_url: &str) -> Option<String> {
        let lower_u = full_url.to_lowercase();
        let is_direct_img = lower_u.contains("zdn.vn")
            || lower_u.ends_with(".png")
            || lower_u.ends_with(".jpg")
            || lower_u.ends_with(".jpeg")
            || lower_u.ends_with(".webp")
            || lower_u.ends_with(".gif")
            || lower_u.ends_with(".bmp")
            || (!lower_u.starts_with("http://") && !lower_u.starts_with("https://"));
        if full_url.is_empty() || !is_direct_img {
            return None;
        }
        let hk = format!("hd_{}", crate::crypto::md5_hex(full_url));
        if !self.sticker_textures.contains_key(&hk) && self.loading_stickers.insert(hk.clone()) {
            if let Some(tx) = WORKER_TX.get() {
                let _ = tx.send(WorkerCmd::FetchStickerImage {
                    key: hk.clone(),
                    url: full_url.to_string(),
                });
            }
        }
        Some(hk)
    }

    pub(super) fn render_chat_workspace(&mut self, ctx: &egui::Context, pal: &ThemePalette) {
        // Responsive breakpoints based on live viewport width
        // -------------------------------------------------------------------
        let screen_rect = ctx.screen_rect();
        let screen_w = screen_rect.width();
        let is_compact_w = screen_w < 880.0;
        let is_narrow_w = screen_w < 620.0;
        let left_rail_w = if is_narrow_w { 52.0 } else { 62.0 };

        // -------------------------------------------------------------------
        // 1. LEFT NAVIGATION RAIL (Replaces top bar & bottom bar)
        // -------------------------------------------------------------------
        egui::SidePanel::left("left_nav_rail")
            .resizable(false)
            .exact_width(left_rail_w)
            .frame(
                egui::Frame::none()
                    .fill(pal.bg_app)
                    .inner_margin(Margin::symmetric(if is_narrow_w { 4.0 } else { 8.0 }, 14.0))
                    .stroke(Stroke::new(1.0_f32, pal.border)),
            )
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    // User Avatar Button with Cutout Online Status Dot
                    let (av_rect, av_resp) =
                        ui.allocate_exact_size(Vec2::new(40.0, 40.0), Sense::click());
                    let user_disp = if self.user_name.is_empty() {
                        "Người dùng Zalo"
                    } else {
                        &self.user_name
                    };
                    paint_user_avatar(
                        ui.painter(),
                        av_rect.center(),
                        20.0,
                        user_disp,
                        false,
                        self.user_avatar.as_deref(),
                        &self.sticker_textures,
                        &mut self.loading_stickers,
                    );
                    if av_resp.hovered() || self.profile_menu_open {
                        ui.painter().circle_stroke(
                            av_rect.center(),
                            21.0,
                            Stroke::new(2.0_f32, pal.primary_hover),
                        );
                    }
                    // Cutout status dot
                    if self.settings.show_online_status {
                        let dot_c = Pos2::new(av_rect.right() - 4.0, av_rect.bottom() - 4.0);
                        ui.painter().circle_filled(dot_c, 6.5, pal.bg_app);
                        ui.painter().circle_filled(dot_c, 4.5, pal.accent_online);
                    }

                    if av_resp
                        .on_hover_cursor(CursorIcon::PointingHand)
                        .on_hover_text(format!(
                            "{} (Đang trực tuyến — Bấm để mở menu tài khoản)",
                            if self.user_name.is_empty() {
                                "Tài khoản Zalo"
                            } else {
                                &self.user_name
                            }
                        ))
                        .clicked()
                    {
                        self.profile_menu_open = !self.profile_menu_open;
                    }

                    ui.add_space(18.0);

                    // Navigation Icon 1: Tin nhắn (All / Active Chat)
                    let is_msg_nav = !self.feed_modal_open
                        && !self.contacts_view_open
                        && self.thread_filter == ThreadFilter::All;
                    let (nav1_rect, nav1_resp) =
                        ui.allocate_exact_size(Vec2::new(42.0, 42.0), Sense::click());
                    if is_msg_nav {
                        ui.painter().rect_filled(
                            nav1_rect,
                            Rounding::same(11.0),
                            pal.primary.linear_multiply(0.18),
                        );
                        let pill = Rect::from_min_size(
                            Pos2::new(nav1_rect.left() - 6.0, nav1_rect.center().y - 10.0),
                            Vec2::new(3.5, 20.0),
                        );
                        ui.painter()
                            .rect_filled(pill, Rounding::same(2.0), pal.primary);
                    } else if nav1_resp.hovered() {
                        ui.painter()
                            .rect_filled(nav1_rect, Rounding::same(11.0), pal.bg_hover);
                    }
                    draw_icon_chat_nav(
                        ui.painter(),
                        nav1_rect.center(),
                        19.0,
                        if is_msg_nav {
                            pal.primary_hover
                        } else {
                            pal.text_secondary
                        },
                    );
                    let nav_unread: usize = self.unread_counts.values().sum();
                    if nav_unread > 0 {
                        let badge_txt = if nav_unread > 99 {
                            "99+".to_string()
                        } else {
                            nav_unread.to_string()
                        };
                        let bw = if badge_txt.len() >= 3 {
                            24.0
                        } else if badge_txt.len() == 2 {
                            19.0
                        } else {
                            15.0
                        };
                        let badge_rect = Rect::from_center_size(
                            Pos2::new(nav1_rect.right() - 6.0, nav1_rect.top() + 7.0),
                            Vec2::new(bw, 15.0),
                        );
                        ui.painter().rect_filled(
                            badge_rect,
                            Rounding::same(7.5),
                            Color32::from_rgb(239, 56, 56),
                        );
                        ui.painter().rect_stroke(
                            badge_rect,
                            Rounding::same(7.5),
                            Stroke::new(1.2_f32, pal.bg_app),
                        );
                        ui.painter().text(
                            badge_rect.center(),
                            egui::Align2::CENTER_CENTER,
                            badge_txt,
                            FontId::proportional(9.8),
                            Color32::WHITE,
                        );
                    }
                    if nav1_resp
                        .on_hover_cursor(CursorIcon::PointingHand)
                        .on_hover_text("Tin nhắn")
                        .clicked()
                    {
                        self.feed_modal_open = false;
                        self.contacts_view_open = false;
                        self.thread_filter = ThreadFilter::All;
                    }

                    ui.add_space(6.0);

                    // Navigation Icon 2: Danh bạ (Unified Friends + Groups + Invites matching Zalo PC)
                    let direct_cnt = self
                        .threads
                        .iter()
                        .filter(|t| !t.is_group && !t.name.contains("Cloud của tôi"))
                        .count();
                    let group_cnt = self.threads.iter().filter(|t| t.is_group).count();
                    let is_contacts_nav = !self.feed_modal_open && self.contacts_view_open;
                    let (nav2_rect, nav2_resp) =
                        ui.allocate_exact_size(Vec2::new(42.0, 42.0), Sense::click());
                    if is_contacts_nav {
                        ui.painter().rect_filled(
                            nav2_rect,
                            Rounding::same(11.0),
                            pal.primary.linear_multiply(0.18),
                        );
                        let pill = Rect::from_min_size(
                            Pos2::new(nav2_rect.left() - 6.0, nav2_rect.center().y - 10.0),
                            Vec2::new(3.5, 20.0),
                        );
                        ui.painter()
                            .rect_filled(pill, Rounding::same(2.0), pal.primary);
                    } else if nav2_resp.hovered() {
                        ui.painter()
                            .rect_filled(nav2_rect, Rounding::same(11.0), pal.bg_hover);
                    }
                    draw_icon_contacts(
                        ui.painter(),
                        nav2_rect.center(),
                        19.0,
                        if is_contacts_nav {
                            pal.primary_hover
                        } else {
                            pal.text_secondary
                        },
                    );
                    if nav2_resp
                        .on_hover_cursor(CursorIcon::PointingHand)
                        .on_hover_text(format!("Danh bạ ({direct_cnt} bạn bè · {group_cnt} nhóm)"))
                        .clicked()
                    {
                        self.feed_modal_open = false;
                        self.contacts_view_open = !self.contacts_view_open;
                    }

                    ui.add_space(6.0);

                    // Navigation Icon 3: Cloud của tôi
                    let is_cloud_selected = !self.feed_modal_open
                        && !self.contacts_view_open
                        && self
                            .threads
                            .get(self.selected_thread_index)
                            .map(|t| t.name.contains("Cloud của tôi"))
                            .unwrap_or(false);
                    let (nav3_rect, nav3_resp) =
                        ui.allocate_exact_size(Vec2::new(42.0, 42.0), Sense::click());
                    if is_cloud_selected {
                        ui.painter().rect_filled(
                            nav3_rect,
                            Rounding::same(11.0),
                            pal.primary.linear_multiply(0.18),
                        );
                    } else if nav3_resp.hovered() {
                        ui.painter()
                            .rect_filled(nav3_rect, Rounding::same(11.0), pal.bg_hover);
                    }
                    draw_icon_cloud(
                        ui.painter(),
                        nav3_rect.center(),
                        19.0,
                        if is_cloud_selected {
                            pal.primary_hover
                        } else {
                            pal.text_secondary
                        },
                    );
                    if nav3_resp
                        .on_hover_cursor(CursorIcon::PointingHand)
                        .on_hover_text("Cloud của tôi")
                        .clicked()
                    {
                        self.feed_modal_open = false;
                        self.contacts_view_open = false;
                        if let Some((cidx, cthread)) = self
                            .threads
                            .iter()
                            .enumerate()
                            .find(|(_, t)| t.name.contains("Cloud của tôi"))
                        {
                            let cid = cthread.id.clone();
                            self.selected_thread_index = cidx;
                            if let Some(tx) = WORKER_TX.get() {
                                let _ = tx.send(WorkerCmd::LoadGroupHistory {
                                    group_id: cid,
                                    is_group: false,
                                    have: 0,
                                });
                            }
                        }
                    }

                    ui.add_space(6.0);

                    // Navigation Icon 4: Nhật ký (Social Feed & Profile)
                    let (nav4_rect, nav4_resp) =
                        ui.allocate_exact_size(Vec2::new(42.0, 42.0), Sense::click());
                    if self.feed_modal_open {
                        ui.painter().rect_filled(
                            nav4_rect,
                            Rounding::same(11.0),
                            pal.primary.linear_multiply(0.18),
                        );
                        let pill = Rect::from_min_size(
                            Pos2::new(nav4_rect.left() - 6.0, nav4_rect.center().y - 10.0),
                            Vec2::new(3.5, 20.0),
                        );
                        ui.painter()
                            .rect_filled(pill, Rounding::same(2.0), pal.primary);
                    } else if nav4_resp.hovered() {
                        ui.painter()
                            .rect_filled(nav4_rect, Rounding::same(11.0), pal.bg_hover);
                    }
                    draw_icon_feed(
                        ui.painter(),
                        nav4_rect.center(),
                        19.0,
                        if self.feed_modal_open {
                            pal.primary_hover
                        } else {
                            pal.text_secondary
                        },
                    );
                    if nav4_resp
                        .on_hover_cursor(CursorIcon::PointingHand)
                        .on_hover_text("Tường nhà / Nhật ký bạn bè")
                        .clicked()
                    {
                        self.contacts_view_open = false;
                        self.feed_modal_open = true;
                        self.trigger_feed_timeline_load();
                    }

                    // Push bottom actions to the bottom of the Left Rail
                    let rem_h = (ui.available_height() - 100.0).max(12.0);
                    ui.add_space(rem_h);

                    // Bottom Action 1: Background Sync Indicator Icon
                    let (sync_rect, sync_resp) =
                        ui.allocate_exact_size(Vec2::new(40.0, 40.0), Sense::click());
                    if sync_resp.hovered() {
                        ui.painter()
                            .rect_filled(sync_rect, Rounding::same(10.0), pal.bg_hover);
                    }
                    draw_icon_refresh(
                        ui.painter(),
                        sync_rect.center(),
                        17.0,
                        if sync_resp.hovered() {
                            pal.accent_online
                        } else {
                            pal.text_secondary
                        },
                    );
                    ui.painter().circle_filled(
                        Pos2::new(sync_rect.right() - 9.0, sync_rect.top() + 9.0),
                        3.0,
                        pal.accent_online,
                    );
                    if sync_resp
                        .on_hover_cursor(CursorIcon::PointingHand)
                        .on_hover_text(format!(
                            "📲 Đồng bộ tin nhắn từ điện thoại & Làm mới (Bấm để đồng bộ) · {}",
                            self.status_msg
                        ))
                        .clicked()
                    {
                        self.phone_sync_progress = 0.05;
                        self.phone_sync_status =
                            "📲 Đang gửi yêu cầu đồng bộ tin nhắn tới điện thoại…".into();
                        let threads = self.threads.clone();
                        if let Some(tx) = WORKER_TX.get() {
                            let _ = tx.send(WorkerCmd::SyncPhoneMessages {
                                threads,
                                send_phone_prompt: true,
                            });
                        }
                        self.show_toast("📲 Đang gửi yêu cầu đồng bộ tin nhắn tới điện thoại…");
                    }

                    ui.add_space(6.0);

                    // Bottom Action 2: Theme / Settings Icon
                    // Bottom Action 2: Settings Modal Button
                    let has_upd_badge = self
                        .update_info
                        .as_ref()
                        .map(|u| u.has_update)
                        .unwrap_or(false);
                    let (set_rect, set_resp) =
                        ui.allocate_exact_size(Vec2::new(40.0, 40.0), Sense::click());
                    if self.settings_modal_open {
                        ui.painter().rect_filled(
                            set_rect,
                            Rounding::same(10.0),
                            pal.primary.linear_multiply(0.18),
                        );
                    } else if set_resp.hovered() {
                        ui.painter()
                            .rect_filled(set_rect, Rounding::same(10.0), pal.bg_hover);
                    }
                    draw_icon_settings(
                        ui.painter(),
                        set_rect.center(),
                        17.5,
                        if self.settings_modal_open {
                            pal.primary_hover
                        } else if set_resp.hovered() {
                            pal.text_primary
                        } else {
                            pal.text_secondary
                        },
                    );
                    if has_upd_badge {
                        ui.painter().circle_filled(
                            Pos2::new(set_rect.right() - 8.0, set_rect.top() + 8.0),
                            4.2,
                            pal.accent_online,
                        );
                    }
                    let set_tip = if let Some(ref u) = self.update_info {
                        if u.has_update {
                            format!(
                                "Cài đặt Zalo (🚀 Có bản cập nhật mới v{})",
                                u.latest_version
                            )
                        } else {
                            format!("Cài đặt Zalo (v{})", env!("CARGO_PKG_VERSION"))
                        }
                    } else {
                        format!("Cài đặt Zalo (v{})", env!("CARGO_PKG_VERSION"))
                    };
                    if set_resp
                        .on_hover_cursor(CursorIcon::PointingHand)
                        .on_hover_text(set_tip)
                        .clicked()
                    {
                        self.settings_modal_open = !self.settings_modal_open;
                        if self.settings_modal_open && has_upd_badge {
                            self.settings_tab = SettingsTab::Update;
                        }
                    }
                });
            });

        if self.contacts_view_open {
            self.render_contacts_page(ctx, pal);
            self.render_profile_popover(ctx, pal);
            self.render_wipe_confirm(ctx, pal);
            self.render_toast_overlay(ctx, pal);
            self.render_forward_dialog(ctx, pal);
            self.render_reactions_dialog(ctx, pal);
            self.render_call_overlay(ctx, pal);
            self.render_image_viewer_modal(ctx, pal);
            self.render_phone_lookup_modal(ctx, pal);
            self.render_settings_modal(ctx, pal);
            self.render_schedule_modal(ctx, pal);
            return;
        }
        if self.feed_modal_open {
            self.render_feed_page(ctx, pal);
            self.render_profile_popover(ctx, pal);
            self.render_wipe_confirm(ctx, pal);
            self.render_toast_overlay(ctx, pal);
            self.render_forward_dialog(ctx, pal);
            self.render_reactions_dialog(ctx, pal);
            self.render_call_overlay(ctx, pal);
            self.render_image_viewer_modal(ctx, pal);
            self.render_phone_lookup_modal(ctx, pal);
            self.render_settings_modal(ctx, pal);
            return;
        }
        // -------------------------------------------------------------------
        // 2. CONVERSATION LIST SIDEBAR (Responsive width, auto-adapts on resize)
        // -------------------------------------------------------------------
        let (sidebar_id, sidebar_min_w, sidebar_max_w) = if is_narrow_w {
            ("conversation_sidebar_narrow", 68.0_f32, 68.0_f32)
        } else {
            (
                "conversation_sidebar",
                180.0_f32,
                (screen_w * 0.42).clamp(240.0, 420.0),
            )
        };
        let sidebar_default_w = if is_narrow_w {
            68.0_f32
        } else {
            280.0_f32.min(screen_w * 0.35).max(sidebar_min_w)
        };
        egui::SidePanel::left(sidebar_id)
            .resizable(!is_narrow_w)
            .default_width(sidebar_default_w)
            .width_range(sidebar_min_w..=sidebar_max_w)
            .frame(
                egui::Frame::none()
                    .fill(pal.bg_sidebar)
                    .stroke(Stroke::new(1.0_f32, pal.border)),
            )
            .show(ctx, |ui| {
                egui::Frame::none()
                    .inner_margin(Margin {
                        left: if is_narrow_w { 4.0 } else { 14.0 },
                        right: if is_narrow_w { 4.0 } else { 14.0 },
                        top: 12.0,
                        bottom: 0.0,
                    })
                    .show(ui, |ui| {
                        if is_narrow_w {
                            // Compact mode: icon-only search / lookup button
                            ui.vertical_centered(|ui| {
                                let (srch_r, srch_resp) =
                                    ui.allocate_exact_size(Vec2::new(42.0, 32.0), Sense::click());
                                ui.painter()
                                    .rect_filled(srch_r, Rounding::same(8.0), pal.bg_input);
                                ui.painter().rect_stroke(
                                    srch_r,
                                    Rounding::same(8.0),
                                    Stroke::new(1.0_f32, pal.border),
                                );
                                draw_icon_search(
                                    ui.painter(),
                                    srch_r.center(),
                                    16.0,
                                    pal.primary_hover,
                                );
                                if srch_resp
                                    .on_hover_cursor(CursorIcon::PointingHand)
                                    .on_hover_text("Tìm kiếm hội thoại & Tra cứu SĐT")
                                    .clicked()
                                {
                                    self.phone_modal_open = true;
                                }
                            });
                            ui.add_space(6.0);
                        } else {
                            // Search Box + Quick Phone Lookup (`+👤`) Button
                            let avail_row_w = ui.available_width();
                            let mut trigger_phone_lookup: Option<String> = None;
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 6.0;
                                let search_box_w = (avail_row_w - 38.0).max(110.0);
                                let has_q = !self.search_query.is_empty();
                                let (sbox_rect, _) = ui.allocate_exact_size(
                                    Vec2::new(search_box_w, 32.0),
                                    Sense::hover(),
                                );
                                ui.painter().rect_filled(
                                    sbox_rect,
                                    Rounding::same(8.0),
                                    pal.bg_input,
                                );
                                ui.painter().rect_stroke(
                                    sbox_rect,
                                    Rounding::same(8.0),
                                    Stroke::new(
                                        1.0_f32,
                                        if has_q {
                                            pal.primary.linear_multiply(0.65)
                                        } else {
                                            pal.border
                                        },
                                    ),
                                );
                                draw_icon_search(
                                    ui.painter(),
                                    Pos2::new(sbox_rect.left() + 16.0, sbox_rect.center().y),
                                    13.5,
                                    if has_q {
                                        pal.primary_hover
                                    } else {
                                        pal.text_muted
                                    },
                                );
                                let edit_rect = Rect::from_min_max(
                                    Pos2::new(sbox_rect.left() + 28.0, sbox_rect.top() + 5.0),
                                    Pos2::new(
                                        sbox_rect.right() - if has_q { 26.0 } else { 8.0 },
                                        sbox_rect.bottom() - 5.0,
                                    ),
                                );
                                let s_resp = ui.put(
                                    edit_rect,
                                    egui::TextEdit::singleline(&mut self.search_query)
                                        .id(Id::new("sidebar_search_input"))
                                        .frame(false)
                                        .hint_text("Tìm hội thoại, tin nhắn, SĐT...")
                                        .desired_width(edit_rect.width()),
                                );
                                let is_ime_busy = self.ime_active || self.ime_just_committed;
                                if is_ime_busy && s_resp.lost_focus() {
                                    s_resp.request_focus();
                                }
                                let s_enter = (s_resp.lost_focus() || s_resp.has_focus())
                                    && ui.input(|i| i.key_pressed(Key::Enter))
                                    && !is_ime_busy;
                                let digits_cnt = self
                                    .search_query
                                    .chars()
                                    .filter(|c| c.is_ascii_digit())
                                    .count();
                                if s_enter && digits_cnt >= 8 {
                                    trigger_phone_lookup =
                                        Some(self.search_query.trim().to_string());
                                }
                                if has_q {
                                    let clr_rect = Rect::from_center_size(
                                        Pos2::new(sbox_rect.right() - 14.0, sbox_rect.center().y),
                                        Vec2::new(18.0, 18.0),
                                    );
                                    let clr_resp = ui.interact(
                                        clr_rect,
                                        Id::new("sidebar_search_clear_btn"),
                                        Sense::click(),
                                    );
                                    if clr_resp.hovered() {
                                        ui.painter().circle_filled(
                                            clr_rect.center(),
                                            8.5,
                                            pal.bg_hover,
                                        );
                                    }
                                    draw_lucide_icon(
                                        ui.painter(),
                                        clr_rect.center(),
                                        lucide::X,
                                        11.5,
                                        if clr_resp.hovered() {
                                            pal.text_primary
                                        } else {
                                            pal.text_muted
                                        },
                                    );
                                    if clr_resp
                                        .on_hover_cursor(CursorIcon::PointingHand)
                                        .on_hover_text("Xóa tìm kiếm")
                                        .clicked()
                                    {
                                        self.search_query.clear();
                                    }
                                }

                                // Add Friend / Lookup by Phone Button (`+👤`)
                                let (add_rect, add_resp) =
                                    ui.allocate_exact_size(Vec2::new(32.0, 32.0), Sense::click());
                                ui.painter().rect_filled(
                                    add_rect,
                                    Rounding::same(8.0),
                                    if self.phone_modal_open {
                                        pal.primary.linear_multiply(0.22)
                                    } else if add_resp.hovered() {
                                        pal.bg_hover
                                    } else {
                                        pal.bg_input
                                    },
                                );
                                ui.painter().rect_stroke(
                                    add_rect,
                                    Rounding::same(8.0),
                                    Stroke::new(
                                        1.0_f32,
                                        if self.phone_modal_open || add_resp.hovered() {
                                            pal.primary
                                        } else {
                                            pal.border
                                        },
                                    ),
                                );
                                draw_icon_user_plus(
                                    ui.painter(),
                                    add_rect.center(),
                                    16.5,
                                    if self.phone_modal_open || add_resp.hovered() {
                                        pal.primary_hover
                                    } else {
                                        pal.text_secondary
                                    },
                                );
                                if add_resp
                                    .on_hover_cursor(CursorIcon::PointingHand)
                                    .on_hover_text("Tìm kiếm số điện thoại Zalo & Kết bạn")
                                    .clicked()
                                {
                                    self.phone_modal_open = true;
                                    let digits_cnt = self
                                        .search_query
                                        .chars()
                                        .filter(|c| c.is_ascii_digit())
                                        .count();
                                    if digits_cnt >= 8 && self.phone_modal_input.is_empty() {
                                        self.phone_modal_input =
                                            self.search_query.trim().to_string();
                                    }
                                }
                            });

                            if let Some(phone_str) = trigger_phone_lookup {
                                self.phone_modal_open = true;
                                self.phone_modal_input = phone_str.clone();
                                self.phone_lookup_loading = true;
                                self.phone_lookup_error = None;
                                self.phone_lookup_result = None;
                                if let Some(tx) = WORKER_TX.get() {
                                    let _ = tx.send(WorkerCmd::LookupPhone { phone: phone_str });
                                }
                            }
                            ui.add_space(10.0);

                            // Clean Underline Segmented Tabs (`Tất cả`, `Cá nhân`, `Nhóm`) + `Đã ghim (1)`
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 16.0;
                                let group_tab_cnt =
                                    self.threads.iter().filter(|t| t.is_group).count();
                                let group_tab_label = if group_tab_cnt > 0 {
                                    format!("Nhóm ({group_tab_cnt})")
                                } else {
                                    "Nhóm".to_string()
                                };
                                for (filter, label) in [
                                    (ThreadFilter::All, "Tất cả".to_string()),
                                    (ThreadFilter::Direct, "Cá nhân".to_string()),
                                    (ThreadFilter::Groups, group_tab_label),
                                ] {
                                    let active = self.thread_filter == filter;
                                    let text_w = ui.fonts(|f| {
                                        f.layout_no_wrap(
                                            label.clone(),
                                            FontId::proportional(12.5),
                                            pal.text_primary,
                                        )
                                        .size()
                                        .x
                                    });
                                    let (tab_rect, tab_resp) = ui.allocate_exact_size(
                                        Vec2::new(text_w + 4.0, 26.0),
                                        Sense::click(),
                                    );
                                    let fg = if active {
                                        pal.primary_hover
                                    } else if tab_resp.hovered() {
                                        pal.text_primary
                                    } else {
                                        pal.text_secondary
                                    };
                                    ui.painter().text(
                                        Pos2::new(tab_rect.center().x, tab_rect.top() + 8.0),
                                        egui::Align2::CENTER_CENTER,
                                        &label,
                                        FontId::proportional(12.5),
                                        fg,
                                    );
                                    if active {
                                        let line_y = tab_rect.bottom() - 1.0;
                                        ui.painter().line_segment(
                                            [
                                                Pos2::new(tab_rect.left(), line_y),
                                                Pos2::new(tab_rect.right(), line_y),
                                            ],
                                            Stroke::new(2.2_f32, pal.primary),
                                        );
                                    }
                                    if tab_resp.on_hover_cursor(CursorIcon::PointingHand).clicked()
                                    {
                                        self.thread_filter = filter;
                                        if filter == ThreadFilter::Groups {
                                            if let Some((gidx, gthread)) = self
                                                .threads
                                                .iter()
                                                .enumerate()
                                                .find(|(_, t)| t.is_group)
                                            {
                                                let gid = gthread.id.clone();
                                                self.selected_thread_index = gidx;
                                                if let Some(tx) = WORKER_TX.get() {
                                                    let _ = tx.send(WorkerCmd::LoadGroupHistory {
                                                        group_id: gid,
                                                        is_group: true,
                                                        have: 0,
                                                    });
                                                }
                                            }
                                            if group_tab_cnt == 0
                                                || self.threads.iter().any(|t| {
                                                    t.is_group
                                                        && crate::chat::is_generic_thread_name(
                                                            &t.name,
                                                        )
                                                })
                                            {
                                                if let Some(tx) = WORKER_TX.get() {
                                                    let _ = tx.send(WorkerCmd::RefreshThreads);
                                                }
                                            }
                                        }
                                    }
                                }

                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    let pinned_cnt = self
                                        .threads
                                        .iter()
                                        .filter(|t| {
                                            t.name.contains("Cloud của tôi")
                                                || t.presentation.pinned
                                                || self.settings.pinned_threads.contains(&t.id)
                                                || crate::chat::is_pinned_in_list(
                                                    &t.id,
                                                    t.is_group,
                                                    &self.settings.pinned_threads,
                                                )
                                        })
                                        .count();
                                    if pinned_cnt > 0 {
                                        ui.label(
                                            RichText::new(format!("Đã ghim ({pinned_cnt})"))
                                                .size(11.0)
                                                .color(pal.text_muted),
                                        );
                                    }
                                });
                            });
                        }
                    });

                ui.add(egui::Separator::default().spacing(1.0));

                // Sort threads: "Cloud của tôi" pinned first, then descending by latest message timestamp
                let query = self.search_query.trim().to_lowercase();
                let query_folded = fold_vn_search(&query);
                let query_digits: String = self
                    .search_query
                    .chars()
                    .filter(|c| c.is_ascii_digit())
                    .collect();
                let is_phone_like_query = query_digits.len() >= 8
                    && self
                        .search_query
                        .trim()
                        .chars()
                        .all(|c| c.is_ascii_digit() || matches!(c, '+' | ' ' | '-' | '.'));

                if is_phone_like_query {
                    let raw_q = self.search_query.trim().to_string();
                    egui::Frame::none()
                        .fill(pal.primary.linear_multiply(0.14))
                        .stroke(Stroke::new(1.0_f32, pal.primary))
                        .rounding(Rounding::same(9.0))
                        .inner_margin(Margin::symmetric(10.0, 7.0))
                        .outer_margin(Margin::symmetric(8.0, 4.0))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                let (ir, _) =
                                    ui.allocate_exact_size(Vec2::splat(20.0), Sense::hover());
                                draw_icon_user_plus(
                                    ui.painter(),
                                    ir.center(),
                                    16.0,
                                    pal.primary_hover,
                                );
                                ui.vertical(|ui| {
                                    ui.label(
                                        RichText::new(format!("Tìm Zalo qua SĐT: {raw_q}"))
                                            .size(12.2)
                                            .color(pal.text_primary)
                                            .strong(),
                                    );
                                    ui.label(
                                        RichText::new("Nhấn nút Tìm hoặc phím Enter để tra cứu")
                                            .size(10.8)
                                            .color(pal.text_secondary),
                                    );
                                });
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    let btn = egui::Button::new(
                                        RichText::new("Tìm SĐT")
                                            .size(11.5)
                                            .color(Color32::WHITE)
                                            .strong(),
                                    )
                                    .fill(pal.primary)
                                    .rounding(Rounding::same(6.0));
                                    if ui
                                        .add(btn)
                                        .on_hover_cursor(CursorIcon::PointingHand)
                                        .clicked()
                                    {
                                        self.phone_modal_open = true;
                                        self.phone_modal_input = raw_q.clone();
                                        self.phone_lookup_loading = true;
                                        self.phone_lookup_error = None;
                                        self.phone_lookup_result = None;
                                        if let Some(tx) = WORKER_TX.get() {
                                            let _ =
                                                tx.send(WorkerCmd::LookupPhone { phone: raw_q });
                                        }
                                    }
                                });
                            });
                        });
                }

                let search_outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let filtered_indices: Vec<usize> = self
                        .threads
                        .iter()
                        .enumerate()
                        .filter(|(idx, t)| {
                            if query.is_empty() && self.settings.hidden_threads.contains(&t.id) {
                                return false;
                            }
                            let pass_filter = match self.thread_filter {
                                ThreadFilter::All => true,
                                ThreadFilter::Direct => !t.is_group,
                                ThreadFilter::Groups => t.is_group,
                            };
                            let not_hidden = !t.presentation.hidden || !query.is_empty();
                            // Like official Zalo: hide contacts that have never had any messages unless
                            // actively searching, explicitly enabled in settings, or currently selected.
                            let has_messages = !t.messages.is_empty()
                                || (!t.preview.trim().is_empty()
                                    && !t.preview.starts_with("Nhóm · ")
                                    && t.preview != "Nhóm trò chuyện"
                                    && t.preview != "Chưa có tin nhắn")
                                || t.name.contains("Cloud của tôi")
                                || !query.is_empty();

                            pass_filter
                                && not_hidden
                                && has_messages
                                && (query.is_empty()
                                    || matches_vn_query(&t.name, &query, &query_folded)
                                    || t.id.contains(&query))
                        })
                        .map(|(i, _)| i)
                        .collect();

                    // Compute the pinned flag and last timestamp once per thread.
                    // The previous comparator re-read `pinned_threads` and the
                    // message tail on every comparison, turning a cheap sort into
                    // O(n log n) hash/lookup work on every frame.
                    let mut sort_entries: Vec<(bool, i64, usize)> = filtered_indices
                        .iter()
                        .map(|&i| {
                            let t = &self.threads[i];
                            let pinned = t.name.contains("Cloud của tôi")
                                || t.presentation.pinned
                                || self.settings.pinned_threads.contains(&t.id)
                                || crate::chat::is_pinned_in_list(
                                    &t.id,
                                    t.is_group,
                                    &self.settings.pinned_threads,
                                );
                            let ts = t.sidebar_activity_ts();
                            (pinned, ts, i)
                        })
                        .collect();
                    sidebar_sort_by_key(&mut sort_entries);
                    let filtered_indices: Vec<usize> =
                        sort_entries.into_iter().map(|(_, _, i)| i).collect();

                    let msg_hits: Vec<(usize, String, String, String, bool)> =
                        if query.chars().count() >= 2 {
                            let mut hits = Vec::new();
                            for (t_idx, t) in self.threads.iter().enumerate() {
                                for m in t.messages.iter().rev() {
                                    if matches_vn_query(&m.text, &query, &query_folded) {
                                        hits.push((
                                            t_idx,
                                            t.name.clone(),
                                            m.text.clone(),
                                            m.time.clone(),
                                            t.is_group,
                                        ));
                                        if hits.len() >= 25 {
                                            break;
                                        }
                                    }
                                }
                                if hits.len() >= 25 {
                                    break;
                                }
                            }
                            hits
                        } else {
                            Vec::new()
                        };
                    (filtered_indices, msg_hits)
                }));
                let (filtered_indices, msg_hits) = match search_outcome {
                    Ok(pair) => pair,
                    Err(_) => {
                        self.search_query.clear();
                        self.show_toast("⚠️ Đã đặt lại bộ lọc tìm kiếm");
                        ((0..self.threads.len()).collect(), Vec::new())
                    }
                };

                let mut clicked_thread: Option<(usize, String, bool)> = None;
                let mut clicked_msg_hit: Option<(usize, String)> = None;
                #[derive(Clone)]
                enum ThreadCtxAction {
                    TogglePin(String, bool, bool),
                    SetLabel(String, Option<String>),
                    ToggleUnread(String, bool, bool),
                    SetMute(String, bool, Option<i64>, String),
                    ToggleHide(String, bool, bool),
                    OpenSchedule(usize),
                    DeleteConv(usize, String, String, bool, String),
                    ReportConv(String),
                }
                let mut pending_ctx_action: Option<ThreadCtxAction> = None;
                let now_unix = crate::login::unix_now() as i64;
                let row_target_w = (ui.available_width() - 8.0).max(44.0);
                egui::ScrollArea::vertical()
                    .id_salt("threads_list_scroll")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.add_space(4.0);
                        for &i in &filtered_indices {
                            let t = &self.threads[i];
                            let is_selected = i == self.selected_thread_index;
                            let is_user_pinned = t.presentation.pinned
                                || self.settings.pinned_threads.contains(&t.id)
                                || crate::chat::is_pinned_in_list(
                                    &t.id,
                                    t.is_group,
                                    &self.settings.pinned_threads,
                                );
                            let is_pinned_cloud =
                                t.name.contains("Cloud của tôi") || is_user_pinned;
                            let is_muted = self
                                .settings
                                .muted_threads
                                .get(&t.id)
                                .map(|&until| until == -1 || until > now_unix)
                                .unwrap_or(false)
                                || t.presentation.muted
                                || t.unread.is_muted();
                            let is_hidden = self.settings.hidden_threads.contains(&t.id)
                                || t.presentation.hidden;
                            let cur_label = self.settings.thread_labels.get(&t.id).cloned();
                            let row_height = if is_narrow_w { 50.0 } else { 58.0 };
                            let (alloc_rect, row_resp) = ui.allocate_exact_size(
                                Vec2::new(row_target_w, row_height),
                                Sense::click(),
                            );
                            let row_rect = alloc_rect.translate(Vec2::new(4.0, 0.0));
                            // Space is still allocated so the scrollbar is right, but off-screen rows
                            // skip painting, and with it the avatar download/texture for every thread.
                            if !ui.is_rect_visible(row_rect) {
                                continue;
                            }

                            let bg = if is_selected {
                                pal.bg_selected
                            } else if row_resp.hovered() {
                                pal.bg_hover
                            } else {
                                Color32::TRANSPARENT
                            };

                            ui.painter().rect_filled(row_rect, Rounding::same(11.0), bg);

                            // Left active indicator bar (`w-1 bg-zalo-500 rounded-r-full`)
                            if is_selected {
                                let bar_rect = Rect::from_min_size(
                                    Pos2::new(row_rect.left() + 1.0, row_rect.top() + 10.0),
                                    Vec2::new(3.5, 38.0),
                                );
                                ui.painter().rect_filled(
                                    bar_rect,
                                    Rounding::same(2.0),
                                    pal.primary,
                                );
                            }

                            // Avatar circle with real user/group image & Cutout Status Dot
                            let av_radius = if is_narrow_w { 18.0 } else { 20.5 };
                            let avatar_center = if is_narrow_w {
                                row_rect.center()
                            } else {
                                Pos2::new(row_rect.left() + 29.0, row_rect.center().y)
                            };
                            paint_user_avatar(
                                ui.painter(),
                                avatar_center,
                                av_radius,
                                &t.name,
                                t.is_group,
                                t.avatar.as_deref(),
                                &self.sticker_textures,
                                &mut self.loading_stickers,
                            );
                            // Cutout status dot (`2.5px` ring matching row background) only when user is actively online
                            let is_user_online = !t.is_group
                                && self
                                    .user_online_map
                                    .get(&t.id)
                                    .map(|(onl, _)| *onl)
                                    .unwrap_or(false);
                            if is_user_online {
                                let cutout_col = if is_selected {
                                    pal.bg_selected
                                } else {
                                    pal.bg_sidebar
                                };
                                let badge_pos = Pos2::new(
                                    avatar_center.x + av_radius * 0.71,
                                    avatar_center.y + av_radius * 0.71,
                                );
                                ui.painter().circle_filled(badge_pos, 6.0, cutout_col);
                                ui.painter()
                                    .circle_filled(badge_pos, 4.0, pal.accent_online);
                            }
                            // Only show text labels when sidebar is expanded
                            if !is_narrow_w {
                                // Thread Name
                                let text_left = row_rect.left() + 58.0;
                                let name_color = if is_selected && pal.is_dark {
                                    Color32::WHITE
                                } else {
                                    pal.text_primary
                                };
                                let max_name_chars = if is_compact_w { 15 } else { 21 };
                                let name_x = if t.is_group {
                                    draw_icon_contacts(
                                        ui.painter(),
                                        Pos2::new(text_left + 7.0, row_rect.top() + 19.5),
                                        12.5,
                                        pal.primary,
                                    );
                                    text_left + 18.0
                                } else {
                                    text_left
                                };
                                let display_name = truncate_chars(&t.name, max_name_chars);
                                ui.painter().text(
                                    Pos2::new(name_x, row_rect.top() + 12.0),
                                    egui::Align2::LEFT_TOP,
                                    display_name,
                                    FontId::proportional(13.6),
                                    name_color,
                                );

                                // Pin icon + Timestamp (top-right)
                                let display_time = t.sidebar_timestamp(crate::login::unix_now() as i64 * 1000);
                                let time_right = row_rect.right() - 9.0;
                                if !display_time.is_empty() {
                                    let tw = ui.fonts(|f| {
                                        f.layout_no_wrap(
                                            display_time.clone(),
                                            FontId::proportional(11.0),
                                            pal.text_muted,
                                        )
                                        .size()
                                        .x
                                    });
                                    ui.painter().text(
                                        Pos2::new(time_right, row_rect.top() + 13.5),
                                        egui::Align2::RIGHT_TOP,
                                        &display_time,
                                        FontId::proportional(11.0),
                                        pal.text_muted,
                                    );
                                    let mut icon_right = time_right - tw - 9.0;
                                    if is_pinned_cloud {
                                        draw_icon_pin(
                                            ui.painter(),
                                            Pos2::new(icon_right, row_rect.top() + 19.5),
                                            11.0,
                                            pal.text_muted,
                                        );
                                        icon_right -= 14.0;
                                    }
                                    if is_muted {
                                        ui.painter().text(
                                            Pos2::new(icon_right + 4.0, row_rect.top() + 13.5),
                                            egui::Align2::RIGHT_TOP,
                                            "🔕",
                                            FontId::proportional(10.5),
                                            pal.text_muted,
                                        );
                                    }
                                }
                                if let Some(ref lbl) = cur_label {
                                    let tag_col = match lbl.as_str() {
                                        "Khách hàng" => Color32::from_rgb(239, 68, 68),
                                        "Gia đình" => Color32::from_rgb(34, 197, 94),
                                        "Công việc" => Color32::from_rgb(249, 115, 22),
                                        "Bạn bè" => Color32::from_rgb(168, 85, 247),
                                        "Trả lời sau" => Color32::from_rgb(234, 179, 8),
                                        _ => Color32::from_rgb(59, 130, 246),
                                    };
                                    ui.painter().circle_filled(
                                        Pos2::new(text_left - 8.0, row_rect.top() + 39.5),
                                        3.5,
                                        tag_col,
                                    );
                                }

                                // Snippet with `Bạn:` prefix when latest message is outgoing
                                let last_outgoing =
                                    t.messages.last().map(|m| m.is_outgoing).unwrap_or(false);
                                let is_typing_in_t = self.settings.show_typing_indicator
                                    && self
                                        .typing_map
                                        .get(&t.id)
                                        .map(|(_, until)| *until > std::time::Instant::now())
                                        .unwrap_or(false);
                                let raw_preview = if is_typing_in_t {
                                    let who = self
                                        .typing_map
                                        .get(&t.id)
                                        .map(|(w, _)| w.as_str())
                                        .unwrap_or("Đang");
                                    format!("✍ {who} đang soạn thảo…")
                                } else if t.preview.is_empty() {
                                    if t.is_group {
                                        "Nhóm trò chuyện".to_string()
                                    } else {
                                        "Chưa có tin nhắn".to_string()
                                    }
                                } else {
                                    let clean_prev = if t.preview.starts_with("[Hình ảnh] ")
                                        && [".png", ".jpg", ".jpeg", ".webp", ".gif", ".bmp"]
                                            .iter()
                                            .any(|ext| {
                                                t.preview.to_ascii_lowercase().ends_with(ext)
                                            }) {
                                        "[Hình ảnh]"
                                    } else {
                                        t.preview.as_str()
                                    };
                                    if last_outgoing && !clean_prev.starts_with("Bạn:") {
                                        format!("Bạn: {clean_prev}")
                                    } else {
                                        clean_prev.to_string()
                                    }
                                };
                                let unread_n = self.unread_counts.get(&t.id).copied().unwrap_or(0);
                                let max_snip_chars = if is_compact_w {
                                    16
                                } else if unread_n > 0 {
                                    23
                                } else {
                                    28
                                };
                                let snippet = truncate_chars(&raw_preview, max_snip_chars);
                                ui.painter().text(
                                    Pos2::new(text_left, row_rect.top() + 33.5),
                                    egui::Align2::LEFT_TOP,
                                    snippet,
                                    FontId::proportional(12.0),
                                    if is_typing_in_t {
                                        pal.primary
                                    } else if unread_n > 0 {
                                        pal.text_primary
                                    } else {
                                        pal.text_secondary
                                    },
                                );
                                if unread_n > 0 {
                                    let badge_txt = if unread_n > 99 {
                                        "99+".to_string()
                                    } else {
                                        unread_n.to_string()
                                    };
                                    let bw = if unread_n >= 10 { 22.0 } else { 18.0 };
                                    let b_rect = Rect::from_min_size(
                                        Pos2::new(
                                            row_rect.right() - bw - 8.0,
                                            row_rect.top() + 31.5,
                                        ),
                                        Vec2::new(bw, 17.0),
                                    );
                                    ui.painter().rect_filled(
                                        b_rect,
                                        Rounding::same(8.5),
                                        pal.danger,
                                    );
                                    ui.painter().text(
                                        b_rect.center(),
                                        egui::Align2::CENTER_CENTER,
                                        badge_txt,
                                        FontId::proportional(10.2),
                                        Color32::WHITE,
                                    );
                                }
                            }

                            let row_resp = row_resp.on_hover_cursor(CursorIcon::PointingHand);
                            let row_resp = if is_narrow_w {
                                row_resp.on_hover_text(&t.name)
                            } else {
                                row_resp
                            };
                            let tid_cl = t.id.clone();
                            let tname_cl = t.name.clone();
                            let tgroup_cl = t.is_group;
                            let unread_now =
                                self.unread_counts.get(&t.id).copied().unwrap_or(0) > 0;
                            let last_msg_id_cl =
                                t.messages.last().map(|m| m.id.clone()).unwrap_or_default();
                            row_resp.context_menu(|ui| {
                                ui.set_min_width(205.0);
                                ui.spacing_mut().item_spacing = Vec2::new(0.0, 2.0);
                                ui.spacing_mut().button_padding = Vec2::new(10.0, 6.5);

                                // 1. Ghim / Bỏ ghim hội thoại
                                let pin_label = if is_user_pinned {
                                    "Bỏ ghim hội thoại"
                                } else {
                                    "Ghim hội thoại"
                                };
                                if ui
                                    .add(
                                        egui::Button::new(
                                            RichText::new(pin_label)
                                                .size(13.5)
                                                .color(pal.text_primary),
                                        )
                                        .frame(false)
                                        .min_size(Vec2::new(195.0, 28.0)),
                                    )
                                    .clicked()
                                {
                                    pending_ctx_action = Some(ThreadCtxAction::TogglePin(
                                        tid_cl.clone(),
                                        tgroup_cl,
                                        !is_user_pinned,
                                    ));
                                    ui.close_menu();
                                }

                                ui.separator();

                                // 2. Phân loại >
                                ui.menu_button(
                                    RichText::new("Phân loại")
                                        .size(13.5)
                                        .color(pal.text_primary),
                                    |ui| {
                                        ui.set_min_width(185.0);
                                        ui.spacing_mut().item_spacing = Vec2::new(0.0, 2.0);
                                        ui.spacing_mut().button_padding = Vec2::new(10.0, 6.0);
                                        let categories = [
                                            ("Khách hàng", Color32::from_rgb(239, 68, 68)),
                                            ("Gia đình", Color32::from_rgb(34, 197, 94)),
                                            ("Công việc", Color32::from_rgb(249, 115, 22)),
                                            ("Bạn bè", Color32::from_rgb(168, 85, 247)),
                                            ("Trả lời sau", Color32::from_rgb(234, 179, 8)),
                                            ("Đồng nghiệp", Color32::from_rgb(59, 130, 246)),
                                        ];
                                        for (cat_name, dot_col) in categories {
                                            let is_cur = cur_label.as_deref() == Some(cat_name);
                                            let txt = if is_cur {
                                                format!("● {cat_name}  {}", lucide::CHECK)
                                            } else {
                                                format!("● {cat_name}")
                                            };
                                            if ui
                                                .add(
                                                    egui::Button::new(
                                                        RichText::new(txt)
                                                            .size(13.2)
                                                            .color(dot_col),
                                                    )
                                                    .frame(false)
                                                    .min_size(Vec2::new(175.0, 26.0)),
                                                )
                                                .clicked()
                                            {
                                                let next_lbl = if is_cur {
                                                    None
                                                } else {
                                                    Some(cat_name.to_string())
                                                };
                                                pending_ctx_action =
                                                    Some(ThreadCtxAction::SetLabel(
                                                        tid_cl.clone(),
                                                        next_lbl,
                                                    ));
                                                ui.close_menu();
                                            }
                                        }
                                        if cur_label.is_some() {
                                            ui.separator();
                                            if ui
                                                .add(
                                                    egui::Button::new(
                                                        RichText::new("Bỏ phân loại")
                                                            .size(13.0)
                                                            .color(pal.text_secondary),
                                                    )
                                                    .frame(false)
                                                    .min_size(Vec2::new(175.0, 26.0)),
                                                )
                                                .clicked()
                                            {
                                                pending_ctx_action = Some(
                                                    ThreadCtxAction::SetLabel(tid_cl.clone(), None),
                                                );
                                                ui.close_menu();
                                            }
                                        }
                                    },
                                );

                                // 3. Đánh dấu chưa đọc / Đánh dấu đã đọc
                                let unread_label = if unread_now {
                                    "Đánh dấu đã đọc"
                                } else {
                                    "Đánh dấu chưa đọc"
                                };
                                if ui
                                    .add(
                                        egui::Button::new(
                                            RichText::new(unread_label)
                                                .size(13.5)
                                                .color(pal.text_primary),
                                        )
                                        .frame(false)
                                        .min_size(Vec2::new(195.0, 28.0)),
                                    )
                                    .clicked()
                                {
                                    pending_ctx_action = Some(ThreadCtxAction::ToggleUnread(
                                        tid_cl.clone(),
                                        tgroup_cl,
                                        !unread_now,
                                    ));
                                    ui.close_menu();
                                }

                                ui.separator();

                                // 4. Tắt thông báo > (hoặc Bật lại thông báo)
                                if is_muted {
                                    if ui
                                        .add(
                                            egui::Button::new(
                                                RichText::new("Mở lại thông báo")
                                                    .size(13.5)
                                                    .color(pal.text_primary),
                                            )
                                            .frame(false)
                                            .min_size(Vec2::new(195.0, 28.0)),
                                        )
                                        .clicked()
                                    {
                                        pending_ctx_action = Some(ThreadCtxAction::SetMute(
                                            tid_cl.clone(),
                                            tgroup_cl,
                                            None,
                                            "Đã mở lại thông báo hội thoại".into(),
                                        ));
                                        ui.close_menu();
                                    }
                                } else {
                                    ui.menu_button(
                                        RichText::new("Tắt thông báo")
                                            .size(13.5)
                                            .color(pal.text_primary),
                                        |ui| {
                                            ui.set_min_width(195.0);
                                            ui.spacing_mut().item_spacing = Vec2::new(0.0, 2.0);
                                            ui.spacing_mut().button_padding = Vec2::new(10.0, 6.5);
                                            let rem_today = 86_400 - (now_unix % 86_400);
                                            let dur_8am = (rem_today + 8 * 3600) % 86_400;
                                            let dur_8am = if dur_8am < 600 {
                                                dur_8am + 86_400
                                            } else {
                                                dur_8am
                                            };
                                            let opts: [(&str, i64); 4] = [
                                                ("Trong 1 giờ", 3600),
                                                ("Trong 4 giờ", 14_400),
                                                ("Cho đến 8:00 AM", dur_8am),
                                                ("Cho đến khi được mở lại", -1),
                                            ];
                                            for (opt_label, dur_s) in opts {
                                                if ui
                                                    .add(
                                                        egui::Button::new(
                                                            RichText::new(opt_label)
                                                                .size(13.5)
                                                                .color(pal.text_primary),
                                                        )
                                                        .frame(false)
                                                        .min_size(Vec2::new(185.0, 28.0)),
                                                    )
                                                    .clicked()
                                                {
                                                    pending_ctx_action =
                                                        Some(ThreadCtxAction::SetMute(
                                                            tid_cl.clone(),
                                                            tgroup_cl,
                                                            Some(dur_s),
                                                            format!(
                                                                "🔕 Đã tắt thông báo ({opt_label})"
                                                            ),
                                                        ));
                                                    ui.close_menu();
                                                }
                                            }
                                        },
                                    );
                                }

                                // 5. Ẩn trò chuyện / Bỏ ẩn trò chuyện
                                let hide_label = if is_hidden {
                                    "Bỏ ẩn trò chuyện"
                                } else {
                                    "Ẩn trò chuyện"
                                };
                                if ui
                                    .add(
                                        egui::Button::new(
                                            RichText::new(hide_label)
                                                .size(13.5)
                                                .color(pal.text_primary),
                                        )
                                        .frame(false)
                                        .min_size(Vec2::new(195.0, 28.0)),
                                    )
                                    .clicked()
                                {
                                    pending_ctx_action = Some(ThreadCtxAction::ToggleHide(
                                        tid_cl.clone(),
                                        tgroup_cl,
                                        !is_hidden,
                                    ));
                                    ui.close_menu();
                                }

                                // 5b. Hẹn giờ gửi tin nhắn / Nhắc hẹn (#38)
                                if ui
                                    .add(
                                        egui::Button::new(
                                            RichText::new("Hẹn giờ gửi tin nhắn / Nhắc hẹn")
                                                .size(13.5)
                                                .color(pal.text_primary),
                                        )
                                        .frame(false)
                                        .min_size(Vec2::new(195.0, 28.0)),
                                    )
                                    .clicked()
                                {
                                    pending_ctx_action = Some(ThreadCtxAction::OpenSchedule(i));
                                    ui.close_menu();
                                }

                                ui.separator();

                                // 6. Xóa hội thoại (màu đỏ)
                                if ui
                                    .add(
                                        egui::Button::new(
                                            RichText::new("Xóa hội thoại")
                                                .size(13.5)
                                                .color(Color32::from_rgb(239, 68, 68)),
                                        )
                                        .frame(false)
                                        .min_size(Vec2::new(195.0, 28.0)),
                                    )
                                    .clicked()
                                {
                                    pending_ctx_action = Some(ThreadCtxAction::DeleteConv(
                                        i,
                                        tid_cl.clone(),
                                        tname_cl.clone(),
                                        tgroup_cl,
                                        last_msg_id_cl.clone(),
                                    ));
                                    ui.close_menu();
                                }

                                ui.separator();

                                // 7. Báo xấu
                                if ui
                                    .add(
                                        egui::Button::new(
                                            RichText::new("Báo xấu")
                                                .size(13.5)
                                                .color(pal.text_primary),
                                        )
                                        .frame(false)
                                        .min_size(Vec2::new(195.0, 28.0)),
                                    )
                                    .clicked()
                                {
                                    pending_ctx_action =
                                        Some(ThreadCtxAction::ReportConv(tname_cl.clone()));
                                    ui.close_menu();
                                }
                            });

                            if row_resp.clicked() {
                                clicked_thread = Some((i, t.id.clone(), t.is_group));
                            }
                        }

                        // Global Message Content Matches (`Tin nhắn tìm thấy`)
                        if !msg_hits.is_empty() {
                            ui.add_space(8.0);
                            ui.horizontal(|ui| {
                                ui.add_space(12.0);
                                ui.label(
                                    RichText::new(format!(
                                        "TIN NHẮN TÌM THẤY ({})",
                                        msg_hits.len()
                                    ))
                                    .size(11.0)
                                    .color(pal.primary_hover)
                                    .strong(),
                                );
                            });
                            ui.add_space(4.0);
                            for (t_idx, t_name, m_text, m_time, t_group) in &msg_hits {
                                let (alloc_r, r_resp) = ui.allocate_exact_size(
                                    Vec2::new(row_target_w, 52.0),
                                    Sense::click(),
                                );
                                let r_rect = alloc_r.translate(Vec2::new(4.0, 0.0));
                                if r_resp.hovered() {
                                    ui.painter().rect_filled(
                                        r_rect,
                                        Rounding::same(10.0),
                                        pal.bg_hover,
                                    );
                                }
                                let av_c = Pos2::new(r_rect.left() + 24.0, r_rect.center().y);
                                let hit_ava =
                                    self.threads.get(*t_idx).and_then(|th| th.avatar.as_deref());
                                paint_user_avatar(
                                    ui.painter(),
                                    av_c,
                                    16.0,
                                    t_name,
                                    *t_group,
                                    hit_ava,
                                    &self.sticker_textures,
                                    &mut self.loading_stickers,
                                );
                                let tl = r_rect.left() + 50.0;
                                ui.painter().text(
                                    Pos2::new(tl, r_rect.top() + 9.0),
                                    egui::Align2::LEFT_TOP,
                                    truncate_chars(t_name, 20),
                                    FontId::proportional(12.8),
                                    pal.text_primary,
                                );
                                if !m_time.is_empty() {
                                    ui.painter().text(
                                        Pos2::new(r_rect.right() - 10.0, r_rect.top() + 10.0),
                                        egui::Align2::RIGHT_TOP,
                                        m_time,
                                        FontId::proportional(10.5),
                                        pal.text_muted,
                                    );
                                }
                                ui.painter().text(
                                    Pos2::new(tl, r_rect.top() + 28.0),
                                    egui::Align2::LEFT_TOP,
                                    truncate_chars(m_text, 30),
                                    FontId::proportional(11.8),
                                    pal.primary_hover,
                                );
                                if r_resp
                                    .on_hover_cursor(CursorIcon::PointingHand)
                                    .on_hover_text(m_text)
                                    .clicked()
                                {
                                    clicked_msg_hit = Some((*t_idx, query.clone()));
                                }
                            }
                        }

                        if !query.is_empty() && filtered_indices.is_empty() && msg_hits.is_empty() {
                            ui.vertical_centered(|ui| {
                                ui.add_space(32.0);
                                ui.label(
                                    RichText::new(
                                        "Không tìm thấy hội thoại hoặc tin nhắn phù hợp.",
                                    )
                                    .size(12.5)
                                    .color(pal.text_muted),
                                );
                            });
                        }
                    });

                if let Some((idx, q_str)) = clicked_msg_hit {
                    self.selected_thread_index = idx;
                    self.in_chat_search_open = true;
                    self.in_chat_search_focus_pending = true;
                    self.in_chat_search_query = q_str;
                }
                if let Some((idx, tid, is_group)) = clicked_thread {
                    let resolved_idx = self.threads.iter().position(|t| t.id == tid).unwrap_or(idx);
                    if self.selected_thread_index != resolved_idx {
                        self.replying_to = None;
                        self.filter_starred_only = false;
                        self.force_scroll_to_bottom = true;
                        self.chat_render_window = (tid.clone(), CHAT_RENDER_STEP);
                    }
                    self.selected_thread_index = resolved_idx;
                    if self.unread_counts.remove(&tid).is_some() {
                        let total_unread: usize = self.unread_counts.values().sum();
                        let title = if total_unread > 0 {
                            format!("({total_unread}) Zalo Lite — Native Rust Client")
                        } else {
                            "Zalo Lite — Native Rust Client".to_string()
                        };
                        ctx.send_viewport_cmd(egui::ViewportCommand::Title(title));
                    }
                    // Persist the read-point so reopening the app does not recount read messages (#119).
                    if let Some(t) = self.threads.get_mut(resolved_idx) {
                        if let Some(last) = t.messages.last() {
                            let mid = last.id.clone();
                            t.unread.last_seen_id = Some(mid.clone());
                            let tid_c = tid.clone();
                            let uin_c = self.user_uin.clone();
                            std::thread::spawn(move || {
                                let db = crate::store::db_path_for_account(&uin_c);
                                if let Ok(mut store) = crate::store::MessageStore::open(&db) {
                                    let _ = store.set_thread_read(&tid_c, &mid);
                                }
                            });
                        }
                    }
                    if !self.settings.block_seen {
                        let last_mid = self
                            .threads
                            .get(resolved_idx)
                            .and_then(|t| t.messages.last())
                            .map(|m| m.id.clone())
                            .unwrap_or_default();
                        if !last_mid.is_empty() {
                            if let Some(tx) = WORKER_TX.get() {
                                let _ = tx.send(WorkerCmd::SendSeen {
                                    thread_id: tid.clone(),
                                    is_group,
                                    last_msg_id: last_mid,
                                });
                            }
                        }
                    }
                    let is_cloud = self
                        .threads
                        .get(resolved_idx)
                        .map(|t| t.name.contains("Cloud của tôi"))
                        .unwrap_or(false);
                    let needs_history = self
                        .threads
                        .get(resolved_idx)
                        .map_or(true, |t| t.messages.is_empty());
                    if needs_history {
                        if let Some(tx) = WORKER_TX.get() {
                            let _ = tx.send(WorkerCmd::LoadGroupHistory {
                                group_id: tid,
                                is_group,
                                have: 0,
                            });
                        }
                    }
                }
                if let Some(act) = pending_ctx_action {
                    match act {
                        ThreadCtxAction::TogglePin(tid, is_group, pin) => {
                            if pin {
                                if !self.settings.pinned_threads.contains(&tid) {
                                    self.settings.pinned_threads.push(tid.clone());
                                }
                                self.show_toast("📌 Đã ghim hội thoại");
                            } else {
                                self.settings.pinned_threads.retain(|x| x != &tid);
                                self.show_toast("Đã bỏ ghim hội thoại");
                            }
                            if let Some(th) = self.threads.iter_mut().find(|t| t.id == tid) {
                                th.presentation.pinned = pin;
                            }
                            self.settings.save();
                            self.persist_threads_async();
                            if let Some(tx) = WORKER_TX.get() {
                                let _ = tx.send(WorkerCmd::PinThread {
                                    thread_id: tid,
                                    is_group,
                                    pin,
                                });
                            }
                        }
                        ThreadCtxAction::SetLabel(tid, lbl_opt) => {
                            if let Some(lbl) = lbl_opt {
                                self.settings.thread_labels.insert(tid, lbl.clone());
                                self.show_toast(format!("🏷️ Đã phân loại: {lbl}"));
                            } else {
                                self.settings.thread_labels.remove(&tid);
                                self.show_toast("Đã bỏ thẻ phân loại");
                            }
                            self.settings.save();
                        }
                        ThreadCtxAction::ToggleUnread(tid, is_group, unread) => {
                            if unread {
                                self.unread_counts.insert(tid.clone(), 1);
                                self.show_toast("Đã đánh dấu chưa đọc");
                            } else {
                                self.unread_counts.remove(&tid);
                                self.show_toast("Đã đánh dấu đã đọc");
                            }
                            let total_unread: usize = self.unread_counts.values().sum();
                            let title = if total_unread > 0 {
                                format!("({total_unread}) Zalo Lite — Native Rust Client")
                            } else {
                                "Zalo Lite — Native Rust Client".to_string()
                            };
                            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title));
                            if let Some(tx) = WORKER_TX.get() {
                                let _ = tx.send(WorkerCmd::MarkThreadUnread {
                                    thread_id: tid,
                                    is_group,
                                    unread,
                                });
                            }
                        }
                        ThreadCtxAction::SetMute(tid, is_group, dur_opt, toast_txt) => {
                            let (dur_sec, mute) = match dur_opt {
                                Some(-1) => {
                                    self.settings.muted_threads.insert(tid.clone(), -1);
                                    (-1, true)
                                }
                                Some(d) => {
                                    self.settings
                                        .muted_threads
                                        .insert(tid.clone(), now_unix + d);
                                    (d, true)
                                }
                                None => {
                                    self.settings.muted_threads.remove(&tid);
                                    (-1, false)
                                }
                            };
                            if let Some(th) = self.threads.iter_mut().find(|t| t.id == tid) {
                                th.presentation.muted = mute;
                                th.unread.muted = mute;
                                th.unread.muted_until = if mute && dur_sec > 0 {
                                    Some(now_unix + dur_sec)
                                } else {
                                    None
                                };
                                th.unread.settings_fresh = true;
                                th.unread.fresh = true;
                            }
                            self.settings.save();
                            self.persist_threads_async();
                            self.show_toast(toast_txt);
                            if let Some(tx) = WORKER_TX.get() {
                                let _ = tx.send(WorkerCmd::MuteThread {
                                    thread_id: tid,
                                    is_group,
                                    duration_sec: dur_sec,
                                    mute,
                                });
                            }
                        }
                        ThreadCtxAction::ToggleHide(tid, is_group, hide) => {
                            if hide {
                                self.settings.hidden_threads.insert(tid.clone());
                                self.show_toast(
                                    "🙈 Đã ẩn trò chuyện (gõ tên vào ô Tìm kiếm để mở lại)",
                                );
                            } else {
                                self.settings.hidden_threads.remove(&tid);
                                self.show_toast("Đã bỏ ẩn trò chuyện");
                            }
                            if let Some(th) = self.threads.iter_mut().find(|t| t.id == tid) {
                                th.presentation.hidden = hide;
                            }
                            self.settings.save();
                            self.persist_threads_async();
                            if let Some(tx) = WORKER_TX.get() {
                                let _ = tx.send(WorkerCmd::HideThread {
                                    thread_id: tid,
                                    is_group,
                                    hide,
                                });
                            }
                        }
                        ThreadCtxAction::OpenSchedule(idx) => {
                            self.selected_thread_index = idx;
                            self.schedule_modal_open = true;
                        }
                        ThreadCtxAction::DeleteConv(idx, tid, tname, is_group, last_msg_id) => {
                            if let Ok(mut store) =
                                crate::store::MessageStore::open("zalo_history.db")
                            {
                                let _ = store.delete_thread(&tid);
                            }
                            if idx < self.threads.len() && self.threads[idx].id == tid {
                                self.threads.remove(idx);
                                if self.selected_thread_index >= self.threads.len() {
                                    self.selected_thread_index =
                                        self.threads.len().saturating_sub(1);
                                }
                                self.unread_counts.remove(&tid);
                                self.settings.pinned_threads.retain(|x| x != &tid);
                                self.settings.save();
                                self.persist_threads_async();
                            }
                            self.show_toast(format!("🗑️ Đã xóa hội thoại \"{tname}\""));
                            if let Some(tx) = WORKER_TX.get() {
                                let _ = tx.send(WorkerCmd::DeleteThread {
                                    thread_id: tid,
                                    is_group,
                                    last_cli_msg_id: last_msg_id,
                                });
                            }
                        }
                        ThreadCtxAction::ReportConv(tname) => {
                            self.show_toast(format!(
                                "🚩 Đã gửi báo xấu hội thoại \"{tname}\" tới hệ thống Zalo"
                            ));
                        }
                    }
                }
            });

        // Extract lightweight metadata for Header, Right Info Drawer, and Central Chat Stream
        let thread_meta = self.threads.get(self.selected_thread_index).map(|t| {
            (
                t.id.clone(),
                t.name.clone(),
                t.avatar.clone(),
                t.is_group,
                t.messages.len(),
            )
        });

        // -------------------------------------------------------------------
        // 3. COLLAPSIBLE RIGHT CONVERSATION INFO DRAWER (responsive width)
        // -------------------------------------------------------------------
        let mut open_chat_member: Option<crate::chat::SidebarMember> = None;
        let mut view_profile_member: Option<String> = None;
        let show_info_drawer = self.info_drawer_open && !is_compact_w;
        let drawer_w = (screen_w * 0.26).clamp(240.0, 310.0);

        if show_info_drawer {
            if let Some((ref d_id, ref d_name, ref d_ava, d_is_group, d_msg_count)) = thread_meta {
                let (shared_photos, shared_files, shared_links) = self
                    .threads
                    .get(self.selected_thread_index)
                    .map(|t| {
                        let mut photos = Vec::new();
                        let mut files = Vec::new();
                        let mut links = Vec::new();
                        for m in t.messages.iter().rev() {
                            let is_sticker = is_sticker_message(m).is_some();
                            if is_sticker {
                                continue;
                            }
                            let is_photo = m.link_src.as_deref() == Some("Hình ảnh Zalo")
                                || m.text.trim() == "[Hình ảnh]"
                                || m.text.trim().starts_with("[Hình ảnh] ");
                            let is_file = m.text.trim().starts_with("[Tập tin] ")
                                || m.link_src.as_deref() == Some("Tập tin Zalo")
                                || is_zalo_file_or_video_message(m);
                            if is_photo && photos.len() < 24 {
                                if let Some(thumb) = m.link_thumb.as_deref().or(m.link_url.as_deref()).filter(|u| !u.is_empty()) {
                                    let full = m.link_url.as_deref().unwrap_or(thumb).to_string();
                                    let title = m.link_title.clone().unwrap_or_else(|| "Hình ảnh".into());
                                    photos.push((format!("thumb_{}", crate::crypto::md5_hex(thumb)), thumb.to_string(), full, title));
                                }
                            } else if is_file && files.len() < 20 {
                                let fname = m.text.strip_prefix("[Tập tin] ").unwrap_or(&m.text).trim().to_string();
                                files.push((fname, m.time.clone(), m.link_url.clone().unwrap_or_default()));
                            } else if let Some(ref url) = m.link_url {
                                if !url.is_empty() && !is_photo && !is_file && links.len() < 20 {
                                    let ltitle = m.link_title.clone().unwrap_or_else(|| url.clone());
                                    links.push((ltitle, url.clone()));
                                }
                            }
                        }
                        (photos, files, links)
                    })
                    .unwrap_or_default();

                egui::SidePanel::right("conversation_info_drawer")
                    .resizable(false)
                    .exact_width(drawer_w)
                    .frame(
                        egui::Frame::none()
                            .fill(pal.bg_sidebar)
                            .stroke(Stroke::new(1.0_f32, pal.border)),
                    )
                    .show(ctx, |ui| {
                        ui.set_max_width(drawer_w);
                        egui::Frame::none()
                            .inner_margin(Margin::symmetric(16.0, 18.0))
                            .show(ui, |ui| {
                                ui.vertical_centered(|ui| {
                                    let (av_rect, _) = ui
                                        .allocate_exact_size(Vec2::new(64.0, 64.0), Sense::hover());
                                    paint_user_avatar(
                                        ui.painter(),
                                        av_rect.center(),
                                        32.0,
                                        d_name,
                                        d_is_group,
                                        d_ava.as_deref(),
                                        &self.sticker_textures,
                                        &mut self.loading_stickers,
                                    );
                                    ui.add_space(10.0);
                                    ui.label(
                                        RichText::new(d_name)
                                            .size(14.5)
                                            .color(pal.text_primary)
                                            .strong(),
                                    );
                                    ui.add_space(2.0);
                                    let d_is_cloud = d_name.contains("Cloud của tôi");
                                    let d_fr_status = if d_is_group || d_is_cloud {
                                        crate::chat::FriendshipStatus::Friend
                                    } else {
                                        self.friend_status_map
                                            .get(d_id)
                                            .copied()
                                            .unwrap_or(crate::chat::FriendshipStatus::Unknown)
                                    };
                                    let d_onl = self.user_online_map.get(d_id);
                                    let (sub_txt, sub_col): (String, Color32) = if d_is_group {
                                        ("Nhóm đang hoạt động".into(), pal.accent_online)
                                    } else if d_is_cloud {
                                        ("Lưu trữ đám mây cá nhân".into(), pal.primary_hover)
                                    } else {
                                        match d_fr_status {
                                            crate::chat::FriendshipStatus::Friend => match d_onl {
                                                Some((true, lbl)) if !lbl.is_empty() => (
                                                    format!("{} Đã kết bạn · {lbl}", lucide::CHECK),
                                                    pal.accent_online,
                                                ),
                                                Some((false, lbl)) if !lbl.is_empty() => (
                                                    format!("{} Đã kết bạn · {lbl}", lucide::CHECK),
                                                    pal.text_secondary,
                                                ),
                                                _ => (format!("{} Đã kết bạn", lucide::CHECK), pal.text_secondary),
                                            },
                                            crate::chat::FriendshipStatus::IncomingPending => (
                                                "📩 Đã gửi lời mời kết bạn cho bạn".into(),
                                                pal.primary_hover,
                                            ),
                                            crate::chat::FriendshipStatus::OutgoingPending => {
                                                ("⏳ Đã gửi lời mời kết bạn".into(), pal.warning)
                                            }
                                            crate::chat::FriendshipStatus::Stranger => (
                                                "Người lạ · Chưa kết bạn".into(),
                                                Color32::from_rgb(249, 115, 22),
                                            ),
                                            crate::chat::FriendshipStatus::Unknown => {
                                                ("Đang kiểm tra kết bạn…".into(), pal.text_muted)
                                            }
                                        }
                                    };
                                    ui.label(RichText::new(sub_txt).size(12.0).color(sub_col));

                                    ui.add_space(14.0);
                                    // Quick Action Buttons (Sync & Pin/Star Filter)
                                    ui.horizontal(|ui| {
                                        ui.add_space(34.0);
                                        let sync_btn = egui::Button::new(
                                            RichText::new("Đồng bộ")
                                                .size(11.5)
                                                .color(pal.text_secondary),
                                        )
                                        .fill(pal.bg_elevated)
                                        .stroke(Stroke::new(1.0_f32, pal.border))
                                        .rounding(Rounding::same(8.0))
                                        .min_size(Vec2::new(92.0, 30.0));
                                        if ui
                                            .add(sync_btn)
                                            .on_hover_cursor(CursorIcon::PointingHand)
                                            .clicked()
                                        {
                                            if let Some(tx) = WORKER_TX.get() {
                                                let _ = tx.send(WorkerCmd::RefreshThreads);
                                                let _ = tx.send(WorkerCmd::LoadGroupHistory {
                                                    group_id: d_id.clone(),
                                                    is_group: d_is_group,
                                                    have: d_msg_count,
                                                });
                                            }
                                            self.show_toast("Đang đồng bộ lịch sử hội thoại…");
                                        }

                                        let star_active = self.filter_starred_only;
                                        let star_btn =
                                            egui::Button::new(
                                                RichText::new(if star_active {
                                                    "Hiện tất cả"
                                                } else {
                                                    "Tin đánh dấu"
                                                })
                                                .size(11.5)
                                                .color(if star_active {
                                                    Color32::WHITE
                                                } else {
                                                    pal.text_secondary
                                                }),
                                            )
                                            .fill(if star_active {
                                                pal.primary
                                            } else {
                                                pal.bg_elevated
                                            })
                                            .stroke(Stroke::new(1.0_f32, pal.border))
                                            .rounding(Rounding::same(8.0))
                                            .min_size(Vec2::new(92.0, 30.0));
                                        if ui
                                            .add(star_btn)
                                            .on_hover_cursor(CursorIcon::PointingHand)
                                            .clicked()
                                        {
                                            self.filter_starred_only = !self.filter_starred_only;
                                        }
                                    });
                                    if !d_is_group && !d_is_cloud {
                                        ui.add_space(6.0);
                                        let prof_btn = egui::Button::new(
                                            RichText::new(format!(
                                                "{} Xem Hồ sơ & Nhật ký",
                                                lucide::NEWSPAPER
                                            ))
                                            .size(11.5)
                                            .color(pal.primary_hover),
                                        )
                                        .fill(pal.primary.linear_multiply(0.12))
                                        .stroke(Stroke::new(1.0_f32, pal.primary))
                                        .rounding(Rounding::same(8.0))
                                        .min_size(Vec2::new(192.0, 28.0));
                                        if ui
                                            .add(prof_btn)
                                            .on_hover_cursor(CursorIcon::PointingHand)
                                            .clicked()
                                        {
                                            self.feed_modal_open = true;
                                            self.feed_modal_tab = FeedModalTab::Profile;
                                            self.feed_target_uid = d_id.clone();
                                            self.feed_loading_profile = true;
                                            self.feed_profile_error = None;
                                            self.trigger_feed_timeline_load();
                                            if let Some(tx) = WORKER_TX.get() {
                                                let _ = tx.send(WorkerCmd::FetchSocialProfile {
                                                    target_uid: d_id.clone(),
                                                });
                                            }
                                        }
                                    }
                                });
                            });

                        ui.add(egui::Separator::default().spacing(1.0));

                        // Group Members List (`Thành viên nhóm`)
                        if d_is_group {
                            let group_members = self
                                .group_members_map
                                .get(d_id)
                                .cloned()
                                .or_else(|| {
                                    self.threads
                                        .get(self.selected_thread_index)
                                        .map(|t| t.presentation.members.clone())
                                })
                                .unwrap_or_default();
                            let is_fetching = self.fetching_group_members.contains(d_id);

                            egui::Frame::none()
                                .inner_margin(Margin::symmetric(16.0, 14.0))
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        ui.label(
                                            RichText::new("Thành viên nhóm")
                                                .size(12.5)
                                                .color(pal.text_primary)
                                                .strong(),
                                        );
                                        ui.with_layout(
                                            Layout::right_to_left(Align::Center),
                                            |ui| {
                                                let refresh_resp = ui
                                                    .add(
                                                        egui::Button::new(
                                                            RichText::new(lucide::REFRESH)
                                                                .size(11.5)
                                                                .color(pal.text_secondary),
                                                        )
                                                        .fill(Color32::TRANSPARENT)
                                                        .frame(false),
                                                    )
                                                    .on_hover_cursor(CursorIcon::PointingHand)
                                                    .on_hover_text("Tải lại danh sách thành viên");
                                                if refresh_resp.clicked() {
                                                    if let Some(tx) = WORKER_TX.get() {
                                                        let _ =
                                                            tx.send(WorkerCmd::FetchGroupMembers {
                                                                group_id: d_id.clone(),
                                                            });
                                                    }
                                                    self.fetching_group_members
                                                        .insert(d_id.clone());
                                                }

                                                ui.label(
                                                    RichText::new(format!(
                                                        "({})",
                                                        group_members.len()
                                                    ))
                                                    .size(11.5)
                                                    .color(pal.primary_hover),
                                                );
                                            },
                                        );
                                    });
                                    ui.add_space(8.0);

                                    if is_fetching && group_members.is_empty() {
                                        ui.horizontal(|ui| {
                                            ui.spinner();
                                            ui.label(
                                                RichText::new("Đang tải danh sách thành viên…")
                                                    .size(11.5)
                                                    .color(pal.text_muted),
                                            );
                                        });
                                    } else if group_members.is_empty() {
                                        let load_btn = egui::Button::new(
                                            RichText::new("Tải danh sách thành viên")
                                                .size(11.5)
                                                .color(pal.primary_hover),
                                        )
                                        .fill(pal.primary.linear_multiply(0.12))
                                        .stroke(Stroke::new(1.0_f32, pal.primary))
                                        .rounding(Rounding::same(6.0));
                                        if ui.add(load_btn).clicked() {
                                            if let Some(tx) = WORKER_TX.get() {
                                                let _ = tx.send(WorkerCmd::FetchGroupMembers {
                                                    group_id: d_id.clone(),
                                                });
                                            }
                                            self.fetching_group_members.insert(d_id.clone());
                                        }
                                    } else {
                                        if group_members.len() > 5 {
                                            ui.add(
                                                egui::TextEdit::singleline(
                                                    &mut self.group_member_search,
                                                )
                                                .hint_text("🔍 Tìm thành viên…")
                                                .margin(Margin::symmetric(6.0, 3.0)),
                                            );
                                            ui.add_space(4.0);
                                        }

                                        let q = self.group_member_search.trim().to_lowercase();
                                        let filtered_members: Vec<&crate::chat::SidebarMember> =
                                            group_members
                                                .iter()
                                                .filter(|m| {
                                                    q.is_empty()
                                                        || m.name.to_lowercase().contains(&q)
                                                        || m.id.contains(&q)
                                                })
                                                .collect();

                                        egui::ScrollArea::vertical()
                                            .id_salt("info_drawer_members_scroll")
                                            .max_height(200.0)
                                            .auto_shrink([false, false])
                                            .show(ui, |ui| {
                                                ui.spacing_mut().item_spacing = Vec2::new(0.0, 5.0);
                                                for m in filtered_members {
                                                    ui.horizontal(|ui| {
                                                        let (av_r, _) = ui.allocate_exact_size(
                                                            Vec2::new(26.0, 26.0),
                                                            Sense::hover(),
                                                        );
                                                        paint_user_avatar(
                                                            ui.painter(),
                                                            av_r.center(),
                                                            13.0,
                                                            &m.name,
                                                            false,
                                                            m.avatar.as_deref(),
                                                            &self.sticker_textures,
                                                            &mut self.loading_stickers,
                                                        );

                                                        ui.add_space(4.0);
                                                        ui.vertical(|ui| {
                                                            ui.horizontal(|ui| {
                                                                ui.label(
                                                                    RichText::new(truncate_chars(
                                                                        &m.name, 16,
                                                                    ))
                                                                    .size(12.0)
                                                                    .color(pal.text_primary)
                                                                    .strong(),
                                                                );
                                                                if m.role == "creator" {
                                                                    ui.label(
                                                                        RichText::new(
                                                                            "Trưởng nhóm",
                                                                        )
                                                                        .size(9.5)
                                                                        .color(Color32::from_rgb(
                                                                            245, 158, 11,
                                                                        )),
                                                                    );
                                                                } else if m.role == "admin" {
                                                                    ui.label(
                                                                        RichText::new("Phó nhóm")
                                                                            .size(9.5)
                                                                            .color(
                                                                                pal.primary_hover,
                                                                            ),
                                                                    );
                                                                }
                                                            });
                                                        });

                                                        ui.with_layout(
                                                            Layout::right_to_left(Align::Center),
                                                            |ui| {
                                                                let chat_btn = ui
                                                                    .add(
                                                                        egui::Button::new(
                                                                            RichText::new(
                                                                                lucide::SEND,
                                                                            )
                                                                            .size(11.0)
                                                                            .color(
                                                                                pal.text_secondary,
                                                                            ),
                                                                        )
                                                                        .fill(Color32::TRANSPARENT)
                                                                        .frame(false),
                                                                    )
                                                                    .on_hover_cursor(
                                                                        CursorIcon::PointingHand,
                                                                    )
                                                                    .on_hover_text(
                                                                        "Nhắn tin riêng",
                                                                    );
                                                                if chat_btn.clicked() {
                                                                    open_chat_member =
                                                                        Some(m.clone());
                                                                }

                                                                let prof_btn = ui
                                                                    .add(
                                                                        egui::Button::new(
                                                                            RichText::new("🪪")
                                                                                .size(10.5),
                                                                        )
                                                                        .fill(Color32::TRANSPARENT)
                                                                        .frame(false),
                                                                    )
                                                                    .on_hover_cursor(
                                                                        CursorIcon::PointingHand,
                                                                    )
                                                                    .on_hover_text(
                                                                        "Xem trang cá nhân",
                                                                    );
                                                                if prof_btn.clicked() {
                                                                    view_profile_member =
                                                                        Some(m.id.clone());
                                                                }
                                                            },
                                                        );
                                                    });
                                                }
                                            });
                                    }
                                });

                            ui.add(egui::Separator::default().spacing(1.0));
                        }

                        // Media Hub: Tabs for Photos, Files, Links
                        egui::Frame::none()
                            .inner_margin(Margin::symmetric(16.0, 10.0))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    let btn_w = (ui.available_width() - 12.0) / 3.0;
                                    for (tab, title, count) in [
                                        (crate::gui::DrawerMediaTab::Photos, "Ảnh", shared_photos.len()),
                                        (crate::gui::DrawerMediaTab::Files, "Tài liệu", shared_files.len()),
                                        (crate::gui::DrawerMediaTab::Links, "Link", shared_links.len()),
                                    ] {
                                        let active = self.info_drawer_tab == tab;
                                        let btn = egui::Button::new(
                                            RichText::new(format!("{title} ({count})"))
                                                .size(11.2)
                                                .color(if active { Color32::WHITE } else { pal.text_secondary })
                                                .strong(),
                                        )
                                        .fill(if active { pal.primary } else { pal.bg_input })
                                        .rounding(Rounding::same(6.0))
                                        .min_size(Vec2::new(btn_w, 24.0));
                                        if ui.add(btn).clicked() {
                                            self.info_drawer_tab = tab;
                                        }
                                    }
                                });
                                ui.add_space(8.0);

                                match self.info_drawer_tab {
                                    crate::gui::DrawerMediaTab::Photos => {
                                        if shared_photos.is_empty() {
                                            ui.label(RichText::new("Chưa có ảnh hoặc video chia sẻ.").size(11.5).color(pal.text_muted));
                                        } else {
                                            ui.horizontal_wrapped(|ui| {
                                                ui.spacing_mut().item_spacing = Vec2::new(6.0, 6.0);
                                                for (tkey, thumb_u, full_u, ptitle) in &shared_photos {
                                                    if !self.sticker_textures.contains_key(tkey)
                                                        && self.loading_stickers.insert(tkey.clone())
                                                    {
                                                        if let Some(tx) = WORKER_TX.get() {
                                                            let _ = tx.send(WorkerCmd::FetchStickerImage {
                                                                key: tkey.clone(),
                                                                url: thumb_u.clone(),
                                                            });
                                                        }
                                                    }
                                                    let (cell_r, cell_resp) = ui.allocate_exact_size(Vec2::splat(74.0), Sense::click());
                                                    ui.painter().rect_filled(cell_r, Rounding::same(8.0), pal.bg_elevated);
                                                    if let Some(tex) = self.sticker_textures.get(tkey) {
                                                        ui.painter().image(
                                                            tex.id(),
                                                            cell_r,
                                                            Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                                                            Color32::WHITE,
                                                        );
                                                    } else {
                                                        draw_icon_image(ui.painter(), cell_r.center(), 20.0, pal.text_muted);
                                                    }
                                                    ui.painter().rect_stroke(
                                                        cell_r,
                                                        Rounding::same(8.0),
                                                        Stroke::new(1.0_f32, if cell_resp.hovered() { pal.primary } else { pal.border }),
                                                    );
                                                    if cell_resp.on_hover_cursor(CursorIcon::PointingHand).on_hover_text("Xem ảnh phóng to").clicked() {
                                                        self.image_viewer = Some(ImageViewerModal {
                                                            tex_key: tkey.clone(),
                                                            hd_tex_key: self.request_hd_texture(full_u),
                                                            full_url: full_u.clone(),
                                                            title: ptitle.clone(),
                                                            sender: d_name.clone(),
                                                            time: String::new(),
                                                            zoom: 1.0,
                                                            pan: Vec2::ZERO,
                                                        });
                                                    }
                                                }
                                            });
                                        }
                                    }
                                    crate::gui::DrawerMediaTab::Files => {
                                        if shared_files.is_empty() {
                                            ui.label(RichText::new("Chưa có tài liệu/file nào.").size(11.5).color(pal.text_muted));
                                        } else {
                                            ui.vertical(|ui| {
                                                for (fname, ftime, furl) in &shared_files {
                                                    let row_resp = ui.horizontal(|ui| {
                                                        ui.label(RichText::new("📄").size(14.0));
                                                        ui.vertical(|ui| {
                                                            ui.label(RichText::new(truncate_chars(fname, 22)).size(12.0).color(pal.text_primary).strong());
                                                            ui.label(RichText::new(ftime).size(10.5).color(pal.text_muted));
                                                        });
                                                    }).response;
                                                    if row_resp.on_hover_cursor(CursorIcon::PointingHand).on_hover_text(format!("Mở file: {fname}")).clicked() {
                                                        if !furl.is_empty() {
                                                            open_external_url(furl);
                                                        }
                                                    }
                                                    ui.add_space(4.0);
                                                }
                                            });
                                        }
                                    }
                                    crate::gui::DrawerMediaTab::Links => {
                                        if shared_links.is_empty() {
                                            ui.label(RichText::new("Chưa có liên kết chia sẻ.").size(11.5).color(pal.text_muted));
                                        } else {
                                            ui.vertical(|ui| {
                                                for (ltitle, lurl) in &shared_links {
                                                    let row_resp = ui.horizontal(|ui| {
                                                        ui.label(RichText::new("🔗").size(13.0));
                                                        ui.vertical(|ui| {
                                                            ui.label(RichText::new(truncate_chars(ltitle, 22)).size(12.0).color(pal.primary_hover).strong());
                                                            ui.label(RichText::new(truncate_chars(lurl, 26)).size(10.5).color(pal.text_muted));
                                                        });
                                                    }).response;
                                                    if row_resp.on_hover_cursor(CursorIcon::PointingHand).on_hover_text(lurl).clicked() {
                                                        open_external_url(lurl);
                                                    }
                                                    ui.add_space(4.0);
                                                }
                                            });
                                        }
                                    }
                                }
                            });

                        // Push technical metadata to the bottom of the drawer
                        let rem = (ui.available_height() - 88.0).max(8.0);
                        ui.add_space(rem);
                        ui.add(egui::Separator::default().spacing(1.0));

                        egui::Frame::none()
                            .fill(pal.bg_app)
                            .inner_margin(Margin::symmetric(16.0, 12.0))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.label(
                                        RichText::new("Tổng số tin nhắn:")
                                            .size(11.5)
                                            .color(pal.text_muted),
                                    );
                                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                        ui.label(
                                            RichText::new(format!("{d_msg_count} tin nhắn"))
                                                .size(11.5)
                                                .color(pal.text_secondary)
                                                .strong(),
                                        );
                                    });
                                });
                                ui.add_space(4.0);
                                ui.horizontal(|ui| {
                                    ui.label(
                                        RichText::new("Mã định danh (ID):")
                                            .size(11.5)
                                            .color(pal.text_muted),
                                    );
                                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                        let char_cnt = d_id.chars().count();
                                        let short_id = if char_cnt > 10 {
                                            let prefix: String = d_id.chars().take(4).collect();
                                            let suffix: String =
                                                d_id.chars().skip(char_cnt - 4).collect();
                                            format!("{prefix}...{suffix}")
                                        } else {
                                            d_id.clone()
                                        };
                                        let (id_rect, id_resp) = ui.allocate_exact_size(
                                            Vec2::new(98.0, 22.0),
                                            Sense::click(),
                                        );
                                        ui.painter().rect_filled(
                                            id_rect,
                                            Rounding::same(5.0),
                                            if id_resp.hovered() {
                                                pal.bg_hover
                                            } else {
                                                pal.bg_elevated
                                            },
                                        );
                                        ui.painter().text(
                                            Pos2::new(id_rect.left() + 7.0, id_rect.center().y),
                                            egui::Align2::LEFT_CENTER,
                                            short_id,
                                            FontId::monospace(10.5),
                                            pal.text_secondary,
                                        );
                                        draw_icon_copy(
                                            ui.painter(),
                                            Pos2::new(id_rect.right() - 10.0, id_rect.center().y),
                                            11.5,
                                            pal.text_secondary,
                                        );
                                        if id_resp
                                            .on_hover_cursor(CursorIcon::PointingHand)
                                            .on_hover_text(format!("Sao chép ID: {d_id}"))
                                            .clicked()
                                        {
                                            ui.ctx().copy_text(d_id.clone());
                                            self.show_toast(
                                                "Đã sao chép ID hội thoại vào bộ nhớ tạm",
                                            );
                                        }
                                    });
                                });
                            });
                    });
            }
        }

        if let Some(m) = open_chat_member {
            if let Some(idx) = self.threads.iter().position(|t| t.id == m.id) {
                self.selected_thread_index = idx;
            } else {
                self.threads.insert(
                    0,
                    Thread {
                        presentation: Default::default(),
                        unread: Default::default(),
                        id: m.id.clone(),
                        name: m.name.clone(),
                        preview: "Bắt đầu cuộc trò chuyện".to_string(),
                        time: crate::ui::clock(),
                        avatar: m.avatar.clone(),
                        is_group: false,
                        messages: Vec::new(),
                    },
                );
                self.selected_thread_index = 0;
                self.persist_threads_async();
            }
            self.show_toast(format!("Đã mở cuộc trò chuyện với {}", m.name));
        }

        if let Some(uid) = view_profile_member {
            self.feed_modal_open = true;
            self.feed_modal_tab = FeedModalTab::Profile;
            self.feed_target_uid = uid.clone();
            self.feed_loading_profile = true;
            self.feed_profile_error = None;
            self.trigger_feed_timeline_load();
            if let Some(tx) = WORKER_TX.get() {
                let _ = tx.send(WorkerCmd::FetchSocialProfile { target_uid: uid });
            }
        }

        // -------------------------------------------------------------------
        // 4. CENTRAL CHAT AREA (Clean Header, Messages Stream, Icon Toolbar)
        // -------------------------------------------------------------------
        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(pal.bg_chat_canvas))
            .show(ctx, |ui| {
                if let Some((thread_id, thread_name, thread_avatar, is_group, _msg_count)) =
                    thread_meta
                {
                    let is_cloud_thread = thread_name.contains("Cloud của tôi");
                    if !is_group
                        && !is_cloud_thread
                        && !self.friend_status_map.contains_key(&thread_id)
                        && self.checking_friend_status.insert(thread_id.clone())
                    {
                        if let Some(tx) = WORKER_TX.get() {
                            let _ = tx.send(WorkerCmd::CheckFriendStatus {
                                user_id: thread_id.clone(),
                            });
                        }
                    }
                    if is_group
                        && !self.group_members_map.contains_key(&thread_id)
                        && self.fetching_group_members.insert(thread_id.clone())
                    {
                        if let Some(tx) = WORKER_TX.get() {
                            let _ = tx.send(WorkerCmd::FetchGroupMembers {
                                group_id: thread_id.clone(),
                            });
                        }
                    }
                    let cur_friendship = if is_group || is_cloud_thread {
                        crate::chat::FriendshipStatus::Friend
                    } else {
                        self.friend_status_map
                            .get(&thread_id)
                            .copied()
                            .unwrap_or(crate::chat::FriendshipStatus::Unknown)
                    };
                    // 1. Clean Conversation Header Bar (No raw 19-digit ID, no bulky sync button)
                    egui::Frame::none()
                        .fill(pal.bg_sidebar)
                        .inner_margin(Margin::symmetric(18.0, 11.0))
                        .stroke(Stroke::new(1.0_f32, pal.border))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                // Header Avatar (40x40) with Cutout Online Dot
                                let (av_rect, _) =
                                    ui.allocate_exact_size(Vec2::new(40.0, 40.0), Sense::hover());
                                paint_user_avatar(
                                    ui.painter(),
                                    av_rect.center(),
                                    20.0,
                                    &thread_name,
                                    is_group,
                                    thread_avatar.as_deref(),
                                    &self.sticker_textures,
                                    &mut self.loading_stickers,
                                );
                                let cur_onl = self.user_online_map.get(&thread_id);
                                if !is_group
                                    && cur_onl.map(|(onl, _)| *onl).unwrap_or(false)
                                {
                                    let dot_pos =
                                        Pos2::new(av_rect.right() - 4.0, av_rect.bottom() - 4.0);
                                    ui.painter().circle_filled(dot_pos, 6.2, pal.bg_sidebar);
                                    ui.painter().circle_filled(dot_pos, 4.2, pal.accent_online);
                                }
                                ui.add_space(6.0);
                                // Reserve room for the right-hand action cluster so the name/tag
                                // block can never be overlapped by the call buttons.
                                let actions_w = if is_compact_w { 126.0 } else { 182.0 };
                                let left_info_w =
                                    (ui.available_width() - actions_w).max(60.0);
                                ui.allocate_ui_with_layout(
                                    Vec2::new(left_info_w, 40.0),
                                    Layout::top_down(Align::Min),
                                    |ui| {
                                        ui.spacing_mut().item_spacing.y = 2.0;
                                        ui.horizontal(|ui| {
                                            let name_max = if is_compact_w { 12 } else { 22 };
                                            ui.add(
                                                egui::Label::new(
                                                    RichText::new(truncate_chars(
                                                        &thread_name,
                                                        name_max,
                                                    ))
                                                    .size(15.0)
                                                    .color(pal.text_primary)
                                                    .strong(),
                                                )
                                                .truncate(),
                                            );
                                            if !is_compact_w && left_info_w > 170.0 {
                                                let (tag_lbl, tag_fg, tag_bg) =
                                                    if is_cloud_thread {
                                                        (
                                                            "Đám mây".to_string(),
                                                            pal.primary_hover,
                                                            pal.primary.linear_multiply(0.14),
                                                        )
                                                    } else if is_group {
                                                        ("Nhóm".to_string(), pal.text_secondary, pal.bg_elevated)
                                                    } else {
                                                        match cur_friendship {
                                                            crate::chat::FriendshipStatus::Friend => (
                                                                format!("{} Bạn bè", lucide::CHECK),
                                                                pal.accent_online,
                                                                pal.accent_online.linear_multiply(0.14),
                                                            ),
                                                            crate::chat::FriendshipStatus::IncomingPending => (
                                                                "Chờ đồng ý kết bạn".to_string(),
                                                                pal.primary_hover,
                                                                pal.primary.linear_multiply(0.16),
                                                            ),
                                                            crate::chat::FriendshipStatus::OutgoingPending => (
                                                                "Đã gửi lời mời".to_string(),
                                                                pal.warning,
                                                                pal.warning.linear_multiply(0.15),
                                                            ),
                                                            crate::chat::FriendshipStatus::Stranger => (
                                                                "Người lạ".to_string(),
                                                                Color32::from_rgb(249, 115, 22),
                                                                Color32::from_rgb(249, 115, 22)
                                                                    .linear_multiply(0.15),
                                                            ),
                                                            crate::chat::FriendshipStatus::Unknown => (
                                                                "Đang kiểm tra…".to_string(),
                                                                pal.text_muted,
                                                                pal.bg_elevated,
                                                            ),
                                                        }
                                                    };
                                                egui::Frame::none()
                                                    .fill(tag_bg)
                                                    .rounding(Rounding::same(4.0))
                                                    .inner_margin(Margin::symmetric(6.0, 1.5))
                                                    .show(ui, |ui| {
                                                        ui.label(
                                                            RichText::new(tag_lbl)
                                                                .size(10.2)
                                                                .color(tag_fg)
                                                                .strong(),
                                                        );
                                                    });
                                            }
                                        });
                                        let is_typing_now = self.settings.show_typing_indicator
                                            && self.typing_map.get(&thread_id)
                                                .map(|(_, until)| *until > std::time::Instant::now())
                                                .unwrap_or(false);
                                        let (sub_status_lbl, sub_status_col): (String, Color32) =
                                            if is_typing_now {
                                                let who = self.typing_map.get(&thread_id).map(|(w, _)| w.as_str()).unwrap_or("Đang");
                                                (format!("✍ {who} đang soạn thảo…"), pal.primary)
                                            } else if is_group {
                                                let m_cnt = self.group_members_map.get(&thread_id)
                                                    .map(|m| m.len())
                                                    .or(self.threads.get(self.selected_thread_index).and_then(|t| t.presentation.member_count).map(|c| c as usize))
                                                    .unwrap_or(0);
                                                let txt = if m_cnt > 0 {
                                                    format!("👥 {m_cnt} thành viên")
                                                } else if self.fetching_group_members.contains(&thread_id) {
                                                    "👥 Đang tải thành viên…".into()
                                                } else {
                                                    "Nhóm trò chuyện".into()
                                                };
                                                (txt, pal.primary_hover)
                                            } else if is_cloud_thread {
                                                ("Lưu trữ đám mây cá nhân".into(), pal.text_secondary)
                                            } else {
                                                match cur_friendship {
                                                    crate::chat::FriendshipStatus::Friend => {
                                                        match cur_onl {
                                                            Some((true, lbl)) if !lbl.is_empty() => (
                                                                format!("Đã kết bạn · {lbl}"),
                                                                pal.accent_online,
                                                            ),
                                                            Some((false, lbl)) if !lbl.is_empty() => (
                                                                format!("Đã kết bạn · {lbl}"),
                                                                pal.text_secondary,
                                                            ),
                                                            _ => (
                                                                "Đã kết bạn".into(),
                                                                pal.text_secondary,
                                                            ),
                                                        }
                                                    }
                                                    crate::chat::FriendshipStatus::IncomingPending => (
                                                        "Đã gửi lời mời kết bạn cho bạn".into(),
                                                        pal.primary_hover,
                                                    ),
                                                    crate::chat::FriendshipStatus::OutgoingPending => (
                                                        "Đã gửi lời mời kết bạn · Chờ đồng ý".into(),
                                                        pal.warning,
                                                    ),
                                                    crate::chat::FriendshipStatus::Stranger => (
                                                        "Người lạ · Chưa kết bạn trên Zalo".into(),
                                                        Color32::from_rgb(249, 115, 22),
                                                    ),
                                                    crate::chat::FriendshipStatus::Unknown => {
                                                        match cur_onl {
                                                            Some((true, lbl)) if !lbl.is_empty() => (
                                                                lbl.clone(),
                                                                pal.accent_online,
                                                            ),
                                                            Some((false, lbl)) if !lbl.is_empty() => (
                                                                lbl.clone(),
                                                                pal.text_secondary,
                                                            ),
                                                            _ => (
                                                                "Trò chuyện cá nhân".into(),
                                                                pal.text_secondary,
                                                            ),
                                                        }
                                                    }
                                                }
                                            };
                                        let sub_resp = ui.add(
                                            egui::Label::new(
                                                RichText::new(sub_status_lbl)
                                                    .size(11.6)
                                                    .color(sub_status_col),
                                            )
                                            .truncate()
                                            .sense(if is_group { egui::Sense::click() } else { egui::Sense::hover() }),
                                        );
                                        if is_group {
                                            let sub_resp = sub_resp
                                                .on_hover_cursor(CursorIcon::PointingHand)
                                                .on_hover_text("Xem thông tin và thành viên nhóm");
                                            if sub_resp.clicked() {
                                                self.info_drawer_open = true;
                                                self.settings.info_drawer_open = true;
                                                self.settings.save();
                                            }
                                        }
                                    },
                                );

                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    // Info Drawer Toggle Button (hidden on compact widths: auto-collapsed)
                                    if !is_compact_w {
                                        let (drw_rect, drw_resp) = ui.allocate_exact_size(
                                            Vec2::new(34.0, 34.0),
                                            Sense::click(),
                                        );
                                        ui.painter().rect_filled(
                                            drw_rect,
                                            Rounding::same(8.0),
                                            if self.info_drawer_open {
                                                pal.primary.linear_multiply(0.18)
                                            } else if drw_resp.hovered() {
                                                pal.bg_hover
                                            } else {
                                                Color32::TRANSPARENT
                                            },
                                        );
                                        draw_icon_drawer(
                                            ui.painter(),
                                            drw_rect.center(),
                                            17.0,
                                            if self.info_drawer_open {
                                                pal.primary_hover
                                            } else if drw_resp.hovered() {
                                                pal.text_primary
                                            } else {
                                                pal.text_secondary
                                            },
                                        );
                                        if drw_resp
                                            .on_hover_cursor(CursorIcon::PointingHand)
                                            .on_hover_text("Thông tin hội thoại")
                                            .clicked()
                                        {
                                            self.info_drawer_open = !self.info_drawer_open;
                                            self.settings.info_drawer_open = self.info_drawer_open;
                                            self.settings.save();
                                        }
                                    }

                                    // Subtle vertical divider
                                    let (div_r, _) = ui.allocate_exact_size(
                                        Vec2::new(8.0, 20.0),
                                        Sense::hover(),
                                    );
                                    ui.painter().line_segment(
                                        [div_r.center_top(), div_r.center_bottom()],
                                        Stroke::new(1.0_f32, pal.border),
                                    );

                                    // Video Call / Zavi SFU Group Call Button
                                    let vid_btn_w = if is_group { 96.0 } else { 34.0 };
                                    let (vid_rect, vid_resp) = ui.allocate_exact_size(
                                        Vec2::new(vid_btn_w, 32.0),
                                        Sense::click(),
                                    );
                                    if is_group {
                                        let bg = if vid_resp.hovered() {
                                            pal.primary.linear_multiply(0.28)
                                        } else {
                                            pal.primary.linear_multiply(0.16)
                                        };
                                        ui.painter().rect_filled(
                                            vid_rect,
                                            Rounding::same(8.0),
                                            bg,
                                        );
                                        ui.painter().rect_stroke(
                                            vid_rect,
                                            Rounding::same(8.0),
                                            Stroke::new(1.0_f32, pal.primary.linear_multiply(0.45)),
                                        );
                                        draw_icon_video(
                                            ui.painter(),
                                            Pos2::new(vid_rect.left() + 18.0, vid_rect.center().y),
                                            15.5,
                                            pal.primary_hover,
                                        );
                                        ui.painter().text(
                                            Pos2::new(vid_rect.left() + 31.0, vid_rect.center().y),
                                            egui::Align2::LEFT_CENTER,
                                            "Họp Zavi",
                                            FontId::proportional(12.2),
                                            pal.primary_hover,
                                        );
                                    } else {
                                        if vid_resp.hovered() {
                                            ui.painter().rect_filled(
                                                vid_rect,
                                                Rounding::same(8.0),
                                                pal.bg_hover,
                                            );
                                        }
                                        draw_icon_video(
                                            ui.painter(),
                                            vid_rect.center(),
                                            17.5,
                                            if vid_resp.hovered() {
                                                pal.text_primary
                                            } else {
                                                pal.text_secondary
                                            },
                                        );
                                    }
                                    if vid_resp
                                        .on_hover_cursor(CursorIcon::PointingHand)
                                        .on_hover_text(if is_group {
                                            "Họp nhóm Zavi (Video)"
                                        } else {
                                            "Gọi Video"
                                        })
                                        .clicked()
                                    {
                                        let ff_ok = crate::media::is_ffmpeg_available();
                                        let (_, cams) = crate::media::list_media_devices(false);
                                        let can_cam = ff_ok && !cams.is_empty();
                                        if !ff_ok {
                                            self.show_toast(
                                                "⚠️ Không tìm thấy ffmpeg trong PATH để bật Camera (cài bằng: winget install Gyan.FFmpeg)",
                                            );
                                        } else if cams.is_empty() {
                                            self.show_toast(
                                                "⚠️ Không phát hiện thiết bị Camera trên máy — đang gọi không hình",
                                            );
                                        }
                                        self.active_call = Some(ActiveCallOverlay {
                                            partner_id: thread_id.clone(),
                                            partner_name: thread_name.clone(),
                                            is_video: true,
                                            is_group,
                                            state_label: if is_group {
                                                "Đang gọi nhóm Zavi…".into()
                                            } else {
                                                "Đang kết nối cuộc gọi video…".into()
                                            },
                                            started_at: std::time::Instant::now(),
                                            mic_muted: false,
                                            cam_enabled: can_cam,
                                            is_incoming_ringing: false,
                                            error_code: None,
                                            error_reason: None,
                                            speaker_view: false,
                                            pinned_tile_idx: 0,
                                            call_page: 0,
                                            members_popover_open: false,
                                            invite_search_query: String::new(),
                                            quick_reply_popover_open: false,
                                        });
                                        if let Some(tx) = WORKER_TX.get() {
                                            let _ = tx.send(WorkerCmd::StartCall {
                                                partner_id: thread_id.clone(),
                                                partner_name: thread_name.clone(),
                                                partner_avatar: None,
                                                is_video: true,
                                                is_group,
                                            });
                                        }
                                    }

                                    // Voice Call Button
                                    let (aud_rect, aud_resp) = ui.allocate_exact_size(
                                        Vec2::new(34.0, 34.0),
                                        Sense::click(),
                                    );
                                    if aud_resp.hovered() {
                                        ui.painter().rect_filled(
                                            aud_rect,
                                            Rounding::same(8.0),
                                            pal.bg_hover,
                                        );
                                    }
                                    draw_icon_phone(
                                        ui.painter(),
                                        aud_rect.center(),
                                        17.5,
                                        if aud_resp.hovered() {
                                            pal.text_primary
                                        } else {
                                            pal.text_secondary
                                        },
                                    );
                                    if aud_resp
                                        .on_hover_cursor(CursorIcon::PointingHand)
                                        .on_hover_text(if is_group {
                                            "Gọi thoại nhóm (Zavi)"
                                        } else {
                                            "Gọi thoại"
                                        })
                                        .clicked()
                                    {
                                        self.active_call = Some(ActiveCallOverlay {
                                            partner_id: thread_id.clone(),
                                            partner_name: thread_name.clone(),
                                            is_video: false,
                                            is_group,
                                            state_label: if is_group {
                                                "Đang gọi nhóm Zavi…".into()
                                            } else {
                                                "Đang kết nối cuộc gọi thoại…".into()
                                            },
                                            started_at: std::time::Instant::now(),
                                            mic_muted: false,
                                            cam_enabled: false,
                                            is_incoming_ringing: false,
                                            error_code: None,
                                            error_reason: None,
                                            speaker_view: false,
                                            pinned_tile_idx: 0,
                                            call_page: 0,
                                            members_popover_open: false,
                                            invite_search_query: String::new(),
                                            quick_reply_popover_open: false,
                                        });
                                        if let Some(tx) = WORKER_TX.get() {
                                            let _ = tx.send(WorkerCmd::StartCall {
                                                partner_id: thread_id.clone(),
                                                partner_name: thread_name.clone(),
                                                partner_avatar: None,
                                                is_video: false,
                                                is_group,
                                            });
                                        }
                                    }
                                    // Search Messages in Conversation Button (`Ctrl+F`)
                                    let (srch_rect, srch_resp) = ui.allocate_exact_size(
                                        Vec2::new(34.0, 34.0),
                                        Sense::click(),
                                    );
                                    ui.painter().rect_filled(
                                        srch_rect,
                                        Rounding::same(8.0),
                                        if self.in_chat_search_open {
                                            pal.primary.linear_multiply(0.18)
                                        } else if srch_resp.hovered() {
                                            pal.bg_hover
                                        } else {
                                            Color32::TRANSPARENT
                                        },
                                    );
                                    draw_icon_search(
                                        ui.painter(),
                                        srch_rect.center(),
                                        16.5,
                                        if self.in_chat_search_open {
                                            pal.primary_hover
                                        } else if srch_resp.hovered() {
                                            pal.text_primary
                                        } else {
                                            pal.text_secondary
                                        },
                                    );
                                    if srch_resp
                                        .on_hover_cursor(CursorIcon::PointingHand)
                                        .on_hover_text("Tìm kiếm tin nhắn trong hội thoại này (Ctrl+F)")
                                        .clicked()
                                        || ui.input(|i| {
                                            (i.modifiers.ctrl || i.modifiers.command)
                                                && i.key_pressed(Key::F)
                                        })
                                    {
                                        self.in_chat_search_open = !self.in_chat_search_open;
                                        if self.in_chat_search_open {
                                            self.in_chat_search_focus_pending = true;
                                        } else {
                                            self.in_chat_search_query.clear();
                                        }
                                    }
                                });
                            });
                        });
                    // 1a. Stranger / Pending Friend Request Banner (matching Zalo PC's top relationship bar)
                    if !is_group
                        && !is_cloud_thread
                        && matches!(
                            cur_friendship,
                            crate::chat::FriendshipStatus::Stranger
                                | crate::chat::FriendshipStatus::IncomingPending
                                | crate::chat::FriendshipStatus::OutgoingPending
                        )
                    {
                        egui::Frame::none()
                            .fill(pal.bg_elevated)
                            .stroke(Stroke::new(1.0_f32, pal.border))
                            .inner_margin(Margin::symmetric(18.0, 7.0))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    match cur_friendship {
                                        crate::chat::FriendshipStatus::IncomingPending => {
                                            ui.label(
                                                RichText::new(format!(
                                                    "{}  {} đã gửi lời mời kết bạn cho bạn",
                                                    lucide::USER_PLUS,
                                                    truncate_chars(&thread_name, 26)
                                                ))
                                                .size(12.2)
                                                .color(pal.primary_hover)
                                                .strong(),
                                            );
                                            ui.with_layout(
                                                Layout::right_to_left(Align::Center),
                                                |ui| {
                                                    ui.spacing_mut().item_spacing.x = 6.0;
                                                    let rej_btn = egui::Button::new(
                                                        RichText::new("Từ chối")
                                                            .size(11.5)
                                                            .color(pal.text_secondary),
                                                    )
                                                    .fill(pal.bg_input)
                                                    .stroke(Stroke::new(1.0_f32, pal.border))
                                                    .rounding(Rounding::same(6.0))
                                                    .min_size(Vec2::new(68.0, 26.0));
                                                    if ui
                                                        .add(rej_btn)
                                                        .on_hover_cursor(CursorIcon::PointingHand)
                                                        .clicked()
                                                    {
                                                        if let Some(tx) = WORKER_TX.get() {
                                                            let _ = tx.send(
                                                                WorkerCmd::RejectFriend {
                                                                    user_id: thread_id.clone(),
                                                                    user_name: thread_name.clone(),
                                                                },
                                                            );
                                                        }
                                                    }

                                                    let acc_btn = egui::Button::new(
                                                        RichText::new(format!(
                                                            "{} Đồng ý kết bạn",
                                                            lucide::CHECK
                                                        ))
                                                        .size(11.8)
                                                        .color(Color32::WHITE)
                                                        .strong(),
                                                    )
                                                    .fill(pal.accent_online)
                                                    .rounding(Rounding::same(6.0))
                                                    .min_size(Vec2::new(124.0, 26.0));
                                                    if ui
                                                        .add(acc_btn)
                                                        .on_hover_cursor(CursorIcon::PointingHand)
                                                        .clicked()
                                                    {
                                                        if let Some(tx) = WORKER_TX.get() {
                                                            let _ = tx.send(
                                                                WorkerCmd::AcceptFriend {
                                                                    user_id: thread_id.clone(),
                                                                    user_name: thread_name.clone(),
                                                                },
                                                            );
                                                        }
                                                    }
                                                },
                                            );
                                        }
                                        crate::chat::FriendshipStatus::OutgoingPending => {
                                            ui.label(
                                                RichText::new(format!(
                                                    "Đã gửi lời mời kết bạn tới {} · Đang chờ đồng ý",
                                                    truncate_chars(&thread_name, 24)
                                                ))
                                                .size(12.0)
                                                .color(pal.warning),
                                            );
                                            ui.with_layout(
                                                Layout::right_to_left(Align::Center),
                                                |ui| {
                                                    let undo_btn = egui::Button::new(
                                                        RichText::new("Hủy lời mời")
                                                            .size(11.5)
                                                            .color(pal.text_secondary),
                                                    )
                                                    .fill(pal.bg_input)
                                                    .stroke(Stroke::new(1.0_f32, pal.border))
                                                    .rounding(Rounding::same(6.0))
                                                    .min_size(Vec2::new(86.0, 26.0));
                                                    if ui
                                                        .add(undo_btn)
                                                        .on_hover_cursor(CursorIcon::PointingHand)
                                                        .clicked()
                                                    {
                                                        if let Some(tx) = WORKER_TX.get() {
                                                            let _ = tx.send(WorkerCmd::UndoFriend {
                                                                user_id: thread_id.clone(),
                                                                user_name: thread_name.clone(),
                                                            });
                                                        }
                                                    }
                                                },
                                            );
                                        }
                                        crate::chat::FriendshipStatus::Stranger => {
                                            ui.label(
                                                RichText::new(format!(
                                                    "Bạn và {} chưa là bạn bè trên Zalo",
                                                    truncate_chars(&thread_name, 26)
                                                ))
                                                .size(12.0)
                                                .color(pal.text_secondary),
                                            );
                                            ui.with_layout(
                                                Layout::right_to_left(Align::Center),
                                                |ui| {
                                                    let add_btn = egui::Button::new(
                                                        RichText::new(format!(
                                                            "{} Kết bạn",
                                                            lucide::USER_PLUS
                                                        ))
                                                        .size(11.8)
                                                        .color(Color32::WHITE)
                                                        .strong(),
                                                    )
                                                    .fill(pal.primary)
                                                    .rounding(Rounding::same(6.0))
                                                    .min_size(Vec2::new(92.0, 26.0));
                                                    if ui
                                                        .add(add_btn)
                                                        .on_hover_cursor(CursorIcon::PointingHand)
                                                        .clicked()
                                                    {
                                                        if let Some(tx) = WORKER_TX.get() {
                                                            let _ = tx.send(WorkerCmd::AddFriend {
                                                                user_id: thread_id.clone(),
                                                                user_name: thread_name.clone(),
                                                            });
                                                        }
                                                        self.show_toast(format!(
                                                            "Đã gửi lời mời kết bạn tới {}",
                                                            thread_name
                                                        ));
                                                    }
                                                },
                                            );
                                        }
                                        _ => {}
                                    }
                                });
                            });
                    }
                    // 1b. Pinned Message Banner (hidden: parked until server sync is fully supported to avoid mock display)
                    let pinned_msg_info: Option<(usize, String, String)> = None;

                    let mut unpin_idx: Option<usize> = None;
                    if let Some((pidx, psender, ptext)) = pinned_msg_info {
                        egui::Frame::none()
                            .fill(pal.bg_elevated)
                            .stroke(Stroke::new(1.0_f32, pal.primary.linear_multiply(0.4)))
                            .inner_margin(Margin::symmetric(14.0, 7.0))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.label(
                                        RichText::new(format!("{} Tin ghim:", lucide::PIN))
                                            .size(12.5)
                                            .color(pal.primary)
                                            .strong(),
                                    );
                                    ui.label(
                                        RichText::new(format!("{psender}: {ptext}"))
                                            .size(12.5)
                                            .color(pal.text_primary),
                                    );
                                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                        if ui
                                            .small_button(format!("Bỏ ghim {}", lucide::X))
                                            .on_hover_cursor(CursorIcon::PointingHand)
                                            .clicked()
                                        {
                                            unpin_idx = Some(pidx);
                                        }
                                    });
                                });
                            });
                    }
                    if let Some(uidx) = unpin_idx {
                        if let Some(real) = self
                            .threads
                            .get_mut(self.selected_thread_index)
                            .and_then(|t| t.messages.get_mut(uidx))
                        {
                            real.is_pinned = false;
                        }
                        self.persist_threads_async();
                        self.status_msg = "Đã bỏ ghim tin nhắn".into();
                    }

                    // 1c-0. GitHub Release Update Available Notification Banner
                    if let Some(upd) = self.update_info.clone() {
                        if upd.has_update
                            && !self.update_banner_dismissed
                            && self.settings.skipped_update_version != upd.latest_version
                        {
                            egui::Frame::none()
                                .fill(pal.accent_online.linear_multiply(0.14))
                                .stroke(Stroke::new(1.0_f32, pal.accent_online))
                                .inner_margin(Margin::symmetric(14.0, 6.0))
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        let msg_txt = if self.update_downloading {
                                            let (dl, tot) = self.update_progress.unwrap_or((0, 0));
                                            if tot > 0 {
                                                let pct = ((dl as f64 / tot as f64) * 100.0)
                                                    .clamp(0.0, 100.0);
                                                format!(
                                                    "⏳ Đang tải bản cập nhật v{} từ GitHub ({pct:.0}%)…",
                                                    upd.latest_version
                                                )
                                            } else {
                                                format!(
                                                    "⏳ Đang tải bản cập nhật v{} từ GitHub…",
                                                    upd.latest_version
                                                )
                                            }
                                        } else {
                                            format!(
                                                "🚀 Đã có bản cập nhật mới ctdozlite v{} trên GitHub Releases!",
                                                upd.latest_version
                                            )
                                        };
                                        let banner_lbl = ui.label(
                                            RichText::new(msg_txt)
                                                .size(12.0)
                                                .color(pal.accent_online)
                                                .strong(),
                                        );
                                        if !upd.release_notes.trim().is_empty() {
                                            banner_lbl.on_hover_text(upd.release_notes.trim());
                                        }
                                        ui.with_layout(
                                            Layout::right_to_left(Align::Center),
                                            |ui| {
                                                if ui
                                                    .small_button(lucide::X)
                                                    .on_hover_cursor(CursorIcon::PointingHand)
                                                    .on_hover_text("Ẩn thanh thông báo cập nhật")
                                                    .clicked()
                                                {
                                                    self.update_banner_dismissed = true;
                                                }
                                                if ui
                                                    .small_button("Chi tiết")
                                                    .on_hover_cursor(CursorIcon::PointingHand)
                                                    .clicked()
                                                {
                                                    self.update_modal_open = true;
                                                }
                                                if !self.update_downloading {
                                                    if let (Some(dl_url), Some(asset_name)) =
                                                        (upd.download_url.clone(), upd.asset_name.clone())
                                                    {
                                                        if ui
                                                            .add(
                                                                egui::Button::new(
                                                                    RichText::new(format!(
                                                                        "⬇ Cập nhật v{}",
                                                                        upd.latest_version
                                                                    ))
                                                                    .size(11.5)
                                                                    .color(Color32::WHITE)
                                                                    .strong(),
                                                                )
                                                                .fill(pal.accent_online)
                                                                .rounding(Rounding::same(5.0)),
                                                            )
                                                            .on_hover_cursor(
                                                                CursorIcon::PointingHand,
                                                            )
                                                            .clicked()
                                                        {
                                                            self.update_downloading = true;
                                                            self.update_progress =
                                                                Some((0, upd.asset_size));
                                                            self.update_modal_open = true;
                                                            if let Some(tx) = WORKER_TX.get() {
                                                                let _ = tx.send(
                                                                    WorkerCmd::DownloadAndApplyUpdate {
                                                                        download_url: dl_url,
                                                                        asset_name,
                                                                        target_version: upd
                                                                            .latest_version
                                                                            .clone(),
                                                                        expected_sha256: upd
                                                                            .expected_sha256
                                                                            .clone(),
                                                                    },
                                                                );
                                                            }
                                                        }
                                                    }
                                                }
                                            },
                                        );
                                    });
                                });
                        }
                    }

                    // 1c. Starred filter active banner
                    if self.filter_starred_only {
                        egui::Frame::none()
                            .fill(Color32::from_rgb(234, 179, 8).linear_multiply(0.12))
                            .stroke(Stroke::new(1.0_f32, Color32::from_rgb(234, 179, 8)))
                            .inner_margin(Margin::symmetric(14.0, 6.0))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.label(
                                        RichText::new(format!("{} Đang lọc tin nhắn đã đánh dấu sao", lucide::STAR))
                                            .size(12.0)
                                            .color(Color32::from_rgb(234, 179, 8))
                                            .strong(),
                                    );
                                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                        if ui
                                            .small_button(format!("Xem tất cả {}", lucide::X))
                                            .on_hover_cursor(CursorIcon::PointingHand)
                                            .clicked()
                                        {
                                            self.filter_starred_only = false;
                                        }
                                    });
                                });
                            });
                    }

                    // 1d. In-Conversation Message Search Bar (`self.in_chat_search_open`)
                    if self.in_chat_search_open {
                        if ui.input(|i| i.key_pressed(Key::Escape)) {
                            self.in_chat_search_open = false;
                            self.in_chat_search_query.clear();
                        }
                    }
                    if self.in_chat_search_open {
                        let q_low = self.in_chat_search_query.trim().to_lowercase();
                        let q_folded = fold_vn_search(&q_low);
                        let match_cnt = if q_low.is_empty() {
                            0
                        } else {
                            self.threads
                                .get(self.selected_thread_index)
                                .map(|t| {
                                    t.messages
                                        .iter()
                                        .filter(|m| {
                                            matches_vn_query(&m.text, &q_low, &q_folded)
                                                || matches_vn_query(&m.sender, &q_low, &q_folded)
                                        })
                                        .count()
                                })
                                .unwrap_or(0)
                        };
                        egui::Frame::none()
                            .fill(pal.bg_sidebar)
                            .stroke(Stroke::new(1.0_f32, pal.border))
                            .inner_margin(Margin::symmetric(18.0, 8.0))
                            .show(ui, |ui| {
                                let bar_avail_w = ui.available_width();
                                ui.set_width(bar_avail_w);
                                ui.horizontal(|ui| {
                                    ui.spacing_mut().item_spacing.x = 8.0;
                                    let right_reserve = if q_low.is_empty() {
                                        86.0
                                    } else {
                                        174.0
                                    };
                                    let pill_w = (bar_avail_w - right_reserve).max(140.0);
                                    egui::Frame::none()
                                        .fill(pal.bg_input)
                                        .rounding(Rounding::same(8.0))
                                        .stroke(Stroke::new(
                                            1.0_f32,
                                            pal.primary.linear_multiply(0.65),
                                        ))
                                        .inner_margin(Margin::symmetric(10.0, 5.0))
                                        .show(ui, |ui| {
                                            let inner_w = (pill_w - 20.0).max(110.0);
                                            ui.set_width(inner_w);
                                            ui.horizontal(|ui| {
                                                ui.spacing_mut().item_spacing.x = 6.0;
                                                let (ir, _) = ui.allocate_exact_size(
                                                    Vec2::new(15.0, 18.0),
                                                    Sense::hover(),
                                                );
                                                draw_icon_search(
                                                    ui.painter(),
                                                    ir.center(),
                                                    13.5,
                                                    pal.primary_hover,
                                                );
                                                let has_txt =
                                                    !self.in_chat_search_query.is_empty();
                                                let te_w = if has_txt {
                                                    (inner_w - 44.0).max(60.0)
                                                } else {
                                                    (inner_w - 22.0).max(80.0)
                                                };
                                                let te_resp = ui.add(
                                                    egui::TextEdit::singleline(
                                                        &mut self.in_chat_search_query,
                                                    )
                                                    .id(Id::new((
                                                        "in_chat_search_input",
                                                        &thread_id,
                                                    )))
                                                    .frame(false)
                                                    .hint_text(
                                                        "Tìm tin nhắn trong hội thoại này...",
                                                    )
                                                    .desired_width(te_w),
                                                );
                                                if self.in_chat_search_focus_pending {
                                                    te_resp.request_focus();
                                                    self.in_chat_search_focus_pending = false;
                                                }
                                                if has_txt {
                                                    let (clr_r, clr_resp) = ui
                                                        .allocate_exact_size(
                                                            Vec2::splat(16.0),
                                                            Sense::click(),
                                                        );
                                                    if clr_resp.hovered() {
                                                        ui.painter().circle_filled(
                                                            clr_r.center(),
                                                            8.0,
                                                            pal.bg_hover,
                                                        );
                                                    }
                                                    draw_lucide_icon(
                                                        ui.painter(),
                                                        clr_r.center(),
                                                        lucide::X,
                                                        11.5,
                                                        pal.text_muted,
                                                    );
                                                    if clr_resp
                                                        .on_hover_cursor(CursorIcon::PointingHand)
                                                        .on_hover_text("Xóa từ khóa")
                                                        .clicked()
                                                    {
                                                        self.in_chat_search_query.clear();
                                                    }
                                                }
                                            });
                                        });

                                    if !q_low.is_empty() {
                                        let badge_col = if match_cnt > 0 {
                                            pal.accent_online
                                        } else {
                                            pal.danger
                                        };
                                        egui::Frame::none()
                                            .fill(badge_col.linear_multiply(0.14))
                                            .rounding(Rounding::same(6.0))
                                            .inner_margin(Margin::symmetric(8.0, 4.0))
                                            .show(ui, |ui| {
                                                ui.label(
                                                    RichText::new(if match_cnt > 0 {
                                                        format!("{match_cnt} kết quả")
                                                    } else {
                                                        "0 kết quả".to_string()
                                                    })
                                                    .size(11.5)
                                                    .color(badge_col)
                                                    .strong(),
                                                );
                                            });
                                    }

                                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                        let close_btn = egui::Button::new(
                                            RichText::new(format!("{} Đóng", lucide::X))
                                                .size(11.8)
                                                .color(pal.text_secondary)
                                                .strong(),
                                        )
                                        .fill(pal.bg_elevated)
                                        .stroke(Stroke::new(1.0_f32, pal.border))
                                        .rounding(Rounding::same(6.0))
                                        .min_size(Vec2::new(66.0, 28.0));
                                        if ui
                                            .add(close_btn)
                                            .on_hover_cursor(CursorIcon::PointingHand)
                                            .on_hover_text("Đóng thanh tìm kiếm (Esc)")
                                            .clicked()
                                        {
                                            self.in_chat_search_open = false;
                                            self.in_chat_search_query.clear();
                                        }
                                    });
                                });
                            });
                    }

                    // 2. Scrollable Messages Area
                    let posting_restricted = self.threads.get(self.selected_thread_index)
                        .is_some_and(|thread| thread.is_group && thread.presentation.posting_restricted == Some(true));

                    let active_mention_info = if self.settings.suggest_mentions {
                        extract_active_mention_query(&self.input_message)
                            .map(|(pos, q)| (pos, q.to_string()))
                    } else {
                        None
                    };
                    let mut mention_candidates: Vec<MentionCandidate> = Vec::new();
                    if let Some((_, ref query)) = active_mention_info {
                        if &self.dismissed_mention_suggest_for != query {
                            let q_trim = query.trim();
                            let q_fold = crate::chat::strip_vn_for_sticker_kw(q_trim);

                            if is_group {
                                if q_trim.is_empty()
                                    || "all".contains(&q_fold)
                                    || "tat ca".contains(&q_fold)
                                    || "nhom".contains(&q_fold)
                                {
                                    mention_candidates.push(MentionCandidate {
                                        id: "-1".to_string(),
                                        name: "All".to_string(),
                                        avatar: None,
                                        role: "Nhac ca nhom (@All)".to_string(),
                                        is_all: true,
                                    });
                                }

                                if let Some(members) = self.group_members_map.get(&thread_id) {
                                    for m in members {
                                        if m.name.trim().is_empty() {
                                            continue;
                                        }
                                        if q_trim.is_empty() {
                                            mention_candidates.push(MentionCandidate {
                                                id: m.id.clone(),
                                                name: m.name.clone(),
                                                avatar: m.avatar.clone(),
                                                role: m.role.clone(),
                                                is_all: false,
                                            });
                                        } else {
                                            let m_fold = crate::chat::strip_vn_for_sticker_kw(&m.name);
                                            if m.name.to_lowercase().contains(&q_trim.to_lowercase())
                                                || m_fold.contains(&q_fold)
                                            {
                                                mention_candidates.push(MentionCandidate {
                                                    id: m.id.clone(),
                                                    name: m.name.clone(),
                                                    avatar: m.avatar.clone(),
                                                    role: m.role.clone(),
                                                    is_all: false,
                                                });
                                            }
                                        }
                                    }
                                } else if self.fetching_group_members.insert(thread_id.clone()) {
                                    if let Some(tx) = WORKER_TX.get() {
                                        let _ = tx.send(WorkerCmd::FetchGroupMembers {
                                            group_id: thread_id.clone(),
                                        });
                                    }
                                }
                            } else {
                                let p_name = thread_name.clone();
                                if !p_name.is_empty() {
                                    let p_fold = crate::chat::strip_vn_for_sticker_kw(&p_name);
                                    if q_trim.is_empty()
                                        || p_name.to_lowercase().contains(&q_trim.to_lowercase())
                                        || p_fold.contains(&q_fold)
                                    {
                                        mention_candidates.push(MentionCandidate {
                                            id: thread_id.clone(),
                                            name: p_name,
                                            avatar: thread_avatar.clone(),
                                            role: "Ban be".to_string(),
                                            is_all: false,
                                        });
                                    }
                                }
                            }
                        }
                    }

                    let contextual_stickers = if posting_restricted { Vec::new() } else { self.resolve_contextual_sticker_suggestions(ctx) };
                    let composer_link_preview = if posting_restricted {
                        None
                    } else {
                        self.resolve_composer_link_preview(ctx)
                    };
                    // Do NOT subtract `contextual_stickers` height from `picker_reserve`!
                    // The sticker strip is a true floating overlay (`egui::Area`) above the composer bar
                    // (matching Zalo PC's `PopoverIdentitiesDefine.STICKER_SUGGESTION`), so the message scroll
                    // area and composer input box never shift or lose IME focus while typing.
                    let picker_reserve = if posting_restricted { 0.0 } else { (if self.sticker_picker_open { 268.0 } else { 0.0 })
                        + (if self.replying_to.is_some() { 48.0 } else { 0.0 })
                        + (if self.pending_clipboard_image.is_some() {
                            64.0
                        } else {
                            0.0
                        }) };
                    let mut pending_action: Option<(usize, BubbleAction)> = None;
                    let mut load_older_clicked = false;
                    let mut custom_toast: Option<String> = None;
                    // each frame, so a 2 000-message group re-laid out 2 000 galleys per repaint.
                    // "Tải thêm tin nhắn cũ" widens the window before asking the server.
                    if self.chat_render_window.0 != thread_id {
                        self.chat_render_window = (thread_id.clone(), CHAT_RENDER_STEP);
                    }
                    let jump_index = self.quote_jump.as_ref().and_then(|jump| {
                        self.threads
                            .get(self.selected_thread_index)
                            .and_then(|thread| jump.target_index(&thread_id, &thread.messages))
                    });
                    if jump_index.is_none() {
                        self.quote_jump = None;
                    }
                    let jump_scroll_pending = self
                        .quote_jump
                        .as_ref()
                        .map(|jump| jump.scroll_pending)
                        .unwrap_or(false);
                    let mut jump_scrolled = false;
                    if let Some(index) = jump_index {
                        let count = self
                            .threads
                            .get(self.selected_thread_index)
                            .map(|thread| thread.messages.len())
                            .unwrap_or(0);
                        self.chat_render_window.1 = crate::message_navigation::render_limit_for_target(
                            self.chat_render_window.1,
                            count,
                            index,
                        );
                    }
                    let render_limit = self.chat_render_window.1;
                    let mut widen_window = false;
                    // Group sender -> avatar and display name map, built once per frame
                    let (member_avatar_by_key, member_name_by_uid): (HashMap<String, String>, HashMap<String, String>) = if is_group {
                        let mut avas = HashMap::new();
                        let mut names = HashMap::new();
                        for th in self.threads.iter().filter(|th| !th.is_group) {
                            if let Some(ref ava) = th.avatar {
                                if !ava.is_empty() {
                                    avas.insert(th.id.clone(), ava.clone());
                                    avas.insert(th.name.clone(), ava.clone());
                                }
                            }
                            names.insert(th.id.clone(), th.name.clone());
                        }
                        let empty_vec = Vec::new();
                        let members = self.group_members_map.get(&thread_id).unwrap_or_else(|| {
                            self.threads.get(self.selected_thread_index).map(|t| &t.presentation.members).unwrap_or(&empty_vec)
                        });
                        for m in members {
                            if let Some(ref ava) = m.avatar {
                                if !ava.is_empty() {
                                    avas.insert(m.id.clone(), ava.clone());
                                    avas.insert(m.name.clone(), ava.clone());
                                }
                            }
                            if !m.name.is_empty() {
                                names.insert(m.id.clone(), m.name.clone());
                            }
                        }
                        (avas, names)
                    } else {
                        (HashMap::new(), HashMap::new())
                    };
                    let composer_lines = self
                        .input_message
                        .split('\n')
                        .count()
                        .clamp(1, 5);
                    let composer_extra_h = if posting_restricted { 0.0 } else { (composer_lines.saturating_sub(1) as f32) * 20.0 };
                    let msg_area_height = (ui.available_height()
                        - 68.0
                        - composer_extra_h
                        - picker_reserve)
                        .max(100.0);
                    let chat_scroll_out = egui::ScrollArea::vertical()
                        .id_salt(Id::new(("chat_scroll", &thread_id)))
                        .max_height(msg_area_height)
                        .auto_shrink([false, false])
                        .stick_to_bottom(jump_index.is_none())
                        .show(ui, |ui| {
                            egui::Frame::none()
                                .inner_margin(Margin::symmetric(18.0, 12.0))
                                .show(ui, |ui| {
                                    if let Some(thread) =
                                        self.threads.get(self.selected_thread_index)
                                    {
                                        if thread.messages.is_empty() {
                                            ui.vertical_centered(|ui| {
                                                ui.add_space(60.0);
                                                ui.label(
                                                    RichText::new(
                                                        "Chưa có tin nhắn trong cuộc trò chuyện này.",
                                                    )
                                                    .size(14.0)
                                                    .color(pal.text_muted),
                                                );
                                            });
                                        } else {
                                            if !self.in_chat_search_open
                                                && !self.filter_starred_only
                                            {
                                                let has_more_in_memory = thread.messages.len() > render_limit;
                                                let reached_start = self.history_reached_start.contains(&thread_id);

                                                if reached_start && !has_more_in_memory {
                                                    ui.vertical_centered(|ui| {
                                                        ui.add_space(8.0);
                                                        ui.label(
                                                            RichText::new("— Đây là tin nhắn đầu tiên của cuộc trò chuyện —")
                                                                .size(11.5)
                                                                .color(pal.text_muted),
                                                        );
                                                        ui.add_space(8.0);
                                                    });
                                                } else {
                                                    ui.vertical_centered(|ui| {
                                                        let hist_btn = egui::Button::new(
                                                            RichText::new(format!(
                                                                "{} Tải thêm tin nhắn cũ",
                                                                lucide::REFRESH
                                                            ))
                                                            .size(11.5)
                                                            .color(pal.text_secondary),
                                                        )
                                                        .fill(pal.bg_elevated)
                                                        .stroke(Stroke::new(1.0_f32, pal.border))
                                                        .rounding(Rounding::same(12.0));
                                                        if ui
                                                            .add(hist_btn)
                                                            .on_hover_cursor(CursorIcon::PointingHand)
                                                            .on_hover_text(
                                                                "Đồng bộ thêm lịch sử tin nhắn cũ từ máy chủ Zalo",
                                                            )
                                                            .clicked()
                                                        {
                                                            if has_more_in_memory {
                                                                widen_window = true;
                                                                let new_count = (render_limit + CHAT_RENDER_STEP).min(thread.messages.len());
                                                                custom_toast = Some(format!("Đã hiển thị thêm tin nhắn cũ ({new_count}/{})", thread.messages.len()));
                                                            } else {
                                                                if let Some(tx) = WORKER_TX.get() {
                                                                    let _ = tx.send(
                                                                        WorkerCmd::LoadGroupHistory {
                                                                            group_id: thread_id.clone(),
                                                                            is_group,
                                                                            have: thread.messages.len(),
                                                                        },
                                                                    );
                                                                }
                                                                load_older_clicked = true;
                                                            }
                                                        }
                                                        ui.add_space(6.0);
                                                    });
                                                }
                                            }
                                            let only_starred = self.filter_starred_only;
                                            let in_chat_q = if self.in_chat_search_open {
                                                self.in_chat_search_query.trim().to_lowercase()
                                            } else {
                                                String::new()
                                            };
                                            let in_chat_folded = fold_vn_search(&in_chat_q);
                                            // Only build an index vector for every message when a
                                            // filter is actually active. The common case renders
                                            // the newest `render_limit` bubbles and must stay O(1)
                                            // in the thread length: scanning the whole thread and
                                            // allocating an index vector per frame dominated long
                                            // conversations.
                                            let visible_indices: Vec<usize> =
                                                if only_starred || !in_chat_q.is_empty() {
                                                    thread
                                                        .messages
                                                        .iter()
                                                        .enumerate()
                                                        .filter(|(_, m)| {
                                                            (!only_starred || m.is_starred)
                                                                && (in_chat_q.is_empty()
                                                                    || matches_vn_query(
                                                                        &m.text,
                                                                        &in_chat_q,
                                                                        &in_chat_folded,
                                                                    )
                                                                    || matches_vn_query(
                                                                        &m.sender,
                                                                        &in_chat_q,
                                                                        &in_chat_folded,
                                                                    ))
                                                        })
                                                        .map(|(i, _)| i)
                                                        .collect()
                                                } else {
                                                    visible_message_window(
                                                        thread.messages.len(),
                                                        render_limit,
                                                    )
                                                    .collect()
                                                };
                                            if visible_indices.is_empty() && !in_chat_q.is_empty() {
                                                ui.vertical_centered(|ui| {
                                                    ui.add_space(56.0);
                                                    ui.label(
                                                        RichText::new(format!(
                                                            "Không tìm thấy tin nhắn nào khớp từ khóa \"{}\".",
                                                            self.in_chat_search_query.trim()
                                                        ))
                                                        .size(13.5)
                                                        .color(pal.text_muted),
                                                    );
                                                });
                                            }
                                            let last_outgoing_idx = thread
                                                .messages
                                                .iter()
                                                .enumerate()
                                                .rev()
                                                .find(|(_, m)| {
                                                    m.is_outgoing && !is_system_notice_message(m)
                                                })
                                                .map(|(i, _)| i);

                                            for (pos, &idx) in visible_indices.iter().enumerate() {
                                                let msg = &thread.messages[idx];
                                                let prev_msg = pos
                                                    .checked_sub(1)
                                                    .and_then(|p| visible_indices.get(p))
                                                    .map(|&pi| &thread.messages[pi]);
                                                let next_msg = visible_indices
                                                    .get(pos + 1)
                                                    .map(|&ni| &thread.messages[ni]);

                                                if should_show_time_divider(prev_msg, msg) {
                                                    if pos > 0 {
                                                        ui.add_space(6.0);
                                                    }
                                                    render_date_divider(ui, msg, pal);
                                                    ui.add_space(6.0);
                                                }

                                                let row = ui.scope(|ui| {
                                                    if is_system_notice_message(msg) {
                                                        render_system_notice_pill(ui, msg, pal);
                                                        ui.add_space(6.0);
                                                        return;
                                                    }

                                                    let prev_same_sender = prev_msg
                                                        .map(|pm| same_message_cluster(pm, msg))
                                                        .unwrap_or(false);
                                                    let next_same_sender = next_msg
                                                        .map(|nm| same_message_cluster(msg, nm))
                                                        .unwrap_or(false);
                                                    let is_latest_outgoing =
                                                        last_outgoing_idx == Some(idx);

                                                    let s_id = if is_group {
                                                        crate::chat::sidebar_sender_id(msg).or_else(|| {
                                                            if msg.sender.chars().all(|c| c.is_ascii_digit()) {
                                                                Some(msg.sender.clone())
                                                            } else {
                                                                None
                                                            }
                                                        })
                                                    } else {
                                                        None
                                                    };
                                                    let sender_ava_opt = if !is_group {
                                                        thread_avatar.as_deref()
                                                    } else {
                                                        s_id.as_deref()
                                                            .and_then(|id| member_avatar_by_key.get(id))
                                                            .or_else(|| member_avatar_by_key.get(msg.sender.as_str()))
                                                            .map(|s| s.as_str())
                                                    };
                                                    let display_sender_fallback = if is_group {
                                                        s_id.as_deref()
                                                            .and_then(|id| member_name_by_uid.get(id))
                                                            .map(|s| s.as_str())
                                                            .unwrap_or(&thread_name)
                                                    } else {
                                                        &thread_name
                                                    };
                                                    if let Some(action) = render_chat_bubble(
                                                        ui,
                                                        msg,
                                                        prev_same_sender,
                                                        next_same_sender,
                                                        is_latest_outgoing,
                                                        is_group,
                                                        display_sender_fallback,
                                                        sender_ava_opt,
                                                        pal,
                                                        &self.sticker_textures,
                                                        &mut self.loading_stickers,
                                                        &self.emoji_textures,
                                                        idx,
                                                    ) {
                                                        pending_action = Some((idx, action));
                                                    }
                                                    ui.add_space(if next_same_sender {
                                                        2.5
                                                    } else {
                                                        9.0
                                                    });
                                                });
                                                if jump_index == Some(idx) {
                                                    render_quote_target(
                                                        ui,
                                                        row.response.rect,
                                                        jump_scroll_pending,
                                                        pal.primary,
                                                    );
                                                    jump_scrolled |= jump_scroll_pending;
                                                }
                                            }
                                            let (bottom_rect, _) = ui.allocate_exact_size(Vec2::new(1.0, 1.0), Sense::hover());
                                            if self.force_scroll_to_bottom {
                                                ui.scroll_to_rect(bottom_rect, Some(Align::BOTTOM));
                                                self.force_scroll_to_bottom = false;
                                            }
                                        }
                                    }
                                });
                        });

                    let dist_from_bottom = (chat_scroll_out.content_size.y
                        - (chat_scroll_out.state.offset.y + chat_scroll_out.inner_rect.height()))
                        .max(0.0);
                    if dist_from_bottom > 180.0 {
                        let btn_size = 36.0;
                        let btn_pos = Pos2::new(
                            chat_scroll_out.inner_rect.right() - btn_size - 22.0,
                            chat_scroll_out.inner_rect.bottom() - btn_size - 16.0,
                        );
                        egui::Area::new(Id::new(("auto_scroll_btn", &thread_id)))
                            .order(egui::Order::Middle)
                            .fixed_pos(btn_pos)
                            .show(ctx, |ui| {
                                egui::Frame::none()
                                    .fill(pal.bg_elevated)
                                    .stroke(Stroke::new(1.0_f32, pal.border))
                                    .rounding(Rounding::same(btn_size / 2.0))
                                    .shadow(egui::epaint::Shadow {
                                        offset: Vec2::new(0.0, 2.0),
                                        blur: 8.0,
                                        spread: 0.0,
                                        color: Color32::from_black_alpha(if pal.is_dark { 80 } else { 35 }),
                                    })
                                    .show(ui, |ui| {
                                        let (rect, resp) = ui.allocate_exact_size(Vec2::splat(btn_size), Sense::click());
                                        if resp.hovered() {
                                            ui.painter().circle_filled(rect.center(), btn_size / 2.0, pal.bg_hover);
                                        }
                                        draw_lucide_icon(
                                            ui.painter(),
                                            rect.center(),
                                            lucide::CHEVRON_DOWN,
                                            16.0,
                                            if resp.hovered() { pal.primary_hover } else { pal.text_primary },
                                        );
                                        if resp
                                            .on_hover_cursor(CursorIcon::PointingHand)
                                            .on_hover_text("Cuộn xuống tin nhắn mới nhất")
                                            .clicked()
                                        {
                                            self.force_scroll_to_bottom = true;
                                            ui.ctx().request_repaint();
                                        }
                                    });
                            });
                    }
                    if jump_scrolled {
                        if let Some(jump) = &mut self.quote_jump {
                            jump.scroll_pending = false;
                        }
                    }
                    if widen_window {
                        self.chat_render_window.1 += CHAT_RENDER_STEP;
                    }
                    if let Some(msg) = custom_toast {
                        self.show_toast(msg);
                    } else if load_older_clicked {
                        self.show_toast("Đang kiểm tra tin nhắn cũ từ máy chủ Zalo…");
                    }
                    if let Some((idx, action)) = pending_action {
                        let target = self
                            .threads
                            .get(self.selected_thread_index)
                            .and_then(|t| t.messages.get(idx))
                            .cloned();
                        if let Some(m) = target {
                            match action {
                                BubbleAction::Reply => {
                                    self.replying_to = Some(m);
                                    self.composer_focus_pending = true;
                                }
                                BubbleAction::Mention(name) => {
                                    let tag = format!("@{} ", name);
                                    if !self.input_message.contains(&tag) {
                                        if !self.input_message.is_empty()
                                            && !self.input_message.ends_with(' ')
                                        {
                                            self.input_message.push(' ');
                                        }
                                        self.input_message.push_str(&tag);
                                    }
                                    self.composer_focus_pending = true;
                                }
                                BubbleAction::JumpToQuoted(reference) => {
                                    let index = self
                                        .threads
                                        .get(self.selected_thread_index)
                                        .and_then(|thread| {
                                            crate::message_navigation::unique_reference_index(&thread.messages, &reference)
                                        });
                                    if index.is_some() {
                                        self.filter_starred_only = false;
                                        self.in_chat_search_open = false;
                                        self.in_chat_search_query.clear();
                                        self.quote_jump = Some(QuoteJump {
                                            thread_id: thread_id.clone(),
                                            reference,
                                            highlight_until: std::time::Instant::now() + std::time::Duration::from_secs(3),
                                            scroll_pending: true,
                                        });
                                        ctx.request_repaint();
                                    } else {
                                        self.show_toast("Chưa có tin nhắn gốc đã tải với ID duy nhất. Hãy tải thêm lịch sử rồi thử lại.");
                                    }
                                }
                                BubbleAction::Forward => {
                                    self.forward_msg = Some(m);
                                    self.forward_filter.clear();
                                }
                                BubbleAction::ShowReactions => {
                                    self.reactions_dialog = Some((thread_id.clone(), m.id.clone()));
                                    self.reactions_tab = None;
                                }
                                BubbleAction::React(r) => {
                                    // The count is never edited locally: the server echoes our own
                                    // reaction back as a `chat.reaction` event, which is the only
                                    // thing that updates the ledger (deduplicated by event id).
                                    self.status_msg = format!("Đã gửi cảm xúc {r}");
                                    if let Some(tx) = WORKER_TX.get() {
                                        let _ = tx.send(WorkerCmd::React {
                                            to_id: thread_id.clone(),
                                            is_group,
                                            msg: m,
                                            reaction: r.to_string(),
                                        });
                                    }
                                }
                                BubbleAction::TogglePin | BubbleAction::ToggleStar => {
                                    let mut pin_now = false;
                                    let mut topic_id = String::new();
                                    if let Some(real) = self
                                        .threads
                                        .get_mut(self.selected_thread_index)
                                        .and_then(|t| t.messages.get_mut(idx))
                                    {
                                        if action == BubbleAction::TogglePin {
                                            real.is_pinned = !real.is_pinned;
                                            pin_now = real.is_pinned;
                                            topic_id = real.board_topic_id.clone();
                                            self.status_msg = if real.is_pinned {
                                                "Đang ghim tin nhắn lên bảng tin nhóm…".into()
                                            } else {
                                                "Đang bỏ ghim tin nhắn…".into()
                                            };
                                        } else {
                                            real.is_starred = !real.is_starred;
                                            self.status_msg = if real.is_starred {
                                                "Đã đánh dấu sao tin nhắn".into()
                                            } else {
                                                "Đã bỏ đánh dấu sao".into()
                                            };
                                        }
                                    }
                                    // Remote pin goes through the group-board API; the local flag is
                                    // optimistic and rolled back by `GuiEvent::MessagePinResult`.
                                    if action == BubbleAction::TogglePin {
                                        if let Some(tx) = WORKER_TX.get() {
                                            let _ = tx.send(WorkerCmd::PinMessage {
                                                thread_id: thread_id.clone(),
                                                is_group,
                                                msg: m,
                                                pin: pin_now,
                                                topic_id,
                                            });
                                        }
                                    }
                                    self.persist_threads_async();
                                }
                                BubbleAction::ScheduleReminder => {
                                    let snippet = m.text.trim();
                                    if self.input_message.trim().is_empty() && !snippet.is_empty() {
                                        self.input_message = format!("⏰ [Nhắc hẹn] {snippet}");
                                    } else if self.input_message.trim().is_empty() {
                                        self.input_message = "⏰ [Nhắc hẹn] Kiểm tra lại tin nhắn".into();
                                    }
                                    self.schedule_editing_id = None;
                                    self.schedule_modal_open = true;
                                    self.refresh_scheduled_items();
                                    self.show_toast("🕒 Chọn mốc thời gian để đặt nhắc hẹn");
                                }
                                BubbleAction::CallAgain { is_video } => {
                                    self.active_call = Some(ActiveCallOverlay {
                                        partner_id: thread_id.clone(),
                                        partner_name: thread_name.clone(),
                                        is_video,
                                        is_group,
                                        state_label: if is_video {
                                            "Đang kết nối cuộc gọi video…".into()
                                        } else {
                                            "Đang kết nối cuộc gọi thoại…".into()
                                        },
                                        started_at: std::time::Instant::now(),
                                        mic_muted: false,
                                        cam_enabled: is_video,
                                        is_incoming_ringing: false,
                                        error_code: None,
                                        error_reason: None,
                                        speaker_view: false,
                                        pinned_tile_idx: 0,
                                        call_page: 0,
                                        members_popover_open: false,
                                        invite_search_query: String::new(),
                                        quick_reply_popover_open: false,
                                    });
                                    if let Some(tx) = WORKER_TX.get() {
                                        let _ = tx.send(WorkerCmd::StartCall {
                                            partner_id: thread_id.clone(),
                                            partner_name: thread_name.clone(),
                                            partner_avatar: None,
                                            is_video,
                                            is_group,
                                        });
                                    }
                                }
                                BubbleAction::DeleteForMe => {
                                    if let Some(tx) = WORKER_TX.get() {
                                        let _ = tx.send(WorkerCmd::DeleteForMe {
                                            to_id: thread_id.clone(),
                                            is_group,
                                            msg: m,
                                        });
                                    }
                                }
                                BubbleAction::RecallForEveryone => {
                                    if let Some(real) = self
                                        .threads
                                        .get_mut(self.selected_thread_index)
                                        .and_then(|t| t.messages.get_mut(idx))
                                    {
                                        if !real.text.starts_with("[Đã thu hồi]") {
                                            if real.text.trim().is_empty() {
                                                real.text = "[Tin nhắn đã thu hồi]".into();
                                            } else {
                                                real.text = format!("[Đã thu hồi] {}", real.text);
                                            }
                                        }
                                    }
                                    self.persist_threads_async();
                                    if let Some(tx) = WORKER_TX.get() {
                                        let _ = tx.send(WorkerCmd::RecallMessage {
                                            to_id: thread_id.clone(),
                                            is_group,
                                            msg: m,
                                        });
                                    }
                                }
                                BubbleAction::OpenImagePopup {
                                    tex_key,
                                    full_url,
                                    title,
                                    sender,
                                    time,
                                } => {
                                    let hd_tex_key = self.request_hd_texture(&full_url);
                                    self.image_viewer = Some(ImageViewerModal {
                                        tex_key,
                                        hd_tex_key,
                                        full_url,
                                        title,
                                        sender,
                                        time,
                                        zoom: 1.0,
                                        pan: Vec2::ZERO,
                                    });
                                }
                                BubbleAction::DownloadFile { url, file_name } => {
                                    self.show_toast(format!(
                                        "⏳ Đang tải tập tin \"{file_name}\"…"
                                    ));
                                    if let Some(tx) = WORKER_TX.get() {
                                        let _ = tx.send(WorkerCmd::DownloadFile {
                                            url,
                                            file_name,
                                            download_dir: self.settings.download_dir.clone(),
                                        });
                                    }
                                }
                            }
                        }
                    }

                    if posting_restricted {
                        egui::Frame::none()
                            .fill(pal.bg_sidebar)
                            .inner_margin(Margin::symmetric(14.0, 18.0))
                            .stroke(Stroke::new(1.0_f32, pal.border))
                            .show(ui, |ui| {
                                ui.vertical_centered(|ui| {
                                    let notice = if self.settings.language == "English" {
                                        "Only group owners and deputies can send messages to this group"
                                    } else {
                                        "Chỉ trưởng/phó nhóm được gửi tin nhắn vào nhóm"
                                    };
                                    ui.label(RichText::new(notice).color(pal.text_secondary).size(14.0));
                                });
                            });
                    } else {
                    // 3. Optional Sticker Picker Drawer
                    if self.sticker_picker_open {
                        self.render_sticker_picker(ui, &thread_id, is_group, pal);
                    }

                    // 3b. Reply banner (quoted message preview above the composer)
                    if self.replying_to.is_some() {
                        let mut cancel = false;
                        if let Some(q) = &self.replying_to {
                            let who = if q.is_outgoing {
                                "chính bạn".to_string()
                            } else if !q.sender.is_empty() && q.sender != "0" {
                                q.sender.clone()
                            } else {
                                thread_name.clone()
                            };
                            egui::Frame::none()
                                .fill(pal.bg_elevated)
                                .inner_margin(Margin::symmetric(14.0, 6.0))
                                .stroke(Stroke::new(1.0_f32, pal.border))
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        ui.vertical(|ui| {
                                            ui.label(
                                                RichText::new(format!("Đang trả lời {who}"))
                                                    .size(12.0)
                                                    .color(pal.primary)
                                                    .strong(),
                                            );
                                            let preview = if q.sticker_id.is_some() {
                                                "[Nhãn dán]".to_string()
                                            } else {
                                                truncate_chars(&q.text, 100).into_owned()
                                            };
                                            ui.label(
                                                RichText::new(preview)
                                                    .size(12.0)
                                                    .color(pal.text_muted),
                                            );
                                        });
                                        ui.with_layout(
                                            Layout::right_to_left(Align::Center),
                                            |ui| {
                                                if ui
                                                    .small_button(lucide::X)
                                                    .on_hover_cursor(CursorIcon::PointingHand)
                                                    .on_hover_text("Hủy trả lời")
                                                    .clicked()
                                                {
                                                    cancel = true;
                                                }
                                            },
                                        );
                                    });
                                });
                        }
                        if cancel {
                            self.replying_to = None;
                        }
                    }
                    // 3b-2. Live Typing Indicator Banner
                    if self.settings.show_typing_indicator {
                        if let Some((typing_who, until)) = self.typing_map.get(&thread_id) {
                            if *until > std::time::Instant::now() {
                                egui::Frame::none()
                                    .fill(pal.bg_elevated)
                                    .inner_margin(Margin::symmetric(14.0, 5.0))
                                    .rounding(Rounding::same(8.0))
                                    .stroke(Stroke::new(1.0_f32, pal.primary.linear_multiply(0.4)))
                                    .show(ui, |ui| {
                                        ui.horizontal(|ui| {
                                            ui.label(
                                                RichText::new(format!("✍  {typing_who} đang soạn thảo…"))
                                                    .size(12.5)
                                                    .italics()
                                                    .color(pal.primary),
                                            );
                                        });
                                    });
                                ui.add_space(3.0);
                                ctx.request_repaint_after(std::time::Duration::from_millis(500));
                            }
                        }
                    }
                    // 3c. Clipboard Image Preview Banner (`Ctrl+V` / `Shift+Insert` screenshot or image file paste)
                    if is_clipboard_paste_triggered(ctx) {
                        if let Some((path, w, h, col_img)) = read_clipboard_image_to_png() {
                            // Consume any text Paste event on this frame so the composer doesn't paste a file path
                            ctx.input_mut(|i| {
                                i.events.retain(|e| !matches!(e, egui::Event::Paste(_)));
                            });
                            let tex = ctx.load_texture(
                                format!("clip_preview_{}", crate::login::unix_now()),
                                col_img,
                                TextureOptions::LINEAR,
                            );
                            self.pending_clipboard_image = Some((path, w, h, tex));
                            self.composer_focus_pending = true;
                            self.show_toast(format!(
                                "Đã dán ảnh ({w}×{h} px) từ Clipboard — nhấn Enter để gửi"
                            ));
                        }
                    }
                    if self.pending_clipboard_image.is_some() {
                        let mut clear_clip = false;
                        if ui.input(|i| i.key_pressed(Key::Escape)) {
                            clear_clip = true;
                        }
                        if let Some((ref path, w, h, ref tex)) = self.pending_clipboard_image {
                            egui::Frame::none()
                                .fill(pal.bg_elevated)
                                .inner_margin(Margin::symmetric(14.0, 6.0))
                                .stroke(Stroke::new(
                                    1.0_f32,
                                    pal.primary.linear_multiply(0.55),
                                ))
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        ui.add(
                                            egui::Image::new(tex)
                                                .fit_to_exact_size(Vec2::new(48.0, 48.0))
                                                .rounding(Rounding::same(6.0)),
                                        );
                                        ui.add_space(6.0);
                                        ui.vertical(|ui| {
                                            ui.label(
                                                RichText::new(format!(
                                                    "{} Ảnh chụp từ bộ nhớ tạm (Ctrl+V)",
                                                    lucide::IMAGE
                                                ))
                                                .size(12.2)
                                                .color(pal.primary)
                                                .strong(),
                                            );
                                            let fname = path
                                                .file_name()
                                                .and_then(|n| n.to_str())
                                                .unwrap_or("clipboard.png");
                                            ui.label(
                                                RichText::new(format!(
                                                    "{fname} · {w}×{h} px — Nhập chú thích bên dưới rồi nhấn Enter để gửi"
                                                ))
                                                .size(11.3)
                                                .color(pal.text_secondary),
                                            );
                                        });
                                        ui.with_layout(
                                            Layout::right_to_left(Align::Center),
                                            |ui| {
                                                if ui
                                                    .small_button(format!("{} Hủy", lucide::X))
                                                    .on_hover_cursor(CursorIcon::PointingHand)
                                                    .on_hover_text("Hủy ảnh dán (Esc)")
                                                    .clicked()
                                                {
                                                    clear_clip = true;
                                                }
                                            },
                                        );
                                    });
                                });
                        }
                        if clear_clip {
                            self.pending_clipboard_image = None;
                        }
                    }

                    // 3d. Pending Scheduled Messages Banner (#38)
                    let thread_sched: Vec<crate::scheduler::ScheduledMessage> = self
                        .scheduled_items
                        .iter()
                        .filter(|s| s.thread_id == thread_id)
                        .cloned()
                        .collect();
                    if let Some(next_sched) = thread_sched.first() {
                        let mut open_sched_mgr = false;
                        let mut cancel_sched_id: Option<i64> = None;
                        egui::Frame::none()
                            .fill(pal.bg_elevated)
                            .inner_margin(Margin::symmetric(14.0, 5.0))
                            .stroke(Stroke::new(1.0_f32, pal.border))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.label(
                                        RichText::new(format!(
                                            "🕒 Hẹn giờ gửi ({}) · Tiếp theo lúc {}: \"{}\"",
                                            thread_sched.len(),
                                            crate::scheduler::format_schedule_time(
                                                next_sched.deliver_at
                                            ),
                                            truncate_chars(&next_sched.text, 36)
                                        ))
                                        .size(11.6)
                                        .color(pal.primary_hover)
                                        .strong(),
                                    );
                                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                        if ui.small_button("Hủy").clicked() {
                                            cancel_sched_id = Some(next_sched.id);
                                        }
                                        if ui.small_button("Quản lý").clicked() {
                                            open_sched_mgr = true;
                                        }
                                    });
                                });
                            });
                        if let Some(cid) = cancel_sched_id {
                            let _ =
                                crate::scheduler::cancel_scheduled_message("zalo_history.db", cid);
                            self.refresh_scheduled_items();
                            self.show_toast("Đã hủy tin nhắn hẹn giờ");
                        }
                        if open_sched_mgr {
                            self.schedule_modal_open = true;
                        }
                    }

                    // 4. Bottom Message Composer Bar (Compact Icon Actions + Spacious Input + Padded Send Button)
                    let composer_frame = egui::Frame::none()
                        .fill(pal.bg_sidebar)
                        .inner_margin(Margin::symmetric(14.0, 10.0))
                        .stroke(Stroke::new(1.0_f32, pal.border))
                        .show(ui, |ui| {
                            // Handle drag-and-drop files onto the chat window
                            let dropped: Vec<std::path::PathBuf> = ui.ctx().input(|i| {
                                i.raw
                                    .dropped_files
                                    .iter()
                                    .filter_map(|df| df.path.clone())
                                    .collect()
                            });
                            for dpath in dropped {
                                if let Some(tx) = WORKER_TX.get() {
                                    let _ = tx.send(WorkerCmd::PickAndSendAttachment {
                                        to_id: thread_id.clone(),
                                        is_group,
                                        images_only: false,
                                        caption: String::new(),
                                        preset_path: Some(dpath),
                                        original_hd: self.send_hd_original,
                                    });
                                }
                            }

                            let is_ime_busy = self.ime_active || self.ime_just_committed;
                            let trigger_screenshot_hotkey = ui.input(|i| {
                                (i.modifiers.alt && i.key_pressed(Key::A))
                                    || ((i.modifiers.command || i.modifiers.ctrl) && i.modifiers.shift && i.key_pressed(Key::A))
                                    || ((i.modifiers.command || i.modifiers.ctrl) && i.modifiers.shift && i.key_pressed(Key::X))
                            });
                            if trigger_screenshot_hotkey {
                                if let Some(tx) = WORKER_TX.get() {
                                    let _ = tx.send(WorkerCmd::TriggerScreenshot);
                                    self.show_toast("Đang bật chế độ chụp màn hình… Kéo chuột để chọn vùng cần chụp");
                                }
                            }
                            let is_cmd_enter_mode = self.settings.send_message_key == "cmd_enter";
                            let (send_via_enter, insert_newline_via_enter) = ui.input(|i| {
                                let ent = i.key_pressed(Key::Enter) && !is_ime_busy;
                                let cmd_or_ctrl = i.modifiers.command || i.modifiers.ctrl;
                                if is_cmd_enter_mode {
                                    (ent && cmd_or_ctrl, ent && !cmd_or_ctrl)
                                } else {
                                    (ent && !i.modifiers.shift, ent && i.modifiers.shift)
                                }
                            });
                            if send_via_enter || insert_newline_via_enter {
                                ui.input_mut(|i| {
                                    i.events.retain(|ev| {
                                        !matches!(
                                            ev,
                                            egui::Event::Key {
                                                key: Key::Enter,
                                                pressed: true,
                                                ..
                                            }
                                        ) && !matches!(
                                            ev,
                                            egui::Event::Text(t)
                                                if t == "\n" || t == "\r" || t == "\r\n"
                                        )
                                    });
                                });
                            }

                            egui::Frame::none()
                                .fill(pal.bg_input)
                                .rounding(Rounding::same(11.0))
                                .stroke(Stroke::new(1.0_f32, pal.border))
                                .inner_margin(Margin::symmetric(8.0, 5.0))
                                .show(ui, |ui| {
                                    let mut send_clicked = false;
                                    let mut send_selected_sticker: Option<
                                        crate::chat::StickerItem,
                                    > = None;
                                    let composer_id =
                                        Id::new(("chat_composer_input", &thread_id));

                                    let response = ui
                                        .horizontal(|ui| {
                                            ui.spacing_mut().item_spacing.x = 4.0;

                                            // 1) Compact Sticker Icon Button
                                            let (stk_rect, stk_resp) = ui.allocate_exact_size(
                                                Vec2::new(34.0, 34.0),
                                                Sense::click(),
                                            );
                                            if self.sticker_picker_open || stk_resp.hovered() {
                                                ui.painter().rect_filled(
                                                    stk_rect,
                                                    Rounding::same(8.0),
                                                    pal.bg_hover,
                                                );
                                            }
                                            draw_icon_sticker(
                                                ui.painter(),
                                                stk_rect.center(),
                                                18.0,
                                                if self.sticker_picker_open || stk_resp.hovered() {
                                                    Color32::from_rgb(251, 191, 36)
                                                } else {
                                                    pal.text_secondary
                                                },
                                            );
                                            if stk_resp
                                                .on_hover_cursor(CursorIcon::PointingHand)
                                                .on_hover_text("Nhãn dán (Sticker)")
                                                .clicked()
                                            {
                                                self.sticker_picker_open =
                                                    !self.sticker_picker_open;
                                            }

                                            // 2) Compact Photo Upload Icon Button
                                            let (img_rect, img_resp) = ui.allocate_exact_size(
                                                Vec2::new(34.0, 34.0),
                                                Sense::click(),
                                            );
                                            if img_resp.hovered() {
                                                ui.painter().rect_filled(
                                                    img_rect,
                                                    Rounding::same(8.0),
                                                    pal.bg_hover,
                                                );
                                            }
                                            draw_icon_image(
                                                ui.painter(),
                                                img_rect.center(),
                                                18.0,
                                                if img_resp.hovered() {
                                                    pal.accent_online
                                                } else {
                                                    pal.text_secondary
                                                },
                                            );
                                            if img_resp
                                                .on_hover_cursor(CursorIcon::PointingHand)
                                                .on_hover_text("Gửi hình ảnh")
                                                .clicked()
                                            {
                                                let cap = self.input_message.trim().to_string();
                                                self.input_message.clear();
                                                if let Some(tx) = WORKER_TX.get() {
                                                    let _ =
                                                        tx.send(WorkerCmd::PickAndSendAttachment {
                                                            to_id: thread_id.clone(),
                                                            is_group,
                                                            images_only: true,
                                                            caption: cap,
                                                            preset_path: None,
                                                            original_hd: self.send_hd_original,
                                                        });
                                                }
                                            }

                                            // HD Original Quality Toggle Button
                                            let hd_active = self.send_hd_original;
                                            let (hd_rect, hd_resp) = ui.allocate_exact_size(
                                                Vec2::new(32.0, 24.0),
                                                Sense::click(),
                                            );
                                            let hd_bg = if hd_active {
                                                if hd_resp.hovered() {
                                                    pal.primary_hover
                                                } else {
                                                    pal.primary
                                                }
                                            } else if hd_resp.hovered() {
                                                pal.bg_hover
                                            } else {
                                                pal.bg_input
                                            };
                                            let hd_text_col = if hd_active {
                                                Color32::WHITE
                                            } else {
                                                pal.text_secondary
                                            };
                                            ui.painter().rect_filled(
                                                hd_rect,
                                                Rounding::same(6.0),
                                                hd_bg,
                                            );
                                            if !hd_active {
                                                ui.painter().rect_stroke(
                                                    hd_rect,
                                                    Rounding::same(6.0),
                                                    Stroke::new(1.0_f32, pal.border),
                                                );
                                            }
                                            ui.painter().text(
                                                hd_rect.center(),
                                                egui::Align2::CENTER_CENTER,
                                                "HD",
                                                FontId::proportional(11.0),
                                                hd_text_col,
                                            );
                                            let hd_tooltip = if hd_active {
                                                "Chế độ gửi ảnh gốc Original / HD: ĐANG BẬT (Ảnh không bị nén, giữ nguyên 100% chất lượng)"
                                            } else {
                                                "Chế độ gửi ảnh gốc Original / HD: Đang TẮT (Bấm để bật gửi ảnh chất lượng cao không nén)"
                                            };
                                            if hd_resp
                                                .on_hover_cursor(CursorIcon::PointingHand)
                                                .on_hover_text(hd_tooltip)
                                                .clicked()
                                            {
                                                self.send_hd_original = !self.send_hd_original;
                                                let msg = if self.send_hd_original {
                                                    "Đã BẬT gửi ảnh gốc Original / HD (không nén)"
                                                } else {
                                                    "Đã TẮT gửi ảnh gốc Original (chế độ thường)"
                                                };
                                                self.show_toast(msg.to_string());
                                            }

                                            // 3) Compact File Upload Icon Button
                                            let (att_rect, att_resp) = ui.allocate_exact_size(
                                                Vec2::new(34.0, 34.0),
                                                Sense::click(),
                                            );
                                            if att_resp.hovered() {
                                                ui.painter().rect_filled(
                                                    att_rect,
                                                    Rounding::same(8.0),
                                                    pal.bg_hover,
                                                );
                                            }
                                            draw_icon_attachment(
                                                ui.painter(),
                                                att_rect.center(),
                                                17.0,
                                                if att_resp.hovered() {
                                                    pal.primary_hover
                                                } else {
                                                    pal.text_secondary
                                                },
                                            );
                                            if att_resp
                                                .on_hover_cursor(CursorIcon::PointingHand)
                                                .on_hover_text("Đính kèm File / Video")
                                                .clicked()
                                            {
                                                let cap = self.input_message.trim().to_string();
                                                self.input_message.clear();
                                                if let Some(tx) = WORKER_TX.get() {
                                                    let _ =
                                                        tx.send(WorkerCmd::PickAndSendAttachment {
                                                            to_id: thread_id.clone(),
                                                            is_group,
                                                            images_only: false,
                                                            caption: cap,
                                                            preset_path: None,
                                                            original_hd: self.send_hd_original,
                                                        });
                                                }
                                            }

                                            // 4) Screenshot Icon Button (Chụp ảnh màn hình)
                                            let (cam_rect, cam_resp) = ui.allocate_exact_size(
                                                Vec2::new(34.0, 34.0),
                                                Sense::click(),
                                            );
                                            if cam_resp.hovered() {
                                                ui.painter().rect_filled(
                                                    cam_rect,
                                                    Rounding::same(8.0),
                                                    pal.bg_hover,
                                                );
                                            }
                                            draw_icon_camera(
                                                ui.painter(),
                                                cam_rect.center(),
                                                18.0,
                                                if cam_resp.hovered() {
                                                    pal.primary_hover
                                                } else {
                                                    pal.text_secondary
                                                },
                                            );
                                            if cam_resp
                                                .on_hover_cursor(CursorIcon::PointingHand)
                                                .on_hover_text("Chụp ảnh màn hình (Alt+A hoặc click)")
                                                .clicked()
                                            {
                                                if let Some(tx) = WORKER_TX.get() {
                                                    let _ = tx.send(WorkerCmd::TriggerScreenshot);
                                                    self.show_toast("Đang bật chế độ chụp màn hình… Kéo chuột để chọn vùng cần chụp");
                                                }
                                            }

                                            let is_multiline = self.input_message.contains('\n');
                                            let vis_lines = if is_multiline {
                                                self.input_message
                                                    .split('\n')
                                                    .count()
                                                    .clamp(2, 5)
                                            } else {
                                                1
                                            };
                                            let box_h = if is_multiline {
                                                14.0 + (vis_lines as f32 * 20.0)
                                            } else {
                                                34.0
                                            };

                                            // Vertical hairline separator
                                            let (sep_r, _) = ui.allocate_exact_size(
                                                Vec2::new(8.0, (box_h - 10.0).clamp(20.0, 72.0)),
                                                Sense::hover(),
                                            );
                                            ui.painter().line_segment(
                                                [sep_r.center_top(), sep_r.center_bottom()],
                                                Stroke::new(1.0_f32, pal.border),
                                            );

                                            let send_btn_w =
                                                if is_narrow_w { 52.0 } else { 76.0 };
                                            let sched_btn_w = 34.0;
                                            let input_w = (ui.available_width()
                                                - send_btn_w
                                                - sched_btn_w
                                                - 16.0)
                                                .max(50.0);
                                            let hint_str = format!(
                                                "Nhập tin nhắn tới {}...",
                                                truncate_chars(&thread_name, 18)
                                            );
                                            let resp = if is_multiline {
                                                ui.allocate_ui_with_layout(
                                                    Vec2::new(input_w, box_h),
                                                    Layout::top_down(Align::Min),
                                                    |ui| {
                                                        ui.set_min_size(Vec2::new(input_w, box_h));
                                                        ui.set_max_size(Vec2::new(input_w, box_h));
                                                        egui::ScrollArea::vertical()
                                                            .id_salt((
                                                                "composer_fixed_scroll",
                                                                &thread_id,
                                                            ))
                                                            .max_width(input_w)
                                                            .max_height(box_h)
                                                            .auto_shrink([false, false])
                                                            .stick_to_bottom(true)
                                                            .show(ui, |ui| {
                                                                ui.add(
                                                                    egui::TextEdit::multiline(
                                                                        &mut self.input_message,
                                                                    )
                                                                    .id(composer_id)
                                                                    .desired_rows(vis_lines)
                                                                    .frame(false)
                                                                    .hint_text(hint_str)
                                                                    .margin(Margin::symmetric(
                                                                        6.0, 6.0,
                                                                    ))
                                                                    .desired_width(input_w - 10.0),
                                                                )
                                                            })
                                                            .inner
                                                    },
                                                )
                                                .inner
                                            } else {
                                                ui.add_sized(
                                                    [input_w, 34.0],
                                                    egui::TextEdit::singleline(
                                                        &mut self.input_message,
                                                    )
                                                    .id(composer_id)
                                                    .frame(false)
                                                    .hint_text(hint_str)
                                                    .margin(Margin::symmetric(6.0, 7.0))
                                                    .desired_width(input_w),
                                                )
                                            };
                                            if resp.changed()
                                                && !self.input_message.trim().is_empty()
                                                && !self.settings.block_typing
                                            {
                                                let should_emit = self
                                                    .last_typing_sent_at
                                                    .as_ref()
                                                    .map(|(last_tid, t)| {
                                                        last_tid != &thread_id
                                                            || t.elapsed()
                                                                >= std::time::Duration::from_millis(
                                                                    3200,
                                                                )
                                                    })
                                                    .unwrap_or(true);
                                                if should_emit {
                                                    self.last_typing_sent_at = Some((
                                                        thread_id.clone(),
                                                        std::time::Instant::now(),
                                                    ));
                                                    if let Some(tx) = WORKER_TX.get() {
                                                        let _ = tx.send(WorkerCmd::SendTyping {
                                                            thread_id: thread_id.clone(),
                                                            is_group,
                                                        });
                                                    }
                                                }
                                            }

                                            // Schedule Send Button (`🕒`)
                                            let (sched_rect, sched_resp) = ui.allocate_exact_size(
                                                Vec2::new(32.0, 34.0),
                                                Sense::click(),
                                            );
                                            if self.schedule_modal_open || sched_resp.hovered() {
                                                ui.painter().rect_filled(
                                                    sched_rect,
                                                    Rounding::same(8.0),
                                                    pal.bg_hover,
                                                );
                                            }
                                            ui.painter().text(
                                                sched_rect.center(),
                                                egui::Align2::CENTER_CENTER,
                                                "🕒",
                                                FontId::proportional(14.5),
                                                if self.schedule_modal_open || sched_resp.hovered()
                                                {
                                                    pal.primary_hover
                                                } else {
                                                    pal.text_secondary
                                                },
                                            );
                                            if sched_resp
                                                .on_hover_cursor(CursorIcon::PointingHand)
                                                .on_hover_text("Hẹn giờ gửi tin nhắn & Nhắc hẹn")
                                                .clicked()
                                            {
                                                self.schedule_modal_open =
                                                    !self.schedule_modal_open;
                                            }

                                            // Send Button
                                            let can_send = !self.input_message.trim().is_empty()
                                                || self.pending_clipboard_image.is_some();
                                            let (send_rect, send_resp) = ui.allocate_exact_size(
                                                Vec2::new(send_btn_w, 34.0),
                                                Sense::click(),
                                            );
                                            if send_resp.secondary_clicked() {
                                                self.schedule_modal_open = true;
                                            }
                                            let send_bg = if !can_send {
                                                pal.primary.linear_multiply(0.42)
                                            } else if send_resp.hovered() {
                                                pal.primary_hover
                                            } else {
                                                pal.primary
                                            };
                                            ui.painter().rect_filled(
                                                send_rect,
                                                Rounding::same(8.0),
                                                send_bg,
                                            );
                                            if !is_narrow_w {
                                                ui.painter().text(
                                                    Pos2::new(
                                                        send_rect.left() + 15.0,
                                                        send_rect.center().y,
                                                    ),
                                                    egui::Align2::LEFT_CENTER,
                                                    "Gửi",
                                                    FontId::proportional(13.0),
                                                    Color32::WHITE,
                                                );
                                                draw_icon_send(
                                                    ui.painter(),
                                                    Pos2::new(
                                                        send_rect.right() - 17.0,
                                                        send_rect.center().y,
                                                    ),
                                                    14.0,
                                                    Color32::WHITE,
                                                );
                                            } else {
                                                draw_icon_send(
                                                    ui.painter(),
                                                    send_rect.center(),
                                                    16.0,
                                                    Color32::WHITE,
                                                );
                                            }
                                            send_clicked = send_resp
                                                .on_hover_cursor(if can_send {
                                                    CursorIcon::PointingHand
                                                } else {
                                                    CursorIcon::Default
                                                })
                                                .clicked();
                                            resp
                                        })
                                        .inner;

                                    if is_ime_busy && response.lost_focus() {
                                        response.request_focus();
                                    }
                                    if insert_newline_via_enter
                                        && (response.has_focus() || response.lost_focus())
                                    {
                                        if let Some(mut st) =
                                            egui::TextEdit::load_state(ui.ctx(), composer_id)
                                        {
                                            let char_idx = st
                                                .cursor
                                                .char_range()
                                                .map(|r| r.primary.index)
                                                .unwrap_or_else(|| {
                                                    self.input_message.chars().count()
                                                });
                                            let byte_idx = self
                                                .input_message
                                                .char_indices()
                                                .nth(char_idx)
                                                .map(|(i, _)| i)
                                                .unwrap_or(self.input_message.len());
                                            self.input_message.insert(byte_idx, '\n');
                                            let next_c = egui::text::CCursor::new(char_idx + 1);
                                            st.cursor.set_char_range(Some(
                                                egui::text::CCursorRange::one(next_c),
                                            ));
                                            st.store(ui.ctx(), composer_id);
                                        } else {
                                            self.input_message.push('\n');
                                        }
                                        response.request_focus();
                                    }
                                    if response.changed() {
                                        ensure_cjk_font(
                                            ui.ctx(),
                                            std::iter::once(self.input_message.as_str()),
                                        );
                                        if !self.input_message.trim().is_empty()
                                            && self.last_typing_ping_at.elapsed()
                                                > std::time::Duration::from_secs(3)
                                        {
                                            self.last_typing_ping_at = std::time::Instant::now();
                                            if let Some(tx) = WORKER_TX.get() {
                                                let _ = tx.send(WorkerCmd::SendTyping {
                                                    thread_id: thread_id.clone(),
                                                    is_group,
                                                });
                                            }
                                        }
                                    }
                                    if self.composer_focus_pending {
                                        response.request_focus();
                                        self.composer_focus_pending = false;
                                    }
                                    response.context_menu(|ui| {
                                        if ui
                                            .button(format!(
                                                "{} Dán hình ảnh từ Clipboard (Ctrl+V)",
                                                lucide::IMAGE
                                            ))
                                            .clicked()
                                        {
                                            ui.close_menu();
                                            if let Some((path, w, h, col_img)) =
                                                read_clipboard_image_to_png()
                                            {
                                                let tex = ctx.load_texture(
                                                    format!(
                                                        "clip_preview_{}",
                                                        crate::login::unix_now()
                                                    ),
                                                    col_img,
                                                    TextureOptions::LINEAR,
                                                );
                                                self.pending_clipboard_image =
                                                    Some((path, w, h, tex));
                                                self.composer_focus_pending = true;
                                                self.show_toast(format!(
                                                    "Đã dán ảnh ({w}×{h} px) từ Clipboard — nhấn Enter để gửi"
                                                ));
                                            } else {
                                                self.show_toast(
                                                    "Không tìm thấy hình ảnh nào trong Clipboard",
                                                );
                                            }
                                        }
                                    });

                                    let mut chosen_mention = None;
                                    if !mention_candidates.is_empty() && response.has_focus() {
                                        let len = mention_candidates.len();
                                        if self.composer_mention_selected >= len {
                                            self.composer_mention_selected = 0;
                                        }
                                        if ui.input(|i| i.key_pressed(Key::ArrowUp)) {
                                            self.composer_mention_selected =
                                                (self.composer_mention_selected + len - 1) % len;
                                        } else if ui.input(|i| i.key_pressed(Key::ArrowDown)) {
                                            self.composer_mention_selected =
                                                (self.composer_mention_selected + 1) % len;
                                        } else if ui.input(|i| i.key_pressed(Key::Escape)) {
                                            if let Some((_, ref q)) = active_mention_info {
                                                self.dismissed_mention_suggest_for = q.clone();
                                            }
                                        } else if ui.input(|i| i.key_pressed(Key::Tab))
                                            || (send_via_enter
                                                && !is_ime_busy
                                                && (response.has_focus() || response.lost_focus()))
                                        {
                                            if let Some(cand) =
                                                mention_candidates.get(self.composer_mention_selected)
                                            {
                                                chosen_mention = Some(cand.name.clone());
                                            }
                                        }
                                    } else if !contextual_stickers.is_empty() && response.has_focus() {
                                        let len = contextual_stickers.len();
                                        if ui.input(|i| i.key_pressed(Key::ArrowUp)) {
                                            self.composer_sticker_selected = Some(0);
                                        } else if let Some(sel) = self.composer_sticker_selected {
                                            if ui.input(|i| i.key_pressed(Key::ArrowRight)) {
                                                self.composer_sticker_selected =
                                                    Some((sel + 1) % len);
                                            } else if ui.input(|i| i.key_pressed(Key::ArrowLeft)) {
                                                self.composer_sticker_selected =
                                                    Some((sel + len - 1) % len);
                                            } else if ui.input(|i| i.key_pressed(Key::ArrowDown)) {
                                                self.composer_sticker_selected = None;
                                            }
                                        }
                                    } else {
                                        self.composer_sticker_selected = None;
                                    }

                                    if let Some(ref m_name) = chosen_mention {
                                        if let Some((at_pos, _)) = active_mention_info {
                                            let prefix = &self.input_message[..at_pos];
                                            self.input_message = format!("{}@{} ", prefix, m_name);
                                            self.composer_mention_selected = 0;
                                            self.dismissed_mention_suggest_for.clear();
                                            response.request_focus();
                                        }
                                    }

                                    let is_ime_busy = self.ime_active || self.ime_just_committed;
                                    if is_ime_busy && response.lost_focus() {
                                        response.request_focus();
                                    }

                                    let enter_pressed = chosen_mention.is_none()
                                        && (response.lost_focus()
                                            || response.has_focus())
                                        && send_via_enter
                                        && !is_ime_busy;
                                    if enter_pressed {
                                        if let Some(sel) = self.composer_sticker_selected {
                                            if let Some(st) = contextual_stickers.get(sel) {
                                                send_selected_sticker = Some(st.clone());
                                            }
                                        }
                                    }
                                    let can_send = !self.input_message.trim().is_empty()
                                        || self.pending_clipboard_image.is_some();

                                    if let Some(chosen) = send_selected_sticker {
                                        self.input_message.clear();
                                        self.dismissed_sticker_suggest_for.clear();
                                        self.composer_sticker_selected = None;
                                        response.request_focus();
                                        if let Some(tx) = WORKER_TX.get() {
                                            let _ = tx.send(WorkerCmd::SendSticker {
                                                to_id: thread_id.clone(),
                                                sticker_id: chosen.sticker_id,
                                                cate_id: chosen.cate_id,
                                                sticker_type: chosen.sticker_type,
                                                is_group,
                                            });
                                        }
                                    } else if (send_clicked || enter_pressed)
                                        && self.pending_clipboard_image.is_some()
                                    {
                                        if let Some((clip_path, _, _, _)) =
                                            self.pending_clipboard_image.take()
                                        {
                                            let cap = self.input_message.trim().to_string();
                                            self.input_message.clear();
                                            self.dismissed_sticker_suggest_for.clear();
                                            self.composer_sticker_selected = None;
                                            response.request_focus();
                                            if let Some(tx) = WORKER_TX.get() {
                                                let _ = tx.send(WorkerCmd::PickAndSendAttachment {
                                                    to_id: thread_id.clone(),
                                                    is_group,
                                                    images_only: true,
                                                    caption: cap,
                                                    preset_path: Some(clip_path),
                                                    original_hd: self.send_hd_original,
                                                });
                                            }
                                        }
                                    } else if (send_clicked || enter_pressed) && can_send {
                                        let text = self.input_message.trim().to_string();
                                        self.input_message.clear();
                                        self.dismissed_sticker_suggest_for.clear();
                                        self.composer_sticker_selected = None;
                                        response.request_focus();
                                        let quoted = self.replying_to.take();
                                        if let Some(tx) = WORKER_TX.get() {
                                            if let Some(quoted) = quoted {
                                                let _ = tx.send(WorkerCmd::SendQuote {
                                                    to_id: thread_id.clone(),
                                                    text,
                                                    quoted,
                                                    is_group,
                                                });
                                            } else {
                                                let _ = tx.send(WorkerCmd::SendMessage {
                                                    to_id: thread_id.clone(),
                                                    text,
                                                    is_group,
                                                });
                                            }
                                        }
                                    } else if enter_pressed {
                                        response.request_focus();
                                    }
                                });
                        });

                    // 4a. Floating Mention Suggestion Popover (`egui::Area`, rendered AFTER composer bar)
                    let mut click_chosen_mention = None;
                    if !mention_candidates.is_empty() {
                        if let Some(chosen) = self.render_mention_suggestion_popover(
                            ctx,
                            composer_frame.response.rect,
                            &mention_candidates,
                            self.composer_mention_selected,
                            pal,
                        ) {
                            click_chosen_mention = Some(chosen);
                        }
                    } else if !contextual_stickers.is_empty() && !self.sticker_picker_open {
                        self.render_contextual_sticker_popover(
                            ctx,
                            composer_frame.response.rect,
                            &contextual_stickers,
                            &thread_id,
                            is_group,
                            pal,
                        );
                    }
                    if let Some(ref link_meta) = composer_link_preview {
                        if mention_candidates.is_empty()
                            && contextual_stickers.is_empty()
                            && !self.sticker_picker_open
                        {
                            self.render_composer_link_preview_popover(
                                ctx,
                                composer_frame.response.rect,
                                link_meta,
                                pal,
                            );
                        }
                    }
                    if let Some(mention_name) = click_chosen_mention {
                        if let Some((at_pos, _)) = active_mention_info {
                            let prefix = &self.input_message[..at_pos];
                            self.input_message = format!("{}@{} ", prefix, mention_name);
                            self.composer_mention_selected = 0;
                            self.dismissed_mention_suggest_for.clear();
                            self.composer_focus_pending = true;
                        }
                    }
                    }
                } else {
                    ui.vertical_centered(|ui| {
                        ui.add_space(120.0);
                        ui.label(
                            RichText::new("Chọn một hội thoại bên trái để bắt đầu trò chuyện")
                                .size(15.0)
                                .color(pal.text_muted),
                        );
                    });
                }
            });

        self.render_profile_popover(ctx, pal);
        self.render_wipe_confirm(ctx, pal);
        self.render_toast_overlay(ctx, pal);
        self.render_forward_dialog(ctx, pal);
        self.render_reactions_dialog(ctx, pal);
        self.render_call_overlay(ctx, pal);
        self.render_image_viewer_modal(ctx, pal);
        self.render_phone_lookup_modal(ctx, pal);
        self.render_schedule_modal(ctx, pal);
        self.render_settings_modal(ctx, pal);
    }
}

/// Highlight the exact resolved row and optionally center it in the message viewport.
fn render_quote_target(ui: &egui::Ui, rect: egui::Rect, scroll_pending: bool, color: Color32) {
    ui.painter().rect_stroke(
        rect.expand(2.0),
        Rounding::same(8.0),
        Stroke::new(2.0_f32, color),
    );
    if scroll_pending {
        ui.scroll_to_rect(rect, Some(Align::Center));
    }
    ui.ctx()
        .request_repaint_after(std::time::Duration::from_millis(100));
}

#[derive(Debug, Clone)]
pub(crate) struct MentionCandidate {
    #[allow(dead_code)]
    pub id: String,
    pub name: String,
    pub avatar: Option<String>,
    pub role: String,
    pub is_all: bool,
}

pub(crate) fn extract_active_mention_query(text: &str) -> Option<(usize, &str)> {
    let at_idx = text.rfind('@')?;
    if at_idx > 0 {
        let prev_char = text[..at_idx].chars().next_back()?;
        if !prev_char.is_whitespace()
            && !matches!(prev_char, '(' | '[' | '{' | '"' | '\'' | '<' | '`')
        {
            return None;
        }
    }
    let after = &text[at_idx + 1..];
    if after.contains('\n') || after.contains('\r') || after.contains(';') {
        return None;
    }
    if after.split_whitespace().count() > 3 {
        return None;
    }
    Some((at_idx, after))
}

impl ZaloGuiApp {
    pub(super) fn render_mention_suggestion_popover(
        &mut self,
        ctx: &egui::Context,
        composer_rect: Rect,
        candidates: &[MentionCandidate],
        selected_idx: usize,
        pal: &ThemePalette,
    ) -> Option<String> {
        if candidates.is_empty() {
            return None;
        }

        let max_visible = 6;
        let item_h = 36.0_f32;
        let header_h = 28.0_f32;
        let footer_h = 24.0_f32;
        let visible_items = candidates.len().min(max_visible);
        let content_h = (visible_items as f32 * item_h) + header_h + footer_h + 16.0;
        let popover_w = composer_rect.width().clamp(280.0, 380.0);
        let pos = Pos2::new(
            composer_rect.left() + 8.0,
            (composer_rect.top() - content_h - 4.0).max(48.0),
        );

        let mut chosen = None;

        egui::Area::new(Id::new("zalo_composer_mention_popover"))
            .order(egui::Order::Foreground)
            .fixed_pos(pos)
            .show(ctx, |ui| {
                egui::Frame::none()
                    .fill(if pal.is_dark {
                        Color32::from_rgb(33, 37, 43)
                    } else {
                        Color32::WHITE
                    })
                    .stroke(Stroke::new(1.0_f32, pal.border))
                    .rounding(Rounding::same(10.0))
                    .shadow(egui::epaint::Shadow {
                        offset: Vec2::new(0.0, -4.0),
                        blur: 16.0,
                        spread: 0.0,
                        color: Color32::from_black_alpha(if pal.is_dark { 120 } else { 35 }),
                    })
                    .inner_margin(Margin::symmetric(10.0, 8.0))
                    .show(ui, |ui| {
                        ui.set_width(popover_w - 20.0);

                        // Header
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new("👥  NHẮC ĐẾN THÀNH VIÊN (@mention)")
                                    .size(11.0)
                                    .color(pal.primary)
                                    .strong(),
                            );
                        });
                        ui.add_space(4.0);
                        ui.separator();
                        ui.add_space(2.0);

                        // Candidates List
                        let scroll_max_h = (visible_items as f32 * item_h) + 2.0;
                        egui::ScrollArea::vertical()
                            .id_salt("mention_candidates_scroll")
                            .max_height(scroll_max_h)
                            .auto_shrink([false, true])
                            .show(ui, |ui| {
                                for (idx, cand) in candidates.iter().enumerate() {
                                    let is_sel = idx == selected_idx;
                                    let (row_rect, row_resp) = ui.allocate_exact_size(
                                        Vec2::new(ui.available_width(), item_h),
                                        Sense::click(),
                                    );

                                    if is_sel || row_resp.hovered() {
                                        ui.painter().rect_filled(
                                            row_rect,
                                            Rounding::same(6.0),
                                            if is_sel {
                                                pal.bg_selected
                                            } else {
                                                pal.bg_hover
                                            },
                                        );
                                    }

                                    // Avatar
                                    let ava_center =
                                        Pos2::new(row_rect.left() + 18.0, row_rect.center().y);
                                    if cand.is_all {
                                        ui.painter().circle_filled(ava_center, 12.0, pal.primary);
                                        ui.painter().text(
                                            ava_center,
                                            egui::Align2::CENTER_CENTER,
                                            "@",
                                            FontId::proportional(12.0),
                                            Color32::WHITE,
                                        );
                                    } else {
                                        paint_user_avatar(
                                            ui.painter(),
                                            ava_center,
                                            12.0,
                                            &cand.name,
                                            false,
                                            cand.avatar.as_deref(),
                                            &self.sticker_textures,
                                            &mut self.loading_stickers,
                                        );
                                    }

                                    // Name
                                    let name_left = row_rect.left() + 38.0;
                                    let name_text = if cand.is_all {
                                        "@All".to_string()
                                    } else {
                                        truncate_chars(&cand.name, 24).to_string()
                                    };
                                    ui.painter().text(
                                        Pos2::new(name_left, row_rect.center().y),
                                        egui::Align2::LEFT_CENTER,
                                        name_text,
                                        FontId::proportional(13.0),
                                        if is_sel {
                                            pal.primary
                                        } else {
                                            pal.text_primary
                                        },
                                    );

                                    // Role or subtitle tag
                                    if !cand.role.is_empty() {
                                        let role_str = match cand.role.as_str() {
                                            "creator" | "Trưởng nhóm" => "Trưởng nhóm",
                                            "admin" | "Phó nhóm" => "Phó nhóm",
                                            other => other,
                                        };
                                        let role_col = match cand.role.as_str() {
                                            "creator" | "Trưởng nhóm" => {
                                                Color32::from_rgb(234, 179, 8)
                                            }
                                            "admin" | "Phó nhóm" => pal.primary,
                                            _ => pal.text_muted,
                                        };
                                        ui.painter().text(
                                            Pos2::new(row_rect.right() - 8.0, row_rect.center().y),
                                            egui::Align2::RIGHT_CENTER,
                                            role_str,
                                            FontId::proportional(11.0),
                                            role_col,
                                        );
                                    }

                                    if row_resp.on_hover_cursor(CursorIcon::PointingHand).clicked()
                                    {
                                        chosen = Some(cand.name.clone());
                                    }
                                }
                            });

                        // Footer hint
                        ui.add_space(4.0);
                        ui.separator();
                        ui.add_space(2.0);
                        ui.label(
                            RichText::new("↑↓ duyệt · Enter/Tab chèn · Esc huỷ")
                                .size(10.5)
                                .color(pal.text_muted),
                        );
                    });
            });

        chosen
    }
}

/// Newest `limit` message indices for the unfiltered chat view.
///
/// Returns a range (not a filtered vector) so the render loop does not scan or
/// allocate for the entire thread on every frame.
fn visible_message_window(len: usize, limit: usize) -> std::ops::Range<usize> {
    len.saturating_sub(limit)..len
}

/// Order sidebar entries: pinned threads first, then newest `last_ts`.
///
/// Stable so entries with an equal key keep their original order.
fn sidebar_sort_by_key(entries: &mut [(bool, i64, usize)]) {
    entries.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.cmp(&a.1)));
}

#[cfg(test)]
mod hot_path_tests {
    use super::*;

    #[test]
    fn visible_window_returns_newest_messages_without_scanning_the_thread() {
        assert_eq!(
            visible_message_window(10, 3).collect::<Vec<_>>(),
            vec![7, 8, 9]
        );
        assert_eq!(
            visible_message_window(3, 150).collect::<Vec<_>>(),
            vec![0, 1, 2]
        );
        assert!(visible_message_window(0, 150).next().is_none());
    }

    #[test]
    fn sidebar_entries_sort_pinned_first_then_newest() {
        let mut entries = vec![(false, 10, 0), (true, 1, 1), (false, 30, 2), (true, 5, 3)];
        sidebar_sort_by_key(&mut entries);
        assert_eq!(
            entries,
            vec![(true, 5, 3), (true, 1, 1), (false, 30, 2), (false, 10, 0)]
        );
    }

    #[test]
    fn sidebar_sort_is_stable_for_equal_keys() {
        let mut entries = vec![(false, 10, 0), (false, 10, 1), (false, 10, 2)];
        sidebar_sort_by_key(&mut entries);
        assert_eq!(
            entries,
            vec![(false, 10, 0), (false, 10, 1), (false, 10, 2)]
        );
    }
}

#[cfg(test)]
mod quote_navigation_tests {
    use super::*;

    #[test]
    fn quote_navigation_invalidates_when_thread_changes_or_highlight_expires() {
        let messages = [ChatMessage {
            id: "original".into(),
            ..Default::default()
        }];
        let mut jump = QuoteJump {
            thread_id: "thread-1".into(),
            reference: "original".into(),
            highlight_until: std::time::Instant::now() + std::time::Duration::from_secs(3),
            scroll_pending: true,
        };
        assert_eq!(jump.target_index("thread-1", &messages), Some(0));
        assert_eq!(jump.target_index("thread-2", &messages), None);
        jump.highlight_until = std::time::Instant::now();
        assert_eq!(jump.target_index("thread-1", &messages), None);
    }

    #[test]
    fn quote_navigation_keeps_animated_jump_visible_after_highlight_expires() {
        for (count, target) in [(1000, 12), (1000, 900), (150, 12)] {
            let ctx = egui::Context::default();
            let color = Color32::from_rgb(1, 2, 3);
            let mut before = 0.0;
            let mut after = 0.0;
            let mut highlighted = false;
            let mut target_visible = false;
            for frame in 0..10 {
                let jumping = frame > 0;
                let highlighting = jumping && frame < 7;
                let limit = if jumping {
                    crate::message_navigation::render_limit_for_target(150, count, target)
                } else {
                    150
                };
                if count == 1000 && target == 12 {
                    assert!(jumping || count - limit > target);
                }
                let output = ctx.run(
                    egui::RawInput {
                        time: Some(frame as f64 * 0.5),
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            Vec2::new(400.0, 300.0),
                        )),
                        ..Default::default()
                    },
                    |ctx| {
                        egui::CentralPanel::default().show(ctx, |ui| {
                            let scroll = egui::ScrollArea::vertical()
                                .id_salt("quote_jump_test")
                                .max_height(200.0)
                                .stick_to_bottom(!highlighting)
                                .show(ui, |ui| {
                                    for index in count - limit..count {
                                        let (rect, _) = ui.allocate_exact_size(
                                            Vec2::new(200.0, 30.0),
                                            Sense::hover(),
                                        );
                                        if jumping && index == target {
                                            if highlighting {
                                                render_quote_target(ui, rect, frame == 1, color);
                                                highlighted = true;
                                            }
                                            if frame == 9 {
                                                target_visible =
                                                    ui.clip_rect().contains(rect.center());
                                            }
                                        }
                                    }
                                });
                            if jumping {
                                after = scroll.state.offset.y;
                            } else {
                                before = scroll.state.offset.y;
                            }
                        });
                    },
                );
                if frame == 4 {
                    assert!(output.shapes.iter().any(|shape| {
                    matches!(&shape.shape, egui::Shape::Rect(rect) if rect.stroke.color == color)
                }), "Exact target must have a visible highlight shape");
                }
            }
            assert!(highlighted);
            assert!(
                before > 1000.0,
                "Initial viewport must start at the newest messages"
            );
            assert!(
                after < before && target_visible,
                "Jump must center the older target, not remain at the bottom: {after}"
            );
        }
    }
}
