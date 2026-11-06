//! Numbers as text, exactly as the UI writes them.
//!
//! - [`shortest`] is JavaScript's `String(v)`: the shortest decimal that
//!   reads back as the same double. CSV files and pgfplots tables use it.
//! - [`to_fixed`] and [`to_exponential`] are `Number.prototype.toFixed` and
//!   `toExponential`, which round the exact decimal value of the double half
//!   away from zero. Rust's `{:.N}` rounds ties to even, so tick labels
//!   built on it would differ from the screen's (`0.125` to two decimals is
//!   `0.13` in JavaScript, `0.12` in Rust).
//! - [`readable`], [`tex`] and [`si`] are the app's `fmt` (a few significant
//!   digits, trailing zeros dropped) for LaTeX: ASCII `-`, and `\num{…}` or
//!   `1.5\times 10^{-7}` for exponents. TeX never sees U+2212.

/// The exact decimal expansion of a finite, non-zero `|v|`: its significant
/// digits (no leading or trailing zeros) and the power of ten of the first.
fn exact_digits(v: f64) -> (Vec<u8>, i32) {
    // A double is m·2^e2 with an odd m, so its decimal expansion ends at the
    // 10^e2 place for e2 < 0 (at most 1074 places); asking for that many
    // digits (plus slack for the log) gives the exact value, not a rounding.
    let bits = v.abs().to_bits();
    let biased = ((bits >> 52) & 0x7ff) as i64;
    let frac = bits & ((1u64 << 52) - 1);
    let (m, e2) = if biased == 0 {
        (frac, -1074)
    } else {
        (frac | (1u64 << 52), biased - 1075)
    };
    let e2 = e2 + i64::from(m.trailing_zeros());
    let places = if e2 < 0 { -e2 } else { 0 };
    let lead = v.abs().log10().floor() as i64;
    let precision = (lead + places + 2).clamp(1, 1100) as usize;
    let s = format!("{:.*e}", precision, v.abs());
    let (mant, exp) = s.split_once('e').unwrap_or((&s, "0"));
    let mut digits: Vec<u8> = mant
        .bytes()
        .filter(u8::is_ascii_digit)
        .map(|b| b - b'0')
        .collect();
    while digits.len() > 1 && digits.last() == Some(&0) {
        digits.pop();
    }
    (digits, exp.parse().unwrap_or(0))
}

/// The first `k` digits of `digits` rounded half up (on the exact value),
/// and whether rounding carried into a new leading digit (`999` to `1000`).
fn round_half_up(digits: &[u8], k: usize) -> (Vec<u8>, bool) {
    let mut out: Vec<u8> = (0..k)
        .map(|i| digits.get(i).copied().unwrap_or(0))
        .collect();
    if digits.get(k).is_some_and(|&d| d >= 5) {
        let mut i = k;
        loop {
            if i == 0 {
                out.insert(0, 1);
                return (out, true);
            }
            i -= 1;
            if out[i] == 9 {
                out[i] = 0;
            } else {
                out[i] += 1;
                break;
            }
        }
    }
    (out, false)
}

fn digits_str(d: &[u8]) -> String {
    d.iter().map(|&b| char::from(b'0' + b)).collect()
}

/// JavaScript's `String(v)`: the shortest round-trip decimal, in plain
/// notation for `1e-6 <= |v| < 1e21` and exponent notation (`1e-7`,
/// `1.5e+21`) otherwise. `-0` is `"0"`.
pub fn shortest(v: f64) -> String {
    if v.is_nan() {
        return "NaN".into();
    }
    if v.is_infinite() {
        return if v > 0.0 { "Infinity" } else { "-Infinity" }.into();
    }
    if v == 0.0 {
        return "0".into();
    }
    let a = v.abs();
    if (1e-6..1e21).contains(&a) {
        format!("{v}")
    } else {
        let s = format!("{v:e}");
        match s.split_once('e') {
            Some((m, e)) if !e.starts_with('-') => format!("{m}e+{e}"),
            _ => s,
        }
    }
}

