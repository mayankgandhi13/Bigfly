//! Types shared by every game.

use serde::{Deserialize, Serialize};

/// Concept ID from `content/concepts/` (snake_case).
pub type ConceptId = String;
/// Misconception ID from `content/misconceptions/` (`M-###`).
pub type MisconceptionId = String;

/// Largest seed accepted: JavaScript's `Number.MAX_SAFE_INTEGER`, so seeds
/// survive JSON round-trips through the browser unchanged.
pub const MAX_SEED: u64 = (1 << 53) - 1;

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "snake_case")]
pub enum Difficulty {
    Intro,
    Standard,
    Advanced,
}

impl Difficulty {
    pub const ALL: [Difficulty; 3] = [
        Difficulty::Intro,
        Difficulty::Standard,
        Difficulty::Advanced,
    ];
}

/// Scaffolding level: worked example → faded hints → independent.
/// Orthogonal to difficulty. The engine records it; the UI and mastery model use it.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum Support {
    WorkedExample,
    FadedHints,
    #[default]
    Independent,
}

/// Inputs to `generate`. `round` optionally pins a round type (see the game's manifest);
/// otherwise the round is chosen from the seed.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct GenParams {
    pub difficulty: Difficulty,
    #[serde(default)]
    pub support: Support,
    pub seed: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub round: Option<String>,
}

/// What is stored and passed around for a generated scenario.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ScenarioEnvelope {
    pub game: String,
    pub engine_version: String,
    pub params: GenParams,
    /// Concepts exercised by this specific scenario.
    pub concepts: Vec<ConceptId>,
    /// Game-specific scenario body.
    pub body: serde_json::Value,
}

/// An action-log entry. `t_ms` is for analytics only and never affects scoring.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct TimedAction<A> {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub t_ms: Option<u32>,
    pub action: A,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct ConceptOutcome {
    pub concept: ConceptId,
    /// Credit in thousandths (0–1000). Integers keep results bit-identical across platforms.
    pub credit_permille: u16,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Evaluation {
    pub solved: bool,
    pub score: u32,
    pub max_score: u32,
    pub hints_used: u16,
    pub concepts: Vec<ConceptOutcome>,
    pub misconceptions: Vec<MisconceptionId>,
    /// Scored separately from correctness where a game has a notion of efficiency.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub efficiency: Option<u32>,
}

pub const MAX_SCORE: u32 = 100;

impl Evaluation {
    /// The shared scoring rule. Hints cost points (score) but only slightly
    /// reduce mastery credit.
    pub fn new(
        solved: bool,
        hints_used: u16,
        concepts: &[ConceptId],
        mut misconceptions: Vec<MisconceptionId>,
    ) -> Self {
        let h = u32::from(hints_used);
        let score = if solved {
            MAX_SCORE.saturating_sub(20 * h).max(40)
        } else {
            0
        };
        let credit: u16 = if solved {
            1000u16.saturating_sub(100 * hints_used).max(700)
        } else {
            0
        };
        misconceptions.sort();
        misconceptions.dedup();
        Self {
            solved,
            score,
            max_score: MAX_SCORE,
            hints_used,
            concepts: concepts
                .iter()
                .map(|c| ConceptOutcome {
                    concept: c.clone(),
                    credit_permille: credit,
                })
                .collect(),
            misconceptions,
            efficiency: None,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Lang {
    Bash,
    R,
    Python,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct ToolVersion {
    pub tool: String,
    pub version: String,
}

/// The "Do it for real" panel.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct RealCode {
    pub language: Lang,
    pub code: String,
    pub tool_versions: Vec<ToolVersion>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dataset: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub doc_url: Option<String>,
}

/// Static declaration of everything a game can emit. Checked against `content/` in CI.
#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
pub struct GameManifest {
    pub id: &'static str,
    pub name: &'static str,
    pub rounds: Vec<&'static str>,
    pub concepts: Vec<&'static str>,
    pub misconceptions: Vec<&'static str>,
}
