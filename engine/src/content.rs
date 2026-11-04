//! Content schemas, loading and the CI schema + science checks.
//! Only compiled with the `content` feature (never shipped in the WASM build).

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde_json::Value;

use crate::core::registry;
use crate::core::types::{Difficulty, GenParams, Support};

// ---------------------------------------------------------------- schemas

#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct Concept {
    pub id: String,
    pub name: String,
    pub area: String,
    pub level: u8,
    #[serde(default)]
    pub requires: Vec<String>,
    pub summary: String,
    #[serde(default)]
    pub misconceptions: Vec<String>,
    #[serde(default)]
    pub glossary: Vec<String>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct Feedback {
    pub short: String,
    pub explain: String,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct Misconception {
    pub id: String,
    pub statement: String,
    pub concepts: Vec<String>,
    pub feedback: Feedback,
    #[serde(default)]
    pub references: Vec<String>,
    /// Set when a domain expert has reviewed the text.
    #[serde(default)]
    pub reviewed_by: Option<String>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct GlossaryTerm {
    pub id: String,
    pub term: String,
    pub definition: String,
    #[serde(default)]
    pub concepts: Vec<String>,
    #[serde(default)]
    pub see_also: Vec<String>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct WrongAnswer {
    /// Action list (without `t_ms` wrappers).
    pub actions: Vec<Value>,
    pub misconceptions: Vec<String>,
}

#[derive(Deserialize, Debug, Clone, Default)]
#[serde(deny_unknown_fields)]
pub struct Expect {
    /// Author's claimed correct answer. It must evaluate as solved.
    #[serde(default)]
    pub solution: Option<Vec<Value>>,
    /// Wrong answers that must be flagged with (at least) these misconceptions.
    #[serde(default)]
    pub wrong_answers: Vec<WrongAnswer>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct ScenarioFile {
    pub id: String,
    pub game: String,
    pub difficulty: Difficulty,
    #[serde(default)]
    pub support: Support,
    #[serde(default)]
    pub targets_misconception: Option<String>,
    pub body: Value,
    #[serde(default)]
    pub expect: Expect,
}

#[derive(Debug, Default)]
pub struct Content {
    pub concepts: Vec<Concept>,
    pub misconceptions: Vec<Misconception>,
    pub glossary: Vec<GlossaryTerm>,
    pub scenarios: Vec<(PathBuf, ScenarioFile)>,
}

// ---------------------------------------------------------------- loading

fn yaml_files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(entries) = fs::read_dir(dir) else {
        return out;
    };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            out.extend(yaml_files(&p));
        } else if matches!(p.extension().and_then(|x| x.to_str()), Some("yaml" | "yml")) {
            out.push(p);
        }
    }
    out.sort();
    out
}

fn parse<T: serde::de::DeserializeOwned>(path: &Path, errs: &mut Vec<String>) -> Option<T> {
    let text = match fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) => {
            errs.push(format!("{}: {e}", path.display()));
            return None;
        }
    };
    match serde_yaml_ng::from_str(&text) {
        Ok(v) => Some(v),
        Err(e) => {
            errs.push(format!("{}: schema error: {e}", path.display()));
            None
        }
    }
}

/// Load `root/{concepts,misconceptions,glossary}` and scenarios from `scenarios`
/// (a directory or a single file; defaults to `root/scenarios`).
pub fn load(root: &Path, scenarios: Option<&Path>) -> (Content, Vec<String>) {
    let mut errs = Vec::new();
    let mut c = Content::default();
    let list = |sub: &str| yaml_files(&root.join(sub));
    for p in list("concepts") {
        c.concepts
            .extend(parse::<Vec<Concept>>(&p, &mut errs).unwrap_or_default());
    }
    for p in list("misconceptions") {
        c.misconceptions
            .extend(parse::<Vec<Misconception>>(&p, &mut errs).unwrap_or_default());
    }
    for p in list("glossary") {
        c.glossary
            .extend(parse::<Vec<GlossaryTerm>>(&p, &mut errs).unwrap_or_default());
    }
    let scen_root = scenarios
        .map(Path::to_path_buf)
        .unwrap_or_else(|| root.join("scenarios"));
    let files = if scen_root.is_file() {
        vec![scen_root]
    } else {
        yaml_files(&scen_root)
    };
    for p in files {
        if let Some(s) = parse::<ScenarioFile>(&p, &mut errs) {
            c.scenarios.push((p, s));
        }
    }
    (c, errs)
}

// ---------------------------------------------------------------- checks

