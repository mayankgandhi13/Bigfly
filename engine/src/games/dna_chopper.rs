//! 01 DNA Chopper: restriction sites, digests, gel patterns, sticky vs blunt ends, ligation.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::bio::digest::{self, Topology};
use crate::bio::enzymes::{self, EndKind, Enzyme, PANEL};
use crate::bio::seq::{is_dna, random_dna};
use crate::core::game::Game;
use crate::core::rng::Rng;
use crate::core::types::{
    ConceptId, Difficulty, Evaluation, GameManifest, GenParams, Lang, RealCode, ToolVersion,
};
use crate::games::common::{first_answer, owned};

pub struct DnaChopper;

// Misconceptions this game can detect (text lives in content/misconceptions/).
pub const M_CUT_AT_SITE_START: &str = "M-010";
pub const M_FRAGMENT_COUNT_TOPOLOGY: &str = "M-011";
pub const M_ALL_STICKY: &str = "M-013";
pub const M_ALWAYS_SAME_POSITION: &str = "M-014";
pub const M_STICKY_ALWAYS_5P: &str = "M-015";
pub const M_POLARITY_ARBITRARY: &str = "M-016";
pub const M_ONLY_SAME_ENZYME_LIGATES: &str = "M-017";
pub const M_BLUNT_CANT_LIGATE: &str = "M-018";
pub const M_ANY_STICKY_LIGATES: &str = "M-020";
pub const M_STICKY_TO_BLUNT: &str = "M-021";

const ROUNDS: [&str; 5] = [
    "mark_cuts",
    "predict_fragments",
    "end_type",
    "ligation",
    "gel_match",
];

/// Largest sequence for the mark-cuts round (it is displayed base by base).
const MARK_CUTS_MAX_LEN: usize = 200;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Scenario {
    pub sequence: String,
    pub topology: Topology,
    pub task: Task,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "round", rename_all = "snake_case")]
pub enum Task {
    /// Mark every top-strand cut position.
    MarkCuts { enzymes: Vec<String> },
    /// Predict fragment sizes.
    PredictFragments { enzymes: Vec<String> },
    /// Classify the ends an enzyme leaves.
    EndType { enzyme: String },
    /// Can an end cut by `end_a` be ligated directly to one cut by `end_b`?
    Ligation { end_a: String, end_b: String },
    /// Choose enzyme(s) from the panel that reproduce the target gel. Any set that does is correct.
    GelMatch {
        panel: Vec<String>,
        target_sizes: Vec<u32>,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Action {
    Hint,
    ToggleCut { position: u32 },
    SubmitCuts,
    SubmitFragments { sizes: Vec<u32> },
    ChooseEndType { end: EndKind },
    ChooseLigation { ligates: bool },
    ChooseEnzymes { enzymes: Vec<String> },
}

impl Action {
    fn is_answer(&self) -> bool {
        !matches!(self, Action::Hint | Action::ToggleCut { .. })
    }
}

impl crate::games::common::HintAction for Action {
    fn is_hint(&self) -> bool {
        matches!(self, Action::Hint)
    }
}

fn resolve(names: &[String]) -> Vec<&'static Enzyme> {
    names.iter().filter_map(|n| enzymes::find(n)).collect()
}

fn enzyme(name: &str) -> Option<&'static Enzyme> {
    enzymes::find(name)
}

fn sorted_desc(mut v: Vec<u32>) -> Vec<u32> {
    v.sort_unstable_by(|a, b| b.cmp(a));
    v
}

// ---------------------------------------------------------------- generation

/// Random background with no panel sites, so every cut comes from a planted site.
/// Each pass mutates the first base of every site found, until none remain.
fn clean_background(rng: &mut Rng, len: usize, topology: Topology) -> String {
    let mut seq = random_dna(rng, len);
    for _ in 0..100 {
        let mut hits: Vec<u32> = PANEL
            .iter()
            .flat_map(|e| digest::find_sites(&seq, e, topology))
            .collect();
        if hits.is_empty() {
            return seq;
        }
        hits.sort_unstable();
        hits.dedup();
        let mut bytes = seq.into_bytes();
        for i in hits {
            let i = i as usize;
            let old = bytes[i];
            let choices: Vec<u8> = b"ACGT".iter().copied().filter(|&b| b != old).collect();
            bytes[i] = *rng.choose(&choices);
        }
        seq = String::from_utf8(bytes).expect("ascii");
    }
    unreachable!("background cleaning did not converge")
}

