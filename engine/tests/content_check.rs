//! The real content must pass every check; each deliberately broken scenario must fail.
#![cfg(feature = "content")]

use std::path::PathBuf;

use seqly_engine::content;

fn content_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../content")
}

#[test]
fn real_content_passes() {
    let (c, mut errs) = content::load(&content_dir(), None);
    errs.extend(content::check(&c));
    assert!(errs.is_empty(), "content errors:\n{}", errs.join("\n"));
    assert!(
        c.concepts.len() >= 40,
        "concept graph v0 should have ~40+ concepts"
    );
    assert!(!c.scenarios.is_empty());
}

#[test]
fn each_broken_scenario_fails() {
    let dir = content_dir().join("tests/broken");
    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "yaml"))
        .collect();
    files.sort();
    assert!(!files.is_empty());
    for f in files {
        let (c, mut errs) = content::load(&content_dir(), Some(&f));
        errs.extend(content::check(&c));
        assert!(
            !errs.is_empty(),
            "{} should fail the content check",
            f.display()
        );
    }
}
