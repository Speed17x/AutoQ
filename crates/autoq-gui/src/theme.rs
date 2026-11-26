//! Nothing OS industrial minimalist design system & custom widgets for `egui`.
//! Faithfully reproduces the obsidian matte aesthetic (`#0B0C0E`), hairline borders,
//! 5x7 dot-matrix headers, Nothing Red (`#D71921`) accents, circular steppers, and
//! precision custom-painted sliders.

use eframe::egui::{
    self, Align2, Color32, CornerRadius, FontId, Pos2, Rect, Response, Sense, Stroke, StrokeKind,
    Vec2,
};

pub const OBSIDIAN: Color32 = Color32::from_rgb(0x0B, 0x0C, 0x0E);
pub const SURFACE: Color32 = Color32::from_rgb(0x13, 0x15, 0x18);
pub const SURFACE_HIGH: Color32 = Color32::from_rgb(0x1B, 0x1D, 0x22);
pub const SURFACE_PILL: Color32 = Color32::from_rgb(0x16, 0x18, 0x1C);
pub const TAB_ACTIVE: Color32 = Color32::from_rgb(0x21, 0x24, 0x2B);
pub const INSET_BG: Color32 = Color32::from_rgb(0x0C, 0x0D, 0x10);
pub const GRAPH_BG: Color32 = Color32::from_rgb(0x0D, 0x0E, 0x11);
pub const BORDER: Color32 = Color32::from_rgb(0x27, 0x2A, 0x32);
pub const BORDER_BRIGHT: Color32 = Color32::from_rgb(0x3D, 0x42, 0x4E);
pub const NOTHING_RED: Color32 = Color32::from_rgb(0xD7, 0x19, 0x21);
pub const NOTHING_RED_HOVER: Color32 = Color32::from_rgb(0xEC, 0x25, 0x2E);
pub const PURE_WHITE: Color32 = Color32::from_rgb(0xF5, 0xF5, 0xF5);
pub const MUTED_GRAY: Color32 = Color32::from_rgb(0x8E, 0x93, 0x9E);
pub const DIM_GRAY: Color32 = Color32::from_rgb(0x56, 0x5B, 0x66);
pub const DOT_GRID: Color32 = Color32::from_rgb(0x25, 0x28, 0x30);
pub const STOCK_LINE: Color32 = Color32::from_rgb(0x78, 0x7E, 0x8A);
pub const TARGET_LINE: Color32 = Color32::from_rgb(0xE0, 0xE4, 0xEC);

pub fn apply_nothing_theme(ctx: &egui::Context) {
    // Load crisp system monospace font if available so typography looks razor sharp
    let mut fonts = egui::FontDefinitions::default();
    let mono_candidates = [
        "/usr/share/fonts/TTF/JetBrainsMonoNerdFontMono-Regular.ttf",
        "/usr/share/fonts/TTF/JetBrainsMono-Regular.ttf",
        "/usr/share/fonts/truetype/jetbrains-mono/JetBrainsMono-Regular.ttf",
        "/usr/share/fonts/noto/NotoSansMono-Regular.ttf",
        "/usr/share/fonts/TTF/DejaVuSansMono.ttf",
    ];
    for path in mono_candidates {
        if let Ok(bytes) = std::fs::read(path) {
            fonts.font_data.insert(
                "nothing_mono".to_owned(),
                egui::FontData::from_owned(bytes).into(),
            );
            fonts
                .families
                .entry(egui::FontFamily::Monospace)
                .or_default()
                .insert(0, "nothing_mono".to_owned());
            fonts
                .families
                .entry(egui::FontFamily::Proportional)
                .or_default()
                .insert(0, "nothing_mono".to_owned());
            break;
        }
    }
    ctx.set_fonts(fonts);

    let mut visuals = egui::Visuals::dark();
    visuals.window_fill = OBSIDIAN;
    visuals.panel_fill = OBSIDIAN;
    visuals.faint_bg_color = SURFACE;
    visuals.extreme_bg_color = INSET_BG;
    visuals.override_text_color = Some(PURE_WHITE);
    visuals.selection.bg_fill = Color32::from_rgba_unmultiplied(0xD7, 0x19, 0x21, 55);
    visuals.selection.stroke = Stroke::new(1.0_f32, NOTHING_RED);

    visuals.widgets.noninteractive.bg_fill = SURFACE;
    visuals.widgets.noninteractive.weak_bg_fill = SURFACE;
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, BORDER);
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, PURE_WHITE);
    visuals.widgets.noninteractive.corner_radius = CornerRadius::same(12);

    visuals.widgets.inactive.bg_fill = SURFACE_PILL;
    visuals.widgets.inactive.weak_bg_fill = SURFACE_PILL;
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, BORDER);
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, PURE_WHITE);
    visuals.widgets.inactive.corner_radius = CornerRadius::same(14);

    visuals.widgets.hovered.bg_fill = SURFACE_HIGH;
    visuals.widgets.hovered.weak_bg_fill = SURFACE_HIGH;
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, BORDER_BRIGHT);
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0_f32, PURE_WHITE);
    visuals.widgets.hovered.corner_radius = CornerRadius::same(14);

    visuals.widgets.active.bg_fill = TAB_ACTIVE;
    visuals.widgets.active.weak_bg_fill = TAB_ACTIVE;
    visuals.widgets.active.bg_stroke = Stroke::new(1.0_f32, NOTHING_RED);
    visuals.widgets.active.fg_stroke = Stroke::new(1.0_f32, PURE_WHITE);
    visuals.widgets.active.corner_radius = CornerRadius::same(14);

    ctx.set_visuals(visuals);

    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = Vec2::new(7.0, 7.0);
    style.spacing.button_padding = Vec2::new(12.0, 6.0);
    style.spacing.scroll.bar_width = 5.0;
    style.spacing.scroll.floating = true;
    ctx.set_style(style);
}

