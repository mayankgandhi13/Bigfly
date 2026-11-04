BIGFLY: source code for Seqly.bio, a bioinformatics learning platform.

Students learn bioinformatics by playing games that simulate real analyses, from restriction digests and BLAST to QC, pipelines, variant calling, DESeq2 and scRNA-seq. Under every game there is a concept graph, a learner model and a misconception library. Each round ends with the real code and a small real dataset.

## Repository layout

| Path | What |
|---|---|
| `apps/web/` | Next.js + Tailwind site |
| `engine/` | Rust simulation engines compiled to WebAssembly (`seqly-engine`) |
| `content/` | Concepts, misconceptions, scenarios, glossary and Case Files (YAML) |
| `data-pipeline/` | Snakemake pipeline that subsets public datasets |
| `docs/` | [Architecture](docs/ARCHITECTURE.md) and design docs |

## Status

Phase 0 (engine core). See [docs/PHASE0_PLAN.md](docs/PHASE0_PLAN.md).

## Development

```sh
cd engine
cargo test
cargo build --target wasm32-unknown-unknown
```
