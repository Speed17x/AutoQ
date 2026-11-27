//! Interactive log-frequency (20 Hz – 20 kHz) PEQ / FR graph canvas driven by
//! the active modular theme palette (`theme::palette()`).

use crate::theme::palette;
use trivium_core::biquad::{
    combined_response_db, geomspace, interp_log_freq, FilterBand, MAX_FREQ_HZ, MAX_Q, MIN_FREQ_HZ,
    MIN_Q, SAMPLE_RATE_HZ,
};
use eframe::egui::{
    self, Align2, Color32, CornerRadius, FontId, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2,
};

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

fn draw_dashed_line(
    painter: &egui::Painter,
    pts: &[Pos2],
    stroke: Stroke,
    dash_len: f32,
    gap_len: f32,
) {
    if pts.len() < 2 {
        return;
    }
    let mut draw_on = true;
    let mut remaining = dash_len;

    for win in pts.windows(2) {
        let mut p0 = win[0];
        let p1 = win[1];
        let mut seg_len = p0.distance(p1);
        if seg_len < 1e-4 {
            continue;
        }
        let dir = (p1 - p0) / seg_len;
        while seg_len > 1e-4 {
            let step = seg_len.min(remaining);
            let next = p0 + dir * step;
            if draw_on {
                painter.line_segment([p0, next], stroke);
            }
            p0 = next;
            seg_len -= step;
            remaining -= step;
            if remaining <= 1e-4 {
                draw_on = !draw_on;
                remaining = if draw_on { dash_len } else { gap_len };
            }
        }
    }
}

fn draw_gradient_fill(
    painter: &egui::Painter,
    pts: &[Pos2],
    plot: Rect,
    active_col: Color32,
    muted_col: Color32,
    enabled: bool,
) {
    if pts.len() < 2 {
        return;
    }
    let c = if enabled { active_col } else { muted_col };
    let top_col = Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), if enabled { 56 } else { 24 });
    let bot_col = Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), if enabled { 4 } else { 2 });

    let mut mesh = egui::epaint::Mesh::default();
    let uv = egui::epaint::WHITE_UV;
    for (i, &p) in pts.iter().enumerate() {
        let clamped_y = p.y.clamp(plot.top(), plot.bottom());
        mesh.vertices.push(egui::epaint::Vertex {
            pos: Pos2::new(p.x, clamped_y),
            uv,
            color: top_col,
        });
        mesh.vertices.push(egui::epaint::Vertex {
            pos: Pos2::new(p.x, plot.bottom()),
            uv,
            color: bot_col,
        });
        if i > 0 {
            let base = ((i - 1) * 2) as u32;
            mesh.indices.extend_from_slice(&[
                base,
                base + 1,
                base + 2,
                base + 1,
                base + 3,
                base + 2,
            ]);
        }
    }
    painter.add(egui::Shape::mesh(mesh));
}