/// Plant one site per entry, each in its own bin of the sequence, away from the ends.
fn plant(rng: &mut Rng, seq: &mut String, sites: &[&str]) {
    if sites.is_empty() {
        return;
    }
    let len = seq.len();
    let bin = len / sites.len();
    let margin = 3;
    let mut bytes = std::mem::take(seq).into_bytes();
    for (i, site) in sites.iter().enumerate() {
        let lo = i * bin + margin;
        let hi = (i + 1) * bin - site.len() - margin;
        assert!(lo <= hi, "sequence too short to plant sites");
        let pos = rng.range(lo as u32, hi as u32) as usize;
        bytes[pos..pos + site.len()].copy_from_slice(site.as_bytes());
    }
    *seq = String::from_utf8(bytes).expect("ascii");
}

fn pick_enzymes(rng: &mut Rng, n: usize) -> Vec<&'static Enzyme> {
    let mut all: Vec<&'static Enzyme> = PANEL.iter().collect();
    rng.shuffle(&mut all);
    all.truncate(n);
    all
}

fn names(es: &[&'static Enzyme]) -> Vec<String> {
    es.iter().map(|e| e.name.to_string()).collect()
}

/// Build a sequence where each enzyme in `es` has `sites_each` planted sites.
fn sequence_with_sites(
    rng: &mut Rng,
    len: usize,
    topology: Topology,
    es: &[&'static Enzyme],
    sites_each: impl Fn(&mut Rng) -> u32,
) -> String {
    let mut seq = clean_background(rng, len, topology);
    let mut sites: Vec<&str> = Vec::new();
    for e in es {
        for _ in 0..sites_each(rng) {
            sites.push(e.site);
        }
    }
    rng.shuffle(&mut sites);
    plant(rng, &mut seq, &sites);
    seq
}

fn generate_round(round: &str, d: Difficulty, rng: &mut Rng) -> Scenario {
    use Difficulty::*;
    match round {
        "mark_cuts" => {
            let (len, n_enz) = match d {
                Intro => (rng.range(40, 60), 1),
                Standard => (rng.range(60, 90), rng.range(1, 2)),
                Advanced => (rng.range(80, 120), 2),
            };
            let es = pick_enzymes(rng, n_enz as usize);
            let seq =
                sequence_with_sites(rng, len as usize, Topology::Linear, &es, |r| r.range(1, 2));
            Scenario {
                sequence: seq,
                topology: Topology::Linear,
                task: Task::MarkCuts {
                    enzymes: names(&es),
                },
            }
        }
        "predict_fragments" => {
            let (len, topology, n_enz, max_sites) = match d {
                Intro => (rng.range(60, 150), Topology::Linear, 1, 2),
                Standard => {
                    let t = if rng.chance(1, 2) {
                        Topology::Circular
                    } else {
                        Topology::Linear
                    };
                    (rng.range(200, 800), t, rng.range(1, 2), 2)
                }
                Advanced => {
                    let t = if rng.chance(3, 4) {
                        Topology::Circular
                    } else {
                        Topology::Linear
                    };
                    (rng.range(1000, 5000), t, 2, 3)
                }
            };
            let es = pick_enzymes(rng, n_enz as usize);
            let seq =
                sequence_with_sites(rng, len as usize, topology, &es, |r| r.range(1, max_sites));
            Scenario {
                sequence: seq,
                topology,
                task: Task::PredictFragments {
                    enzymes: names(&es),
                },
            }
        }
        "end_type" => {
            let e = rng.choose(&PANEL);
            let seq = sequence_with_sites(rng, 36, Topology::Linear, &[e], |_| 1);
            Scenario {
                sequence: seq,
                topology: Topology::Linear,
                task: Task::EndType {
                    enzyme: e.name.into(),
                },
            }
        }
        "ligation" => {
            let sticky: Vec<&'static Enzyme> = PANEL
                .iter()
                .filter(|e| e.end_kind() != EndKind::Blunt)
                .collect();
            let haeiii = enzyme("HaeIII").expect("panel");
            let (a, b) = match rng.below(4) {
                0 => {
                    let pairs = [("BamHI", "BglII"), ("SalI", "XhoI")];
                    let (x, y) = *rng.choose(&pairs);
                    (enzyme(x).expect("panel"), enzyme(y).expect("panel"))
                }
                1 => (haeiii, haeiii),
                2 => loop {
                    let x = *rng.choose(&sticky);
                    let y = *rng.choose(&sticky);
                    if x != y && !enzymes::compatible(x, y) {
                        break (x, y);
                    }
                },
                _ => (*rng.choose(&sticky), haeiii),
            };
            let (a, b) = if rng.chance(1, 2) { (a, b) } else { (b, a) };
            let seq = sequence_with_sites(rng, 60, Topology::Linear, &[a, b], |_| 1);
            Scenario {
                sequence: seq,
                topology: Topology::Linear,
                task: Task::Ligation {
                    end_a: a.name.into(),
                    end_b: b.name.into(),
                },
            }
        }
        "gel_match" => {
            let topology = if rng.chance(1, 2) {
                Topology::Circular
            } else {
                Topology::Linear
            };
            let len = rng.range(1500, 6000) as usize;
            let panel = pick_enzymes(rng, 4);
            let seq = sequence_with_sites(rng, len, topology, &panel, |r| r.range(1, 2));
            let n_hidden = if d == Advanced { rng.range(1, 2) } else { 1 } as usize;
            let hidden = &panel[..n_hidden];
            let target_sizes = digest::fragment_sizes(&seq, hidden, topology);
            let mut panel_names = names(&panel);
            panel_names.sort();
            Scenario {
                sequence: seq,
                topology,
                task: Task::GelMatch {
                    panel: panel_names,
                    target_sizes,
                },
            }
        }
        other => unreachable!("round validated by registry: {other}"),
    }
}

fn rounds_for(d: Difficulty) -> &'static [&'static str] {
    match d {
        Difficulty::Intro => &ROUNDS[..3],
        Difficulty::Standard => &ROUNDS[..4],
        Difficulty::Advanced => &ROUNDS,
    }
}

// ---------------------------------------------------------------- evaluation

fn true_cut_positions(s: &Scenario, names: &[String]) -> BTreeSet<u32> {
    digest::cuts(&s.sequence, &resolve(names), s.topology)
        .iter()
        .map(|c| c.position)
        .collect()
}

fn end_type_misconception(truth: EndKind, chosen: EndKind) -> Option<&'static str> {
    use EndKind::*;
    match (truth, chosen) {
        (Blunt, FivePrime | ThreePrime) => Some(M_ALL_STICKY),
        (FivePrime | ThreePrime, Blunt) => Some(M_ALWAYS_SAME_POSITION),
        (ThreePrime, FivePrime) => Some(M_STICKY_ALWAYS_5P),
        (FivePrime, ThreePrime) => Some(M_POLARITY_ARBITRARY),
        _ => None,
    }
}

