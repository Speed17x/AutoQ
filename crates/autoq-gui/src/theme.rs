//! Nothing OS industrial minimalist color palette, styling, and 5x7 Dot-Matrix renderer.

use eframe::egui::{self, Color32, CornerRadius, Pos2, Stroke, Vec2};

pub const OBSIDIAN: Color32 = Color32::from_rgb(0x0B, 0x0C, 0x0E);
pub const SURFACE: Color32 = Color32::from_rgb(0x14, 0x15, 0x17);
pub const SURFACE_HIGH: Color32 = Color32::from_rgb(0x1C, 0x1E, 0x22);
pub const SURFACE_PILL: Color32 = Color32::from_rgb(0x20, 0x22, 0x27);
pub const GRAPH_BG: Color32 = Color32::from_rgb(0x0E, 0x0F, 0x12);
pub const BORDER: Color32 = Color32::from_rgb(0x2B, 0x2E, 0x36);
pub const BORDER_BRIGHT: Color32 = Color32::from_rgb(0x3C, 0x40, 0x4B);
pub const NOTHING_RED: Color32 = Color32::from_rgb(0xD7, 0x19, 0x21);
pub const NOTHING_RED_HOVER: Color32 = Color32::from_rgb(0xEA, 0x26, 0x2F);
pub const PURE_WHITE: Color32 = Color32::from_rgb(0xF5, 0xF5, 0xF5);
pub const MUTED_GRAY: Color32 = Color32::from_rgb(0x8E, 0x92, 0x9B);
pub const DIM_GRAY: Color32 = Color32::from_rgb(0x58, 0x5C, 0x66);
pub const DOT_GRID: Color32 = Color32::from_rgb(0x26, 0x29, 0x30);
pub const STOCK_LINE: Color32 = Color32::from_rgb(0x6E, 0x74, 0x80);
pub const TARGET_LINE: Color32 = Color32::from_rgb(0xD8, 0xDC, 0xE4);

pub fn apply_nothing_theme(ctx: &egui::Context) {
    let mut visuals = egui::Visuals::dark();
    visuals.window_fill = OBSIDIAN;
    visuals.panel_fill = OBSIDIAN;
    visuals.faint_bg_color = SURFACE;
    visuals.extreme_bg_color = GRAPH_BG;
    visuals.override_text_color = Some(PURE_WHITE);
    visuals.selection.bg_fill = NOTHING_RED;
    visuals.selection.stroke = Stroke::new(1.0_f32, PURE_WHITE);

    visuals.widgets.noninteractive.bg_fill = SURFACE;
    visuals.widgets.noninteractive.weak_bg_fill = SURFACE;
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, BORDER);
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, PURE_WHITE);
    visuals.widgets.noninteractive.corner_radius = CornerRadius::same(10);

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

    visuals.widgets.active.bg_fill = NOTHING_RED;
    visuals.widgets.active.weak_bg_fill = NOTHING_RED;
    visuals.widgets.active.bg_stroke = Stroke::new(1.0_f32, NOTHING_RED_HOVER);
    visuals.widgets.active.fg_stroke = Stroke::new(1.0_f32, PURE_WHITE);
    visuals.widgets.active.corner_radius = CornerRadius::same(14);

    ctx.set_visuals(visuals);

    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = Vec2::new(6.0, 6.0);
    style.spacing.button_padding = Vec2::new(10.0, 5.0);
    style.spacing.slider_width = 160.0;
    ctx.set_style(style);
}

pub fn sub_card_frame() -> egui::Frame {
    egui::Frame::NONE
        .fill(SURFACE)
        .stroke(Stroke::new(1.0_f32, BORDER))
        .corner_radius(CornerRadius::same(14))
        .inner_margin(egui::Margin::symmetric(10, 8))
}

pub fn pill_button(ui: &mut egui::Ui, label: &str, active: bool) -> egui::Response {
    let fill = if active { NOTHING_RED } else { SURFACE_PILL };
    let stroke = if active {
        Stroke::new(1.0_f32, NOTHING_RED_HOVER)
    } else {
        Stroke::new(1.0_f32, BORDER)
    };
    let text_col = if active { PURE_WHITE } else { MUTED_GRAY };
    let btn = egui::Button::new(egui::RichText::new(label).size(11.0).strong().color(text_col))
        .fill(fill)
        .stroke(stroke)
        .corner_radius(CornerRadius::same(13));
    ui.add(btn)
}

pub fn red_cta_button(ui: &mut egui::Ui, label: &str) -> egui::Response {
    let btn = egui::Button::new(egui::RichText::new(label).size(11.5).strong().color(PURE_WHITE))
        .fill(NOTHING_RED)
        .stroke(Stroke::new(1.0_f32, NOTHING_RED_HOVER))
        .corner_radius(CornerRadius::same(14));
    ui.add(btn)
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

/// Renders Nothing OS 5x7 circular dot-matrix lettering directly on `egui::Painter`.
pub fn draw_dot_matrix(ui: &mut egui::Ui, text: &str, dot_step: f32, color: Color32) {
    let chars: Vec<char> = text.chars().collect();
    let char_w = 5.0 * dot_step;
    let char_gap = dot_step * 1.4;
    let total_w = (chars.len() as f32) * (char_w + char_gap);
    let total_h = 7.0 * dot_step;

    let (rect, _) = ui.allocate_exact_size(Vec2::new(total_w, total_h), egui::Sense::hover());
    let painter = ui.painter();
    let radius = (dot_step * 0.36).max(1.0);

    for (ci, &ch) in chars.iter().enumerate() {
        let g = glyph_5x7(ch);
        let ox = rect.left() + (ci as f32) * (char_w + char_gap);
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
