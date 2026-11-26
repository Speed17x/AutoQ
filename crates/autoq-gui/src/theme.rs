//! Modular UI theme system for AutoQ Desktop (`egui`).
//!
//! Supported themes:
//! - `UiThemeId::PureHyper` (default): Clean Android / HyperOS dark look with Consolas/Inconsolata
//!   typography, subtle rounded cards, and HyperOS blue (`#3482FF`) accents.
//! - `UiThemeId::PureLight`: Crisp Android / HyperOS light theme with deep blue (`#2563EB`) accents.
//! - `UiThemeId::NothingOs` (optional): Obsidian matte `#0B0C0E`, 5x7 dot-matrix headers, and
//!   Nothing Red `#D71921` accents.

use autoq_core::config::{UiAccentId, UiThemeId};
use eframe::egui::{
    self, Align2, Color32, CornerRadius, FontId, Pos2, Rect, Response, Sense, Stroke, StrokeKind,
    Vec2,
};
use std::sync::{OnceLock, RwLock};

#[derive(Clone, Copy)]
pub struct ThemePalette {
    pub is_dark: bool,
    pub bg: Color32,
    pub surface: Color32,
    pub surface_high: Color32,
    pub surface_pill: Color32,
    pub tab_active: Color32,
    pub inset_bg: Color32,
    pub graph_bg: Color32,
    pub border: Color32,
    pub border_bright: Color32,
    pub accent: Color32,
    pub accent_hover: Color32,
    pub accent_tint: Color32,
    pub text_primary: Color32,
    pub text_on_accent: Color32,
    pub text_muted: Color32,
    pub text_subtle: Color32,
    pub grid: Color32,
    pub zero_line: Color32,
    pub stock_line: Color32,
    pub target_line: Color32,
    pub use_dot_matrix_headers: bool,
    pub use_dot_matrix_grid: bool,
    pub card_radius: u8,
    pub pill_radius: u8,
}