fn is_snake_id(s: &str) -> bool {
    let mut chars = s.chars();
    matches!(chars.next(), Some('a'..='z'))
        && s.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

fn is_misconception_id(s: &str) -> bool {
    s.len() == 5 && s.starts_with("M-") && s[2..].chars().all(|c| c.is_ascii_digit())
}

fn unique<'a>(
    kind: &str,
    ids: impl Iterator<Item = &'a str>,
    errs: &mut Vec<String>,
) -> BTreeSet<String> {
    let mut seen = BTreeSet::new();
    for id in ids {
        if !seen.insert(id.to_string()) {
            errs.push(format!("duplicate {kind} id '{id}'"));
        }
    }
    seen
}

/// Returns a cycle (as a path of IDs) if the `requires` graph has one.
pub fn find_cycle(edges: &BTreeMap<String, Vec<String>>) -> Option<Vec<String>> {
    #[derive(Clone, Copy, PartialEq)]
    enum Mark {
        Visiting,
        Done,
    }
    fn visit(
        n: &str,
        edges: &BTreeMap<String, Vec<String>>,
        marks: &mut BTreeMap<String, Mark>,
        stack: &mut Vec<String>,
    ) -> Option<Vec<String>> {
        match marks.get(n) {
            Some(Mark::Done) => return None,
            Some(Mark::Visiting) => {
                let start = stack.iter().position(|s| s == n).unwrap_or(0);
                let mut cycle = stack[start..].to_vec();
                cycle.push(n.to_string());
                return Some(cycle);
            }
            None => {}
        }
        marks.insert(n.to_string(), Mark::Visiting);
        stack.push(n.to_string());
        for m in edges.get(n).into_iter().flatten() {
            if let Some(c) = visit(m, edges, marks, stack) {
                return Some(c);
            }
        }
        stack.pop();
        marks.insert(n.to_string(), Mark::Done);
        None
    }
    let mut marks = BTreeMap::new();
    for n in edges.keys() {
        if let Some(c) = visit(n, edges, &mut marks, &mut Vec::new()) {
            return Some(c);
        }
    }
    None
}

fn wrap_log(actions: &[Value]) -> Value {
    Value::Array(
        actions
            .iter()
            .map(|a| serde_json::json!({ "action": a }))
            .collect(),
    )
}

