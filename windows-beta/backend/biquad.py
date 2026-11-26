"""RBJ biquad EQ math and 5-band PEQ optimizer for KTMicro (Moondrop Chu 2 DSP)."""

from __future__ import annotations

import dataclasses
import math
from typing import Iterable, List, Literal, Sequence, Tuple

import numpy as np

FilterType = Literal["peaking", "low_shelf", "high_shelf"]

SAMPLE_RATE_HZ = 48000.0
MIN_FREQ_HZ = 20.0
MAX_FREQ_HZ = 20000.0
MAX_GAIN_DB = 12.0
MIN_Q = 0.1
MAX_Q = 10.0
MAX_BANDS = 5


@dataclasses.dataclass
class FilterBand:
    """Single hardware PEQ filter band."""

    type: FilterType = "peaking"
    frequency: float = 1000.0
    gain: float = 0.0
    q: float = 1.0

    def clamped(self) -> "FilterBand":
        ftype: FilterType = self.type if self.type in ("peaking", "low_shelf", "high_shelf") else "peaking"
        freq = float(round(min(MAX_FREQ_HZ, max(MIN_FREQ_HZ, self.frequency))))
        gain = float(round(min(MAX_GAIN_DB, max(-MAX_GAIN_DB, self.gain)), 1))
        q = float(round(min(MAX_Q, max(MIN_Q, self.q)), 3))
        return FilterBand(type=ftype, frequency=freq, gain=gain, q=q)


def biquad_coeffs(band: FilterBand, fs: float = SAMPLE_RATE_HZ) -> Tuple[float, float, float, float, float, float]:
    """Return normalized RBJ Audio EQ Cookbook coefficients (b0, b1, b2, a0, a1, a2)."""
    f0 = min(fs * 0.495, max(1.0, float(band.frequency)))
    q = min(MAX_Q, max(MIN_Q, float(band.q)))
    gain_db = min(MAX_GAIN_DB, max(-MAX_GAIN_DB, float(band.gain)))

    if abs(gain_db) < 1e-6:
        return (1.0, 0.0, 0.0, 1.0, 0.0, 0.0)

    A = 10.0 ** (gain_db / 40.0)
    w0 = 2.0 * math.pi * f0 / fs
    cos_w0 = math.cos(w0)
    sin_w0 = math.sin(w0)
    alpha = sin_w0 / (2.0 * q)

    if band.type == "peaking":
        b0 = 1.0 + alpha * A
        b1 = -2.0 * cos_w0
        b2 = 1.0 - alpha * A
        a0 = 1.0 + alpha / A
        a1 = -2.0 * cos_w0
        a2 = 1.0 - alpha / A
    elif band.type == "low_shelf":
        # RBJ shelf uses S or Q; for Q compatibility sqrt((A + 1/A)*(1/Q - 1) + 2) or standard alpha*2*sqrt(A)
        two_sqrt_a_alpha = 2.0 * math.sqrt(A) * alpha
        b0 = A * ((A + 1.0) - (A - 1.0) * cos_w0 + two_sqrt_a_alpha)
        b1 = 2.0 * A * ((A - 1.0) - (A + 1.0) * cos_w0)
        b2 = A * ((A + 1.0) - (A - 1.0) * cos_w0 - two_sqrt_a_alpha)
        a0 = (A + 1.0) + (A - 1.0) * cos_w0 + two_sqrt_a_alpha
        a1 = -2.0 * ((A - 1.0) + (A + 1.0) * cos_w0)
        a2 = (A + 1.0) + (A - 1.0) * cos_w0 - two_sqrt_a_alpha
    elif band.type == "high_shelf":
        two_sqrt_a_alpha = 2.0 * math.sqrt(A) * alpha
        b0 = A * ((A + 1.0) + (A - 1.0) * cos_w0 + two_sqrt_a_alpha)
        b1 = -2.0 * A * ((A - 1.0) + (A + 1.0) * cos_w0)
        b2 = A * ((A + 1.0) + (A - 1.0) * cos_w0 - two_sqrt_a_alpha)
        a0 = (A + 1.0) - (A - 1.0) * cos_w0 + two_sqrt_a_alpha
        a1 = 2.0 * ((A - 1.0) - (A + 1.0) * cos_w0)
        a2 = (A + 1.0) - (A - 1.0) * cos_w0 - two_sqrt_a_alpha
    else:
        return (1.0, 0.0, 0.0, 1.0, 0.0, 0.0)

    return (b0, b1, b2, a0, a1, a2)


