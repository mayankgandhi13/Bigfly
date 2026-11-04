# Phase 0 plan: engine core

## Status (2026-11-03)
**Engine core done. Exit criteria met.** The minimal web shell (§10) is still open.

- Both games are deterministic: golden files are identical natively and as WebAssembly (CI runs them under Node WASI). Property tests cover 150 seeds × every round × every difficulty.
- CI rejects each file in `content/tests/broken/` (4 deliberate authoring mistakes).
- Content v0: 49 concepts, 24 misconceptions (all awaiting expert review), 12 glossary terms, 4 hand-written edge-case scenarios.

Decisions taken: `serde_yaml_ng`. Two axes, Difficulty × Support. XhoI and BglII added to the panel. TS type generation (`ts-rs`) is deferred to the web shell. Licences not yet chosen.

Where the code differs from the proposal below:
- Each game is one file (`games/<game>.rs`) rather than five. Split them when a game grows.
- The scenario envelope holds `engine_version` but not `content_version`. The web app stamps `content_version` on the attempt, since generated scenarios don't depend on content.
- `GenParams` gained an optional `round` (to pin a round type), and seeds are capped at 2^53−1 so they survive JSON in the browser.
- The checker is the `content-check` binary (feature `content`). Hand-written scenarios use `expect.solution` and `expect.wrong_answers` (see `content/scenarios/`).

---


**Goal:** the engine runs DNA Chopper and Sequence Scramble deterministically, and CI catches a deliberately broken scenario. There is a static site and no DB.

Items marked **[DECIDE]** need sign-off before any code is written.

---

## 1. Crate layout

There is one crate (`seqly-engine`), with features to keep the WASM build lean. A second binary target holds the content checker, so the science checks reuse the real algorithms.

```
engine/
  Cargo.toml                 features: default = [], wasm, content
  src/
    lib.rs                   public API, ENGINE_VERSION, game registry
    core/
      rng.rs                 own seeded PRNG (SplitMix64 → xoshiro256**) + derive_seed()
      types.rs               GameId, ConceptId, MisconceptionId, Difficulty, Support
      game.rs                Game trait, Evaluation, RealCode, GameManifest
      registry.rs            type-erased DynGame + JSON entry points (all games listed here)
      error.rs
    bio/                     shared real biology, no game logic
      seq.rs                 DNA/RNA types, validation, reverse complement
      codon.rs               standard genetic code (NCBI transl_table=1)
      enzymes.rs             v0 enzyme panel: site, cut offsets (top/bottom strand), REBASE ref
      digest.rs              digest(seq, enzymes, topology) → fragments + ends
      gel.rs                 fragment size → migration distance (log-linear vs ladder)
    games/
      dna_chopper/           mod.rs (Game impl), scenario.rs, action.rs, evaluate.rs, bridge.rs, solver.rs
      sequence_scramble/     same shape
    wasm.rs                  #[cfg(feature = "wasm")] wasm-bindgen exports
    content/                 #[cfg(feature = "content")] YAML schema structs + loaders
    bin/content_check.rs     required-features = ["content"]
  tests/
    golden/                  pinned JSON outputs per (game, difficulty, seed)
    determinism.rs           golden tests
    properties.rs            solve→evaluate is correct for N seeds; every wrong option is tagged
```

Every game module has the same shape, so adding a game is copy-and-fill.

---

## 2. The Game trait

The trait is static and strongly typed inside Rust. A thin type-erased layer exposes JSON for WASM and the server.

```rust
pub trait Game {
    const ID: GameId;                         // e.g. "dna_chopper"
    type Scenario: Serialize + DeserializeOwned + Clone;
    type Action:   Serialize + DeserializeOwned + Clone;

    /// Static declaration of everything this game can emit (checked against content/ in CI).
    fn manifest() -> GameManifest;            // concepts[], misconceptions[], difficulties[]

    /// Pure function of (params, rng). It never reads the clock or global state.
    fn generate(params: &GenParams, rng: &mut Rng) -> Self::Scenario;

    /// Validates outcomes and state, not a memorised sequence. Must be total: any log returns a result.
    fn evaluate(s: &Self::Scenario, log: &[Self::Action]) -> Evaluation;

    /// One valid solution. Used by property tests, hints and worked examples.
    fn solve(s: &Self::Scenario) -> Vec<Self::Action>;

    /// "Do it for real" panel: code with the player's parameters filled in.
    fn bridge(s: &Self::Scenario, log: &[Self::Action]) -> RealCode;
}
```

`solve` and `manifest` are additions to your sketch. They are what make the CI science checks and the "every wrong option is tagged" property test possible.

---

## 3. Core types