pub fn sub_card_frame() -> egui::Frame {
    egui::Frame::NONE
        .fill(SURFACE)
        .stroke(Stroke::new(1.0_f32, BORDER))
        .corner_radius(CornerRadius::same(16))
        .inner_margin(egui::Margin::symmetric(12, 10))
}

pub fn inset_list_frame() -> egui::Frame {
    egui::Frame::NONE
        .fill(INSET_BG)
        .stroke(Stroke::new(1.0_f32, BORDER))
        .corner_radius(CornerRadius::same(12))
        .inner_margin(egui::Margin::symmetric(8, 6))
}

pub fn section_label(ui: &mut egui::Ui, title: &str) {
    ui.label(
        egui::RichText::new(title)
            .size(10.0)
            .strong()
            .monospace()
            .color(MUTED_GRAY),
    );
}

/// Nothing OS dark pill button with red outline when active (restrained, refined look).
pub fn outline_pill(ui: &mut egui::Ui, label: &str, active: bool, min_size: Vec2) -> Response {
    let fill = if active {
        Color32::from_rgb(0x16, 0x14, 0x17)
    } else {
        SURFACE_PILL
    };
    let stroke = if active {
        Stroke::new(1.2_f32, NOTHING_RED)
    } else {
        Stroke::new(1.0_f32, BORDER)
    };
    let text_col = if active { PURE_WHITE } else { MUTED_GRAY };
    ui.add_sized(
        min_size,
        egui::Button::new(egui::RichText::new(label).size(10.5).strong().monospace().color(text_col))
            .fill(fill)
            .stroke(stroke)
            .corner_radius(CornerRadius::same(15)),
    )
}

/// Solid Nothing Red primary CTA button (used sparingly for ON, APPLY/FLASH, RUN AUTOQ).
pub fn red_pill(ui: &mut egui::Ui, label: &str, min_size: Vec2) -> Response {
    ui.add_sized(
        min_size,
        egui::Button::new(egui::RichText::new(label).size(11.0).strong().monospace().color(PURE_WHITE))
            .fill(NOTHING_RED)
            .stroke(Stroke::new(1.0_f32, NOTHING_RED_HOVER))
            .corner_radius(CornerRadius::same(16)),
    )
}

