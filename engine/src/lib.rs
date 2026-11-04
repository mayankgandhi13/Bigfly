//! `seqly-engine`: deterministic simulation engines for Seqly.bio.
//!
//! Placeholder for Phase 0. See `docs/PHASE0_PLAN.md` for the proposed layout.

/// Engine version. Bump it whenever generated scenarios or evaluation results change for a given seed.
pub const ENGINE_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    #[test]
    fn version_is_set() {
        assert!(!super::ENGINE_VERSION.is_empty());
    }
}
