# CLAUDE.md: BIGFLY project memory

BIGFLY is the internal codename and repo for **Seqly.bio**, a game-based bioinformatics learning platform.
Use "BIGFLY" for the repo and internal docs. Use **"Seqly"** for anything public: the website (seqly.bio), published package names (`seqly-engine`), share cards and UI text.

The full architecture is in [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md). The current phase plan is in [docs/PHASE0_PLAN.md](docs/PHASE0_PLAN.md).

## Owner and constraints
- Solo developer: a bioinformatics grad student (sequencing pipelines, scRNA-seq, bulk RNA-seq) who knows Rust.
- Everything must be **low-maintenance and cheap to run**. Prefer static over dynamic, boring over clever, and fewer dependencies.
- The business model is undecided (free, free for students + paid institutions, or freemium). Don't make choices that close any of these off.
- **Ask before any major decision not covered by docs/ARCHITECTURE.md**, such as a new dependency with real weight, a new service, a schema change or a licence.

## What Seqly is
A learning **system**, not just a set of games. It has three layers:
1. **Concept graph** (~120-150 concepts, a YAML DAG in `content/`). Every round is tagged with the concepts it exercises. Unlocks depend on mastery, not on finishing games.
2. **Learner model**: per-user, per-concept mastery (Elo-style first). It is always **derived from the append-only attempts log**.
3. **Misconception library**: every wrong option maps to an ID like `M-012`, with reusable targeted feedback.

Score/XP (fun: hints cost points) and mastery (real learning: hints barely reduce credit) are **separate**.

