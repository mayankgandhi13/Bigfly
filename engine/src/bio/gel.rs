//! Agarose gel layout: migration is roughly linear in log10(fragment size) over the
//! resolving range. This is a display model for the gel view, not used in scoring.

/// Migration distance as thousandths of the gel length (0 = well, 1000 = bottom).
/// Calibrated so 100 bp → 900 and 10,000 bp → 100. Clamped to 20..=980.
/// Uses `libm` so the result is identical on every platform.
pub fn migration_permille(size_bp: u32) -> u16 {
    let size = f64::from(size_bp.max(1));
    let d = 900.0 - 400.0 * (libm::log10(size) - 2.0);
    libm::round(d).clamp(20.0, 980.0) as u16
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calibration_points() {
        assert_eq!(migration_permille(100), 900);
        assert_eq!(migration_permille(1000), 500);
        assert_eq!(migration_permille(10_000), 100);
    }

    #[test]
    fn smaller_runs_further() {
        assert!(migration_permille(250) > migration_permille(500));
        assert_eq!(migration_permille(1), 980);
        assert_eq!(migration_permille(1_000_000), 20);
    }
}