def band_response_db(band: FilterBand, freqs: np.ndarray, fs: float = SAMPLE_RATE_HZ) -> np.ndarray:
    """Evaluate magnitude response in dB at ``freqs`` Hz for a single biquad."""
    if abs(band.gain) < 1e-6:
        return np.zeros_like(freqs, dtype=np.float64)

    b0, b1, b2, a0, a1, a2 = biquad_coeffs(band, fs=fs)
    w = 2.0 * math.pi * freqs / fs
    phi = np.sin(w / 2.0) ** 2

    # Numerically stable |H(e^jw)|^2 identity for real biquads
    b0n, b1n, b2n = b0 / a0, b1 / a0, b2 / a0
    a1n, a2n = a1 / a0, a2 / a0

    num = (b0n + b1n + b2n) ** 2 - 4.0 * (b0n * b1n + 4.0 * b0n * b2n + b1n * b2n) * phi + 16.0 * b0n * b2n * (phi ** 2)
    den = (1.0 + a1n + a2n) ** 2 - 4.0 * (a1n + 4.0 * a2n + a1n * a2n) * phi + 16.0 * a2n * (phi ** 2)

    mag_sq = np.maximum(num, 1e-18) / np.maximum(den, 1e-18)
    return 10.0 * np.log10(mag_sq)


def combined_response_db(bands: Iterable[FilterBand], freqs: np.ndarray, fs: float = SAMPLE_RATE_HZ) -> np.ndarray:
    """Total cascaded magnitude response in dB across all bands."""
    total = np.zeros_like(freqs, dtype=np.float64)
    for b in bands:
        total += band_response_db(b, freqs, fs=fs)
    return total


def _make_weights(freqs: np.ndarray, max_freq_fit: float) -> np.ndarray:
    """Log-frequency perceptual weights, strictly zeroing above ``max_freq_fit``."""
    w = np.ones_like(freqs, dtype=np.float64)
    w[freqs < 28.0] = 0.65
    if max_freq_fit < 19000.0:
        taper_hi = max_freq_fit * 1.12
        mask_taper = (freqs > max_freq_fit) & (freqs <= taper_hi)
        if np.any(mask_taper):
            frac = (freqs[mask_taper] - max_freq_fit) / max(1.0, taper_hi - max_freq_fit)
            w[mask_taper] = 0.5 * (1.0 + np.cos(np.pi * frac)) * 0.25
        w[freqs > taper_hi] = 0.0
    return w


def _weighted_rmse(err: np.ndarray, weights: np.ndarray) -> float:
    return float(np.sqrt(np.sum(weights * (err ** 2)) / np.sum(weights)))


def _estimate_initial_q(freqs: np.ndarray, residual: np.ndarray, peak_idx: int) -> float:
    """Estimate Q from half-height bandwidth around ``peak_idx``."""
    peak_val = residual[peak_idx]
    if abs(peak_val) < 1e-3:
        return 1.0
    sign = 1.0 if peak_val > 0 else -1.0
    half_val = abs(peak_val) * 0.5

    left = peak_idx
    while left > 0 and (residual[left] * sign) > half_val:
        left -= 1
    right = peak_idx
    while right < len(freqs) - 1 and (residual[right] * sign) > half_val:
        right += 1

    f_lo = freqs[left]
    f_hi = freqs[right]
    if f_hi <= f_lo:
        return 2.0
    bw_oct = math.log2(f_hi / f_lo)
    q_est = 1.4 / max(bw_oct, 0.14)
    return float(min(6.0, max(0.35, q_est)))