impl ThemePalette {
    pub const fn for_theme(id: UiThemeId) -> Self {
        match id {
            UiThemeId::PureHyper => Self {
                is_dark: true,
                bg: Color32::from_rgb(0x0E, 0x11, 0x16),
                surface: Color32::from_rgb(0x17, 0x1B, 0x22),
                surface_high: Color32::from_rgb(0x1F, 0x25, 0x30),
                surface_pill: Color32::from_rgb(0x1D, 0x23, 0x2E),
                tab_active: Color32::from_rgb(0x24, 0x34, 0x4D),
                inset_bg: Color32::from_rgb(0x12, 0x15, 0x1B),
                graph_bg: Color32::from_rgb(0x12, 0x15, 0x1C),
                border: Color32::from_rgb(0x28, 0x30, 0x3D),
                border_bright: Color32::from_rgb(0x3D, 0x4A, 0x5E),
                accent: Color32::from_rgb(0x34, 0x82, 0xFF),
                accent_hover: Color32::from_rgb(0x52, 0x95, 0xFF),
                accent_tint: Color32::from_rgb(0x18, 0x22, 0x34),
                text_primary: Color32::from_rgb(0xF2, 0xF5, 0xFA),
                text_on_accent: Color32::from_rgb(0xFF, 0xFF, 0xFF),
                text_muted: Color32::from_rgb(0x94, 0x9F, 0xB2),
                text_subtle: Color32::from_rgb(0x63, 0x6E, 0x80),
                grid: Color32::from_rgb(0x23, 0x2A, 0x36),
                zero_line: Color32::from_rgb(0x39, 0x43, 0x54),
                stock_line: Color32::from_rgb(0x7B, 0x88, 0x9B),
                target_line: Color32::from_rgb(0xD8, 0xE2, 0xF2),
                use_dot_matrix_headers: false,
                use_dot_matrix_grid: false,
                card_radius: 18,
                pill_radius: 14,
            },
            UiThemeId::PureLight => Self {
                is_dark: false,
                bg: Color32::from_rgb(0xF4, 0xF6, 0xFA),
                surface: Color32::from_rgb(0xFF, 0xFF, 0xFF),
                surface_high: Color32::from_rgb(0xEE, 0xF2, 0xF8),
                surface_pill: Color32::from_rgb(0xE9, 0xEE, 0xF6),
                tab_active: Color32::from_rgb(0xDC, 0xE8, 0xFF),
                inset_bg: Color32::from_rgb(0xF0, 0xF3, 0xF8),
                graph_bg: Color32::from_rgb(0xF9, 0xFA, 0xFC),
                border: Color32::from_rgb(0xD5, 0xDC, 0xE6),
                border_bright: Color32::from_rgb(0xAD, 0xBA, 0xCC),
                accent: Color32::from_rgb(0x25, 0x63, 0xEB),
                accent_hover: Color32::from_rgb(0x1D, 0x4E, 0xD8),
                accent_tint: Color32::from_rgb(0xE6, 0xEE, 0xFF),
                text_primary: Color32::from_rgb(0x11, 0x18, 0x27),
                text_on_accent: Color32::from_rgb(0xFF, 0xFF, 0xFF),
                text_muted: Color32::from_rgb(0x52, 0x5E, 0x71),
                text_subtle: Color32::from_rgb(0x7A, 0x86, 0x99),
                grid: Color32::from_rgb(0xE2, 0xE8, 0xF0),
                zero_line: Color32::from_rgb(0xB8, 0xC4, 0xD6),
                stock_line: Color32::from_rgb(0x64, 0x74, 0x8B),
                target_line: Color32::from_rgb(0x1E, 0x29, 0x3B),
                use_dot_matrix_headers: false,
                use_dot_matrix_grid: false,
                card_radius: 18,
                pill_radius: 14,
            },
            UiThemeId::NothingOs => Self {
                is_dark: true,
                bg: Color32::from_rgb(0x0B, 0x0C, 0x0E),
                surface: Color32::from_rgb(0x13, 0x15, 0x18),
                surface_high: Color32::from_rgb(0x1B, 0x1D, 0x22),
                surface_pill: Color32::from_rgb(0x16, 0x18, 0x1C),
                tab_active: Color32::from_rgb(0x21, 0x24, 0x2B),
                inset_bg: Color32::from_rgb(0x0C, 0x0D, 0x10),
                graph_bg: Color32::from_rgb(0x0D, 0x0E, 0x11),
                border: Color32::from_rgb(0x27, 0x2A, 0x32),
                border_bright: Color32::from_rgb(0x3D, 0x42, 0x4E),
                accent: Color32::from_rgb(0xD7, 0x19, 0x21),
                accent_hover: Color32::from_rgb(0xEC, 0x25, 0x2E),
                accent_tint: Color32::from_rgb(0x18, 0x14, 0x17),
                text_primary: Color32::from_rgb(0xF5, 0xF5, 0xF5),
                text_on_accent: Color32::from_rgb(0xFF, 0xFF, 0xFF),
                text_muted: Color32::from_rgb(0x8E, 0x93, 0x9E),
                text_subtle: Color32::from_rgb(0x56, 0x5B, 0x66),
                grid: Color32::from_rgb(0x25, 0x28, 0x30),
                zero_line: Color32::from_rgb(0x30, 0x34, 0x3E),
                stock_line: Color32::from_rgb(0x78, 0x7E, 0x8A),
                target_line: Color32::from_rgb(0xE0, 0xE4, 0xEC),
                use_dot_matrix_headers: true,
                use_dot_matrix_grid: true,
                card_radius: 16,
                pill_radius: 15,
            },
        }
    }

    pub fn for_theme_and_accent(id: UiThemeId, accent_id: UiAccentId) -> Self {
        let mut p = Self::for_theme(id);
        if let Some((r, g, b)) = accent_id.rgb(p.is_dark) {
            let accent = Color32::from_rgb(r, g, b);
            p.accent = accent;
            p.accent_hover = adjust_brightness(accent, if p.is_dark { 22 } else { -22 });
            p.accent_tint = blend_color(p.surface, accent, if p.is_dark { 0.16 } else { 0.14 });
            p.tab_active = blend_color(p.surface_high, accent, if p.is_dark { 0.24 } else { 0.20 });
            p.text_on_accent = Color32::WHITE;
        }
        p
    }
}

