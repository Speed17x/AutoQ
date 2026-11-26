//! RBJ Audio EQ Cookbook biquad math and multi-band PEQ optimizer.

use serde::{Deserialize, Serialize};
use std::f64::consts::PI;

pub const SAMPLE_RATE_HZ: f64 = 48_000.0;
pub const MIN_FREQ_HZ: f64 = 20.0;
pub const MAX_FREQ_HZ: f64 = 20_000.0;
pub const MAX_GAIN_DB: f64 = 12.0;
pub const MIN_Q: f64 = 0.1;
pub const MAX_Q: f64 = 10.0;
pub const DEFAULT_MAX_BANDS: usize = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FilterType {
    Peaking,
    LowShelf,
    HighShelf,
}

impl Default for FilterType {
    fn default() -> Self {
        Self::Peaking
    }
}

impl FilterType {
    pub fn from_loose_str(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "low_shelf" | "lsc" | "ls" | "lowshelf" => Self::LowShelf,
            "high_shelf" | "hsc" | "hs" | "highshelf" => Self::HighShelf,
            _ => Self::Peaking,
        }
    }

    pub fn apo_code(self) -> &'static str {
        match self {
            Self::Peaking => "PK",
            Self::LowShelf => "LSC",
            Self::HighShelf => "HSC",
        }
    }

    pub fn pipewire_label(self) -> &'static str {
        match self {
            Self::Peaking => "bq_peaking",
            Self::LowShelf => "bq_lowshelf",
            Self::HighShelf => "bq_highshelf",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FilterBand {
    #[serde(rename = "type", default)]
    pub filter_type: FilterType,
    #[serde(default = "default_freq")]
    pub frequency: f64,
    #[serde(default)]
    pub gain: f64,
    #[serde(default = "default_q")]
    pub q: f64,
}

fn default_freq() -> f64 {
    1000.0
}

fn default_q() -> f64 {
    1.0
}

impl Default for FilterBand {
    fn default() -> Self {
        Self {
            filter_type: FilterType::Peaking,
            frequency: 1000.0,
            gain: 0.0,
            q: 1.0,
        }
    }
}

impl FilterBand {
    pub fn new(filter_type: FilterType, frequency: f64, gain: f64, q: f64) -> Self {
        Self {
            filter_type,
            frequency,
            gain,
            q,
        }
    }

    pub fn clamped(&self) -> Self {
        let freq = self.frequency.clamp(MIN_FREQ_HZ, MAX_FREQ_HZ).round();
        let gain = (self.gain.clamp(-MAX_GAIN_DB, MAX_GAIN_DB) * 10.0).round() / 10.0;
        let q = (self.q.clamp(MIN_Q, MAX_Q) * 1000.0).round() / 1000.0;
        Self {
            filter_type: self.filter_type,
            frequency: freq,
            gain,
            q,
        }
    }

    pub fn clamped_with_max_gain(&self, max_gain_db: f64) -> Self {
        let cap = max_gain_db.abs().max(0.1);
        let freq = self.frequency.clamp(MIN_FREQ_HZ, MAX_FREQ_HZ).round();
        let gain = (self.gain.clamp(-cap, cap) * 10.0).round() / 10.0;
        let q = (self.q.clamp(MIN_Q, MAX_Q) * 1000.0).round() / 1000.0;
        Self {
            filter_type: self.filter_type,
            frequency: freq,
            gain,
            q,
        }
    }
}

pub fn geomspace(start: f64, stop: f64, n: usize) -> Vec<f64> {
    if n == 0 {
        return Vec::new();
    }
    if n == 1 {
        return vec![start];
    }
    let log_start = start.ln();
    let log_stop = stop.ln();
    let step = (log_stop - log_start) / ((n - 1) as f64);
    (0..n).map(|i| (log_start + (i as f64) * step).exp()).collect()
}

pub fn standard_grid() -> Vec<f64> {
    geomspace(20.0, 20_000.0, 128)
}

/// Linear interpolation over log10(freq), clamping outside endpoints like `numpy.interp`.
pub fn interp_log_freq(target_freqs: &[f64], src_freqs: &[f64], src_vals: &[f64]) -> Vec<f64> {
    if src_freqs.is_empty() || src_vals.is_empty() {
        return vec![0.0; target_freqs.len()];
    }
    let n = src_freqs.len().min(src_vals.len());
    if n == 1 {
        return vec![src_vals[0]; target_freqs.len()];
    }

    let log_src: Vec<f64> = src_freqs[..n].iter().map(|f| f.max(1e-6).log10()).collect();
    target_freqs
        .iter()
        .map(|&tf| {
            let lx = tf.max(1e-6).log10();
            if lx <= log_src[0] {
                return src_vals[0];
            }
            if lx >= log_src[n - 1] {
                return src_vals[n - 1];
            }
            let idx = match log_src.binary_search_by(|v| v.partial_cmp(&lx).unwrap()) {
                Ok(i) => return src_vals[i],
                Err(i) => i,
            };
            let x0 = log_src[idx - 1];
            let x1 = log_src[idx];
            let y0 = src_vals[idx - 1];
            let y1 = src_vals[idx];
            let span = x1 - x0;
            if span.abs() < 1e-12 {
                y0
            } else {
                y0 + (lx - x0) / span * (y1 - y0)
            }
        })
        .collect()
}

/// Return RBJ Audio EQ Cookbook coefficients `(b0, b1, b2, a0, a1, a2)`.
pub fn biquad_coeffs(band: &FilterBand, fs: f64) -> (f64, f64, f64, f64, f64, f64) {
    let f0 = band.frequency.clamp(1.0, fs * 0.495);
    let q = band.q.clamp(MIN_Q, MAX_Q);
    let gain_db = band.gain.clamp(-18.0, 18.0);

    if gain_db.abs() < 1e-6 {
        return (1.0, 0.0, 0.0, 1.0, 0.0, 0.0);
    }

    let a = 10.0_f64.powf(gain_db / 40.0);
    let w0 = 2.0 * PI * f0 / fs;
    let cos_w0 = w0.cos();
    let sin_w0 = w0.sin();
    let alpha = sin_w0 / (2.0 * q);

    match band.filter_type {
        FilterType::Peaking => {
            let b0 = 1.0 + alpha * a;
            let b1 = -2.0 * cos_w0;
            let b2 = 1.0 - alpha * a;
            let a0 = 1.0 + alpha / a;
            let a1 = -2.0 * cos_w0;
            let a2 = 1.0 - alpha / a;
            (b0, b1, b2, a0, a1, a2)
        }
        FilterType::LowShelf => {
            let two_sqrt_a_alpha = 2.0 * a.sqrt() * alpha;
            let b0 = a * ((a + 1.0) - (a - 1.0) * cos_w0 + two_sqrt_a_alpha);
            let b1 = 2.0 * a * ((a - 1.0) - (a + 1.0) * cos_w0);
            let b2 = a * ((a + 1.0) - (a - 1.0) * cos_w0 - two_sqrt_a_alpha);
            let a0 = (a + 1.0) + (a - 1.0) * cos_w0 + two_sqrt_a_alpha;
            let a1 = -2.0 * ((a - 1.0) + (a + 1.0) * cos_w0);
            let a2 = (a + 1.0) + (a - 1.0) * cos_w0 - two_sqrt_a_alpha;
            (b0, b1, b2, a0, a1, a2)
        }
        FilterType::HighShelf => {
            let two_sqrt_a_alpha = 2.0 * a.sqrt() * alpha;
            let b0 = a * ((a + 1.0) + (a - 1.0) * cos_w0 + two_sqrt_a_alpha);
            let b1 = -2.0 * a * ((a - 1.0) + (a + 1.0) * cos_w0);
            let b2 = a * ((a + 1.0) + (a - 1.0) * cos_w0 - two_sqrt_a_alpha);
            let a0 = (a + 1.0) - (a - 1.0) * cos_w0 + two_sqrt_a_alpha;
            let a1 = 2.0 * ((a - 1.0) - (a + 1.0) * cos_w0);
            let a2 = (a + 1.0) - (a - 1.0) * cos_w0 - two_sqrt_a_alpha;
            (b0, b1, b2, a0, a1, a2)
        }
    }
}

pub fn band_response_db_into(band: &FilterBand, freqs: &[f64], fs: f64, out: &mut [f64]) {
    debug_assert_eq!(freqs.len(), out.len());
    if band.gain.abs() < 1e-6 {
        out.fill(0.0);
        return;
    }

    let (b0, b1, b2, a0, a1, a2) = biquad_coeffs(band, fs);
    let b0n = b0 / a0;
    let b1n = b1 / a0;
    let b2n = b2 / a0;
    let a1n = a1 / a0;
    let a2n = a2 / a0;

    let c_num0 = (b0n + b1n + b2n).powi(2);
    let c_num1 = 4.0 * (b0n * b1n + 4.0 * b0n * b2n + b1n * b2n);
    let c_num2 = 16.0 * b0n * b2n;

    let c_den0 = (1.0 + a1n + a2n).powi(2);
    let c_den1 = 4.0 * (a1n + 4.0 * a2n + a1n * a2n);
    let c_den2 = 16.0 * a2n;

    let inv_fs_pi = PI / fs;
    for (dst, &f) in out.iter_mut().zip(freqs.iter()) {
        let s = (inv_fs_pi * f).sin();
        let phi = s * s;
        let phi2 = phi * phi;
        let num = c_num0 - c_num1 * phi + c_num2 * phi2;
        let den = c_den0 - c_den1 * phi + c_den2 * phi2;
        let mag_sq = num.max(1e-18) / den.max(1e-18);
        *dst = 10.0 * mag_sq.log10();
    }
}

pub fn band_response_db(band: &FilterBand, freqs: &[f64], fs: f64) -> Vec<f64> {
    let mut out = vec![0.0; freqs.len()];
    band_response_db_into(band, freqs, fs, &mut out);
    out
}

pub fn combined_response_db(bands: &[FilterBand], freqs: &[f64], fs: f64) -> Vec<f64> {
    let mut total = vec![0.0; freqs.len()];
    let mut tmp = vec![0.0; freqs.len()];
    for b in bands {
        if b.gain.abs() < 1e-6 {
            continue;
        }
        band_response_db_into(b, freqs, fs, &mut tmp);
        for (t, &v) in total.iter_mut().zip(tmp.iter()) {
            *t += v;
        }
    }
    total
}

pub fn compute_safe_preamp_db(bands: &[FilterBand]) -> f64 {
    if bands.is_empty() {
        return 0.0;
    }
    let grid = geomspace(20.0, 20_000.0, 256);
    let resp = combined_response_db(bands, &grid, SAMPLE_RATE_HZ);
    let peak = resp.into_iter().fold(f64::NEG_INFINITY, f64::max);
    if peak > 0.0 {
        ((-peak - 0.2) * 100.0).round() / 100.0
    } else {
        0.0
    }
}

pub fn make_weights(freqs: &[f64], max_freq_fit: f64) -> Vec<f64> {
    let mut w = vec![1.0; freqs.len()];
    for (wi, &f) in w.iter_mut().zip(freqs.iter()) {
        if f < 28.0 {
            *wi = 0.65;
        }
    }
    if max_freq_fit < 19_000.0 {
        let taper_hi = max_freq_fit * 1.12;
        let denom = (taper_hi - max_freq_fit).max(1.0);
        for (wi, &f) in w.iter_mut().zip(freqs.iter()) {
            if f > taper_hi {
                *wi = 0.0;
            } else if f > max_freq_fit {
                let frac = (f - max_freq_fit) / denom;
                *wi = 0.5 * (1.0 + (PI * frac).cos()) * 0.25;
            }
        }
    }
    w
}

pub fn weighted_rmse(err: &[f64], weights: &[f64]) -> f64 {
    let mut num = 0.0;
    let mut den = 0.0;
    for (&e, &w) in err.iter().zip(weights.iter()) {
        num += w * e * e;
        den += w;
    }
    if den <= 1e-12 {
        0.0
    } else {
        (num / den).sqrt()
    }
}

fn weighted_sq_loss(res: &[f64], curve: &[f64], weights: &[f64]) -> f64 {
    let mut loss = 0.0;
    for ((&r, &c), &w) in res.iter().zip(curve.iter()).zip(weights.iter()) {
        let d = r - c;
        loss += w * d * d;
    }
    loss
}

fn estimate_initial_q(freqs: &[f64], residual: &[f64], peak_idx: usize) -> f64 {
    let peak_val = residual[peak_idx];
    if peak_val.abs() < 1e-3 {
        return 1.0;
    }
    let sign = if peak_val > 0.0 { 1.0 } else { -1.0 };
    let half_val = peak_val.abs() * 0.5;

    let mut left = peak_idx;
    while left > 0 && (residual[left] * sign) > half_val {
        left -= 1;
    }
    let mut right = peak_idx;
    while right + 1 < freqs.len() && (residual[right] * sign) > half_val {
        right += 1;
    }

    let f_lo = freqs[left];
    let f_hi = freqs[right];
    if f_hi <= f_lo {
        return 2.0;
    }
    let bw_oct = (f_hi / f_lo).log2();
    let q_est = 1.4 / bw_oct.max(0.14);
    q_est.clamp(0.35, 6.0)
}

/// Fit up to `max_bands` hardware-constrained biquad filters to `target_db`.
/// Returns `(bands, weighted_rmse_db)` sorted by ascending frequency.
pub fn fit_peq_bands(
    freqs: &[f64],
    target_db: &[f64],
    max_bands: usize,
    max_freq_fit: f64,
    max_gain_db: f64,
    supports_shelves: bool,
) -> (Vec<FilterBand>, f64) {
    let n_pts = freqs.len().min(target_db.len());
    let freqs = &freqs[..n_pts];
    let target_db = &target_db[..n_pts];

    let freq_cap = max_freq_fit.clamp(1000.0, MAX_FREQ_HZ);
    let weights = make_weights(freqs, freq_cap);

    let mut bands: Vec<FilterBand> = Vec::with_capacity(max_bands);
    let mut band_curves: Vec<Vec<f64>> = Vec::with_capacity(max_bands);
    let mut current = vec![0.0; n_pts];
    let mut residual = vec![0.0; n_pts];
    let mut cand_curve = vec![0.0; n_pts];

    // Stage 1: Greedy residual matching
    for _ in 0..max_bands {
        let mut best_idx = 0usize;
        let mut best_score = -1.0_f64;
        for i in 0..n_pts {
            residual[i] = target_db[i] - current[i];
            let sc = residual[i].abs() * weights[i].sqrt();
            if sc > best_score {
                best_score = sc;
                best_idx = i;
            }
        }
        if n_pts == 0 || weights[best_idx] <= 0.0 || residual[best_idx].abs() < 0.20 {
            break;
        }

        let f0 = freqs[best_idx].clamp(MIN_FREQ_HZ, freq_cap);
        let g0 = residual[best_idx].clamp(-max_gain_db, max_gain_db);
        let q_est = ((estimate_initial_q(freqs, &residual, best_idx) * 100.0).round()) / 100.0;

        let mut candidates = Vec::with_capacity(12);
        for q_try in [q_est, 0.707, 1.4, 2.8, 4.5] {
            if !candidates
                .iter()
                .any(|c: &FilterBand| (c.q - q_try).abs() < 1e-4)
            {
                candidates.push(FilterBand::new(FilterType::Peaking, f0, g0, q_try));
            }
        }
        if supports_shelves && f0 <= 250.0 {
            for f_ls in [60.0, 105.0, 160.0] {
                candidates.push(FilterBand::new(FilterType::LowShelf, f_ls, g0 * 0.85, 0.707));
            }
        }
        if supports_shelves && f0 >= 4500.0 {
            for f_hs in [6000.0, 8500.0, 10500.0] {
                if f_hs <= freq_cap {
                    candidates.push(FilterBand::new(
                        FilterType::HighShelf,
                        f_hs,
                        g0 * 0.85,
                        0.707,
                    ));
                }
            }
        }

        let mut best_band = candidates[0].clone();
        let mut best_curve = band_response_db(&best_band, freqs, SAMPLE_RATE_HZ);
        let mut best_loss = weighted_sq_loss(&residual, &best_curve, &weights);

        for cand in candidates.into_iter().skip(1) {
            band_response_db_into(&cand, freqs, SAMPLE_RATE_HZ, &mut cand_curve);
            let loss = weighted_sq_loss(&residual, &cand_curve, &weights);
            if loss < best_loss {
                best_loss = loss;
                best_band = cand;
                best_curve.copy_from_slice(&cand_curve);
            }
        }

        for (c, &bc) in current.iter_mut().zip(best_curve.iter()) {
            *c += bc;
        }
        bands.push(best_band);
        band_curves.push(best_curve);
    }

    // Stage 2: Joint coordinate-descent refinement (log-freq, gain, log-Q)
    if !bands.is_empty() {
        let n_b = bands.len();
        let step_schedules = [
            (0.18, 2.0, 0.22),
            (0.09, 1.0, 0.12),
            (0.04, 0.4, 0.06),
            (0.015, 0.15, 0.025),
            (0.006, 0.1, 0.01),
        ];
        let mut others = vec![0.0; n_pts];
        let mut res_i = vec![0.0; n_pts];

        for (df_oct, dg_db, dq_oct) in step_schedules {
            for _pass in 0..2 {
                for i in 0..n_b {
                    for k in 0..n_pts {
                        others[k] = current[k] - band_curves[i][k];
                        res_i[k] = target_db[k] - others[k];
                    }
                    let mut b = bands[i].clone();
                    let mut best_loss = weighted_sq_loss(&res_i, &band_curves[i], &weights);

                    for delta in [-df_oct, df_oct] {
                        let nf = b.frequency * 2.0_f64.powf(delta);
                        if (MIN_FREQ_HZ..=freq_cap).contains(&nf) {
                            let nb = FilterBand::new(b.filter_type, nf, b.gain, b.q);
                            band_response_db_into(&nb, freqs, SAMPLE_RATE_HZ, &mut cand_curve);
                            let loss = weighted_sq_loss(&res_i, &cand_curve, &weights);
                            if loss < best_loss {
                                best_loss = loss;
                                b = nb;
                                band_curves[i].copy_from_slice(&cand_curve);
                            }
                        }
                    }

                    for delta in [-dg_db, dg_db] {
                        let ng = (b.gain + delta).clamp(-max_gain_db, max_gain_db);
                        if (ng - b.gain).abs() > 1e-9 {
                            let nb = FilterBand::new(b.filter_type, b.frequency, ng, b.q);
                            band_response_db_into(&nb, freqs, SAMPLE_RATE_HZ, &mut cand_curve);
                            let loss = weighted_sq_loss(&res_i, &cand_curve, &weights);
                            if loss < best_loss {
                                best_loss = loss;
                                b = nb;
                                band_curves[i].copy_from_slice(&cand_curve);
                            }
                        }
                    }

                    for delta in [-dq_oct, dq_oct] {
                        let nq = (b.q * 2.0_f64.powf(delta)).clamp(MIN_Q, MAX_Q);
                        if (nq - b.q).abs() > 1e-9 {
                            let nb = FilterBand::new(b.filter_type, b.frequency, b.gain, nq);
                            band_response_db_into(&nb, freqs, SAMPLE_RATE_HZ, &mut cand_curve);
                            let loss = weighted_sq_loss(&res_i, &cand_curve, &weights);
                            if loss < best_loss {
                                best_loss = loss;
                                b = nb;
                                band_curves[i].copy_from_slice(&cand_curve);
                            }
                        }
                    }

                    bands[i] = b;
                    for k in 0..n_pts {
                        current[k] = others[k] + band_curves[i][k];
                    }
                }
            }
        }
    }

    // Clamp & quantize to hardware resolution, sort by ascending frequency, pad to max_bands
    let mut quantized: Vec<FilterBand> = bands
        .into_iter()
        .map(|b| b.clamped_with_max_gain(max_gain_db))
        .filter(|b| b.gain.abs() >= 0.1)
        .collect();
    quantized.sort_by(|a, b| a.frequency.partial_cmp(&b.frequency).unwrap());
    while quantized.len() < max_bands {
        quantized.push(FilterBand::new(FilterType::Peaking, 1000.0, 0.0, 1.0));
    }
    quantized.truncate(max_bands);

    let final_resp = combined_response_db(&quantized, freqs, SAMPLE_RATE_HZ);
    let err: Vec<f64> = target_db
        .iter()
        .zip(final_resp.iter())
        .map(|(&t, &r)| t - r)
        .collect();
    let rmse = weighted_rmse(&err, &weights);
    (quantized, rmse)
}

pub fn flat_bands(n: usize) -> Vec<FilterBand> {
    let n = n.max(1);
    let freqs: &[f64] = if n <= 5 {
        &[60.0, 250.0, 1000.0, 3500.0, 8000.0]
    } else {
        &[31.0, 63.0, 125.0, 250.0, 500.0, 1000.0, 2000.0, 4000.0, 8000.0, 16000.0]
    };
    (0..n)
        .map(|i| FilterBand::new(FilterType::Peaking, freqs[i % freqs.len()], 0.0, 1.0))
        .collect()
}

pub fn fit_band_count(bands: &[FilterBand], n: usize) -> Vec<FilterBand> {
    let n = n.max(1);
    let mut out: Vec<FilterBand> = bands.iter().take(n).map(FilterBand::clamped).collect();
    let pad = flat_bands(n);
    while out.len() < n {
        out.push(pad[out.len()].clone());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flat_filter_has_zero_db_response() {
        let grid = standard_grid();
        let b = FilterBand::new(FilterType::Peaking, 1000.0, 0.0, 1.0);
        let resp = band_response_db(&b, &grid, SAMPLE_RATE_HZ);
        assert!(resp.iter().all(|&v| v.abs() < 1e-9));
    }

    #[test]
    fn peaking_filter_hits_target_gain_at_center_freq() {
        let b = FilterBand::new(FilterType::Peaking, 1000.0, 6.0, 1.414);
        let resp = band_response_db(&b, &[1000.0], SAMPLE_RATE_HZ);
        assert!((resp[0] - 6.0).abs() < 0.05, "got {}", resp[0]);
    }

    #[test]
    fn optimizer_reconstructs_synthetic_curve_accurately() {
        let grid = standard_grid();
        let target_bands = vec![
            FilterBand::new(FilterType::LowShelf, 105.0, 4.5, 0.707),
            FilterBand::new(FilterType::Peaking, 1200.0, -3.2, 1.8),
            FilterBand::new(FilterType::HighShelf, 8500.0, 2.5, 0.707),
        ];
        let target_curve = combined_response_db(&target_bands, &grid, SAMPLE_RATE_HZ);
        let (fitted, rmse) = fit_peq_bands(&grid, &target_curve, 5, 12_000.0, 12.0, true);
        assert_eq!(fitted.len(), 5);
        assert!(rmse < 0.35, "expected low RMSE, got {rmse}");
    }
}
