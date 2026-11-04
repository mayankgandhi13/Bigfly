//! `seqly-engine`: deterministic simulation engines for Seqly.bio.
//!
//! Every game implements [`core::game::Game`]. The [`core::registry`] exposes all
//! games through a JSON interface used by WASM, server replay and the content checker.
//! See `docs/PHASE0_PLAN.md`.

pub mod bio {
    pub mod codon;
    pub mod digest;
    pub mod enzymes;
    pub mod gel;
    pub mod seq;
}

pub mod core {
    pub mod error;
    pub mod game;
    pub mod registry;
    pub mod rng;
    pub mod types;
}

pub mod games {
    pub mod common;
    pub mod dna_chopper;
    pub mod sequence_scramble;
}

#[cfg(feature = "content")]
pub mod content;

#[cfg(feature = "wasm")]
pub mod wasm;

/// Engine version. Bump it whenever generated scenarios or evaluation results change
/// for a given seed (the golden tests will fail until you do).
pub const ENGINE_VERSION: &str = env!("CARGO_PKG_VERSION");