fn blend_color(base: Color32, tint: Color32, alpha: f32) -> Color32 {
    let inv = 1.0 - alpha;
    let r = (base.r() as f32 * inv + tint.r() as f32 * alpha).round().clamp(0.0, 255.0) as u8;
    let g = (base.g() as f32 * inv + tint.g() as f32 * alpha).round().clamp(0.0, 255.0) as u8;
    let b = (base.b() as f32 * inv + tint.b() as f32 * alpha).round().clamp(0.0, 255.0) as u8;
    Color32::from_rgb(r, g, b)
}

fn adjust_brightness(c: Color32, delta: i16) -> Color32 {
    let r = (c.r() as i16 + delta).clamp(0, 255) as u8;
    let g = (c.g() as i16 + delta).clamp(0, 255) as u8;
    let b = (c.b() as i16 + delta).clamp(0, 255) as u8;
    Color32::from_rgb(r, g, b)
}

static ACTIVE_PALETTE: RwLock<ThemePalette> = RwLock::new(ThemePalette::for_theme(UiThemeId::PureHyper));
static FONTS_INSTALLED: OnceLock<()> = OnceLock::new();

pub fn palette() -> ThemePalette {
    *ACTIVE_PALETTE
        .read()
        .unwrap_or_else(|e| e.into_inner())
}

fn ensure_consolas_fonts(ctx: &egui::Context) {
    FONTS_INSTALLED.get_or_init(|| {
        let mut fonts = egui::FontDefinitions::default();
        let home_font = std::env::var("HOME")
            .map(|h| format!("{h}/.local/share/fonts/consola.ttf"))
            .unwrap_or_default();
        let candidates = [
            "C:\\Windows\\Fonts\\consola.ttf",
            home_font.as_str(),
            "/usr/share/fonts/TTF/consola.ttf",
            "/usr/share/fonts/truetype/msttcorefonts/consola.ttf",
        ];
        let mut loaded: Option<Vec<u8>> = None;
        for path in candidates {
            if !path.is_empty() {
                if let Ok(bytes) = std::fs::read(path) {
                    loaded = Some(bytes);
                    break;
                }
            }
        }
        let font_data = match loaded {
            Some(bytes) => egui::FontData::from_owned(bytes),
            None => egui::FontData::from_static(include_bytes!("../assets/fonts/Inconsolata-Regular.ttf")),
        };
        fonts.font_data.insert("autoq_consolas".to_owned(), font_data.into());
        fonts
            .families
            .entry(egui::FontFamily::Monospace)
            .or_default()
            .insert(0, "autoq_consolas".to_owned());
        fonts
            .families
            .entry(egui::FontFamily::Proportional)
            .or_default()
            .insert(0, "autoq_consolas".to_owned());
        ctx.set_fonts(fonts);
    });
}

pub fn apply_theme(ctx: &egui::Context, theme_id: UiThemeId, accent_id: UiAccentId) {
    ensure_consolas_fonts(ctx);
    let p = ThemePalette::for_theme_and_accent(theme_id, accent_id);
    if let Ok(mut guard) = ACTIVE_PALETTE.write() {
        *guard = p;
    }

    let mut visuals = if p.is_dark {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };
    visuals.window_fill = p.bg;
    visuals.panel_fill = p.bg;
    visuals.faint_bg_color = p.surface;
    visuals.extreme_bg_color = p.inset_bg;
    visuals.override_text_color = Some(p.text_primary);
    visuals.selection.bg_fill = Color32::from_rgba_unmultiplied(p.accent.r(), p.accent.g(), p.accent.b(), 55);
    visuals.selection.stroke = Stroke::new(1.0_f32, p.accent);

    visuals.widgets.noninteractive.bg_fill = p.surface;
    visuals.widgets.noninteractive.weak_bg_fill = p.surface;
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, p.border);
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, p.text_primary);
    visuals.widgets.noninteractive.corner_radius = CornerRadius::same(12);

    visuals.widgets.inactive.bg_fill = p.surface_pill;
    visuals.widgets.inactive.weak_bg_fill = p.surface_pill;
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, p.border);
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, p.text_primary);
    visuals.widgets.inactive.corner_radius = CornerRadius::same(p.pill_radius);

    visuals.widgets.hovered.bg_fill = p.surface_high;
    visuals.widgets.hovered.weak_bg_fill = p.surface_high;
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, p.border_bright);
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0_f32, p.text_primary);
    visuals.widgets.hovered.corner_radius = CornerRadius::same(p.pill_radius);

    visuals.widgets.active.bg_fill = p.tab_active;
    visuals.widgets.active.weak_bg_fill = p.tab_active;
    visuals.widgets.active.bg_stroke = Stroke::new(1.0_f32, p.accent);
    visuals.widgets.active.fg_stroke = Stroke::new(1.0_f32, p.text_primary);
    visuals.widgets.active.corner_radius = CornerRadius::same(p.pill_radius);

    ctx.set_visuals(visuals);

    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = Vec2::new(7.0, 7.0);
    style.spacing.button_padding = Vec2::new(12.0, 6.0);
    style.spacing.scroll.bar_width = 5.0;
    style.spacing.scroll.floating = true;
    ctx.set_style(style);
}

