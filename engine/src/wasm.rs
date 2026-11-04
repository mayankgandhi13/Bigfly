//! wasm-bindgen exports. JSON strings in, JSON strings out: the same format that is
//! stored in `attempts` and replayed on the server.

use wasm_bindgen::prelude::*;

use crate::core::registry::{self, GameEntry};
use crate::core::types::{GenParams, ScenarioEnvelope};

fn err(e: impl std::fmt::Display) -> JsError {
    JsError::new(&e.to_string())
}

fn json<T: serde::Serialize>(v: &T) -> Result<String, JsError> {
    serde_json::to_string(v).map_err(err)
}

fn scenario(scenario_json: &str) -> Result<(GameEntry, ScenarioEnvelope), JsError> {
    let s: ScenarioEnvelope = serde_json::from_str(scenario_json).map_err(err)?;
    Ok((registry::find(&s.game).map_err(err)?, s))
}

fn log(log_json: &str) -> Result<serde_json::Value, JsError> {
    serde_json::from_str(log_json).map_err(err)
}

#[wasm_bindgen]
pub fn engine_version() -> String {
    crate::ENGINE_VERSION.to_string()
}

/// Manifests of every game, as a JSON array.
#[wasm_bindgen]
pub fn list_games() -> Result<String, JsError> {
    json(
        &registry::games()
            .iter()
            .map(|g| (g.manifest)())
            .collect::<Vec<_>>(),
    )
}

#[wasm_bindgen]
pub fn generate(game: &str, params_json: &str) -> Result<String, JsError> {
    let p: GenParams = serde_json::from_str(params_json).map_err(err)?;
    json(
        &registry::find(game)
            .map_err(err)?
            .generate(&p)
            .map_err(err)?,
    )
}

#[wasm_bindgen]
pub fn evaluate(scenario_json: &str, log_json: &str) -> Result<String, JsError> {
    let (g, s) = scenario(scenario_json)?;
    json(&g.evaluate(&s, &log(log_json)?).map_err(err)?)
}

#[wasm_bindgen]
pub fn solve(scenario_json: &str) -> Result<String, JsError> {
    let (g, s) = scenario(scenario_json)?;
    json(&g.solve(&s).map_err(err)?)
}

/// JSON array of answer actions, or `null` for free-response rounds.
#[wasm_bindgen]
pub fn choices(scenario_json: &str) -> Result<String, JsError> {
    let (g, s) = scenario(scenario_json)?;
    json(&g.choices(&s).map_err(err)?)
}

#[wasm_bindgen]
pub fn bridge(scenario_json: &str, log_json: &str) -> Result<String, JsError> {
    let (g, s) = scenario(scenario_json)?;
    json(&g.bridge(&s, &log(log_json)?).map_err(err)?)
}

/// Seed for the daily question (`YYYY-MM-DD`). Returned as a JS number: it is
/// always <= Number.MAX_SAFE_INTEGER, so no precision is lost.
#[wasm_bindgen]
pub fn daily_seed(date: &str) -> f64 {
    crate::core::rng::daily_seed(date) as f64
}

/// Gel migration (thousandths of gel length) for a JSON array of fragment sizes.
#[wasm_bindgen]
pub fn gel_positions(sizes_json: &str) -> Result<String, JsError> {
    let sizes: Vec<u32> = serde_json::from_str(sizes_json).map_err(err)?;
    json(
        &sizes
            .iter()
            .map(|&s| crate::bio::gel::migration_permille(s))
            .collect::<Vec<_>>(),
    )
}
