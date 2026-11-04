//! 02 Sequence Scramble: base pairing, reverse complement, transcription, translation, frameshifts.

use serde::{Deserialize, Serialize};

use crate::bio::codon::{STOP_CODONS, random_sense_codon, translate};
use crate::bio::seq::{
    complement, is_dna, is_rna, normalise_answer, random_dna, reverse, reverse_complement, to_rna,
};
use crate::core::game::Game;
use crate::core::rng::Rng;
use crate::core::types::{
    ConceptId, Difficulty, Evaluation, GameManifest, GenParams, Lang, RealCode, ToolVersion,
};
use crate::games::common::{HintAction, first_answer, owned};

pub struct SequenceScramble;

pub const M_COMPLEMENT_NOT_REVERSED: &str = "M-001";
pub const M_REVERSE_NOT_COMPLEMENTED: &str = "M-002";
pub const M_MRNA_EQUALS_TEMPLATE: &str = "M-003";
pub const M_MRNA_COMPLEMENTS_CODING: &str = "M-004";
pub const M_RNA_HAS_T: &str = "M-005";
pub const M_TRANSLATE_FROM_FIRST_BASE: &str = "M-006";
pub const M_READ_THROUGH_STOP: &str = "M-007";
pub const M_ANY_INDEL_FRAMESHIFTS: &str = "M-008";
pub const M_INDEL_ONLY_ONE_CODON: &str = "M-009";