pub fn sub_card_frame() -> egui::Frame {
    let p = palette();
    egui::Frame::NONE
        .fill(p.surface)
        .stroke(Stroke::new(1.0_f32, p.border))
        .corner_radius(CornerRadius::same(p.card_radius))
        .inner_margin(egui::Margin::symmetric(12, 10))
}

pub fn inset_list_frame() -> egui::Frame {
    let p = palette();
    egui::Frame::NONE
        .fill(p.inset_bg)
        .stroke(Stroke::new(1.0_f32, p.border))
        .corner_radius(CornerRadius::same(12))
        .inner_margin(egui::Margin::symmetric(8, 6))
}

pub fn section_label(ui: &mut egui::Ui, title: &str) {
    let p = palette();
    ui.label(
        egui::RichText::new(title)
            .size(10.0)
            .strong()
            .monospace()
            .color(p.text_muted),
    );
}

pub fn outline_pill(ui: &mut egui::Ui, label: &str, active: bool, min_size: Vec2) -> Response {
    let p = palette();
    let fill = if active { p.accent_tint } else { p.surface_pill };
    let stroke = if active {
        Stroke::new(1.2_f32, p.accent)
    } else {
        Stroke::new(1.0_f32, p.border)
    };
    let text_col = if active { p.text_primary } else { p.text_muted };
    ui.add_sized(
        min_size,
        egui::Button::new(egui::RichText::new(label).size(10.5).strong().monospace().color(text_col))
            .fill(fill)
            .stroke(stroke)
            .corner_radius(CornerRadius::same(p.pill_radius)),
    )
}

pub fn red_pill(ui: &mut egui::Ui, label: &str, min_size: Vec2) -> Response {
    let p = palette();
    ui.add_sized(
        min_size,
        egui::Button::new(
            egui::RichText::new(label)
                .size(11.0)
                .strong()
                .monospace()
                .color(p.text_on_accent),
        )
        .fill(p.accent)
        .stroke(Stroke::new(1.0_f32, p.accent_hover))
        .corner_radius(CornerRadius::same(p.pill_radius)),
    )
}

pub fn segment_tab(ui: &mut egui::Ui, label: &str, active: bool, width: f32) -> Response {
    let p = palette();
    let fill = if active { p.tab_active } else { Color32::TRANSPARENT };
    let stroke = if active {
        Stroke::new(1.0_f32, p.border_bright)
    } else {
        Stroke::NONE
    };
    let col = if active { p.text_primary } else { p.text_muted };
    ui.add_sized(
        Vec2::new(width, 28.0),
        egui::Button::new(egui::RichText::new(label).size(11.0).strong().monospace().color(col))
            .fill(fill)
            .stroke(stroke)
            .corner_radius(CornerRadius::same(p.pill_radius)),
    )
}

pub fn circle_icon_btn(ui: &mut egui::Ui, symbol: &str, diameter: f32) -> Response {
    let p = palette();
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(diameter), Sense::click());
    let painter = ui.painter();
    let fill = if resp.is_pointer_button_down_on() {
        p.tab_active
    } else if resp.hovered() {
        p.surface_high
    } else {
        p.surface_pill
    };
    let stroke_col = if resp.hovered() { p.border_bright } else { p.border };
    painter.circle(rect.center(), diameter * 0.5, fill, Stroke::new(1.0_f32, stroke_col));
    painter.text(
        rect.center(),
        Align2::CENTER_CENTER,
        symbol,
        FontId::monospace((diameter * 0.48).clamp(10.0, 14.0)),
        p.text_primary,
    );
    resp
}

