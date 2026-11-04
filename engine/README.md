# seqly-engine

Deterministic simulation engines for Seqly.bio games, written in Rust and compiled to WebAssembly. The same build runs in the browser (play) and on the server (score replay verification).

## Layout
- `src/core/`: `Game` trait, shared types, seeded RNG, JSON registry
- `src/bio/`: real biology: sequences, codon table, restriction enzymes (REBASE), digests, gel model
- `src/games/`: one file per game (`dna_chopper`, `sequence_scramble`)
- `src/wasm.rs`: wasm-bindgen exports (feature `wasm`)
- `src/content.rs` + `src/bin/content_check.rs`: content schema and science checks (feature `content`)
- `tests/`: property tests, golden files, content checks

## WASM API (JSON strings in and out)
`engine_version()`, `list_games()`, `generate(game, params)`, `evaluate(scenario, log)`, `solve(scenario)`, `choices(scenario)`, `bridge(scenario, log)`, `daily_seed(date)`, `gel_positions(sizes)`

```jsonc
// params
{ "difficulty": "intro|standard|advanced", "support": "worked_example|faded_hints|independent", "seed": 42, "round": "mark_cuts" }
// log: only the first answer counts; t_ms is optional and never scored
[{ "t_ms": 1200, "action": { "type": "toggle_cut", "position": 4 } }, { "action": { "type": "submit_cuts" } }]
```

See the root `CLAUDE.md` for commands.
