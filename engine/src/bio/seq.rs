//! Nucleotide sequence helpers. Sequences are uppercase ASCII strings.

use crate::core::rng::Rng;

pub const DNA_BASES: [u8; 4] = *b"ACGT";

pub fn complement_base(b: u8) -> u8 {
    match b {
        b'A' => b'T',
        b'T' => b'A',
        b'U' => b'A',
        b'C' => b'G',
        b'G' => b'C',
        other => other,
    }
}

/// Base-by-base complement, *not* reversed (still reads 3'→5' relative to the input).
pub fn complement(seq: &str) -> String {
    seq.bytes().map(|b| complement_base(b) as char).collect()
}

/// Reverse complement: the other strand, written 5'→3'.
pub fn reverse_complement(seq: &str) -> String {
    seq.bytes()
        .rev()
        .map(|b| complement_base(b) as char)
        .collect()
}

pub fn reverse(seq: &str) -> String {
    seq.chars().rev().collect()
}

/// DNA → RNA alphabet (T → U). Strand choice is the caller's job.
pub fn to_rna(seq: &str) -> String {
    seq.replace('T', "U")
}

pub fn is_dna(seq: &str) -> bool {
    !seq.is_empty() && seq.bytes().all(|b| DNA_BASES.contains(&b))
}

pub fn is_rna(seq: &str) -> bool {
    !seq.is_empty() && seq.bytes().all(|b| b"ACGU".contains(&b))
}

pub fn random_dna(rng: &mut Rng, len: usize) -> String {
    (0..len).map(|_| *rng.choose(&DNA_BASES) as char).collect()
}

/// Uppercase and strip whitespace from free-text answers.
pub fn normalise_answer(s: &str) -> String {
    s.chars()
        .filter(|c| !c.is_whitespace())
        .collect::<String>()
        .to_uppercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reverse_complement_examples() {
        assert_eq!(reverse_complement("ATGC"), "GCAT");
        assert_eq!(complement("ATGC"), "TACG");
        assert_eq!(reverse_complement("GAATTC"), "GAATTC"); // EcoRI site is palindromic
    }

    #[test]
    fn reverse_complement_is_involution() {
        let mut r = Rng::from_seed(1);
        for _ in 0..50 {
            let s = random_dna(&mut r, 30);
            assert_eq!(reverse_complement(&reverse_complement(&s)), s);
        }
    }

    #[test]
    fn rna_conversion() {
        assert_eq!(to_rna("ATGT"), "AUGU");
        assert!(is_rna("AUGU"));
        assert!(!is_dna("AUGU"));
    }
}