pub fn band_card(
    ui: &mut egui::Ui,
    index: usize,
    freq_hz: f64,
    gain_db: f64,
    selected: bool,
    width: f32,
) -> Response {
    let p = palette();
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, 44.0), Sense::click());
    let painter = ui.painter();

    let fill = if selected {
        p.accent_tint
    } else if resp.hovered() {
        p.surface_high
    } else {
        p.inset_bg
    };
    let stroke = if selected {
        Stroke::new(1.3_f32, p.accent)
    } else {
        Stroke::new(1.0_f32, p.border)
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
        if selected { p.text_primary } else { p.text_muted },
    );
    painter.text(
        Pos2::new(rect.center().x, rect.bottom() - 13.0),
        Align2::CENTER_CENTER,
        bot_txt,
        FontId::monospace(10.5),
        if gain_db.abs() >= 0.05 { p.text_primary } else { p.text_subtle },
    );

    resp
}

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
    let p = palette();
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
                    .color(p.text_muted),
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

        let (rect, resp) = ui.allocate_exact_size(Vec2::new(track_w, btn_d), Sense::click_and_drag());
        let t = if logarithmic {
            let lmin = min.max(1e-4).ln();
            let lmax = max.max(min + 1e-4).ln();
            ((value.clamp(min, max).ln() - lmin) / (lmax - lmin)).clamp(0.0, 1.0) as f32
        } else {
            ((value.clamp(min, max) - min) / (max - min)).clamp(0.0, 1.0) as f32
        };

        if resp.clicked() || resp.dragged() {
            if let Some(pos) = resp.interact_pointer_pos() {
                let nt = ((pos.x - rect.left()) / rect.width().max(1.0)).clamp(0.0, 1.0) as f64;
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
        painter.rect_filled(track_rect, CornerRadius::same(2), p.border);

        let thumb_x = rect.left() + t * rect.width();
        let fill_rect = Rect::from_min_max(
            Pos2::new(rect.left(), cy - track_h * 0.5),
            Pos2::new(thumb_x, cy + track_h * 0.5),
        );
        painter.rect_filled(fill_rect, CornerRadius::same(2), p.accent);

        let thumb_r = if resp.hovered() || resp.dragged() { 7.5 } else { 6.5 };
        painter.circle(
            Pos2::new(thumb_x, cy),
            thumb_r,
            p.surface,
            Stroke::new(2.0_f32, p.accent),
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
                        .color(p.text_primary),
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

pub fn draw_brand_title(ui: &mut egui::Ui, text: &str, dot_step: f32, show_dot: bool) {
    let p = palette();
    if p.use_dot_matrix_headers {
        let chars: Vec<char> = text.chars().collect();
        let char_w = 5.0 * dot_step;
        let char_gap = dot_step * 1.45;
        let lead_w = if show_dot { dot_step * 5.2 } else { 0.0 };
        let total_w = lead_w + (chars.len() as f32) * (char_w + char_gap);
        let total_h = 7.0 * dot_step;

        let (rect, _) = ui.allocate_exact_size(Vec2::new(total_w, total_h), Sense::hover());
        let painter = ui.painter();
        let radius = (dot_step * 0.35).max(1.0);

        if show_dot {
            painter.circle_filled(
                Pos2::new(rect.left() + dot_step * 1.8, rect.center().y),
                dot_step * 1.65,
                p.accent,
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
                        painter.circle_filled(Pos2::new(cx, cy), radius, p.text_primary);
                    }
                }
            }
        }
    } else {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            if show_dot {
                let (dot_rect, _) = ui.allocate_exact_size(Vec2::splat(9.0), Sense::hover());
                ui.painter().circle_filled(dot_rect.center(), 4.0, p.accent);
            }
            ui.label(
                egui::RichText::new(text)
                    .size(15.0)
                    .strong()
                    .monospace()
                    .color(p.text_primary),
            );
        });
    }
}
