//! Axis ticks and their labels: exact ports of `niceTicks`, `tickLabel` and
//! the half-step grid of `drawGrid` (`app/src/canvas/viewport.ts`,
//! `draw.ts`). `tests/vectors/ticks.json` holds cases computed by the
//! TypeScript, which both test suites check.

use crate::num::{to_exponential, to_fixed};

/// Tick values and the step between them.
#[derive(Debug, Clone, PartialEq)]
pub struct Ticks {
    /// Tick values, increasing.
    pub ticks: Vec<f64>,
    /// Distance between neighbouring ticks.
    pub step: f64,
}

/// `niceTicks(lo, hi, target)`: about `target` ticks on a 1, 2, 5 × 10^k
/// step, counted from `ceil(lo / step)` (never accumulated).
pub fn nice(lo: f64, hi: f64, target: f64) -> Ticks {
    let span = hi - lo;
    // `!(span > 0)` in the TypeScript: NaN spans have no ticks either.
    if span.is_nan() || span <= 0.0 || !span.is_finite() {
        return Ticks {
            ticks: Vec::new(),
            step: 1.0,
        };
    }
    let raw = span / target.max(1.0);
    let p = 10f64.powf(raw.log10().floor());
    let m = raw / p;
    let step = if m < 1.5 {
        1.0
    } else if m < 3.5 {
        2.0
    } else if m < 7.5 {
        5.0
    } else {
        10.0
    } * p;
    let first = (lo / step).ceil();
    let count = (hi / step + 1e-9).floor() - first;
    let mut ticks = Vec::new();
    let mut k = 0.0;
    while k <= count {
        let v = (first + k) * step;
        ticks.push(if v.abs() < step * 1e-9 { 0.0 } else { v });
        k += 1.0;
    }
    Ticks { ticks, step }
}

/// The screen's tick counts: one tick per 140 px across, one per 120 px up.
pub fn for_viewport(
    width: f64,
    height: f64,
    x_range: (f64, f64),
    y_range: (f64, f64),
) -> (Ticks, Ticks) {
    (
        nice(x_range.0, x_range.1, (width / 140.0).round().max(2.0)),
        nice(y_range.0, y_range.1, (height / 120.0).round().max(2.0)),
    )
}

/// Most digits a label shows after the point.
const MAX_LABEL_DECIMALS: i32 = 20;

/// `tickLabel(v, step)`: down to the step's digit, with the real minus sign
/// (U+2212) in plain notation and exponent notation (`1.5e-7`, ASCII `-`)
/// from 1e5 and below 1e-4.
pub fn label(v: f64, step: f64) -> String {
    let step_exp = (step.log10() + 1e-9).floor() as i32;
    let abs = v.abs();
    if abs >= 1e5 || (abs > 0.0 && abs < 1e-4) {
        let digits = ((abs.log10() + 1e-9).floor() as i32 - step_exp).clamp(1, 15);
        return to_exponential(v, digits as usize).replacen('+', "", 1);
    }
    let decimals = (-step_exp).clamp(0, MAX_LABEL_DECIMALS);
    let text = to_fixed(v, decimals as usize);
    let text = if text.contains('.') {
        // `/\.?0+$/`
        let t = text.trim_end_matches('0');
        if t.len() < text.len() {
            t.strip_suffix('.').unwrap_or(t).to_string()
        } else {
            text
        }
    } else {
        text
    };
    text.replacen('-', "\u{2212}", 1)
}

/// The grid lines `drawGrid` draws: every half step across `[lo, hi]`,
/// including those beyond the first and last tick.
pub fn half_steps(lo: f64, hi: f64, step: f64) -> Vec<f64> {
    let s = step / 2.0;
    let a = lo.min(hi);
    let b = lo.max(hi);
    let first = (a / s).ceil();
    let count = (b / s + 1e-9).floor() - first;
    let mut out = Vec::new();
    if !count.is_finite() {
        return out;
    }
    let mut k = 0.0;
    while k <= count {
        out.push((first + k) * s);
        k += 1.0;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nice_ticks_and_labels() {
        let t = nice(-0.25, 3.5, 4.0);
        assert_eq!(t.step, 1.0);
        assert_eq!(t.ticks, [0.0, 1.0, 2.0, 3.0]);
        let t = nice(-25.0, 25.0, 4.0);
        assert_eq!(t.step, 10.0);
        assert_eq!(label(-20.0, 10.0), "\u{2212}20");
        assert_eq!(label(0.5, 0.5), "0.5");
        assert_eq!(label(1.5e-7, 1e-7), "1.5e-7");
        assert_eq!(label(-1.5e-7, 1e-7), "-1.5e-7");
        assert_eq!(label(250000.0, 50000.0), "2.5e5");
        assert!(nice(1.0, 1.0, 3.0).ticks.is_empty());
        assert!(nice(0.0, f64::INFINITY, 3.0).ticks.is_empty());
        assert_eq!(
            half_steps(-0.25, 3.5, 1.0),
            [0.0, 0.5, 1.0, 1.5, 2.0, 2.5, 3.0, 3.5]
        );
        assert!(half_steps(0.0, f64::INFINITY, 1.0).is_empty());
    }
}