/// JavaScript's `v.toFixed(decimals)` (`decimals <= 100`).
pub fn to_fixed(v: f64, decimals: usize) -> String {
    if !v.is_finite() {
        return shortest(v);
    }
    if v.abs() >= 1e21 {
        return shortest(v);
    }
    let neg = v < 0.0;
    let body = if v == 0.0 {
        digits_str(&vec![0; decimals + 1])
    } else {
        let (digits, e) = exact_digits(v);
        // Digits at places 10^e … 10^-decimals.
        let keep = e + 1 + decimals as i32;
        if keep < 0 {
            "0".repeat(decimals + 1)
        } else {
            let (mut r, _) = round_half_up(&digits, keep as usize);
            if r.is_empty() {
                r.push(0);
            }
            let mut s = digits_str(&r);
            while s.len() < decimals + 1 {
                s.insert(0, '0');
            }
            s
        }
    };
    let (int, frac) = body.split_at(body.len() - decimals);
    let int = int.trim_start_matches('0');
    let int = if int.is_empty() { "0" } else { int };
    let mut out = String::new();
    if neg {
        out.push('-');
    }
    out.push_str(int);
    if decimals > 0 {
        out.push('.');
        out.push_str(frac);
    }
    out
}

/// JavaScript's `v.toExponential(fraction_digits)`: `"1.50e-7"`,
/// `"1.5e+21"`.
pub fn to_exponential(v: f64, fraction_digits: usize) -> String {
    if !v.is_finite() {
        return shortest(v);
    }
    let neg = v < 0.0;
    let (digits, exp) = if v == 0.0 {
        (vec![0; fraction_digits + 1], 0)
    } else {
        let (d, e) = exact_digits(v);
        let (r, carried) = round_half_up(&d, fraction_digits + 1);
        if carried {
            (r[..fraction_digits + 1].to_vec(), e + 1)
        } else {
            (r, e)
        }
    };
    let s = digits_str(&digits);
    let mut out = String::new();
    if neg {
        out.push('-');
    }
    out.push_str(&s[..1]);
    if fraction_digits > 0 {
        out.push('.');
        out.push_str(&s[1..]);
    }
    out.push('e');
    out.push(if exp < 0 { '-' } else { '+' });
    out.push_str(&exp.unsigned_abs().to_string());
    out
}

/// `v` rounded half away from zero at the `10^place` digit, from its exact
/// decimal value, in plain notation (`fixed_at(1234.5, 1)` is `"1230"`,
/// `fixed_at(0.90561, -3)` is `"0.906"`). A value that rounds to zero has
/// no sign.
fn fixed_at(v: f64, place: i32) -> String {
    let s = if place <= 0 {
        to_fixed(v, place.unsigned_abs() as usize)
    } else if v == 0.0 {
        "0".to_string()
    } else {
        let (digits, e) = exact_digits(v);
        let keep = e - place + 1;
        let r = if keep < 0 {
            Vec::new()
        } else {
            round_half_up(&digits, keep as usize).0
        };
        if r.iter().all(|&d| d == 0) {
            "0".to_string()
        } else {
            let sign = if v < 0.0 { "-" } else { "" };
            format!("{sign}{}{}", digits_str(&r), "0".repeat(place as usize))
        }
    };
    if s.starts_with('-') && s.bytes().all(|b| matches!(b, b'-' | b'0' | b'.')) {
        s[1..].to_string()
    } else {
        s
    }
}

/// A measured value and its standard error as siunitx writes them in
/// `\num{0.906 +- 0.004}`: the error rounded half up to one significant
/// digit (two when that digit is 1), and the value rounded at the same
/// decimal place. Both are rounded from their exact decimal expansions,
/// never through `log10`, so the text is the same on every platform. None
/// when either is not finite or the error is not positive.
pub fn with_uncertainty(v: f64, u: f64) -> Option<(String, String)> {
    if !v.is_finite() || !u.is_finite() || u <= 0.0 {
        return None;
    }
    let (digits, e) = exact_digits(u);
    let sig = if digits[0] == 1 { 2 } else { 1 };
    let (_, carried) = round_half_up(&digits, sig);
    // 0.0096 rounds to 0.01: one digit, one place further up.
    let place = e - (sig as i32 - 1) + i32::from(carried);
    Some((fixed_at(v, place), fixed_at(u, place)))
}

fn trim_zeros(s: &str) -> String {
    if s.contains('.') {
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    } else {
        s.to_string()
    }
}

