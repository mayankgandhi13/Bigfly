# data-pipeline/

Offline Snakemake pipeline. It downloads public data (GEO, Ensembl, NCBI, 10x PBMC3k, and so on), subsets and validates it, and writes small JSON/Parquet files with the source and licence recorded. Small outputs ship with the app; larger ones go to Cloudflare R2.

The browser never calls these sources live.

Status: not started. It is needed from Phase 1 onwards for the bridge-panel datasets.