fn ligation_misconception(a: &Enzyme, b: &Enzyme, said_ligates: bool) -> Option<&'static str> {
    let truth = enzymes::compatible(a, b);
    if said_ligates == truth {
        return None;
    }
    let blunt_a = a.end_kind() == EndKind::Blunt;
    let blunt_b = b.end_kind() == EndKind::Blunt;
    match (truth, blunt_a, blunt_b) {
        (true, true, true) => Some(M_BLUNT_CANT_LIGATE),
        (true, false, false) if a.name != b.name => Some(M_ONLY_SAME_ENZYME_LIGATES),
        (false, false, false) => Some(M_ANY_STICKY_LIGATES),
        (false, _, _) => Some(M_STICKY_TO_BLUNT),
        _ => None,
    }
}

/// Smallest panel subset (≤ 2 enzymes, panel order) that reproduces the target gel.
fn gel_match_solution(s: &Scenario, panel: &[String], target: &[u32]) -> Option<Vec<String>> {
    let es = resolve(panel);
    let matches =
        |set: &[&'static Enzyme]| digest::fragment_sizes(&s.sequence, set, s.topology) == target;
    for e in &es {
        if matches(&[e]) {
            return Some(vec![e.name.to_string()]);
        }
    }
    for i in 0..es.len() {
        for j in i + 1..es.len() {
            if matches(&[es[i], es[j]]) {
                return Some(vec![es[i].name.to_string(), es[j].name.to_string()]);
            }
        }
    }
    None
}

// ---------------------------------------------------------------- bridge

const BIOPYTHON: &str = "1.85";
const DOC_RESTRICTION: &str = "https://biopython.org/docs/latest/api/Bio.Restriction.html";

fn py_list(names: &[String]) -> String {
    let quoted: Vec<String> = names.iter().map(|n| format!("\"{n}\"")).collect();
    format!("[{}]", quoted.join(", "))
}

fn digest_code(seq: &str, topology: Topology, names: &[String]) -> String {
    let linear = if topology == Topology::Linear {
        "True"
    } else {
        "False"
    };
    let sizes = if topology == Topology::Linear {
        "bounds = [1] + cuts + [len(seq) + 1]\nsizes = [b - a for a, b in zip(bounds, bounds[1:])]"
    } else {
        "# Circular: n cuts give n fragments\nsizes = [(cuts[(i + 1) % len(cuts)] - c) % len(seq) or len(seq) for i, c in enumerate(cuts)]"
    };
    format!(
        "# Biopython {BIOPYTHON}\n\
         from Bio.Seq import Seq\n\
         from Bio.Restriction import RestrictionBatch\n\n\
         seq = Seq(\"{seq}\")\n\
         enzymes = RestrictionBatch({list})\n\n\
         # 1-based position of the first base after each top-strand cut\n\
         sites = enzymes.search(seq, linear={linear})\n\
         print(sites)\n\n\
         cuts = sorted({{pos for hits in sites.values() for pos in hits}})\n\
         {sizes}\n\
         print(sorted(sizes, reverse=True))\n",
        list = py_list(names),
    )
}

fn real_code(code: String) -> RealCode {
    RealCode {
        language: Lang::Python,
        code,
        tool_versions: vec![ToolVersion {
            tool: "Biopython".into(),
            version: BIOPYTHON.into(),
        }],
        dataset: None,
        doc_url: Some(DOC_RESTRICTION.into()),
    }
}

// ---------------------------------------------------------------- Game impl

impl Game for DnaChopper {
    const ID: &'static str = "dna_chopper";
    type Scenario = Scenario;
    type Action = Action;

    fn manifest() -> GameManifest {
        GameManifest {
            id: Self::ID,
            name: "DNA Chopper",
            rounds: ROUNDS.to_vec(),
            concepts: vec![
                "restriction_site",
                "palindromic_site",
                "cut_position",
                "restriction_digest",
                "fragment_size",
                "dna_topology",
                "sticky_end",
                "blunt_end",
                "overhang_polarity",
                "compatible_ends",
                "ligation",
                "gel_electrophoresis",
                "restriction_mapping",
            ],
            misconceptions: vec![
                M_CUT_AT_SITE_START,
                M_FRAGMENT_COUNT_TOPOLOGY,
                M_ALL_STICKY,
                M_ALWAYS_SAME_POSITION,
                M_STICKY_ALWAYS_5P,
                M_POLARITY_ARBITRARY,
                M_ONLY_SAME_ENZYME_LIGATES,
                M_BLUNT_CANT_LIGATE,
                M_ANY_STICKY_LIGATES,
                M_STICKY_TO_BLUNT,
            ],
        }
    }

    fn generate(params: &GenParams, rng: &mut Rng) -> Scenario {
        let round = match &params.round {
            Some(r) => r.clone(),
            None => rng.choose(rounds_for(params.difficulty)).to_string(),
        };
        generate_round(&round, params.difficulty, rng)
    }

    fn concepts(s: &Scenario) -> Vec<ConceptId> {
        let mut c = match &s.task {
            Task::MarkCuts { .. } => {
                owned(&["restriction_site", "palindromic_site", "cut_position"])
            }
            Task::PredictFragments { .. } => owned(&["restriction_digest", "fragment_size"]),
            Task::EndType { .. } => owned(&["sticky_end", "blunt_end", "overhang_polarity"]),
            Task::Ligation { .. } => owned(&["ligation", "compatible_ends"]),
            Task::GelMatch { .. } => owned(&[
                "gel_electrophoresis",
                "restriction_digest",
                "restriction_mapping",
            ]),
        };
        if s.topology == Topology::Circular
            && !matches!(s.task, Task::EndType { .. } | Task::Ligation { .. })
        {
            c.push("dna_topology".into());
        }
        c
    }

    fn evaluate(s: &Scenario, log: &[Action]) -> Evaluation {
        let concepts = Self::concepts(s);
        let (hints, answer) = first_answer(log, Action::is_answer);
        let mut misc: Vec<String> = Vec::new();

        let solved = match (&s.task, answer) {
            (Task::MarkCuts { enzymes }, Some(Action::SubmitCuts)) => {
                let mut marks = BTreeSet::new();
                for a in log.iter().take_while(|a| !a.is_answer()) {
                    if let Action::ToggleCut { position } = a
                        && !marks.remove(position)
                    {
                        marks.insert(*position);
                    }
                }
                let truth = true_cut_positions(s, enzymes);
                let site_starts: BTreeSet<u32> = resolve(enzymes)
                    .iter()
                    .flat_map(|e| digest::find_sites(&s.sequence, e, s.topology))
                    .collect();
                if marks != truth
                    && marks
                        .iter()
                        .any(|m| site_starts.contains(m) && !truth.contains(m))
                {
                    misc.push(M_CUT_AT_SITE_START.into());
                }
                marks == truth
            }
            (Task::PredictFragments { enzymes }, Some(Action::SubmitFragments { sizes })) => {
                let truth = digest::fragment_sizes(&s.sequence, &resolve(enzymes), s.topology);
                let n_cuts = digest::cuts(&s.sequence, &resolve(enzymes), s.topology).len();
                let wrong_count_for_topology = match s.topology {
                    Topology::Circular => sizes.len() == n_cuts + 1,
                    Topology::Linear => sizes.len() == n_cuts,
                };
                let solved = sorted_desc(sizes.clone()) == truth;
                if !solved && wrong_count_for_topology {
                    misc.push(M_FRAGMENT_COUNT_TOPOLOGY.into());
                }
                solved
            }
            (Task::EndType { enzyme: name }, Some(Action::ChooseEndType { end })) => {
                match enzyme(name) {
                    Some(e) => {
                        if let Some(m) = end_type_misconception(e.end_kind(), *end) {
                            misc.push(m.into());
                        }
                        e.end_kind() == *end
                    }
                    None => false,
                }
            }
            (Task::Ligation { end_a, end_b }, Some(Action::ChooseLigation { ligates })) => {
                match (enzyme(end_a), enzyme(end_b)) {
                    (Some(a), Some(b)) => {
                        if let Some(m) = ligation_misconception(a, b, *ligates) {
                            misc.push(m.into());
                        }
                        enzymes::compatible(a, b) == *ligates
                    }
                    _ => false,
                }
            }
            (
                Task::GelMatch {
                    panel,
                    target_sizes,
                },
                Some(Action::ChooseEnzymes { enzymes }),
            ) => {
                let chosen: Vec<String> = enzymes
                    .iter()
                    .filter(|e| panel.contains(e))
                    .cloned()
                    .collect();
                !chosen.is_empty()
                    && chosen.len() == enzymes.len()
                    && digest::fragment_sizes(&s.sequence, &resolve(&chosen), s.topology)
                        == *target_sizes
            }
            _ => false,
        };
        Evaluation::new(solved, hints, &concepts, misc)
    }

    fn solve(s: &Scenario) -> Vec<Action> {
        match &s.task {
            Task::MarkCuts { enzymes } => {
                let mut v: Vec<Action> = true_cut_positions(s, enzymes)
                    .into_iter()
                    .map(|position| Action::ToggleCut { position })
                    .collect();
                v.push(Action::SubmitCuts);
                v
            }
            Task::PredictFragments { enzymes } => vec![Action::SubmitFragments {
                sizes: digest::fragment_sizes(&s.sequence, &resolve(enzymes), s.topology),
            }],
            Task::EndType { enzyme: name } => match enzyme(name) {
                Some(e) => vec![Action::ChooseEndType { end: e.end_kind() }],
                None => vec![],
            },
            Task::Ligation { end_a, end_b } => match (enzyme(end_a), enzyme(end_b)) {
                (Some(a), Some(b)) => vec![Action::ChooseLigation {
                    ligates: enzymes::compatible(a, b),
                }],
                _ => vec![],
            },
            Task::GelMatch {
                panel,
                target_sizes,
            } => match gel_match_solution(s, panel, target_sizes) {
                Some(enzymes) => vec![Action::ChooseEnzymes { enzymes }],
                None => vec![],
            },
        }
    }

    fn choices(s: &Scenario) -> Option<Vec<Action>> {
        match &s.task {
            Task::EndType { .. } => Some(
                EndKind::ALL
                    .iter()
                    .map(|&end| Action::ChooseEndType { end })
                    .collect(),
            ),
            Task::Ligation { .. } => Some(vec![
                Action::ChooseLigation { ligates: true },
                Action::ChooseLigation { ligates: false },
            ]),
            _ => None,
        }
    }

    fn bridge(s: &Scenario, log: &[Action]) -> RealCode {
        match &s.task {
            Task::MarkCuts { enzymes } | Task::PredictFragments { enzymes } => {
                real_code(digest_code(&s.sequence, s.topology, enzymes))
            }
            Task::GelMatch {
                panel,
                target_sizes,
            } => {
                // Use the player's chosen enzymes if they made a choice; otherwise a valid answer.
                let chosen = match first_answer(log, Action::is_answer).1 {
                    Some(Action::ChooseEnzymes { enzymes }) => enzymes.clone(),
                    _ => gel_match_solution(s, panel, target_sizes).unwrap_or_default(),
                };
                real_code(digest_code(&s.sequence, s.topology, &chosen))
            }
            Task::EndType { enzyme: name } => real_code(format!(
                "# Biopython {BIOPYTHON}\n\
                 from Bio.Restriction import {name}\n\n\
                 print({name}.elucidate())   # ^ = top-strand cut, _ = bottom-strand cut\n\
                 print(\"5' overhang:\", {name}.is_5overhang())\n\
                 print(\"3' overhang:\", {name}.is_3overhang())\n\
                 print(\"blunt:\", {name}.is_blunt())\n\
                 print(\"overhang sequence:\", {name}.ovhgseq)\n"
            )),
            Task::Ligation { end_a, end_b } => {
                let import = if end_a == end_b {
                    end_a.clone()
                } else {
                    format!("{end_a}, {end_b}")
                };
                real_code(format!(
                    "# Biopython {BIOPYTHON}\n\
                     from Bio.Restriction import {import}\n\n\
                     print({end_a}.elucidate(), {end_b}.elucidate())\n\
                     # Enzymes whose ends can be ligated directly to {end_a} ends:\n\
                     print({end_b} in {end_a}.compatible_end())\n"
                ))
            }
        }
    }

    fn check(s: &Scenario) -> Vec<String> {
        let mut errs = Vec::new();
        if !is_dna(&s.sequence) {
            errs.push("sequence must be non-empty uppercase DNA (ACGT)".into());
        }
        let known = |names: &[String], errs: &mut Vec<String>| {
            for n in names {
                if enzyme(n).is_none() {
                    errs.push(format!("enzyme '{n}' is not in the panel"));
                }
            }
        };
        match &s.task {
            Task::MarkCuts { enzymes } | Task::PredictFragments { enzymes } => {
                known(enzymes, &mut errs);
                if enzymes.is_empty() {
                    errs.push("at least one enzyme required".into());
                } else if digest::cuts(&s.sequence, &resolve(enzymes), s.topology).is_empty() {
                    errs.push("enzymes do not cut this sequence".into());
                }
                if matches!(s.task, Task::MarkCuts { .. }) && s.sequence.len() > MARK_CUTS_MAX_LEN {
                    errs.push(format!(
                        "mark_cuts sequence longer than {MARK_CUTS_MAX_LEN} bp"
                    ));
                }
            }
            Task::EndType { enzyme: name } => known(std::slice::from_ref(name), &mut errs),
            Task::Ligation { end_a, end_b } => known(&[end_a.clone(), end_b.clone()], &mut errs),
            Task::GelMatch {
                panel,
                target_sizes,
            } => {
                known(panel, &mut errs);
                if target_sizes.iter().sum::<u32>() != s.sequence.len() as u32 {
                    errs.push("target_sizes do not sum to the sequence length".into());
                }
                if *target_sizes != sorted_desc(target_sizes.clone()) {
                    errs.push("target_sizes must be sorted largest first".into());
                }
                if gel_match_solution(s, panel, target_sizes).is_none() {
                    errs.push("no enzyme set from the panel reproduces target_sizes".into());
                }
            }
        }
        errs
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scenario(task: Task, seq: &str, topology: Topology) -> Scenario {
        Scenario {
            sequence: seq.into(),
            topology,
            task,
        }
    }

    #[test]
    fn circular_counted_as_linear_flags_misconception() {
        let s = scenario(
            Task::PredictFragments {
                enzymes: vec!["EcoRI".into()],
            },
            "GAATTCAAAAAAGAATTCAAAAAAAAAA",
            Topology::Circular,
        );
        let ok = DnaChopper::evaluate(
            &s,
            &[Action::SubmitFragments {
                sizes: vec![12, 16],
            }],
        );
        assert!(ok.solved);
        // Linear-style answer: three fragments.
        let bad = DnaChopper::evaluate(
            &s,
            &[Action::SubmitFragments {
                sizes: vec![1, 12, 15],
            }],
        );
        assert!(!bad.solved);
        assert_eq!(bad.misconceptions, vec![M_FRAGMENT_COUNT_TOPOLOGY]);
    }

    #[test]
    fn marking_site_start_flags_misconception() {
        let s = scenario(
            Task::MarkCuts {
                enzymes: vec!["EcoRI".into()],
            },
            "AAAGAATTCAAA",
            Topology::Linear,
        );
        let good =
            DnaChopper::evaluate(&s, &[Action::ToggleCut { position: 4 }, Action::SubmitCuts]);
        assert!(good.solved);
        let bad =
            DnaChopper::evaluate(&s, &[Action::ToggleCut { position: 3 }, Action::SubmitCuts]);
        assert_eq!(bad.misconceptions, vec![M_CUT_AT_SITE_START]);
        // Toggling twice removes the mark.
        let toggled = DnaChopper::evaluate(
            &s,
            &[
                Action::ToggleCut { position: 3 },
                Action::ToggleCut { position: 3 },
                Action::ToggleCut { position: 4 },
                Action::SubmitCuts,
            ],
        );
        assert!(toggled.solved);
    }

    #[test]
    fn first_answer_counts_and_hints_cost_points() {
        let s = scenario(
            Task::EndType {
                enzyme: "PstI".into(),
            },
            "AAACTGCAGAAA",
            Topology::Linear,
        );
        let wrong_then_right = DnaChopper::evaluate(
            &s,
            &[
                Action::ChooseEndType {
                    end: EndKind::FivePrime,
                },
                Action::ChooseEndType {
                    end: EndKind::ThreePrime,
                },
            ],
        );
        assert!(!wrong_then_right.solved);
        assert_eq!(wrong_then_right.misconceptions, vec![M_STICKY_ALWAYS_5P]);
        let hinted = DnaChopper::evaluate(
            &s,
            &[
                Action::Hint,
                Action::ChooseEndType {
                    end: EndKind::ThreePrime,
                },
            ],
        );
        assert!(hinted.solved);
        assert_eq!(hinted.score, 80);
        assert_eq!(hinted.concepts[0].credit_permille, 900);
    }

    #[test]
    fn gel_match_accepts_any_valid_set() {
        // One EcoRI site (cut at 7) in a 28 bp linear molecule → 21 + 7.
        let seq = "AAAAAAGAATTCAAAAAAAAAAAAAAAA";
        let s = scenario(
            Task::GelMatch {
                panel: vec!["EcoRI".into(), "BamHI".into()],
                target_sizes: vec![21, 7],
            },
            seq,
            Topology::Linear,
        );
        assert!(
            DnaChopper::check(&s).is_empty(),
            "{:?}",
            DnaChopper::check(&s)
        );
        let e = DnaChopper::evaluate(
            &s,
            &[Action::ChooseEnzymes {
                enzymes: vec!["EcoRI".into()],
            }],
        );
        assert!(e.solved);
        // BamHI doesn't cut: adding it still reproduces the gel, so it's also valid.
        let e2 = DnaChopper::evaluate(
            &s,
            &[Action::ChooseEnzymes {
                enzymes: vec!["EcoRI".into(), "BamHI".into()],
            }],
        );
        assert!(e2.solved);
    }

    #[test]
    fn check_rejects_bad_hand_written_scenarios() {
        let s = scenario(
            Task::PredictFragments {
                enzymes: vec!["NotI".into()],
            },
            "AAAA",
            Topology::Linear,
        );
        assert!(!DnaChopper::check(&s).is_empty());
    }
}
