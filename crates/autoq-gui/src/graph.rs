//! Interactive log-frequency (20 Hz – 20 kHz) PEQ / FR graph canvas with
//! draggable filter nodes and mouse-wheel Q adjustment.

use crate::theme::{
    BORDER, DIM_GRAY, DOT_GRID, GRAPH_BG, MUTED_GRAY, NOTHING_RED, PURE_WHITE, STOCK_LINE,
    TARGET_LINE,
};
use autoq_core::biquad::{
    combined_response_db, geomspace, interp_log_freq, FilterBand, MAX_FREQ_HZ, MAX_Q, MIN_FREQ_HZ,
    MIN_Q, SAMPLE_RATE_HZ,
};
use eframe::egui::{self, Align2, Color32, CornerRadius, FontId, Pos2, Rect, Sense, Stroke, Vec2};

pub struct GraphState<'a> {
    pub bands: &'a [FilterBand],
    pub selected_band: usize,
    pub dsp_enabled: bool,
    pub overlay_fr: bool,
    pub stock_fr: &'a [[f64; 2]],
    pub target_fr: &'a [[f64; 2]],
    pub target_delta: &'a [[f64; 2]],
    pub target_label: &'a str,
    pub rmse_text: &'a str,
    pub max_gain_db: f64,
    pub height: f32,
}

#[derive(Debug, Clone)]
pub enum GraphEvent {
    None,
    Select(usize),
    DragBand {
        idx: usize,
        freq: f64,
        gain: f64,
        released: bool,
    },
    ScrollQ {
        idx: usize,
        q: f64,
    },
}

fn freq_to_x(freq: f64, plot: Rect) -> f32 {
    let lf = freq.clamp(MIN_FREQ_HZ, MAX_FREQ_HZ).log10();
    let lmin = MIN_FREQ_HZ.log10();
    let lmax = MAX_FREQ_HZ.log10();
    let t = ((lf - lmin) / (lmax - lmin)) as f32;
    plot.left() + t * plot.width()
}

fn x_to_freq(x: f32, plot: Rect) -> f64 {
    let t = ((x - plot.left()) / plot.width().max(1.0)).clamp(0.0, 1.0) as f64;
    let lmin = MIN_FREQ_HZ.log10();
    let lmax = MAX_FREQ_HZ.log10();
    10.0_f64.powf(lmin + t * (lmax - lmin)).round()
}

fn db_to_y(db: f64, plot: Rect, db_min: f64, db_max: f64) -> f32 {
    let t = ((db - db_min) / (db_max - db_min)).clamp(0.0, 1.0) as f32;
    plot.bottom() - t * plot.height()
}

fn y_to_db(y: f32, plot: Rect, db_min: f64, db_max: f64) -> f64 {
    let t = ((plot.bottom() - y) / plot.height().max(1.0)).clamp(0.0, 1.0) as f64;
    ((db_min + t * (db_max - db_min)) * 10.0).round() / 10.0
}

fn interp_curve_pairs(pts: &[[f64; 2]], freqs: &[f64]) -> Vec<f64> {
    if pts.is_empty() {
        return Vec::new();
    }
    let sf: Vec<f64> = pts.iter().map(|p| p[0]).collect();
    let sv: Vec<f64> = pts.iter().map(|p| p[1]).collect();
    interp_log_freq(freqs, &sf, &sv)
}

