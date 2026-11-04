//! The plugin interface every game implements.

use serde::{Serialize, de::DeserializeOwned};

use super::rng::Rng;
use super::types::{ConceptId, Evaluation, GameManifest, GenParams, RealCode};

pub trait Game {
    /// Stable ID, e.g. `"dna_chopper"`. Never change it: it is stored with every attempt.
    const ID: &'static str;
    type Scenario: Serialize + DeserializeOwned + Clone;
    type Action: Serialize + DeserializeOwned + Clone + PartialEq;

    fn manifest() -> GameManifest;

    /// Pure function of `(params, rng)`. Must not read clocks or global state.
    /// `params.round`, if set, has already been validated against the manifest.
    fn generate(params: &GenParams, rng: &mut Rng) -> Self::Scenario;

    /// Concepts exercised by this scenario (a subset of the manifest's).
    fn concepts(s: &Self::Scenario) -> Vec<ConceptId>;

    /// Validate outcome/state, not a memorised sequence. Must accept any log.
    fn evaluate(s: &Self::Scenario, log: &[Self::Action]) -> Evaluation;

    /// One valid solution: used by property tests, hints and worked examples.
    fn solve(s: &Self::Scenario) -> Vec<Self::Action>;

    /// For multiple-choice rounds, every option as a single answer action.
    /// Each wrong option must map to at least one misconception (property-tested).
    /// `None` for free-response rounds.
    fn choices(s: &Self::Scenario) -> Option<Vec<Self::Action>>;

    /// "Do it for real" code with the player's parameters filled in.
    fn bridge(s: &Self::Scenario, log: &[Self::Action]) -> RealCode;

    /// Science sanity checks for hand-written scenarios. Empty = OK.
    fn check(_s: &Self::Scenario) -> Vec<String> {
        Vec::new()
    }
}