/// Raised dark capsule tab button for the 3-tab segmented switcher.
pub fn segment_tab(ui: &mut egui::Ui, label: &str, active: bool, width: f32) -> Response {
    let fill = if active { TAB_ACTIVE } else { Color32::TRANSPARENT };
    let stroke = if active {
        Stroke::new(1.0_f32, BORDER_BRIGHT)
    } else {
        Stroke::NONE
    };
    let col = if active { PURE_WHITE } else { MUTED_GRAY };
    ui.add_sized(
        Vec2::new(width, 28.0),
        egui::Button::new(egui::RichText::new(label).size(11.0).strong().monospace().color(col))
            .fill(fill)
            .stroke(stroke)
            .corner_radius(CornerRadius::same(14)),
    )
}

/// Small circular dark icon button (`-`, `+`, `<`, `>`, `⤡`, `×`).
pub fn circle_icon_btn(ui: &mut egui::Ui, symbol: &str, diameter: f32) -> Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(diameter), Sense::click());
    let painter = ui.painter();
    let fill = if resp.is_pointer_button_down_on() {
        TAB_ACTIVE
    } else if resp.hovered() {
        SURFACE_HIGH
    } else {
        SURFACE_PILL
    };
    let stroke_col = if resp.hovered() { BORDER_BRIGHT } else { BORDER };
    painter.circle(rect.center(), diameter * 0.5, fill, Stroke::new(1.0_f32, stroke_col));
    painter.text(
        rect.center(),
        Align2::CENTER_CENTER,
        symbol,
        FontId::monospace((diameter * 0.48).clamp(10.0, 14.0)),
        PURE_WHITE,
    );
    resp
}

/// 2-line Nothing OS Band Card (`B1 21` / `+1.1`) for the 5×2 band selector grid.
pub fn band_card(
    ui: &mut egui::Ui,
    index: usize,
    freq_hz: f64,
    gain_db: f64,
    selected: bool,
    width: f32,
) -> Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, 44.0), Sense::click());
    let painter = ui.painter();

    let fill = if selected {
        Color32::from_rgb(0x18, 0x15, 0x18)
    } else if resp.hovered() {
        SURFACE_HIGH
    } else {
        INSET_BG
    };
    let stroke = if selected {
        Stroke::new(1.3_f32, NOTHING_RED)
    } else {
        Stroke::new(1.0_f32, BORDER)
    };

    painter.rect(rect, CornerRadius::same(12), fill, stroke, StrokeKind::Inside);

    let freq_str = if freq_hz >= 1000.0 {
        format!("{:.1}k", freq_hz / 1000.0)
    } else {
        format!("{:.0}", freq_hz)
    };
    let top_txt = format!("B{} {}", index + 1, freq_str);
    let bot_txt = format!("{:+.1}", gain_db);

    painter.text(
        Pos2::new(rect.center().x, rect.top() + 14.0),
        Align2::CENTER_CENTER,
        top_txt,
        FontId::monospace(10.0),
        if selected { PURE_WHITE } else { MUTED_GRAY },
    );
    painter.text(
        Pos2::new(rect.center().x, rect.bottom() - 13.0),
        Align2::CENTER_CENTER,
        bot_txt,
        FontId::monospace(10.5),
        if gain_db.abs() >= 0.05 { PURE_WHITE } else { DIM_GRAY },
    );

    resp
}

