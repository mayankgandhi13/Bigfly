# BIGFLY architecture (Seqly.bio)

BIGFLY is the source code for **Seqly.bio**, a game-based bioinformatics learning platform. Students learn by playing interactive games that simulate real analyses: restriction digests, BLAST, QC, pipelines, variant calling, DESeq2, scRNA-seq annotation and CLI tools.

Seqly is a learning **system**, not just a collection of games. A concept graph, a learner model and a misconception library sit under every game.

- **Users:** bioinformatics and computational biology students and learners.
- **Business model:** undecided. The architecture keeps every option open (fully free, free for students + paid institutions, or freemium).
- **Team:** one developer. Low maintenance and low running cost are design constraints, not nice-to-haves.

---

## 1. Design principles

1. **Learning first, game second.** Every round exercises specific, tagged concepts.
2. **Never teach anything a working bioinformatician would have to unlearn.** Simplify, never distort.
3. **Wrong answers are the product.** Every wrong option maps to a named misconception with targeted feedback.
4. **Many valid answers.** Validate outcomes and state, not one memorised sequence.
5. **Every game ends in reality.** Show real R/Python/bash code and a small real dataset.
6. **Static first.** Use dynamic infrastructure (server/DB) only for user-specific state.
7. **Measure learning** (pre/post tests, retention), not just engagement.

---

## 2. System overview

```
                 ┌──────────────────────── static (CDN) ─────────────────────────┐
                 │                                                                │
  content/ YAML ─┼─► build ─► JSON bundles ──┐                                    │
                 │                            ▼                                   │
                 │   apps/web (Next.js) ──► seqly-engine.wasm                     │
                 │      UI, game shells,     generate / evaluate / bridge         │
                 │      feedback text,       (deterministic)                      │
                 │      bridge panels                                             │
  data-pipeline ─┼─► small datasets (JSON/Parquet) ── large ones ──► R2           │
                 └────────────────────────────────┬───────────────────────────────┘
                                                  │ attempts (seed + action log)
                                                  ▼
                 ┌──────────────────────── dynamic (Phase 2+) ────────────────────┐
                 │  Supabase: Postgres + auth + RLS                               │
                 │    attempts (append-only) ─► mastery (derived)                 │
                 │    replay verification (same WASM) for public scores           │
                 │    classes, reports, leaderboards (materialised views)         │
                 └────────────────────────────────────────────────────────────────┘
```

---

## 3. Learning layer

### 3.1 Concept graph
- ~120-150 concepts by Phase 3 (~40 in Phase 0). Each concept is a node with prerequisites, stored as YAML in `content/concepts/`.
- Every round is tagged with the concepts it exercises.
- **Unlocks depend on concept mastery**, not on finishing games.
- The graph must be **acyclic**. CI checks this.
- The same graph later powers a public **Tool Map** page.

Example:
```yaml
id: fdr
name: False discovery rate
requires: [p_value, multiple_testing]
exercised_by: [deseq2_sim, expression_heatmap, quiz.stats]
misconceptions: [M-041, M-042, M-057]
```

### 3.2 Learner model
- Mastery is estimated per user and per concept, and updated after each attempt.
- v1 uses an **Elo-style** update (learner rating vs item difficulty). Bayesian Knowledge Tracing may come later.
- **"Mastered"** means the estimate is above a threshold on **at least 3 distinct scenarios**.
- Mastery **decays** without practice, which drives spaced review.
- **Recommendation:** the weakest concept whose prerequisites are all mastered.
- Scenario difficulty is **calibrated from player data** over time.
- Mastery is **always derived from the append-only attempts log**, so it can be fully recomputed when the model changes.

### 3.3 Misconception library
- Every wrong option is tagged with an ID such as `M-012`.
- Feedback is written **per misconception**, so the same feedback can be reused across games.
- Examples:
  - M-012 "High expression means upregulated"
  - M-019 "Any FastQC FAIL means the sample is bad"
  - M-027 "Low % identity always means unrelated"
  - M-041 "Lower p-value threshold = more reliable in every case"
  - M-063 "Correlation across omics layers implies causation"

### 3.4 Score vs mastery

|            | Score / XP                        | Mastery                                   |
|------------|-----------------------------------|-------------------------------------------|
| Purpose    | Fun, leaderboards, streaks        | Real learning state                       |
| Hints      | Cost points                       | Slightly reduce credit, never punish      |
| Drives     | Ranks, badges                     | Unlocks, recommendations, spaced review   |
| Visibility | Can be public                     | Private to the learner (+ their instructor) |

