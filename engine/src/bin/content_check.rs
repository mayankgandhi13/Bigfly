//! CI content checker.
//!
//! Usage: content-check <content_dir> [--scenarios <dir-or-file>]
//! Exits non-zero if any schema or science check fails.

use std::path::PathBuf;
use std::process::ExitCode;

use seqly_engine::content;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(root) = args.first().map(PathBuf::from) else {
        eprintln!("usage: content-check <content_dir> [--scenarios <dir-or-file>]");
        return ExitCode::from(2);
    };
    let scenarios = args
        .iter()
        .position(|a| a == "--scenarios")
        .and_then(|i| args.get(i + 1))
        .map(PathBuf::from);

    let (c, mut errs) = content::load(&root, scenarios.as_deref());
    errs.extend(content::check(&c));
    for w in content::warnings(&c) {
        println!("warning: {w}");
    }
    println!(
        "loaded {} concepts, {} misconceptions, {} glossary terms, {} scenarios",
        c.concepts.len(),
        c.misconceptions.len(),
        c.glossary.len(),
        c.scenarios.len()
    );
    if errs.is_empty() {
        println!("content OK");
        ExitCode::SUCCESS
    } else {
        for e in &errs {
            eprintln!("error: {e}");
        }
        eprintln!("{} error(s)", errs.len());
        ExitCode::FAILURE
    }
}