/// Custom-painted Nothing OS precision slider:
/// `LABEL   (-) [======●------] (+)   VALUE`
#[allow(clippy::too_many_arguments)]
pub fn nothing_slider(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut f64,
    min: f64,
    max: f64,
    step: f64,
    logarithmic: bool,
    value_text: &str,
) -> bool {
    let mut changed = false;
    let total_w = ui.available_width();
    let label_w = 46.0;
    let btn_d = 24.0;
    let val_w = 64.0;
    let gap = 7.0;
    let track_w = (total_w - label_w - val_w - 2.0 * btn_d - 4.0 * gap).max(70.0);

    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = gap;

        ui.add_sized(
            Vec2::new(label_w, btn_d),
            egui::Label::new(
                egui::RichText::new(label)
                    .size(10.0)
                    .strong()
                    .monospace()
                    .color(MUTED_GRAY),
            ),
        );

        if circle_icon_btn(ui, "−", btn_d).clicked() {
            let next = if logarithmic {
                (*value / (1.0 + step)).clamp(min, max)
            } else {
                (*value - step).clamp(min, max)
            };
            if (next - *value).abs() > 1e-6 {
                *value = next;
                changed = true;
            }
        }

        // Custom track + red fill + white/red thumb
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(track_w, btn_d), Sense::click_and_drag());
        let t = if logarithmic {
            let lmin = min.max(1e-4).ln();
            let lmax = max.max(min + 1e-4).ln();
            ((value.clamp(min, max).ln() - lmin) / (lmax - lmin)).clamp(0.0, 1.0) as f32
        } else {
            ((value.clamp(min, max) - min) / (max - min)).clamp(0.0, 1.0) as f32
        };

        if resp.clicked() || resp.dragged() {
            if let Some(p) = resp.interact_pointer_pos() {
                let nt = ((p.x - rect.left()) / rect.width().max(1.0)).clamp(0.0, 1.0) as f64;
                let nv = if logarithmic {
                    let lmin = min.max(1e-4).ln();
                    let lmax = max.max(min + 1e-4).ln();
                    (lmin + nt * (lmax - lmin)).exp()
                } else {
                    min + nt * (max - min)
                };
                if (nv - *value).abs() > 1e-6 {
                    *value = nv.clamp(min, max);
                    changed = true;
                }
            }
        }

        let painter = ui.painter();
        let cy = rect.center().y;
        let track_h = 3.5;
        let track_rect = Rect::from_min_max(
            Pos2::new(rect.left(), cy - track_h * 0.5),
            Pos2::new(rect.right(), cy + track_h * 0.5),
        );
        painter.rect_filled(track_rect, CornerRadius::same(2), Color32::from_rgb(0x28, 0x2B, 0x33));

        let thumb_x = rect.left() + t * rect.width();
        let fill_rect = Rect::from_min_max(
            Pos2::new(rect.left(), cy - track_h * 0.5),
            Pos2::new(thumb_x, cy + track_h * 0.5),
        );
        painter.rect_filled(fill_rect, CornerRadius::same(2), NOTHING_RED);

        let thumb_r = if resp.hovered() || resp.dragged() { 7.5 } else { 6.5 };
        painter.circle(
            Pos2::new(thumb_x, cy),
            thumb_r,
            PURE_WHITE,
            Stroke::new(2.0_f32, NOTHING_RED),
        );

        if circle_icon_btn(ui, "+", btn_d).clicked() {
            let next = if logarithmic {
                (*value * (1.0 + step)).clamp(min, max)
            } else {
                (*value + step).clamp(min, max)
            };
            if (next - *value).abs() > 1e-6 {
                *value = next;
                changed = true;
            }
        }

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add_sized(
                Vec2::new(val_w, btn_d),
                egui::Label::new(
                    egui::RichText::new(value_text)
                        .size(11.0)
                        .strong()
                        .monospace()
                        .color(PURE_WHITE),
                ),
            );
        });
    });

    changed
}