```rust
pub struct GenParams {
    pub difficulty: Difficulty,   // Intro | Standard | Advanced
    pub support: Support,         // WorkedExample | FadedHints | Independent
    pub seed: u64,
}

pub struct ScenarioEnvelope<S> {  // what crosses the boundary and gets stored
    pub game: GameId,
    pub engine_version: String,
    pub content_version: String,
    pub params: GenParams,
    pub concepts: Vec<ConceptId>, // concepts exercised by *this* scenario
    pub body: S,
}

pub struct TimedAction<A> { pub t_ms: u32, pub action: A } // time is for analytics only, never scoring

pub struct Evaluation {
    pub solved: bool,
    pub score: u32,                           // integer → bit-identical across platforms
    pub max_score: u32,
    pub hints_used: u16,
    pub concepts: Vec<ConceptOutcome>,        // { concept, credit_permille: u16 } (0-1000)
    pub misconceptions: Vec<MisconceptionHit>,// { id: "M-012", at_action: Option<u32> }
    pub efficiency: Option<u32>,              // scored separately from correctness (e.g. Pipeline Builder)
}

pub struct RealCode {
    pub language: Lang,                       // Bash | R | Python
    pub code: String,
    pub tool_versions: Vec<(String, String)>, // pinned and displayed
    pub dataset: Option<DatasetRef>,          // id into the data-pipeline catalogue
    pub doc_url: Option<String>,
}
```

Actions are per-game serde-tagged enums. For example, DNA Chopper has:
`SelectEnzymes { enzymes }`, `PredictFragments { sizes }`, `ChooseLigationPartner { end_a, end_b }`, `UseHint`.

Credit is stored in **permille integers** rather than floats, which keeps results bit-identical. The mastery model (outside the engine) can convert to floats freely, because it is recomputed from the attempts log anyway.

---

## 4. Determinism and RNG

- **Own PRNG, no `rand` dependency.** The `rand` crate's sampling algorithms (`gen_range`, shuffles) have changed between versions, which would silently change scenarios for the same seed. A ~60-line SplitMix64 + xoshiro256** with our own `range_u32` (Lemire's method) and `shuffle` is fully under our control.
- **Seed derivation:** `derive_seed(root, &[game_id, purpose])` uses a SplitMix64 hash. Independent sub-streams (e.g. "sequence", "distractors") mean adding a new random draw to one part doesn't shift everything else.
- **Daily question:** `root = derive_seed(hash("YYYY-MM-DD"), "daily")`, computed client-side.
- **Banned in the engine:** `HashMap`/`HashSet` iteration (use `BTreeMap`/`Vec`), system time, floats in scoring decisions, and std transcendental functions (use `libm` where needed, e.g. gel log-scale).
- **Golden tests:** for each game, difficulty and a few fixed seeds, the generated scenario, the `solve` output and the evaluation are pinned as JSON in `tests/golden/`. Updating a golden file means bumping `ENGINE_VERSION`. A `UPDATE_GOLDEN=1 cargo test` helper regenerates them.
- **Cross-platform check:** CI runs the golden tests natively and under WASM (`wasm32-wasip1` via wasmtime, or Node). The same file must match on both.

---

## 5. WASM bindings

- `wasm-bindgen` behind the `wasm` feature, with a **JSON string in / JSON string out** boundary:
  ```
  generate(game, params_json) -> scenario_json
  evaluate(game, scenario_json, log_json) -> evaluation_json
  solve(game, scenario_json) -> log_json
  bridge(game, scenario_json, log_json) -> realcode_json
  manifest(game) -> json, list_games() -> json, engine_version() -> string
  ```
  JSON is simple to debug, matches what's stored in `attempts.action_log`/`result`, and is identical for server replay. The performance cost is irrelevant at these sizes.
