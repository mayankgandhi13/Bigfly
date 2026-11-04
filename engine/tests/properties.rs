//! Property tests over many seeds, through the same JSON registry the browser uses.

use seqly_engine::core::registry::games;
use seqly_engine::core::types::{Difficulty, GenParams, Support};

const SEEDS: u64 = 150;

fn params(d: Difficulty, round: &str, seed: u64) -> GenParams {
    GenParams {
        difficulty: d,
        support: Support::Independent,
        seed,
        round: Some(round.to_string()),
    }
}

#[test]
fn every_round_is_deterministic_valid_and_solvable() {
    for g in games() {
        let m = (g.manifest)();
        for d in Difficulty::ALL {
            for round in &m.rounds {
                for seed in 0..SEEDS {
                    let p = params(d, round, seed);
                    let ctx = format!("{} {round} {d:?} seed {seed}", g.id);
                    let s = g.generate(&p).unwrap();
                    assert_eq!(s, g.generate(&p).unwrap(), "non-deterministic: {ctx}");

                    assert!(
                        g.check(&s).unwrap().is_empty(),
                        "check failed: {ctx}: {:?}",
                        g.check(&s)
                    );
                    for c in &s.concepts {
                        assert!(
                            m.concepts.contains(&c.as_str()),
                            "undeclared concept {c}: {ctx}"
                        );
                    }

                    let solution = g.solve(&s).unwrap();
                    let e = g.evaluate(&s, &solution).unwrap();
                    assert!(e.solved, "reference solution fails: {ctx}");
                    assert!(e.misconceptions.is_empty(), "solution flagged: {ctx}");
                    assert_eq!(e.score, e.max_score, "{ctx}");

                    let empty = g.evaluate(&s, &serde_json::json!([])).unwrap();
                    assert!(!empty.solved, "empty log solved: {ctx}");

                    if let Some(choices) = g.choices(&s).unwrap() {
                        let mut n_correct = 0;
                        for choice in choices.as_array().unwrap() {
                            let log = serde_json::json!([{ "action": choice }]);
                            let e = g.evaluate(&s, &log).unwrap();
                            if e.solved {
                                n_correct += 1;
                            } else {
                                assert!(
                                    !e.misconceptions.is_empty(),
                                    "untagged wrong option {choice}: {ctx}"
                                );
                            }
                            for mid in &e.misconceptions {
                                assert!(
                                    m.misconceptions.contains(&mid.as_str()),
                                    "undeclared {mid}: {ctx}"
                                );
                            }
                        }
                        assert_eq!(n_correct, 1, "exactly one option should be correct: {ctx}");
                    }

                    let bridge = g.bridge(&s, &solution).unwrap();
                    assert!(!bridge.code.is_empty(), "{ctx}");
                }
            }
        }
    }
}

#[test]
fn unpinned_round_picks_a_declared_round() {
    for g in games() {
        for seed in 0..50 {
            let p = GenParams {
                difficulty: Difficulty::Standard,
                support: Support::Independent,
                seed,
                round: None,
            };
            let s = g.generate(&p).unwrap();
            let round = s.body["task"]["round"].as_str().unwrap().to_string();
            assert!((g.manifest)().rounds.contains(&round.as_str()));
        }
    }
}

#[test]
fn rejects_bad_params() {
    let g = &games()[0];
    let mut p = params(Difficulty::Intro, "mark_cuts", 0);
    p.seed = 1 << 60;
    assert!(g.generate(&p).is_err());
    let p = params(Difficulty::Intro, "no_such_round", 0);
    assert!(g.generate(&p).is_err());
}
