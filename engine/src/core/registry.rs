//! Type-erased, JSON-in/JSON-out access to every game. This is the surface used by
//! WASM, server replay and the content checker.

use serde_json::Value;

use super::error::{EngineError, EngineResult};
use super::game::Game;
use super::rng::{Rng, derive_seed};
use super::types::{
    Evaluation, GameManifest, GenParams, MAX_SEED, RealCode, ScenarioEnvelope, TimedAction,
};
use crate::ENGINE_VERSION;

/// Maximum number of actions accepted in a log (bounds replay work).
pub const MAX_LOG_LEN: usize = 10_000;

#[derive(Clone, Copy)]
pub struct GameEntry {
    pub id: &'static str,
    pub manifest: fn() -> GameManifest,
    generate: fn(&GenParams) -> EngineResult<ScenarioEnvelope>,
    wrap: fn(&GenParams, &Value) -> EngineResult<ScenarioEnvelope>,
    evaluate: fn(&Value, &Value) -> EngineResult<Evaluation>,
    solve: fn(&Value) -> EngineResult<Value>,
    choices: fn(&Value) -> EngineResult<Option<Value>>,
    bridge: fn(&Value, &Value) -> EngineResult<RealCode>,
    check: fn(&Value) -> EngineResult<Vec<String>>,
}

impl GameEntry {
    pub fn generate(&self, params: &GenParams) -> EngineResult<ScenarioEnvelope> {
        (self.generate)(params)
    }
    /// Wrap a hand-written scenario body into an envelope (validates the body).
    pub fn wrap(&self, params: &GenParams, body: &Value) -> EngineResult<ScenarioEnvelope> {
        (self.wrap)(params, body)
    }
    pub fn evaluate(&self, s: &ScenarioEnvelope, log: &Value) -> EngineResult<Evaluation> {
        (self.evaluate)(&s.body, log)
    }
    pub fn solve(&self, s: &ScenarioEnvelope) -> EngineResult<Value> {
        (self.solve)(&s.body)
    }
    pub fn choices(&self, s: &ScenarioEnvelope) -> EngineResult<Option<Value>> {
        (self.choices)(&s.body)
    }
    pub fn bridge(&self, s: &ScenarioEnvelope, log: &Value) -> EngineResult<RealCode> {
        (self.bridge)(&s.body, log)
    }
    pub fn check(&self, s: &ScenarioEnvelope) -> EngineResult<Vec<String>> {
        (self.check)(&s.body)
    }
}

/// Every game the engine knows about. Add new games here.
pub fn games() -> Vec<GameEntry> {
    vec![
        entry::<crate::games::dna_chopper::DnaChopper>(),
        entry::<crate::games::sequence_scramble::SequenceScramble>(),
    ]
}

pub fn find(id: &str) -> EngineResult<GameEntry> {
    games()
        .into_iter()
        .find(|g| g.id == id)
        .ok_or_else(|| EngineError::UnknownGame(id.to_string()))
}

fn entry<G: Game>() -> GameEntry {
    GameEntry {
        id: G::ID,
        manifest: G::manifest,
        generate: generate_impl::<G>,
        wrap: wrap_impl::<G>,
        evaluate: evaluate_impl::<G>,
        solve: solve_impl::<G>,
        choices: choices_impl::<G>,
        bridge: bridge_impl::<G>,
        check: check_impl::<G>,
    }
}

fn validate_params<G: Game>(p: &GenParams) -> EngineResult<()> {
    if p.seed > MAX_SEED {
        return Err(EngineError::InvalidParams(format!(
            "seed must be <= {MAX_SEED}"
        )));
    }
    if let Some(r) = &p.round
        && !G::manifest().rounds.contains(&r.as_str())
    {
        return Err(EngineError::InvalidParams(format!(
            "unknown round '{r}' for {}",
            G::ID
        )));
    }
    Ok(())
}

fn envelope<G: Game>(p: &GenParams, body: &G::Scenario) -> EngineResult<ScenarioEnvelope> {
    Ok(ScenarioEnvelope {
        game: G::ID.to_string(),
        engine_version: ENGINE_VERSION.to_string(),
        params: p.clone(),
        concepts: G::concepts(body),
        body: serde_json::to_value(body)
            .map_err(|e| EngineError::InvalidScenario(e.to_string()))?,
    })
}

fn generate_impl<G: Game>(p: &GenParams) -> EngineResult<ScenarioEnvelope> {
    validate_params::<G>(p)?;
    let mut rng = Rng::from_seed(derive_seed(p.seed, G::ID));
    let body = G::generate(p, &mut rng);
    envelope::<G>(p, &body)
}

fn parse_body<G: Game>(body: &Value) -> EngineResult<G::Scenario> {
    serde_json::from_value(body.clone()).map_err(|e| EngineError::InvalidScenario(e.to_string()))
}

fn parse_log<G: Game>(log: &Value) -> EngineResult<Vec<G::Action>> {
    let entries: Vec<TimedAction<G::Action>> =
        serde_json::from_value(log.clone()).map_err(|e| EngineError::InvalidLog(e.to_string()))?;
    if entries.len() > MAX_LOG_LEN {
        return Err(EngineError::InvalidLog(format!(
            "more than {MAX_LOG_LEN} actions"
        )));
    }
    Ok(entries.into_iter().map(|e| e.action).collect())
}

fn to_log<G: Game>(actions: Vec<G::Action>) -> EngineResult<Value> {
    let entries: Vec<TimedAction<G::Action>> = actions
        .into_iter()
        .map(|action| TimedAction { t_ms: None, action })
        .collect();
    serde_json::to_value(entries).map_err(|e| EngineError::InvalidLog(e.to_string()))
}

fn wrap_impl<G: Game>(p: &GenParams, body: &Value) -> EngineResult<ScenarioEnvelope> {
    validate_params::<G>(p)?;
    let s = parse_body::<G>(body)?;
    envelope::<G>(p, &s)
}

fn evaluate_impl<G: Game>(body: &Value, log: &Value) -> EngineResult<Evaluation> {
    Ok(G::evaluate(&parse_body::<G>(body)?, &parse_log::<G>(log)?))
}

fn solve_impl<G: Game>(body: &Value) -> EngineResult<Value> {
    to_log::<G>(G::solve(&parse_body::<G>(body)?))
}

fn choices_impl<G: Game>(body: &Value) -> EngineResult<Option<Value>> {
    match G::choices(&parse_body::<G>(body)?) {
        None => Ok(None),
        Some(c) => Ok(Some(
            serde_json::to_value(c).map_err(|e| EngineError::InvalidLog(e.to_string()))?,
        )),
    }
}

fn bridge_impl<G: Game>(body: &Value, log: &Value) -> EngineResult<RealCode> {
    Ok(G::bridge(&parse_body::<G>(body)?, &parse_log::<G>(log)?))
}

fn check_impl<G: Game>(body: &Value) -> EngineResult<Vec<String>> {
    Ok(G::check(&parse_body::<G>(body)?))
}