### 3.5 Learning science
- **Retrieval practice:** daily question and flashcards.
- **Spaced repetition:** driven by mastery decay.
- **Interleaving:** mixed review rounds.
- **Faded scaffolding:** difficulty tiers go worked example → faded hints → independent.
- **Self-explanation:** an optional one-line "explain your reasoning", graded by a keyword rubric at first.

---

## 4. Case Files and the bridge to reality

### 4.1 Case Files
In a Case File, one experiment flows through every game with the same samples.

**Case File 1: "Tumour vs normal"**
QC Inspector → Pipeline Builder → Experimental Design Lab (spot batch confounding) → DESeq2 Simulator → Expression Heatmap → Variant Hunter → scRNA Explorer → Multi-omics Connector → final written case report, graded by rubric.

Planned: infection response, antimicrobial resistance (WGS), developmental scRNA-seq time course.

**New Case Files should be mostly content, not code.** A Case File is a YAML manifest that pins hand-written scenarios (or fixed seeds) for each stage, plus shared sample metadata.

### 4.2 "Do it for real" panel
After every round, the panel shows:
- the **real code** (R/Python/bash) with the player's parameters filled in (`Game::bridge`)
- a **small downloadable real dataset** (e.g. a PBMC3k subset or a GEO count matrix subset)
- a **documentation link** for the tool

From Level 3, **Notebook mode** runs the code in the browser with Pyodide/JupyterLite. It is fully static.

---

## 5. Game roster (17 modules)

Each game carries **science corrections** that are binding. They are repeated in `CLAUDE.md`.

### Level 1: Foundation
| ID | Game | Teaches | Notes / corrections |
|----|------|---------|---------------------|
| 01 | DNA Chopper | Restriction enzymes, recognition sites, gel simulation, sticky vs blunt ends | Add a ligation compatibility round. Panel: EcoRI, BamHI, HindIII, HaeIII, PstI, SalI |
| 02 | Sequence Scramble | Base pairing, transcription, translation, reading frame | Add a frameshift round |
| 03 | BLAST Detective | E-value, identity, bit score, homology | **No ">70% identity = homology".** E-value first (it depends on database size). Nucleotide and protein intuition differ. Protein 25-40% identity with a tiny E-value is often clear homology. Twilight zone ~20-35% |
| N1 | Format Forge | FASTA, FASTQ, Phred encoding, GTF, SAM/BAM, VCF | Decode and repair broken files |
| N2 | Alignment Grid | Needleman-Wunsch / Smith-Waterman DP fill + traceback, gap penalties | |

### Level 2: Depth
| ID | Game | Notes / corrections |
|----|------|---------------------|
| 04 | Pipeline Builder | **No fixed order.** Validate a **typed graph** by data-type compatibility (FASTQ → BAM → counts → DE table). Accept all valid routes: STAR/HISAT2 + featureCounts, and Salmon/kallisto + tximport. STAR soft-clips, so trimming isn't always mandatory. Reference genome **and** GTF are required. Efficiency is scored separately |
| 05 | QC Inspector | For RNA-seq, per-base sequence content (hexamer bias) and duplication commonly FAIL and are **expected**. Include an "Expected for this assay" option. Assay type (RNA-seq/WGS/amplicon) changes what counts as a problem. **No "subsample" remediation** |
| 06 | Variant Hunter | Not "coverage <3x = error". Combine depth, allele fraction, base quality, mapping quality and strand bias. Germline vs somatic at Advanced. Show VCF records at the end |
| 07 | Expression Heatmap | Colour can't show significance. Heatmaps are usually row z-scored. Pair with a DE table (log2FC, padj). z-score ≠ fold change ≠ significance. Heatmaps are for clustering and patterns |
| 08 | Genome Assembler | Two modes: OLC (reads as nodes) and de Bruijn (k-mers as nodes; show the effect of k on repeats). N50/L50 only with multiple contigs |
| N3 | Experimental Design Lab | **High priority.** Replicates, batch, confounding, randomisation. Fix a flawed study before it's sequenced |
| N4 | Normalisation Arena | Library size, CPM, TPM, size factors. Why raw counts mislead |