fn glyph_5x7(ch: char) -> [&'static str; 7] {
    match ch.to_ascii_uppercase() {
        'A' => [".###.", "#...#", "#...#", "#####", "#...#", "#...#", "#...#"],
        'B' => ["####.", "#...#", "#...#", "####.", "#...#", "#...#", "####."],
        'C' => [".####", "#....", "#....", "#....", "#....", "#....", ".####"],
        'D' => ["####.", "#...#", "#...#", "#...#", "#...#", "#...#", "####."],
        'E' => ["#####", "#....", "#....", "####.", "#....", "#....", "#####"],
        'F' => ["#####", "#....", "#....", "####.", "#....", "#....", "#...."],
        'G' => [".####", "#....", "#....", "#..##", "#...#", "#...#", ".####"],
        'H' => ["#...#", "#...#", "#...#", "#####", "#...#", "#...#", "#...#"],
        'I' => [".###.", "..#..", "..#..", "..#..", "..#..", "..#..", ".###."],
        'J' => ["..###", "...#.", "...#.", "...#.", "#..#.", "#..#.", ".##.."],
        'K' => ["#...#", "#..#.", "#.#..", "##...", "#.#..", "#..#.", "#...#"],
        'L' => ["#....", "#....", "#....", "#....", "#....", "#....", "#####"],
        'M' => ["#...#", "##.##", "#.#.#", "#...#", "#...#", "#...#", "#...#"],
        'N' => ["#...#", "##..#", "#.#.#", "#..##", "#...#", "#...#", "#...#"],
        'O' => [".###.", "#...#", "#...#", "#...#", "#...#", "#...#", ".###."],
        'P' => ["####.", "#...#", "#...#", "####.", "#....", "#....", "#...."],
        'Q' => [".###.", "#...#", "#...#", "#...#", "#.#.#", "#..#.", ".##.#"],
        'R' => ["####.", "#...#", "#...#", "####.", "#.#..", "#..#.", "#...#"],
        'S' => [".####", "#....", "#....", ".###.", "....#", "....#", "####."],
        'T' => ["#####", "..#..", "..#..", "..#..", "..#..", "..#..", "..#.."],
        'U' => ["#...#", "#...#", "#...#", "#...#", "#...#", "#...#", ".###."],
        'V' => ["#...#", "#...#", "#...#", "#...#", ".#.#.", ".#.#.", "..#.."],
        'W' => ["#...#", "#...#", "#...#", "#...#", "#.#.#", "##.##", "#...#"],
        'X' => ["#...#", "#...#", ".#.#.", "..#..", ".#.#.", "#...#", "#...#"],
        'Y' => ["#...#", "#...#", ".#.#.", "..#..", "..#..", "..#..", "..#.."],
        'Z' => ["#####", "....#", "...#.", "..#..", ".#...", "#....", "#####"],
        '0' => [".###.", "#..##", "#.#.#", "#.#.#", "##..#", "#...#", ".###."],
        '1' => ["..#..", ".##..", "..#..", "..#..", "..#..", "..#..", ".###."],
        '2' => [".###.", "#...#", "....#", "..##.", ".#...", "#....", "#####"],
        '3' => ["####.", "....#", "....#", ".###.", "....#", "....#", "####."],
        '4' => ["...#.", "..##.", ".#.#.", "#..#.", "#####", "...#.", "...#."],
        '5' => ["#####", "#....", "####.", "....#", "....#", "#...#", ".###."],
        '6' => [".###.", "#....", "####.", "#...#", "#...#", "#...#", ".###."],
        '7' => ["#####", "....#", "...#.", "..#..", ".#...", ".#...", ".#..."],
        '8' => [".###.", "#...#", "#...#", ".###.", "#...#", "#...#", ".###."],
        '9' => [".###.", "#...#", "#...#", ".####", "....#", "....#", ".###."],
        '-' => [".....", ".....", ".....", "#####", ".....", ".....", "....."],
        '.' => [".....", ".....", ".....", ".....", ".....", ".##..", ".##.."],
        '·' => [".....", ".....", ".....", "..#..", ".....", ".....", "....."],
        _ => [".....", ".....", ".....", ".....", ".....", ".....", "....."],
    }
}

/// Renders Nothing OS 5x7 dot-matrix lettering with optional leading red indicator dot.
pub fn draw_dot_matrix(ui: &mut egui::Ui, text: &str, dot_step: f32, color: Color32, red_dot: bool) {
    let chars: Vec<char> = text.chars().collect();
    let char_w = 5.0 * dot_step;
    let char_gap = dot_step * 1.45;
    let lead_w = if red_dot { dot_step * 5.2 } else { 0.0 };
    let total_w = lead_w + (chars.len() as f32) * (char_w + char_gap);
    let total_h = 7.0 * dot_step;

    let (rect, _) = ui.allocate_exact_size(Vec2::new(total_w, total_h), Sense::hover());
    let painter = ui.painter();
    let radius = (dot_step * 0.35).max(1.0);

    if red_dot {
        painter.circle_filled(
            Pos2::new(rect.left() + dot_step * 1.8, rect.center().y),
            dot_step * 1.65,
            NOTHING_RED,
        );
    }

    for (ci, &ch) in chars.iter().enumerate() {
        let g = glyph_5x7(ch);
        let ox = rect.left() + lead_w + (ci as f32) * (char_w + char_gap);
        for (row, line) in g.iter().enumerate() {
            for (col, byte) in line.bytes().enumerate() {
                if byte == b'#' {
                    let cx = ox + (col as f32 + 0.5) * dot_step;
                    let cy = rect.top() + (row as f32 + 0.5) * dot_step;
                    painter.circle_filled(Pos2::new(cx, cy), radius, color);
                }
            }
        }
    }
}
