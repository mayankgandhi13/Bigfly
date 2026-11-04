# content/

Learning content as code. CI validates everything here with schema checks and science checks.

| Dir | What |
|---|---|
| `concepts/` | Concept graph nodes (must form a DAG) |
| `misconceptions/` | Misconception library (`M-###`) with feedback text |
| `scenarios/` | Hand-written scenarios (Case Files, misconception edge cases) |
| `glossary/` | Glossary terms linked to concepts and games |
| `case-files/` | Case File manifests |

`VERSION` holds the content release version. Every attempt records it.
Schemas: Rust structs in `engine/src/content.rs` (unknown fields are rejected). Check locally:

```sh
cd engine && cargo run --features content --bin content-check -- ../content
```

`tests/broken/` holds scenarios that are **meant** to fail. CI checks that each one is rejected.

Misconception text needs expert review before release. Set `reviewed_by` once it's been reviewed.