def fit_peq_bands(
    freqs: np.ndarray,
    target_db: np.ndarray,
    max_bands: int = MAX_BANDS,
    max_freq_fit: float = 10000.0,
    max_gain_db: float = MAX_GAIN_DB,
    supports_shelves: bool = True,
) -> Tuple[List[FilterBand], float]:
    """Fit up to ``max_bands`` hardware-constrained biquad filters to ``target_db``.

    Returns ``(bands, weighted_rmse_db)`` sorted by ascending frequency.
    """
    freqs = np.asarray(freqs, dtype=np.float64)
    target_db = np.asarray(target_db, dtype=np.float64)
    freq_cap = float(min(MAX_FREQ_HZ, max(1000.0, max_freq_fit)))
    weights = _make_weights(freqs, freq_cap)

    bands: List[FilterBand] = []
    band_curves: List[np.ndarray] = []
    current = np.zeros_like(freqs, dtype=np.float64)

    # Stage 1: Greedy residual matching
    for _ in range(max_bands):
        residual = target_db - current
        score = np.abs(residual) * np.sqrt(weights)
        idx = int(np.argmax(score))
        if weights[idx] <= 0.0 or abs(residual[idx]) < 0.20:
            break

        f0 = float(min(freq_cap, max(MIN_FREQ_HZ, freqs[idx])))
        g0 = float(min(max_gain_db, max(-max_gain_db, residual[idx])))
        q_est = _estimate_initial_q(freqs, residual, idx)

        candidates: List[FilterBand] = []
        for q_try in {round(q_est, 2), 0.707, 1.4, 2.8, 4.5}:
            candidates.append(FilterBand("peaking", f0, g0, q_try))
        if supports_shelves and f0 <= 250.0:
            for f_ls in (60.0, 105.0, 160.0):
                candidates.append(FilterBand("low_shelf", f_ls, g0 * 0.85, 0.707))
        if supports_shelves and f0 >= 4500.0:
            for f_hs in (6000.0, 8500.0, 10500.0):
                if f_hs <= freq_cap:
                    candidates.append(FilterBand("high_shelf", f_hs, g0 * 0.85, 0.707))

        best_band = candidates[0]
        best_curve = band_response_db(best_band, freqs)
        best_loss = float(np.sum(weights * ((residual - best_curve) ** 2)))

        for cand in candidates[1:]:
            curve = band_response_db(cand, freqs)
            loss = float(np.sum(weights * ((residual - curve) ** 2)))
            if loss < best_loss:
                best_loss = loss
                best_band = cand
                best_curve = curve

        bands.append(best_band)
        band_curves.append(best_curve)
        current = current + best_curve

    # Stage 2: Joint coordinate-descent refinement (log-freq, gain, log-Q)
    if bands:
        n_b = len(bands)
        step_schedules = [
            (0.18, 2.0, 0.22),
            (0.09, 1.0, 0.12),
            (0.04, 0.4, 0.06),
            (0.015, 0.15, 0.025),
            (0.006, 0.1, 0.01),
        ]
        for df_oct, dg_db, dq_oct in step_schedules:
            for _pass in range(2):
                for i in range(n_b):
                    others = current - band_curves[i]
                    res_i = target_db - others
                    b = bands[i]
                    best_loss = float(np.sum(weights * ((res_i - band_curves[i]) ** 2)))

                    for delta in (-df_oct, df_oct):
                        nf = b.frequency * (2.0 ** delta)
                        if MIN_FREQ_HZ <= nf <= freq_cap:
                            nb = FilterBand(b.type, nf, b.gain, b.q)
                            nc = band_response_db(nb, freqs)
                            loss = float(np.sum(weights * ((res_i - nc) ** 2)))
                            if loss < best_loss:
                                best_loss, b, band_curves[i] = loss, nb, nc

                    for delta in (-dg_db, dg_db):
                        ng = min(max_gain_db, max(-max_gain_db, b.gain + delta))
                        if ng != b.gain:
                            nb = FilterBand(b.type, b.frequency, ng, b.q)
                            nc = band_response_db(nb, freqs)
                            loss = float(np.sum(weights * ((res_i - nc) ** 2)))
                            if loss < best_loss:
                                best_loss, b, band_curves[i] = loss, nb, nc

                    for delta in (-dq_oct, dq_oct):
                        nq = min(MAX_Q, max(MIN_Q, b.q * (2.0 ** delta)))
                        if nq != b.q:
                            nb = FilterBand(b.type, b.frequency, b.gain, nq)
                            nc = band_response_db(nb, freqs)
                            loss = float(np.sum(weights * ((res_i - nc) ** 2)))
                            if loss < best_loss:
                                best_loss, b, band_curves[i] = loss, nb, nc

                    bands[i] = b
                    current = others + band_curves[i]

    # Clamp and quantize to hardware resolution, pad to max_bands
    quantized = []
    for b in bands:
        bc = b.clamped()
        bc.gain = float(round(min(max_gain_db, max(-max_gain_db, bc.gain)), 1))
        if abs(bc.gain) >= 0.1:
            quantized.append(bc)
    quantized.sort(key=lambda b: b.frequency)
    while len(quantized) < max_bands:
        quantized.append(FilterBand("peaking", 1000.0, 0.0, 1.0))

    final_resp = combined_response_db(quantized, freqs)
    rmse = _weighted_rmse(target_db - final_resp, weights)
    return quantized[:max_bands], rmse