/// The app's `fmt(v, sig)`: `sig` significant digits with trailing zeros
/// dropped (`3`, `-0.5`, `2.667`), exponent notation below 1e-4 and from
/// 1e5 (`1.23e-5`), always with an ASCII `-`. Non-finite values are `NaN`,
/// `inf` and `-inf`.
pub fn readable(v: f64, sig: usize) -> String {
    let sig = sig.clamp(1, 17);
    if v.is_nan() {
        return "NaN".into();
    }
    if v.is_infinite() {
        return if v > 0.0 { "inf" } else { "-inf" }.into();
    }
    if v == 0.0 {
        return "0".into();
    }
    let a = v.abs();
    if !(1e-4..1e5).contains(&a) {
        let s = to_exponential(v, sig - 1);
        let (m, e) = s.split_once('e').unwrap_or((&s, "0"));
        let m = trim_zeros(m);
        if m == "10" {
            // Rounding never carries past the mantissa's first digit, but keep
            // the text well formed if it ever did.
            return format!("1e{}", e.replace('+', ""));
        }
        return format!("{m}e{}", e.replace('+', ""));
    }
    let decimals = (sig as i32 - 1 - a.log10().floor() as i32).clamp(0, 10) as usize;
    let s = trim_zeros(&to_fixed(v, decimals));
    if s == "-0" {
        "0".into()
    } else {
        s
    }
}

/// [`readable`] for TeX maths: `1.23\times 10^{-5}` for exponents,
/// `\infty` for infinities.
pub fn tex(v: f64, sig: usize) -> String {
    let s = readable(v, sig);
    match s.as_str() {
        "inf" => return "\\infty".into(),
        "-inf" => return "-\\infty".into(),
        "NaN" => return "\\mathrm{NaN}".into(),
        _ => {}
    }
    match s.split_once('e') {
        Some(("1", e)) => format!("10^{{{e}}}"),
        Some(("-1", e)) => format!("-10^{{{e}}}"),
        Some((m, e)) => format!("{m}\\times 10^{{{e}}}"),
        None => s,
    }
}

