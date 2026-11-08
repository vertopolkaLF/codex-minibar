//! Money is stored as whole micro-dollars (`u64`) so sums stay exact.

const MICROS_PER_USD: f64 = 1_000_000.0;

/// Rounded micro-dollars for a dollar amount. Negative and NaN amounts are
/// zero; amounts past `u64::MAX` saturate.
pub fn from_usd(usd: f64) -> u64 {
    (usd * MICROS_PER_USD).round() as u64
}

pub fn to_usd(micros: u64) -> f64 {
    micros as f64 / MICROS_PER_USD
}

/// Whole cents, half a cent rounding up. Integer math keeps it exact at the
/// `u64` upper bound.
pub fn to_cents(micros: u64) -> u64 {
    micros / 10_000 + u64::from(micros % 10_000 >= 5_000)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dollars_round_to_micros_and_clamp() {
        assert_eq!(from_usd(0.0025004), 2_500);
        assert_eq!(from_usd(-1.0), 0);
        assert_eq!(from_usd(f64::NAN), 0);
        assert_eq!(from_usd(f64::INFINITY), u64::MAX);
        assert_eq!(to_usd(1_250_000), 1.25);
    }
}