pub fn draw_peq_graph(
    ui: &mut egui::Ui,
    st: &GraphState<'_>,
    dragging_idx: &mut Option<usize>,
) -> GraphEvent {
    let pal = palette();
    let desired = Vec2::new(ui.available_width().max(260.0), st.height);
    let (outer, resp) = ui.allocate_exact_size(desired, Sense::click_and_drag());
    let painter = ui.painter_at(outer);

    painter.rect(
        outer,
        CornerRadius::same(14),
        pal.graph_bg,
        Stroke::new(1.0_f32, pal.border),
        StrokeKind::Inside,
    );

    let plot = Rect::from_min_max(
        Pos2::new(outer.left() + 30.0, outer.top() + 22.0),
        Pos2::new(outer.right() - 12.0, outer.bottom() - 20.0),
    );
    let clipped = painter.with_clip_rect(plot);

    let show_fr = st.overlay_fr && !st.stock_fr.is_empty();
    let (db_min, db_max, db_step) = if show_fr {
        (-15.0, 15.0, 10.0)
    } else {
        let cap = st.max_gain_db.max(12.0);
        (-cap, cap, 6.0)
    };

    let grid_freqs = [20.0, 50.0, 100.0, 200.0, 500.0, 1000.0, 2000.0, 5000.0, 10000.0, 20000.0];
    let mut db_tick = if show_fr { -10.0 } else { db_min };
    let db_end = if show_fr { 10.0 } else { db_max };
    while db_tick <= db_end + 0.1 {
        let y = db_to_y(db_tick, plot, db_min, db_max);
        if db_tick.abs() < 0.01 {
            draw_dashed_line(
                &clipped,
                &[Pos2::new(plot.left(), y), Pos2::new(plot.right(), y)],
                Stroke::new(1.0_f32, pal.zero_line),
                4.0,
                4.0,
            );
        } else if pal.use_dot_matrix_grid {
            let mut x = plot.left();
            while x <= plot.right() {
                clipped.circle_filled(Pos2::new(x, y), 0.9, pal.grid);
                x += 12.0;
            }
        } else {
            clipped.line_segment(
                [Pos2::new(plot.left(), y), Pos2::new(plot.right(), y)],
                Stroke::new(0.8_f32, pal.grid),
            );
        }
        painter.text(
            Pos2::new(plot.left() - 5.0, y),
            Align2::RIGHT_CENTER,
            if db_tick.abs() < 0.01 {
                "0".to_string()
            } else {
                format!("{:+.0}", db_tick)
            },
            FontId::monospace(9.0),
            pal.text_subtle,
        );
        db_tick += db_step;
    }

    for &f in &grid_freqs {
        let x = freq_to_x(f, plot);
        if pal.use_dot_matrix_grid {
            let mut y = plot.top();
            while y <= plot.bottom() {
                clipped.circle_filled(Pos2::new(x, y), 0.9, pal.grid);
                y += 12.0;
            }
        } else {
            clipped.line_segment(
                [Pos2::new(x, plot.top()), Pos2::new(x, plot.bottom())],
                Stroke::new(0.8_f32, pal.grid),
            );
        }
        if matches!(f as i32, 20 | 100 | 500 | 1000 | 5000 | 20000) {
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
                pal.text_subtle,
            );
        }
    }

    let freqs = geomspace(MIN_FREQ_HZ, MAX_FREQ_HZ, 200);
    let eq_resp = if st.dsp_enabled {
        combined_response_db(st.bands, &freqs, SAMPLE_RATE_HZ)
    } else {
        vec![0.0; freqs.len()]
    };

    let active_col = if st.dsp_enabled { pal.accent } else { pal.text_muted };
    let glow_col = if st.dsp_enabled {
        Color32::from_rgba_unmultiplied(pal.accent.r(), pal.accent.g(), pal.accent.b(), 48)
    } else {
        Color32::from_rgba_unmultiplied(pal.text_muted.r(), pal.text_muted.g(), pal.text_muted.b(), 25)
    };

    if show_fr {
        let stock_vals = interp_curve_pairs(st.stock_fr, &freqs);
        let stock_pts: Vec<Pos2> = freqs
            .iter()
            .zip(stock_vals.iter())
            .map(|(&f, &v)| Pos2::new(freq_to_x(f, plot), db_to_y(v, plot, db_min, db_max)))
            .collect();
        draw_dashed_line(&clipped, &stock_pts, Stroke::new(1.3_f32, pal.stock_line), 5.0, 4.0);

        if !st.target_fr.is_empty() {
            let tgt_vals = interp_curve_pairs(st.target_fr, &freqs);
            let tgt_pts: Vec<Pos2> = freqs
                .iter()
                .zip(tgt_vals.iter())
                .map(|(&f, &v)| Pos2::new(freq_to_x(f, plot), db_to_y(v, plot, db_min, db_max)))
                .collect();
            draw_dashed_line(&clipped, &tgt_pts, Stroke::new(1.3_f32, pal.target_line), 3.5, 3.5);
        }

        let res_pts: Vec<Pos2> = freqs
            .iter()
            .zip(stock_vals.iter().zip(eq_resp.iter()))
            .map(|(&f, (&s, &e))| Pos2::new(freq_to_x(f, plot), db_to_y(s + e, plot, db_min, db_max)))
            .collect();
        draw_gradient_fill(&clipped, &res_pts, plot, pal.accent, pal.text_muted, st.dsp_enabled);
        clipped.add(egui::Shape::line(res_pts.clone(), Stroke::new(5.5_f32, glow_col)));
        clipped.add(egui::Shape::line(res_pts, Stroke::new(2.3_f32, active_col)));
    } else {
        if !st.target_delta.is_empty() {
            let td_vals = interp_curve_pairs(st.target_delta, &freqs);
            let td_pts: Vec<Pos2> = freqs
                .iter()
                .zip(td_vals.iter())
                .map(|(&f, &v)| Pos2::new(freq_to_x(f, plot), db_to_y(v, plot, db_min, db_max)))
                .collect();
            draw_dashed_line(&clipped, &td_pts, Stroke::new(1.3_f32, pal.target_line), 3.5, 3.5);
        }
        let eq_pts: Vec<Pos2> = freqs
            .iter()
            .zip(eq_resp.iter())
            .map(|(&f, &v)| Pos2::new(freq_to_x(f, plot), db_to_y(v, plot, db_min, db_max)))
            .collect();
        draw_gradient_fill(&clipped, &eq_pts, plot, pal.accent, pal.text_muted, st.dsp_enabled);
        clipped.add(egui::Shape::line(eq_pts.clone(), Stroke::new(5.5_f32, glow_col)));
        clipped.add(egui::Shape::line(eq_pts, Stroke::new(2.3_f32, active_col)));
    }

    let mut lx = outer.left() + 14.0;
    let ly = outer.top() + 11.0;
    painter.circle_filled(Pos2::new(lx + 3.0, ly), 3.0, active_col);
    lx += 9.0;
    let live_lbl = if st.dsp_enabled { "LIVE" } else { "BYPASS" };
    painter.text(
        Pos2::new(lx, ly),
        Align2::LEFT_CENTER,
        live_lbl,
        FontId::monospace(9.5),
        active_col,
    );
    lx += 42.0;

    if show_fr {
        painter.text(
            Pos2::new(lx, ly),
            Align2::LEFT_CENTER,
            "-- STOCK",
            FontId::monospace(9.5),
            pal.stock_line,
        );
        lx += 62.0;
    }
    if !st.target_label.is_empty() {
        let short_tgt = if st.target_label.len() > 16 {
            format!("— {}", &st.target_label[..16])
        } else {
            format!("— {}", st.target_label)
        };
        painter.text(
            Pos2::new(lx, ly),
            Align2::LEFT_CENTER,
            short_tgt.to_uppercase(),
            FontId::monospace(9.5),
            pal.target_line,
        );
    }
    if !st.rmse_text.is_empty() {
        painter.text(
            Pos2::new(outer.right() - 12.0, ly),
            Align2::RIGHT_CENTER,
            st.rmse_text,
            FontId::monospace(9.5),
            pal.text_muted,
        );
    }

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
            let y = db_to_y(y_db, plot, db_min, db_max).clamp(plot.top() + 4.0, plot.bottom() - 4.0);
            Pos2::new(x, y)
        })
        .collect();

    for (idx, &pos) in node_positions.iter().enumerate() {
        if idx == st.selected_band {
            clipped.circle_filled(
                pos,
                11.5,
                Color32::from_rgba_unmultiplied(pal.accent.r(), pal.accent.g(), pal.accent.b(), 58),
            );
            clipped.circle(pos, 7.2, pal.accent, Stroke::new(2.0_f32, pal.text_primary));
        } else {
            clipped.circle_filled(pos, 3.8, active_col);
        }
    }

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