const ROUNDS: [&str; 4] = [
    "reverse_complement",
    "transcribe",
    "translate",
    "frameshift",
];

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Strand {
    /// Template (antisense) strand, written 5'→3'.
    Template,
    /// Coding (sense) strand, written 5'→3'. Same sequence as the mRNA, with T for U.
    Coding,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Change {
    Insertion { bases: String },
    Deletion { length: u32 },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Mutation {
    /// 0-based position in the coding sequence where the change happens.
    pub position: u32,
    pub change: Change,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Effect {
    Frameshift,
    InFrame,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Scenario {
    pub task: Task,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "round", rename_all = "snake_case")]
pub enum Task {
    /// Write the reverse complement (5'→3').
    ReverseComplement { dna: String },
    /// Write the mRNA (5'→3') transcribed from this gene, given one strand.
    Transcribe { strand: Strand, dna: String },
    /// Translate from the first AUG to the first in-frame stop (one-letter codes).
    Translate { mrna: String },
    /// Does this indel shift the reading frame?
    Frameshift { coding: String, mutation: Mutation },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Action {
    Hint,
    SubmitSequence { sequence: String },
    ChooseEffect { effect: Effect },
}

impl HintAction for Action {
    fn is_hint(&self) -> bool {
        matches!(self, Action::Hint)
    }
}

fn is_answer(a: &Action) -> bool {
    !matches!(a, Action::Hint)
}

// ---------------------------------------------------------------- science

fn transcript(strand: Strand, dna: &str) -> String {
    match strand {
        Strand::Template => to_rna(&reverse_complement(dna)),
        Strand::Coding => to_rna(dna),
    }
}

fn first_aug(mrna: &str) -> Option<usize> {
    mrna.find("AUG")
}

fn orf_protein(mrna: &str) -> String {
    first_aug(mrna)
        .map(|i| translate(mrna, i, true))
        .unwrap_or_default()
}

fn apply_mutation(coding: &str, m: &Mutation) -> String {
    let p = (m.position as usize).min(coding.len());
    match &m.change {
        Change::Insertion { bases } => format!("{}{}{}", &coding[..p], bases, &coding[p..]),
        Change::Deletion { length } => {
            let end = (p + *length as usize).min(coding.len());
            format!("{}{}", &coding[..p], &coding[end..])
        }
    }
}

fn effect_of(m: &Mutation) -> Effect {
    let len = match &m.change {
        Change::Insertion { bases } => bases.len() as u32,
        Change::Deletion { length } => *length,
    };
    if len % 3 == 0 {
        Effect::InFrame
    } else {
        Effect::Frameshift
    }
}

/// Known wrong answers for a free-response round, each with its misconception(s).
fn wrong_patterns(task: &Task) -> Vec<(String, Vec<&'static str>)> {
    match task {
        Task::ReverseComplement { dna } => vec![
            (complement(dna), vec![M_COMPLEMENT_NOT_REVERSED]),
            (reverse(dna), vec![M_REVERSE_NOT_COMPLEMENTED]),
        ],
        Task::Transcribe {
            strand: Strand::Template,
            dna,
        } => vec![
            (to_rna(dna), vec![M_MRNA_EQUALS_TEMPLATE]),
            (to_rna(&complement(dna)), vec![M_COMPLEMENT_NOT_REVERSED]),
            (reverse_complement(dna), vec![M_RNA_HAS_T]),
            (
                complement(dna),
                vec![M_COMPLEMENT_NOT_REVERSED, M_RNA_HAS_T],
            ),
        ],
        Task::Transcribe {
            strand: Strand::Coding,
            dna,
        } => vec![
            (
                to_rna(&reverse_complement(dna)),
                vec![M_MRNA_COMPLEMENTS_CODING],
            ),
            (to_rna(&complement(dna)), vec![M_MRNA_COMPLEMENTS_CODING]),
            (dna.clone(), vec![M_RNA_HAS_T]),
        ],
        Task::Translate { mrna } => {
            let readthrough: String = first_aug(mrna)
                .map(|i| translate(mrna, i, false).replace('*', ""))
                .unwrap_or_default();
            vec![
                (translate(mrna, 0, true), vec![M_TRANSLATE_FROM_FIRST_BASE]),
                (readthrough, vec![M_READ_THROUGH_STOP]),
            ]
        }
        Task::Frameshift { .. } => vec![],
    }
}

fn expected_sequence(task: &Task) -> Option<String> {
    match task {
        Task::ReverseComplement { dna } => Some(reverse_complement(dna)),
        Task::Transcribe { strand, dna } => Some(transcript(*strand, dna)),
        Task::Translate { mrna } => Some(orf_protein(mrna)),
        Task::Frameshift { .. } => None,
    }
}

/// True when no known wrong pattern coincides with the right answer (so each is detectable).
fn patterns_distinct(task: &Task) -> bool {
    let truth = expected_sequence(task);
    wrong_patterns(task)
        .iter()
        .all(|(w, _)| Some(w) != truth.as_ref() && !w.is_empty())
}

// ---------------------------------------------------------------- generation

fn dna_len(d: Difficulty, rng: &mut Rng) -> usize {
    let n = match d {
        Difficulty::Intro => rng.range(8, 12),
        Difficulty::Standard => rng.range(15, 25),
        Difficulty::Advanced => rng.range(30, 45),
    };
    n as usize
}

fn orf_dna(rng: &mut Rng, n_codons: u32) -> String {
    let mut s = String::from("ATG");
    for _ in 0..n_codons {
        s.push_str(&random_sense_codon(rng));
    }
    let stop: &&str = rng.choose(&STOP_CODONS);
    s.push_str(stop);
    s
}

fn generate_round(round: &str, d: Difficulty, rng: &mut Rng) -> Scenario {
    use Difficulty::*;
    // Retry until every misconception pattern is distinguishable from the answer.
    // Deterministic: retries consume the same seeded stream.
    loop {
        let task = match round {
            "reverse_complement" => {
                let n = dna_len(d, rng);
                Task::ReverseComplement {
                    dna: random_dna(rng, n),
                }
            }
            "transcribe" => {
                let strand = if rng.chance(1, 2) {
                    Strand::Template
                } else {
                    Strand::Coding
                };
                let n = dna_len(d, rng);
                Task::Transcribe {
                    strand,
                    dna: random_dna(rng, n),
                }
            }
            "translate" => {
                let (utr, codons) = match d {
                    Intro => (rng.range(3, 6), rng.range(3, 5)),
                    Standard => (rng.range(4, 9), rng.range(6, 10)),
                    Advanced => (rng.range(6, 15), rng.range(12, 18)),
                };
                let utr = random_dna(rng, utr as usize);
                let orf = orf_dna(rng, codons);
                let tail: String = (0..rng.range(2, 4))
                    .map(|_| random_sense_codon(rng))
                    .collect();
                let dna = format!("{utr}{orf}{tail}");
                if dna.find("ATG") != Some(utr.len()) {
                    continue; // the planted start must be the first AUG
                }
                Task::Translate { mrna: to_rna(&dna) }
            }
            "frameshift" => {
                let codons = match d {
                    Intro => rng.range(5, 7),
                    Standard => rng.range(8, 12),
                    Advanced => rng.range(12, 16),
                };
                let coding = orf_dna(rng, codons);
                let lengths: &[u32] = match d {
                    Intro => &[1, 3],
                    Standard => &[1, 2, 3],
                    Advanced => &[1, 2, 3, 4, 6],
                };
                let len = *rng.choose(lengths);
                // Keep the start and stop codons intact.
                let last = coding.len() as u32 - 3 - len;
                let position = rng.range(3, last);
                let change = if rng.chance(1, 2) {
                    Change::Insertion {
                        bases: random_dna(rng, len as usize),
                    }
                } else {
                    Change::Deletion { length: len }
                };
                Task::Frameshift {
                    coding,
                    mutation: Mutation { position, change },
                }
            }
            other => unreachable!("round validated by registry: {other}"),
        };
        if patterns_distinct(&task) {
            return Scenario { task };
        }
    }
}

// ---------------------------------------------------------------- bridge

const BIOPYTHON: &str = "1.85";
const DOC_SEQ: &str = "https://biopython.org/docs/latest/api/Bio.Seq.html";

fn py(code: String) -> RealCode {
    RealCode {
        language: Lang::Python,
        code: format!("# Biopython {BIOPYTHON}\nfrom Bio.Seq import Seq\n\n{code}"),
        tool_versions: vec![ToolVersion {
            tool: "Biopython".into(),
            version: BIOPYTHON.into(),
        }],
        dataset: None,
        doc_url: Some(DOC_SEQ.into()),
    }
}

// ---------------------------------------------------------------- Game impl

impl Game for SequenceScramble {
    const ID: &'static str = "sequence_scramble";
    type Scenario = Scenario;
    type Action = Action;

    fn manifest() -> GameManifest {
        GameManifest {
            id: Self::ID,
            name: "Sequence Scramble",
            rounds: ROUNDS.to_vec(),
            concepts: vec![
                "base_pairing",
                "antiparallel_strands",
                "reverse_complement",
                "transcription",
                "template_vs_coding_strand",
                "rna_vs_dna",
                "translation",
                "start_codon",
                "stop_codon",
                "reading_frame",
                "genetic_code",
                "indel",
                "frameshift_mutation",
            ],
            misconceptions: vec![
                M_COMPLEMENT_NOT_REVERSED,
                M_REVERSE_NOT_COMPLEMENTED,
                M_MRNA_EQUALS_TEMPLATE,
                M_MRNA_COMPLEMENTS_CODING,
                M_RNA_HAS_T,
                M_TRANSLATE_FROM_FIRST_BASE,
                M_READ_THROUGH_STOP,
                M_ANY_INDEL_FRAMESHIFTS,
                M_INDEL_ONLY_ONE_CODON,
            ],
        }
    }

    fn generate(params: &GenParams, rng: &mut Rng) -> Scenario {
        let round = match &params.round {
            Some(r) => r.clone(),
            None => {
                let allowed: &[&str] = if params.difficulty == Difficulty::Intro {
                    &ROUNDS[..3]
                } else {
                    &ROUNDS
                };
                rng.choose(allowed).to_string()
            }
        };
        generate_round(&round, params.difficulty, rng)
    }

    fn concepts(s: &Scenario) -> Vec<ConceptId> {
        owned(match &s.task {
            Task::ReverseComplement { .. } => {
                &["base_pairing", "antiparallel_strands", "reverse_complement"]
            }
            Task::Transcribe { .. } => {
                &["transcription", "template_vs_coding_strand", "rna_vs_dna"]
            }
            Task::Translate { .. } => &["translation", "start_codon", "stop_codon", "genetic_code"],
            Task::Frameshift { .. } => &["reading_frame", "indel", "frameshift_mutation"],
        })
    }

    fn evaluate(s: &Scenario, log: &[Action]) -> Evaluation {
        let concepts = Self::concepts(s);
        let (hints, answer) = first_answer(log, is_answer);
        let mut misc: Vec<String> = Vec::new();
        let solved = match (&s.task, answer) {
            (Task::Frameshift { mutation, .. }, Some(Action::ChooseEffect { effect })) => {
                let truth = effect_of(mutation);
                match (truth, effect) {
                    (Effect::InFrame, Effect::Frameshift) => {
                        misc.push(M_ANY_INDEL_FRAMESHIFTS.into())
                    }
                    (Effect::Frameshift, Effect::InFrame) => {
                        misc.push(M_INDEL_ONLY_ONE_CODON.into())
                    }
                    _ => {}
                }
                truth == *effect
            }
            (task, Some(Action::SubmitSequence { sequence }))
                if !matches!(task, Task::Frameshift { .. }) =>
            {
                let mut given = normalise_answer(sequence);
                if matches!(task, Task::Translate { .. }) {
                    given = given.trim_end_matches('*').to_string();
                }
                let truth = expected_sequence(task).unwrap_or_default();
                if given != truth {
                    for (pattern, ms) in wrong_patterns(task) {
                        if given == pattern {
                            misc.extend(ms.iter().map(|m| m.to_string()));
                        }
                    }
                }
                given == truth
            }
            _ => false,
        };
        Evaluation::new(solved, hints, &concepts, misc)
    }

    fn solve(s: &Scenario) -> Vec<Action> {
        match &s.task {
            Task::Frameshift { mutation, .. } => vec![Action::ChooseEffect {
                effect: effect_of(mutation),
            }],
            task => vec![Action::SubmitSequence {
                sequence: expected_sequence(task).unwrap_or_default(),
            }],
        }
    }

    fn choices(s: &Scenario) -> Option<Vec<Action>> {
        match &s.task {
            Task::Frameshift { .. } => Some(vec![
                Action::ChooseEffect {
                    effect: Effect::Frameshift,
                },
                Action::ChooseEffect {
                    effect: Effect::InFrame,
                },
            ]),
            _ => None,
        }
    }

    fn bridge(s: &Scenario, _log: &[Action]) -> RealCode {
        match &s.task {
            Task::ReverseComplement { dna } => py(format!(
                "dna = Seq(\"{dna}\")\n\
                 print(dna.reverse_complement())   # the other strand, read 5'->3'\n\
                 print(dna.complement())           # complement only: still reads 3'->5'\n"
            )),
            Task::Transcribe {
                strand: Strand::Coding,
                dna,
            } => py(format!(
                "coding = Seq(\"{dna}\")   # coding (sense) strand, 5'->3'\n\
                 print(coding.transcribe())   # mRNA = coding strand with U for T\n"
            )),
            Task::Transcribe {
                strand: Strand::Template,
                dna,
            } => py(format!(
                "template = Seq(\"{dna}\")   # template (antisense) strand, 5'->3'\n\
                 coding = template.reverse_complement()\n\
                 print(coding.transcribe())   # Biopython's transcribe() expects the coding strand\n"
            )),
            Task::Translate { mrna } => py(format!(
                "mrna = Seq(\"{mrna}\")\n\
                 start = mrna.find(\"AUG\")\n\
                 orf = mrna[start:]\n\
                 orf = orf[: len(orf) - len(orf) % 3]   # whole codons only\n\
                 print(orf.translate(to_stop=True))\n"
            )),
            Task::Frameshift { coding, mutation } => {
                let mutant = apply_mutation(coding, mutation);
                py(format!(
                    "wild_type = Seq(\"{coding}\")\n\
                     mutant    = Seq(\"{mutant}\")\n\n\
                     def protein(s):\n    \
                         return s[: len(s) - len(s) % 3].translate(to_stop=True)\n\n\
                     print(protein(wild_type))\n\
                     print(protein(mutant))   # compare the residues after position {pos}\n",
                    pos = mutation.position,
                ))
            }
        }
    }

    fn check(s: &Scenario) -> Vec<String> {
        let mut errs = Vec::new();
        match &s.task {
            Task::ReverseComplement { dna } | Task::Transcribe { dna, .. } => {
                if !is_dna(dna) {
                    errs.push("dna must be uppercase ACGT".into());
                }
            }
            Task::Translate { mrna } => {
                if !is_rna(mrna) {
                    errs.push("mrna must be uppercase ACGU".into());
                } else if first_aug(mrna).is_none() {
                    errs.push("mrna has no AUG start codon".into());
                } else {
                    let i = first_aug(mrna).unwrap_or(0);
                    if !translate(mrna, i, false).contains('*') {
                        errs.push("no in-frame stop codon after the first AUG".into());
                    }
                }
            }
            Task::Frameshift { coding, mutation } => {
                if !is_dna(coding) || !coding.starts_with("ATG") || coding.len() % 3 != 0 {
                    errs.push(
                        "coding must be DNA starting with ATG, length a multiple of 3".into(),
                    );
                }
                let p = mutation.position as usize;
                match &mutation.change {
                    Change::Insertion { bases } => {
                        if !is_dna(bases) {
                            errs.push("inserted bases must be ACGT".into());
                        }
                        if p > coding.len() {
                            errs.push("mutation position beyond sequence".into());
                        }
                    }
                    Change::Deletion { length } => {
                        if *length == 0 || p + *length as usize > coding.len() {
                            errs.push("deletion out of range".into());
                        }
                    }
                }
            }
        }
        if errs.is_empty() && !patterns_distinct(&s.task) {
            errs.push(
                "a misconception pattern equals the correct answer; pick another sequence".into(),
            );
        }
        errs
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(task: Task) -> Scenario {
        Scenario { task }
    }

    fn submit(seq: &str) -> Vec<Action> {
        vec![Action::SubmitSequence {
            sequence: seq.into(),
        }]
    }

    #[test]
    fn reverse_complement_misconceptions() {
        let sc = s(Task::ReverseComplement { dna: "AACG".into() });
        assert!(SequenceScramble::evaluate(&sc, &submit("cgtt")).solved); // case-insensitive
        assert_eq!(
            SequenceScramble::evaluate(&sc, &submit("TTGC")).misconceptions,
            vec![M_COMPLEMENT_NOT_REVERSED]
        );
        assert_eq!(
            SequenceScramble::evaluate(&sc, &submit("GCAA")).misconceptions,
            vec![M_REVERSE_NOT_COMPLEMENTED]
        );
    }

    #[test]
    fn transcription_from_template_and_coding() {
        let t = s(Task::Transcribe {
            strand: Strand::Template,
            dna: "TACG".into(),
        });
        assert!(SequenceScramble::evaluate(&t, &submit("CGUA")).solved);
        assert_eq!(
            SequenceScramble::evaluate(&t, &submit("UACG")).misconceptions,
            vec![M_MRNA_EQUALS_TEMPLATE]
        );
        assert_eq!(
            SequenceScramble::evaluate(&t, &submit("CGTA")).misconceptions,
            vec![M_RNA_HAS_T]
        );
        let c = s(Task::Transcribe {
            strand: Strand::Coding,
            dna: "ATGC".into(),
        });
        assert!(SequenceScramble::evaluate(&c, &submit("AUGC")).solved);
        assert_eq!(
            SequenceScramble::evaluate(&c, &submit("GCAU")).misconceptions,
            vec![M_MRNA_COMPLEMENTS_CODING]
        );
    }

    #[test]
    fn translation_misconceptions() {
        // UTR "GC", start AUG, GCC (A), AAA (K), stop UAA, tail GGG (G).
        let sc = s(Task::Translate {
            mrna: "GCAUGGCCAAAUAAGGG".into(),
        });
        assert!(SequenceScramble::evaluate(&sc, &submit("MAK")).solved);
        assert!(SequenceScramble::evaluate(&sc, &submit("MAK*")).solved);
        assert_eq!(
            SequenceScramble::evaluate(&sc, &submit("MAKG")).misconceptions,
            vec![M_READ_THROUGH_STOP]
        );
        // From base 0: GCA UGG CCA AAU AAG GG → A W P N K
        assert_eq!(
            SequenceScramble::evaluate(&sc, &submit("AWPNK")).misconceptions,
            vec![M_TRANSLATE_FROM_FIRST_BASE]
        );
    }

    #[test]
    fn frameshift_effects() {
        let coding = "ATGGCCAAATTTGGGTAA".to_string();
        let in_frame = s(Task::Frameshift {
            coding: coding.clone(),
            mutation: Mutation {
                position: 6,
                change: Change::Deletion { length: 3 },
            },
        });
        let e = SequenceScramble::evaluate(
            &in_frame,
            &[Action::ChooseEffect {
                effect: Effect::Frameshift,
            }],
        );
        assert_eq!(e.misconceptions, vec![M_ANY_INDEL_FRAMESHIFTS]);
        let shift = s(Task::Frameshift {
            coding,
            mutation: Mutation {
                position: 4,
                change: Change::Insertion { bases: "A".into() },
            },
        });
        assert!(
            SequenceScramble::evaluate(
                &shift,
                &[Action::ChooseEffect {
                    effect: Effect::Frameshift
                }]
            )
            .solved
        );
        assert_eq!(
            apply_mutation(
                "ATGGCC",
                &Mutation {
                    position: 4,
                    change: Change::Insertion { bases: "A".into() }
                }
            ),
            "ATGGACC"
        );
    }
}