/// [`readable`] as siunitx `\num{…}` (`\num{-0.5}`, `\num{1.23e-5}`).
pub fn si(v: f64, sig: usize) -> String {
    match readable(v, sig).as_str() {
        "inf" => "$\\infty$".into(),
        "-inf" => "$-\\infty$".into(),
        "NaN" => "NaN".into(),
        s => format!("\\num{{{s}}}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shortest_matches_javascript_string() {
        let cases: &[(f64, &str)] = &[
            (0.0, "0"),
            (-0.0, "0"),
            (1.0, "1"),
            (-1.5, "-1.5"),
            (0.1, "0.1"),
            (1e-6, "0.000001"),
            (1.5e-6, "0.0000015"),
            (1e-7, "1e-7"),
            (-1.5e-7, "-1.5e-7"),
            (1e20, "100000000000000000000"),
            (1e21, "1e+21"),
            (1.5e300, "1.5e+300"),
            (5e-324, "5e-324"),
            (f64::MAX, "1.7976931348623157e+308"),
            (2.6666666666666665, "2.6666666666666665"),
            (0.1 + 0.2, "0.30000000000000004"),
            (123456789012345680000.0, "123456789012345680000"),
        ];
        for &(v, want) in cases {
            assert_eq!(shortest(v), want, "{v:e}");
            if v.is_finite() {
                assert_eq!(shortest(v).parse::<f64>().unwrap(), v);
            }
        }
    }

    #[test]
    fn exact_digits_are_the_whole_expansion() {
        let full = |v: f64| {
            let s = format!("{:.1100e}", v.abs());
            let (m, e) = s.split_once('e').unwrap();
            let mut d: Vec<u8> = m
                .bytes()
                .filter(u8::is_ascii_digit)
                .map(|b| b - b'0')
                .collect();
            while d.len() > 1 && d.last() == Some(&0) {
                d.pop();
            }
            (d, e.parse::<i32>().unwrap())
        };
        let mut v = 0.123_456_789_f64;
        for _ in 0..2000 {
            v = (v * 7.919 + 0.31).fract() * 10f64.powi(((v * 1e6) as i32 % 40) - 20);
            assert_eq!(exact_digits(v), full(v), "{v:e}");
        }
        for v in [
            5e-324,
            1e-310,
            f64::MAX,
            1e300,
            0.1,
            0.125,
            1.0,
            9.995,
            1e22,
            123456.789,
        ] {
            assert_eq!(exact_digits(v), full(v), "{v:e}");
        }
    }

    #[test]
    fn to_fixed_rounds_ties_away_from_zero_like_javascript() {
        assert_eq!(to_fixed(0.125, 2), "0.13");
        assert_eq!(to_fixed(-0.125, 2), "-0.13");
        assert_eq!(to_fixed(2.5, 0), "3");
        assert_eq!(to_fixed(1.005, 2), "1.00"); // 1.005 is below the tie
        assert_eq!(to_fixed(0.0, 3), "0.000");
        assert_eq!(to_fixed(-0.0, 1), "0.0");
        assert_eq!(to_fixed(-0.0001, 2), "-0.00");
        assert_eq!(to_fixed(9.995, 2), "9.99"); // below the tie in binary
        assert_eq!(to_fixed(99.5, 0), "100");
        assert_eq!(to_fixed(1234.5678, 1), "1234.6");
        assert_eq!(to_fixed(0.0005, 3), "0.001"); // just above the tie in binary
        assert_eq!(to_fixed(5e-7, 0), "0");
        assert_eq!(to_fixed(0.6, 0), "1");
    }

    #[test]
    fn to_exponential_matches_javascript() {
        assert_eq!(to_exponential(1.5e-7, 1), "1.5e-7");
        assert_eq!(to_exponential(1.25, 1), "1.3e+0");
        assert_eq!(to_exponential(-1.25, 1), "-1.3e+0");
        assert_eq!(to_exponential(9.96, 1), "1.0e+1");
        assert_eq!(to_exponential(123456.0, 2), "1.23e+5");
        assert_eq!(to_exponential(0.0, 2), "0.00e+0");
        assert_eq!(to_exponential(1e21, 0), "1e+21");
    }

    #[test]
    fn values_round_at_the_uncertainty_place() {
        let w = |v: f64, u: f64| {
            let (a, b) = with_uncertainty(v, u).unwrap();
            format!("{a} +- {b}")
        };
        assert_eq!(w(0.90561, 0.00437), "0.906 +- 0.004");
        assert_eq!(w(0.4192, 0.0012), "0.4192 +- 0.0012");
        assert_eq!(w(-14.57316, 0.0010), "-14.5732 +- 0.0010");
        assert_eq!(w(0.123, 0.0096), "0.12 +- 0.01");
        assert_eq!(w(0.1234, 0.0196), "0.123 +- 0.020");
        assert_eq!(w(-0.00002, 0.00026), "0.0000 +- 0.0003");
        assert_eq!(w(1234.5, 23.0), "1230 +- 20");
        assert_eq!(w(1250.0, 160.0), "1250 +- 160");
        assert_eq!(w(5.0, 950.0), "0 +- 1000");
        assert_eq!(w(2.5, 0.5), "2.5 +- 0.5");
        assert_eq!(with_uncertainty(1.0, 0.0), None);
        assert_eq!(with_uncertainty(f64::NAN, 0.1), None);
    }

    #[test]
    fn readable_is_the_apps_fmt_with_ascii_minus() {
        assert_eq!(readable(3.0, 4), "3");
        assert_eq!(readable(-0.5, 4), "-0.5");
        assert_eq!(readable(2.6666666666666665, 4), "2.667");
        assert_eq!(readable(1.3228756555322954, 3), "1.32");
        assert_eq!(readable(1.23456e-5, 3), "1.23e-5");
        assert_eq!(readable(123456.0, 3), "1.23e5");
        assert_eq!(readable(-1e-9, 4), "-1e-9");
        assert_eq!(readable(0.0, 4), "0");
        assert_eq!(tex(-1.5e-7, 4), "-1.5\\times 10^{-7}");
        assert_eq!(tex(1e-7, 4), "10^{-7}");
        assert_eq!(si(-0.5, 4), "\\num{-0.5}");
        assert_eq!(si(1.23e-5, 3), "\\num{1.23e-5}");
        for v in [-0.5, -1e-9, -123456.0] {
            assert!(!readable(v, 4).contains('\u{2212}'));
        }
    }
}