- TypeScript types are generated from the Rust structs with `ts-rs` (or `tsify`) so the web app and engine can't drift. **[DECIDE: ok to add?]**
- Build: `cargo build --target wasm32-unknown-unknown --features wasm`, then `wasm-bindgen --target web` (run the CLI directly rather than depending on wasm-pack, whose maintenance status is shaky; I'll verify the current state before committing).
- Server replay (Phase 2) uses the same `.wasm` in a Supabase Edge Function (Deno) or a native Rust binary. Either way it calls the same functions.

---

## 6. Content YAML schemas (v0)

Loaded by `content/` structs in the engine (behind the `content` feature) and also compiled to JSON bundles for the web app.

**`content/concepts/*.yaml`** (one file per area, list of nodes)
```yaml
- id: restriction_site            # snake_case, unique, stable forever
  name: Restriction enzyme recognition site
  area: molecular_biology
  level: 1
  requires: [dna_double_strand]
  summary: Short palindromic sequence where a restriction enzyme binds and cuts.
  misconceptions: [M-003]
  glossary: [restriction_enzyme, palindrome]
```
`exercised_by` is **derived** from game manifests and scenario tags rather than hand-maintained, which avoids two sources of truth. CI produces it and the Tool Map reads it.

**`content/misconceptions/*.yaml`**
```yaml
- id: M-019
  statement: Any FastQC FAIL means the sample is bad.
  concepts: [fastqc_modules, assay_specific_qc]
  feedback:
    short: Some FastQC modules are expected to fail for certain assays.
    explain: |
      For RNA-seq, "Per base sequence content" often fails in the first ~10 bases
      due to random hexamer priming bias, and "Sequence duplication" fails because
      highly expressed transcripts are sampled many times. Judge each module
      against what is expected for the assay.
  references: ["https://www.bioinformatics.babraham.ac.uk/projects/fastqc/Help/"]
  reviewed_by: null               # set when an expert signs off
```

**`content/glossary/*.yaml`**: `{ id, term, definition, concepts[], see_also[] }`

**`content/scenarios/<game>/*.yaml`** (hand-written: edge cases and Case File stages)
```yaml
id: dna_chopper.bamhi_bglii_compatible
game: dna_chopper
difficulty: standard
targets_misconception: M-008      # optional
body: { ... }                     # must deserialise into that game's Scenario type
expect:
  solvable: true                  # checker runs solve() + evaluate() and asserts it
  answer: { ... }                 # optional: author's claimed answer, verified against the engine
```

**`content/case-files/*.yaml`**: `{ id, title, samples[], stages: [{ game, scenario | seed }] , report_rubric }`

---

## 7. CI science checks (`content-check` binary)

1. **Schema:** every file parses into its struct, and IDs are unique and well-formed.
2. **Referential integrity:** every concept/misconception ID referenced anywhere (content *and* game manifests) exists.
3. **DAG:** the concept graph is acyclic (topological sort, which reports the cycle path on failure).
4. **Scenario validity:** every hand-written scenario deserialises, `solve()` → `evaluate()` reports `solved`, and any author-claimed `answer` matches the engine's computed answer (e.g. the claimed fragment sizes equal the real digest).
5. **Game-specific checks** in each game's `check()`. DNA Chopper: enzyme sites match the panel and the digest yields ≥1 cut. Sequence Scramble: translations use the codon table and the frameshift scenario actually changes the protein.
6. **Negative test:** `content/tests/broken/` holds deliberately wrong scenarios (e.g. wrong fragment sizes, an unknown misconception ID). A CI step asserts the checker **fails** on them. This is the Phase 0 exit criterion.

Plus property tests in `cargo test`: for 500 seeds per game per difficulty, `solve` → `evaluate` is solved, generation is deterministic, and every non-solution option carries a misconception ID.

---

## 8. Phase 0 games (scope)

**DNA Chopper v0:** a linear or circular sequence with planted sites. Rounds:
(a) which enzyme(s) cut and where; (b) predict fragment sizes and see them on a gel; (c) sticky vs blunt, 5′ vs 3′ overhang (PstI leaves a 3′ overhang); (d) ligation compatibility. Within the six-enzyme panel, no two *different* sticky ends are compatible. Same-enzyme ends ligate, HaeIII blunt ends ligate with any blunt end, and the rest don't. **[DECIDE]** The most instructive case is two different enzymes leaving the same overhang, so I suggest adding XhoI (C^TCGAG, compatible with SalI's TCGA overhang) and/or BglII (A^GATCT, compatible with BamHI's GATC overhang). Circular topology at Standard or above (n cuts → n fragments, vs n+1 for linear).

**Sequence Scramble v0:** complement / reverse complement, transcription (template vs coding strand), translation in a given frame, finding the ORF, and a frameshift round (an indel's effect on downstream codons and premature stops).

Bridge panels: Biopython (`Bio.Restriction`, `Seq.translate`) and/or EMBOSS (`restrict`, `transeq`) snippets.

## 9. Content v0
~40 concepts (molecular biology basics, sequences and formats, enough of the stats/QC spine to test the graph shape) and ~20 misconceptions, focused on the two Phase 0 games plus the canonical examples (M-012, M-019, M-027, M-041, M-063).

## 10. Minimal web shell (end of Phase 0)
A bare Next.js static export page that loads the WASM, generates a scenario from a seed, and shows JSON with a "replay" check. It just proves the pipeline works end to end. The real UI is Phase 1. **[DECIDE: include in Phase 0, or defer to Phase 1?]**

---

## Open decisions (original list; see Status above for what's resolved)
1. **YAML parser:** `serde_yaml` is archived/unmaintained. The options are `serde_yaml_ng` (a maintained fork, drop-in) or authoring content in YAML but validating via JSON Schema in a small script. Recommendation: `serde_yaml_ng`.
2. **TS type generation:** `ts-rs` vs `tsify` vs hand-written types. Recommendation: `ts-rs`.
3. **Web shell in Phase 0:** yes or no (§10).
4. **Licences:** engine (MIT/Apache-2.0 dual is the Rust norm) vs content (CC BY-NC as noted). I haven't added a LICENSE file yet.
5. **Enzyme panel:** add XhoI and/or BglII for compatible-overhang rounds (§8).
6. **Difficulty axes:** two separate axes (Difficulty × Support) as proposed, or one combined level.