/// Run every schema and science check. Returns human-readable errors (empty = pass).
pub fn check(c: &Content) -> Vec<String> {
    let mut errs = Vec::new();

    // IDs: format and uniqueness.
    for x in &c.concepts {
        if !is_snake_id(&x.id) {
            errs.push(format!("concept id '{}' must be snake_case", x.id));
        }
    }
    for x in &c.misconceptions {
        if !is_misconception_id(&x.id) {
            errs.push(format!("misconception id '{}' must look like M-012", x.id));
        }
    }
    for x in &c.glossary {
        if !is_snake_id(&x.id) {
            errs.push(format!("glossary id '{}' must be snake_case", x.id));
        }
    }
    let concepts = unique(
        "concept",
        c.concepts.iter().map(|x| x.id.as_str()),
        &mut errs,
    );
    let miscs = unique(
        "misconception",
        c.misconceptions.iter().map(|x| x.id.as_str()),
        &mut errs,
    );
    let terms = unique(
        "glossary",
        c.glossary.iter().map(|x| x.id.as_str()),
        &mut errs,
    );
    unique(
        "scenario",
        c.scenarios.iter().map(|(_, s)| s.id.as_str()),
        &mut errs,
    );

    // Referential integrity.
    fn refs(
        errs: &mut Vec<String>,
        kind: &str,
        owner: &str,
        ids: &[String],
        set: &BTreeSet<String>,
    ) {
        for id in ids {
            if !set.contains(id) {
                errs.push(format!("{owner} references unknown {kind} '{id}'"));
            }
        }
    }
    for x in &c.concepts {
        let owner = format!("concept '{}'", x.id);
        refs(&mut errs, "concept", &owner, &x.requires, &concepts);
        refs(
            &mut errs,
            "misconception",
            &owner,
            &x.misconceptions,
            &miscs,
        );
        refs(&mut errs, "glossary term", &owner, &x.glossary, &terms);
    }
    for x in &c.misconceptions {
        let owner = format!("misconception '{}'", x.id);
        refs(&mut errs, "concept", &owner, &x.concepts, &concepts);
        if x.concepts.is_empty() {
            errs.push(format!("{owner} must list at least one concept"));
        }
    }
    for x in &c.glossary {
        let owner = format!("glossary '{}'", x.id);
        refs(&mut errs, "concept", &owner, &x.concepts, &concepts);
        refs(&mut errs, "glossary term", &owner, &x.see_also, &terms);
    }

    // Concept graph must be a DAG.
    let edges: BTreeMap<String, Vec<String>> = c
        .concepts
        .iter()
        .map(|x| (x.id.clone(), x.requires.clone()))
        .collect();
    if let Some(cycle) = find_cycle(&edges) {
        errs.push(format!("concept graph has a cycle: {}", cycle.join(" -> ")));
    }

    // Every ID a game can emit must exist in content.
    for g in registry::games() {
        let m = (g.manifest)();
        for id in &m.concepts {
            if !concepts.contains(*id) {
                errs.push(format!("game '{}' declares unknown concept '{id}'", m.id));
            }
        }
        for id in &m.misconceptions {
            if !miscs.contains(*id) {
                errs.push(format!(
                    "game '{}' declares unknown misconception '{id}'",
                    m.id
                ));
            }
        }
    }

    // Hand-written scenarios: validity + author's claims verified by the engine.
    for (path, s) in &c.scenarios {
        let at = format!("{} ({})", s.id, path.display());
        let g = match registry::find(&s.game) {
            Ok(g) => g,
            Err(e) => {
                errs.push(format!("{at}: {e}"));
                continue;
            }
        };
        let manifest = (g.manifest)();
        if let Some(t) = &s.targets_misconception
            && (!miscs.contains(t) || !manifest.misconceptions.contains(&t.as_str()))
        {
            errs.push(format!(
                "{at}: targets_misconception '{t}' unknown to library or game"
            ));
        }
        let params = GenParams {
            difficulty: s.difficulty,
            support: s.support,
            seed: 0,
            round: None,
        };
        let env = match g.wrap(&params, &s.body) {
            Ok(e) => e,
            Err(e) => {
                errs.push(format!("{at}: {e}"));
                continue;
            }
        };
        for e in g.check(&env).unwrap_or_default() {
            errs.push(format!("{at}: science check: {e}"));
        }
        // The engine's own solution must solve it.
        match g.solve(&env).and_then(|sol| g.evaluate(&env, &sol)) {
            Ok(e) if e.solved => {}
            Ok(_) => errs.push(format!(
                "{at}: scenario is not solvable by the reference solver"
            )),
            Err(e) => errs.push(format!("{at}: {e}")),
        }
        if let Some(sol) = &s.expect.solution {
            match g.evaluate(&env, &wrap_log(sol)) {
                Ok(e) if e.solved => {}
                Ok(_) => errs.push(format!(
                    "{at}: expect.solution is wrong (engine answer: {})",
                    g.solve(&env).map(|v| v.to_string()).unwrap_or_default()
                )),
                Err(e) => errs.push(format!("{at}: expect.solution: {e}")),
            }
        }
        for (i, w) in s.expect.wrong_answers.iter().enumerate() {
            for m in &w.misconceptions {
                if !miscs.contains(m) {
                    errs.push(format!(
                        "{at}: wrong_answers[{i}] references unknown misconception '{m}'"
                    ));
                }
            }
            match g.evaluate(&env, &wrap_log(&w.actions)) {
                Ok(e) if e.solved => {
                    errs.push(format!("{at}: wrong_answers[{i}] is actually correct"))
                }
                Ok(e) => {
                    for m in &w.misconceptions {
                        if !e.misconceptions.contains(m) {
                            errs.push(format!(
                                "{at}: wrong_answers[{i}] not flagged with {m} (engine flagged {:?})",
                                e.misconceptions
                            ));
                        }
                    }
                }
                Err(e) => errs.push(format!("{at}: wrong_answers[{i}]: {e}")),
            }
        }
    }
    errs
}

/// Non-fatal notes (e.g. misconceptions awaiting expert review).
pub fn warnings(c: &Content) -> Vec<String> {
    let unreviewed: Vec<&str> = c
        .misconceptions
        .iter()
        .filter(|m| m.reviewed_by.is_none())
        .map(|m| m.id.as_str())
        .collect();
    if unreviewed.is_empty() {
        vec![]
    } else {
        vec![format!(
            "{} misconception(s) not yet expert-reviewed: {}",
            unreviewed.len(),
            unreviewed.join(", ")
        )]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_cycle_with_path() {
        let mut e = BTreeMap::new();
        e.insert("a".to_string(), vec!["b".to_string()]);
        e.insert("b".to_string(), vec!["c".to_string()]);
        e.insert("c".to_string(), vec!["a".to_string()]);
        let cycle = find_cycle(&e).unwrap();
        assert_eq!(cycle.first(), cycle.last());
        assert_eq!(cycle.len(), 4);
    }

    #[test]
    fn accepts_dag() {
        let mut e = BTreeMap::new();
        e.insert("a".to_string(), vec!["b".to_string(), "c".to_string()]);
        e.insert("b".to_string(), vec!["c".to_string()]);
        e.insert("c".to_string(), vec![]);
        assert!(find_cycle(&e).is_none());
    }

    #[test]
    fn id_formats() {
        assert!(is_snake_id("fdr"));
        assert!(is_snake_id("p_value2"));
        assert!(!is_snake_id("FDR"));
        assert!(!is_snake_id("2fdr"));
        assert!(is_misconception_id("M-012"));
        assert!(!is_misconception_id("M-12"));
        assert!(!is_misconception_id("X-012"));
    }
}