### Level 3: Mastery
| ID | Game | Notes / corrections |
|----|------|---------------------|
| 09 | scRNA Explorer | UMAP cluster annotation with marker genes. QC first (doublets, mito fraction). NK (NKG7/GNLY) vs T (CD3E), etc. |
| 10 | DESeq2 Simulator | **Critical:** v1 rewarded tuning thresholds until the objective was met, which is p-hacking. It is now split into **Design mode** (choose n, thresholds and design formula *before* seeing data: a power game) and **Analysis mode** (thresholds locked; interpret). Bonferroni's conservativeness comes from the number of tests, not sample size. Batch in the design formula, independent filtering, lfcShrink, BH vs Bonferroni |
| 11 | Multi-omics Connector | Causal chain: WGS variant → RNA → protein. mRNA-protein correlation is only moderate, so discordance is normal |
| 12 | Tool Terminal | Simulated samtools/bcftools/STAR. Don't teach `-S` in `samtools view -bS` (obsolete). **No string matching:** parse commands into a **virtual file system** and validate the resulting state (any flag order, long flags). Pin and display tool versions. **Never execute real shell commands** |
| N5 | Workflow Forge | Turn a manual pipeline into Snakemake/Nextflow rules. Containers. Reproducibility |

### Platform extras
- BioTrivia quiz packs
- Daily question (seed = hash(date), so no server is needed)
- Learn-mode explainers
- Glossary (50+ terms linked to games)
- Tool flashcards (L2+)
- XP, badges and ranks (Novice → Researcher → Bioinformatician → Principal)
- Leaderboards, share cards, career zone and community submissions (L3)

---

## 6. Shared engine (the core technical decision)

All games are **plugins on one engine** with a fixed loop:

```
scenario = generate(game, difficulty, seed)    // procedural, deterministic
actions  = player.play(scenario)               // recorded as an action log
result   = evaluate(scenario, actions)         // correctness + concept IDs + misconception IDs
feedback = explain(result.misconceptions)      // text from the misconception library
mastery  = update(user, scenario.concepts, result)
bridge   = real_code(game, scenario, actions)  // "Do it for real" panel
```

- The engines are written in **Rust** and compiled to **WebAssembly**, as one crate (`seqly-engine`).
- The **same WASM** runs in the browser and on the server (for score verification).
- They must be **deterministic across machines** and unit-testable.
- `explain` and `update` live outside the engine. The engine returns IDs. The web app maps them to text from `content/`. The mastery model consumes results, and is kept separate because it changes more often and may run server-side.

Plugin interface (sketch; detailed in [PHASE0_PLAN.md](PHASE0_PLAN.md)):
```rust
trait Game {
    fn generate(difficulty: Level, seed: u64) -> Scenario;
    fn evaluate(s: &Scenario, log: &[Action]) -> Evaluation; // score, concepts, misconceptions
    fn bridge(s: &Scenario, log: &[Action]) -> RealCode;
}
```

**Procedural generation uses real algorithms:** real restriction digests, the real codon table, simulated reads with an error model, and BH correction computed live. Scenarios are hand-written only for Case Files and for specific misconception edge cases.

---

## 7. Infrastructure

### 7.1 Static vs dynamic
| Static (CDN, no server) | Dynamic (DB/server) |
|---|---|
| Game engines + UI | Accounts |
| Scenario generation | Attempt events |
| Concept graph, misconception library | Mastery |
| Enzyme DB, codon table, marker panels, tool docs | XP, streaks, badges, leaderboards |
| Glossary, learn mode, flashcards | Instructor classes and dashboards |
| Precomputed datasets | Community submissions, error reports |
| Daily question | Share card images, score replay verification |

### 7.2 Stack
| Concern | Choice | Notes |
|---|---|---|
| Frontend | Next.js (static export where possible) + Tailwind CSS | |
| Engines | Rust → WASM | |
| Hosting | Vercel or Cloudflare Pages | Vercel Hobby is **non-commercial only**, so keep the app portable (static export). Check current pricing and limits before committing |
| DB + auth | Supabase (Postgres + auth + RLS) | Replaces NextAuth and PlanetScale (which dropped its free tier in 2024). **Free projects pause when inactive**, so plan backups |
| File storage | Cloudflare R2 | No egress fees. For larger datasets |
| Notebook mode | Pyodide / JupyterLite | Fully static |
| Monitoring | Sentry + an uptime monitor | |