pub fn draw_peq_graph(ui: &mut egui::Ui, st: &GraphState<'_>, dragging_idx: &mut Option<usize>) -> GraphEvent {
    let desired = Vec2::new(ui.available_width().max(260.0), st.height);
    let (outer, resp) = ui.allocate_exact_size(desired, Sense::click_and_drag());
    let painter = ui.painter_at(outer);

    // Outer obsidian card
    painter.rect(
        outer,
        CornerRadius::same(14),
        GRAPH_BG,
        Stroke::new(1.0_f32, BORDER),
        egui::StrokeKind::Inside,
    );

    let plot = Rect::from_min_max(
        Pos2::new(outer.left() + 30.0, outer.top() + 18.0),
        Pos2::new(outer.right() - 12.0, outer.bottom() - 20.0),
    );

    let show_fr = st.overlay_fr && !st.stock_fr.is_empty();
    let (db_min, db_max, db_step) = if show_fr {
        (-18.0, 18.0, 6.0)
    } else {
        let cap = st.max_gain_db.max(12.0);
        (-cap, cap, 6.0)
    };

    // Dot-matrix background grid
    let grid_freqs = [20.0, 50.0, 100.0, 200.0, 500.0, 1000.0, 2000.0, 5000.0, 10000.0, 20000.0];
    let mut db_tick = db_min;
    while db_tick <= db_max + 0.1 {
        let y = db_to_y(db_tick, plot, db_min, db_max);
        let is_zero = db_tick.abs() < 0.01;
        if is_zero {
            painter.line_segment(
                [Pos2::new(plot.left(), y), Pos2::new(plot.right(), y)],
                Stroke::new(1.0_f32, Color32::from_rgb(0x30, 0x34, 0x3E)),
            );
        } else {
            let mut x = plot.left();
            while x <= plot.right() {
                painter.circle_filled(Pos2::new(x, y), 0.9, DOT_GRID);
                x += 9.0;
            }
        }
        painter.text(
            Pos2::new(plot.left() - 5.0, y),
            Align2::RIGHT_CENTER,
            format!("{:+.0}", db_tick),
            FontId::monospace(9.0),
            DIM_GRAY,
        );
        db_tick += db_step;
    }

    for &f in &grid_freqs {
        let x = freq_to_x(f, plot);
        let mut y = plot.top();
        while y <= plot.bottom() {
            painter.circle_filled(Pos2::new(x, y), 0.9, DOT_GRID);
            y += 9.0;
        }
        let lbl = if f >= 1000.0 {
            format!("{}k", (f / 1000.0).round() as i32)
        } else {
            format!("{}", f.round() as i32)
        };
        painter.text(
            Pos2::new(x, outer.bottom() - 9.0),
            Align2::CENTER_CENTER,
            lbl,
            FontId::monospace(9.0),
            DIM_GRAY,
        );
    }

    // Evaluate frequency curves (200 log points)
    let freqs = geomspace(MIN_FREQ_HZ, MAX_FREQ_HZ, 200);
    let eq_resp = if st.dsp_enabled {
        combined_response_db(st.bands, &freqs, SAMPLE_RATE_HZ)
    } else {
        vec![0.0; freqs.len()]
    };

    if show_fr {
        let stock_vals = interp_curve_pairs(st.stock_fr, &freqs);
        let stock_pts: Vec<Pos2> = freqs
            .iter()
            .zip(stock_vals.iter())
            .map(|(&f, &v)| Pos2::new(freq_to_x(f, plot), db_to_y(v, plot, db_min, db_max)))
            .collect();
        painter.add(egui::Shape::line(stock_pts, Stroke::new(1.4_f32, STOCK_LINE)));

        if !st.target_fr.is_empty() {
            let tgt_vals = interp_curve_pairs(st.target_fr, &freqs);
            let tgt_pts: Vec<Pos2> = freqs
                .iter()
                .zip(tgt_vals.iter())
                .map(|(&f, &v)| Pos2::new(freq_to_x(f, plot), db_to_y(v, plot, db_min, db_max)))
                .collect();
            painter.add(egui::Shape::line(tgt_pts, Stroke::new(1.4_f32, TARGET_LINE)));
        }

        // Resulting FR = Stock + EQ
        let res_pts: Vec<Pos2> = freqs
            .iter()
            .zip(stock_vals.iter().zip(eq_resp.iter()))
            .map(|(&f, (&s, &e))| Pos2::new(freq_to_x(f, plot), db_to_y(s + e, plot, db_min, db_max)))
            .collect();
        let active_col = if st.dsp_enabled { NOTHING_RED } else { MUTED_GRAY };
        painter.add(egui::Shape::line(res_pts, Stroke::new(2.2_f32, active_col)));
    } else {
        if !st.target_delta.is_empty() {
            let td_vals = interp_curve_pairs(st.target_delta, &freqs);
            let td_pts: Vec<Pos2> = freqs
                .iter()
                .zip(td_vals.iter())
                .map(|(&f, &v)| Pos2::new(freq_to_x(f, plot), db_to_y(v, plot, db_min, db_max)))
                .collect();
            painter.add(egui::Shape::line(td_pts, Stroke::new(1.3_f32, TARGET_LINE)));
        }
        let eq_pts: Vec<Pos2> = freqs
            .iter()
            .zip(eq_resp.iter())
            .map(|(&f, &v)| Pos2::new(freq_to_x(f, plot), db_to_y(v, plot, db_min, db_max)))
            .collect();
        let active_col = if st.dsp_enabled { NOTHING_RED } else { MUTED_GRAY };
        painter.add(egui::Shape::line(eq_pts, Stroke::new(2.2_f32, active_col)));
    }

    // Legend / RMSE badges at top of graph
    let mode_tag = if show_fr { "FR OVERLAY" } else { "ΔEQ CURVE" };
    painter.text(
        Pos2::new(plot.left() + 4.0, outer.top() + 9.0),
        Align2::LEFT_CENTER,
        mode_tag,
        FontId::monospace(9.0),
        MUTED_GRAY,
    );
    if !st.target_label.is_empty() || !st.rmse_text.is_empty() {
        let badge = format!("{}  {}", st.target_label, st.rmse_text).trim().to_string();
        painter.text(
            Pos2::new(plot.right() - 4.0, outer.top() + 9.0),
            Align2::RIGHT_CENTER,
            badge,
            FontId::monospace(9.0),
            TARGET_LINE,
        );
    }

    // Compute node positions for all bands
    let stock_at_band = |f: f64| -> f64 {
        if show_fr {
            interp_curve_pairs(st.stock_fr, &[f]).first().copied().unwrap_or(0.0)
        } else {
            0.0
        }
    };

    let node_positions: Vec<Pos2> = st
        .bands
        .iter()
        .map(|b| {
            let x = freq_to_x(b.frequency, plot);
            let y_db = if show_fr {
                stock_at_band(b.frequency) + b.gain
            } else {
                b.gain
            };
            let y = db_to_y(y_db, plot, db_min, db_max);
            Pos2::new(x, y)
        })
        .collect();

    for (idx, (&pos, _b)) in node_positions.iter().zip(st.bands.iter()).enumerate() {
        let is_sel = idx == st.selected_band;
        if is_sel {
            painter.circle_filled(pos, 11.0, Color32::from_rgba_unmultiplied(0xD7, 0x19, 0x21, 55));
            painter.circle_filled(pos, 7.0, NOTHING_RED);
            painter.circle_stroke(pos, 7.0, Stroke::new(1.5_f32, PURE_WHITE));
        } else {
            painter.circle_filled(pos, 5.5, Color32::from_rgb(0x20, 0x22, 0x27));
            painter.circle_stroke(pos, 5.5, Stroke::new(1.2_f32, MUTED_GRAY));
        }
        painter.text(
            pos,
            Align2::CENTER_CENTER,
            format!("{}", idx + 1),
            FontId::monospace(if is_sel { 8.5 } else { 7.5 }),
            PURE_WHITE,
        );
    }

    // Mouse interaction: click/drag nodes & mouse-wheel Q
    let mut event = GraphEvent::None;

    if resp.drag_started() {
        if let Some(p) = resp.interact_pointer_pos() {
            if let Some((idx, _)) = node_positions
                .iter()
                .enumerate()
                .min_by(|a, b| a.1.distance(p).partial_cmp(&b.1.distance(p)).unwrap())
            {
                if node_positions[idx].distance(p) <= 22.0 {
                    *dragging_idx = Some(idx);
                    event = GraphEvent::Select(idx);
                }
            }
        }
    }

    if resp.dragged() {
        if let (Some(idx), Some(p)) = (*dragging_idx, resp.interact_pointer_pos()) {
            let nf = x_to_freq(p.x, plot);
            let raw_db = y_to_db(p.y, plot, db_min, db_max);
            let ng = if show_fr {
                (raw_db - stock_at_band(nf)).clamp(-st.max_gain_db, st.max_gain_db)
            } else {
                raw_db.clamp(-st.max_gain_db, st.max_gain_db)
            };
            let ng_r = (ng * 10.0).round() / 10.0;
            event = GraphEvent::DragBand {
                idx,
                freq: nf,
                gain: ng_r,
                released: false,
            };
        }
    }

    if resp.drag_stopped() {
        if let Some(idx) = dragging_idx.take() {
            if let Some(b) = st.bands.get(idx) {
                event = GraphEvent::DragBand {
                    idx,
                    freq: b.frequency,
                    gain: b.gain,
                    released: true,
                };
            }
        }
    } else if resp.clicked() {
        if let Some(p) = resp.interact_pointer_pos() {
            if let Some((idx, _)) = node_positions
                .iter()
                .enumerate()
                .min_by(|a, b| a.1.distance(p).partial_cmp(&b.1.distance(p)).unwrap())
            {
                if node_positions[idx].distance(p) <= 24.0 {
                    event = GraphEvent::Select(idx);
                }
            }
        }
    }

    if resp.hovered() {
        let scroll_y = ui.input(|i| i.raw_scroll_delta.y);
        if scroll_y.abs() > 0.1 {
            let idx = st.selected_band.min(st.bands.len().saturating_sub(1));
            if let Some(b) = st.bands.get(idx) {
                let factor = if scroll_y > 0.0 { 1.12 } else { 1.0 / 1.12 };
                let nq = ((b.q * factor).clamp(MIN_Q, MAX_Q) * 100.0).round() / 100.0;
                event = GraphEvent::ScrollQ { idx, q: nq };
            }
        }
    }

    event
}