## Design principles (non-negotiable)
- Learning first, game second. Every round exercises specific concepts.
- **Never teach anything a working bioinformatician would have to unlearn.** Simplify, never distort.
- Wrong answers are the product. Every wrong option maps to a named misconception.
- Many valid answers. **Validate outcomes and state, never one memorised sequence.**
- Every game ends in reality: a "Do it for real" panel with real R/Python/bash code (with the player's parameters filled in) and a small real dataset.
- Static first. Use a server or DB only for user-specific state.
- Measure learning (pre/post tests, retention), not just engagement.

## Science correction rules (always apply these when writing content, scenarios, feedback or code)
When unsure about a scientific claim, **say so and ask** rather than inventing. Cite tool docs or papers in content where possible.

- **BLAST**: never teach a fixed "% identity threshold for homology" (e.g. ">70%"). Lead with the **E-value** and explain that it depends on database size (and query length). Nucleotide and protein intuitions differ: protein identity of 25-40% with a tiny E-value is often clear homology. The "twilight zone" is ~20-35% protein identity. Low identity does not always mean unrelated (M-027).
- **Pipelines**: there is no single fixed order. Validate on a **typed graph** by data-type compatibility (FASTQ → BAM → counts → DE table). Accept every valid route: STAR or HISAT2 + featureCounts, and Salmon/kallisto + tximport. STAR soft-clips, so adapter trimming isn't always mandatory. The reference genome **and** the GTF are required inputs. Score efficiency separately from validity.
- **FastQC / QC**: for RNA-seq, per-base sequence content (random hexamer priming bias) and sequence duplication commonly FAIL and are **expected**. The assay type (RNA-seq/WGS/amplicon) changes what counts as a problem. Always offer an "Expected for this assay" option. A FAIL doesn't automatically mean a bad sample (M-019). Never offer "subsample" as a remediation.
- **Variant calling**: never use a single rule like "coverage <3x = error". Combine depth, allele fraction, base quality, mapping quality and strand bias. Germline vs somatic comes at Advanced level. End by showing the real VCF records.
- **Heatmaps**: you can't call significance from colour. Heatmaps are usually **row z-scored**. Pair them with a DE table (log2FC, padj). z-score ≠ fold change ≠ significance. Heatmaps are for clustering and patterns. High expression ≠ upregulated (M-012).
- **Genome assembly**: there are two distinct modes. In OLC, reads are nodes. In de Bruijn, k-mers are nodes; show how k affects repeat resolution. Only report N50/L50 when there are multiple contigs.
- **DESeq2 / statistics**: **no p-hacking mechanics.** Design mode sets n, thresholds and the design formula **before** the player sees data (a power game). In Analysis mode the thresholds are locked and the player interprets. Bonferroni's conservativeness comes from the **number of tests**, not sample size. Include batch in the design formula, independent filtering, lfcShrink, and BH vs Bonferroni. A lower threshold is not "more reliable" in every case (M-041).
- **Multi-omics**: mRNA-protein correlation is only moderate, so discordance is normal. Correlation across layers doesn't imply causation (M-063).
- **Tool Terminal**: don't teach `-S` in `samtools view -bS`. It's obsolete because input format is auto-detected. **Never validate commands by string matching.** Parse commands into a virtual file system and validate the resulting state (any flag order, long or short flags). Pin and display tool versions. **Never execute real shell commands.**
- **scRNA-seq**: run QC (doublets, mitochondrial fraction) before annotation. Use correct marker logic, e.g. NK (NKG7/GNLY, CD3E-negative) vs T cells (CD3E+).
- **Restriction enzymes**: use real recognition sites and cut positions from a cited source (REBASE). The v0 panel is EcoRI, BamHI, HindIII, HaeIII, PstI and SalI. Sticky vs blunt ends and ligation compatibility must be correct. Overhang sequence and polarity matter, not just whether an end is "sticky".

## Engine conventions (Rust → WASM, `engine/`, crate `seqly-engine`)
- Every game is a plugin on one engine loop: `generate → play (action log) → evaluate → explain → update mastery → bridge`.
- **Determinism is a hard requirement.** The same (game, difficulty, seed, content_version, engine version) must give the same scenario and result on every machine and in browser and server alike.
  - Use only the engine's own seeded RNG. Never use OS randomness, wall-clock time or `HashMap` iteration order (use `BTreeMap`/`Vec`).
  - Scores are integers. Avoid floats in anything that decides a score. Where floats are unavoidable, use `libm` for transcendental functions.
  - Golden tests pin outputs for fixed seeds. Changing a golden output is a breaking change and needs an engine version bump.
- The engine emits **IDs** (concepts, misconceptions). Human-readable text lives in `content/`.
- Every game has a **reference solver** and **property tests**: for many seeds, solve → evaluate must be correct, and every wrong option must carry a misconception ID.
- Keep the engine free of web/DB concerns so it can be open-sourced on its own.

## Content as code (`content/`)
- Concepts, misconceptions, glossary, hand-written scenarios and Case Files live as YAML/Markdown.
- CI runs **schema checks and science checks**: the concept graph is acyclic, every referenced ID exists, every digest scenario has a valid solution, the correct BLAST answer has the best E-value, and so on.
- Content releases are versioned (`content/VERSION`). Every attempt stores `content_version`.
- New Case Files should be mostly content, not code.

## Security rules (full detail in docs/ARCHITECTURE.md §7.9)
- **Anything on the CDN is public.** Never put gated or paid content or user data in the static bundle.
- **RLS on every table, deny by default.** Client flags only hide UI. Entitlements and roles are enforced on the server.
- Secrets (`service_role`, R2, GitHub tokens) are **server-only**. Never use `NEXT_PUBLIC_` for them, and never commit them.
- Attempts are append-only (insert via a server function only). Mastery is written only by the server.
- Rate-limit every write path. Cap action-log size and replay work.
- Encrypt backups before upload. Minimal PII. Render user text as text, never as HTML.
- No payment card handling, ever. If payments are added: hosted checkout + webhook → `entitlements`.

## Conventions
- **Spelling: British English** in prose, UI text, content, comments and our own identifiers (`colour`, `normalisation`, `tumour`, `randomise`). Keep external spellings unchanged (CSS `color`, library APIs, tool flags such as `--normalize`).
- Prefer simple solutions over clever ones. Keep code well tested. Write tests alongside every engine feature.
- Make small, focused commits. Ask before adding heavyweight dependencies.
- Privacy: minimal PII, no ads, no third-party tracking. Design for GDPR export and deletion from the start.
- Accessibility: use colour-blind-safe palettes (viridis-style) **plus** a second visual channel. Every drag-and-drop needs a keyboard/click alternative.
- The browser never calls NCBI/Ensembl/GEO live. Data comes from the offline `data-pipeline/` with its source and licence recorded.

## Repo map
- `apps/web/`: Next.js + Tailwind site (static export where possible)
- `engine/`: Rust crate → WASM (`seqly-engine`)
- `content/`: YAML for concepts, misconceptions, scenarios, glossary and Case Files
- `data-pipeline/`: Snakemake pipeline for subsetting public datasets
- `docs/`: architecture and design docs
- `.github/workflows/`: CI

## Common commands (run from `engine/`)
- All tests: `cargo test --all-features`
- Lint: `cargo fmt --check && cargo clippy --all-targets --all-features -- -D warnings`
- Content checks: `cargo run --features content --bin content-check -- ../content`
- Browser WASM build: `cargo build --release --target wasm32-unknown-unknown --features wasm`
- Same tests as WASM: `CARGO_TARGET_WASM32_WASIP1_RUNNER="node $PWD/scripts/wasi-runner.mjs" cargo test --target wasm32-wasip1 --test determinism --test properties`
- Regenerate golden files (**only for intended behaviour changes, and bump the version in `Cargo.toml`**): `UPDATE_GOLDEN=1 cargo test --test determinism`

## Adding a game
1. Create `engine/src/games/<id>.rs` implementing `Game` (copy the shape of `dna_chopper.rs`), and register it in `core/registry.rs::games()`.
2. Declare every concept and misconception it can emit in `manifest()`, and add any missing ones to `content/`.
3. Each multiple-choice option must map to a misconception (the property tests enforce this).
4. Add hand-written edge cases under `content/scenarios/<id>/`, and generate golden files.

## Current phase
**Phase 0 (Engine).** Engine core, the Game trait, concept graph v0 (~40 concepts), misconception library v0 and CI science checks. A static site with no DB.
Exit criteria: the engine runs DNA Chopper and Sequence Scramble deterministically, and CI catches a deliberately broken scenario.