### 7.3 Data model (core tables)
```sql
users       (id, display_name, created_at, role)                 -- minimal PII
attempts    (id, user_id, game, scenario_seed, difficulty,
             content_version, engine_version, action_log, result, ts)  -- append-only
mastery     (user_id, concept_id, estimate, n_items, updated_at)  -- derived from attempts
classes     (id, instructor_id, code, name)
enrolments  (class_id, user_id)
reports     (id, user_id, game, seed, content_version, text, status)
```
`engine_version` was added to `attempts` (it wasn't in the original sketch) so replays can pick the matching engine build.

### 7.4 Score integrity
- For anything **public** (leaderboards, share cards, instructor grades), the client submits the seed and action log. The server **replays** them through the same WASM engine, and the server's result is authoritative.
- Personal practice skips verification.

### 7.5 Content as code
- Concepts, misconceptions, scenarios, glossary and Case Files live as YAML/Markdown in `content/`.
- CI runs **schema checks** and **science checks**, for example:
  - every digest scenario has a valid solution
  - the correct BLAST answer has the best E-value
  - every referenced misconception or concept ID exists
  - the concept graph is acyclic
- Every content release is **versioned**, and each attempt stores `content_version`.

### 7.6 Offline data pipeline
- The browser **never** calls NCBI/Ensembl/GEO live.
- A Snakemake pipeline in `data-pipeline/` downloads public data, subsets and validates it, and writes small JSON/Parquet files, with the **source and licence recorded**.
- Small files ship with the app. Larger ones go to R2.

### 7.7 Maintenance
- "Report an error" on every round opens a GitHub issue with the game, seed and content_version.
- Renovate/Dependabot.
- Pinned tool versions in the terminal game.
- Weekly `pg_dump` backup to R2 via a GitHub Action.
- Leaderboards are materialised views refreshed on a schedule (no realtime).
- Rate limiting on submissions.

### 7.8 Privacy and compliance
- Minimal PII, no ads, no third-party tracking.
- **GDPR:** data export and account deletion from day one.
- **FERPA** applies if US universities use instructor mode. Instructors see only their own class, enforced by RLS.

### 7.9 Accessibility
- Colour-blind-safe palettes (viridis-style) for gels, heatmaps and volcano plots, **plus a second visual channel** (shape, pattern or label).
- A keyboard/click alternative for every drag-and-drop.
- L1 games are mobile-friendly. Terminal and Pipeline Builder are desktop-first.

### 7.10 Keeping the business model open
- Static-first (near-zero cost) and portable hosting.
- Feature flags around instructor mode, Case Files and Notebook mode.
- The engine is a separate crate, so it can be open-sourced on its own as `seqly-engine`.
- The content licence is likely CC BY-NC (to be confirmed).
- Learning-gain data is collected from day one.

---

## 8. Repository layout

```
bigfly/
  apps/web/            Next.js + Tailwind site
  engine/              Rust crate compiled to WASM (crate name: seqly-engine)
  content/             YAML: concepts, misconceptions, scenarios, glossary, case files
  data-pipeline/       Snakemake pipeline for dataset subsetting
  docs/                ARCHITECTURE.md and design docs
  .github/workflows/   CI: Rust tests, WASM build, content schema + science checks, web build
  CLAUDE.md            project memory for Claude
  README.md
```

---

## 9. Roadmap

**Phase 0: Engine** ← current
Rust/WASM engine core, the Game trait, concept graph v0 (~40 concepts), misconception library v0 and CI science checks. A static site with no DB.
*Exit:* the engine runs DNA Chopper and Sequence Scramble deterministically, and CI catches a deliberately broken scenario.

**Phase 1: Foundation**
DNA Chopper, Sequence Scramble, BLAST Detective, Format Forge, Alignment Grid, the quiz, the glossary and bridge panels. Static only. Progress is stored in localStorage. Sentry.
*Exit:* 50+ real students and a measurable pre/post gain.

**Phase 2: Depth**
Games 04-08, Experimental Design Lab, Normalisation Arena, accounts, the mastery model, XP/badges and a partial Case File 1. Add Supabase, R2, backups and verified scores.

**Phase 3: Mastery**
Games 09-12, Workflow Forge, full Case Files, instructor mode, leaderboards, share cards, Notebook mode and the Tool Map.

### Success metrics
1. Pre/post concept gain (**primary**)
2. 1-week and 1-month retention
3. Misconception recurrence rate
4. Bridge panel click-through
5. Error reports per 1,000 rounds
6. Weekly returning learners (tracked, but never optimised at the expense of learning)

Publication target: *PLOS Computational Biology* (Education) or the *Journal of Open Source Education*.

### Top risks
| Risk | Mitigation |
|---|---|
| Scope is too big for one developer | One engine. Ship 5 polished games first |
| Scientific errors | CI science checks, expert PR review, versioned content |
| Games feel like drills | Case Files. Playtest each game with 5 students |
| Free tiers change | Static-first, portable hosting, our own backups |
