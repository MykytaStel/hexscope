//! Numbers with a fixed count of decimals, written by hand.
//!
//! The standard library's float formatting is exact, and large: using it
//! anywhere puts two whole algorithms, over 10 KB, into the WebAssembly
//! build. What this crate shows — sizes, seconds, coordinates — needs a
//! handful of digits, so the value is scaled, rounded half away from zero
//! and printed as integers. That can differ from `{:.N}` in the last digit
//! of an exact tie, which no reader of a size or a latitude would notice.

/// `v` with `places` digits after the point, at most nine: `fixed(2.345, 1)`
/// is `2.3`. Past about ten to the eighteenth over the scale, the value is
/// written as a power of ten instead, such as `1.500e21`.
pub(crate) fn fixed(v: f64, places: u32) -> String {
    if v.is_nan() {
        return "NaN".into();
    }
    if v.is_infinite() {
        return if v > 0.0 { "inf".into() } else { "-inf".into() };
    }
    let places = places.min(9);
    let scale = 10u64.pow(places);
    let scaled = (v.abs() * scale as f64).round();
    // u64 holds up to about 1.8e19; keep well clear of it.
    if scaled >= 1e18 {
        let exp = v.abs().log10().floor();
        let mantissa = v / 10f64.powi(exp as i32);
        return format!("{}e{}", fixed(mantissa, 3), exp as i64);
    }
    let n = scaled as u64;
    let mut out = String::new();
    if v < 0.0 && n != 0 {
        out.push('-');
    }
    out.push_str(&(n / scale).to_string());
    if places > 0 {
        let frac = (n % scale).to_string();
        out.push('.');
        for _ in frac.len()..places as usize {
            out.push('0');
        }
        out.push_str(&frac);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::fixed;

    #[test]
    fn rounds_and_pads_like_a_person_would() {
        assert_eq!(fixed(2.345, 1), "2.3");
        assert_eq!(fixed(2.35, 1), "2.4");
        assert_eq!(fixed(48.858_4, 5), "48.85840");
        assert_eq!(fixed(0.05, 1), "0.1");
        assert_eq!(fixed(1.0 / 3.0, 6), "0.333333");
        assert_eq!(fixed(7.0, 0), "7");
        assert_eq!(fixed(0.000_001, 6), "0.000001");
        assert_eq!(fixed(-2.294_5, 5), "-2.29450");
        // Rounded to nothing, it has no sign.
        assert_eq!(fixed(-0.000_01, 1), "0.0");
    }

    #[test]
    fn odd_values_do_not_break_it() {
        assert_eq!(fixed(f64::NAN, 2), "NaN");
        assert_eq!(fixed(f64::INFINITY, 2), "inf");
        assert_eq!(fixed(f64::NEG_INFINITY, 2), "-inf");
        assert_eq!(fixed(1.5e21, 1), "1.500e21");
        assert_eq!(fixed(-2.0e300, 6), "-2.000e300");
        assert_eq!(fixed(123.456, 30), "123.456000000");
    }
}
