//! Golden tests: pinned outputs for fixed seeds. If one fails, either you broke
//! determinism, or you changed behaviour on purpose: then bump ENGINE_VERSION
//! (engine/Cargo.toml) and regenerate with `UPDATE_GOLDEN=1 cargo test --test determinism`.

use std::path::PathBuf;

use seqly_engine::core::registry::games;
use seqly_engine::core::types::{Difficulty, GenParams, Support};

const SEED: u64 = 7;

#[test]
fn golden_outputs() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/golden");
    let update = std::env::var("UPDATE_GOLDEN").is_ok();
    let mut failures = Vec::new();
    for g in games() {
        for round in (g.manifest)().rounds {
            for d in Difficulty::ALL {
                let p = GenParams {
                    difficulty: d,
                    support: Support::Independent,
                    seed: SEED,
                    round: Some(round.into()),
                };
                let s = g.generate(&p).unwrap();
                let solution = g.solve(&s).unwrap();
                let out = serde_json::json!({
                    "scenario": s,
                    "solution": solution,
                    "evaluation": g.evaluate(&s, &solution).unwrap(),
                    "bridge": g.bridge(&s, &solution).unwrap(),
                });
                let text = serde_json::to_string_pretty(&out).unwrap() + "\n";
                let d_name = serde_json::to_value(d)
                    .unwrap()
                    .as_str()
                    .unwrap()
                    .to_string();
                let path = dir.join(format!("{}__{round}__{d_name}.json", g.id));
                if update {
                    std::fs::write(&path, &text).unwrap();
                } else {
                    match std::fs::read_to_string(&path) {
                        Ok(existing) if existing == text => {}
                        Ok(_) => failures.push(format!("changed: {}", path.display())),
                        Err(_) => failures.push(format!(
                            "missing: {} (run with UPDATE_GOLDEN=1)",
                            path.display()
                        )),
                    }
                }
            }
        }
    }
    assert!(
        failures.is_empty(),
        "golden mismatches:\n{}",
        failures.join("\n")
    );
}
