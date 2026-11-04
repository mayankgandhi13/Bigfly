//! The standard genetic code (NCBI translation table 1).

use crate::core::rng::Rng;

/// Amino acids in TCAG order: index = 16*b1 + 4*b2 + b3 with T=0, C=1, A=2, G=3.
/// Source: NCBI Genetic Codes, transl_table=1.
const TABLE1: &[u8; 64] = b"FFLLSSSSYY**CC*WLLLLPPPPHHQQRRRRIIIMTTTTNNKKSSRRVVVVAAAADDEEGGGG";

pub const STOP: u8 = b'*';

fn base_index(b: u8) -> Option<usize> {
    match b {
        b'T' | b'U' => Some(0),
        b'C' => Some(1),
        b'A' => Some(2),
        b'G' => Some(3),
        _ => None,
    }
}

/// Translate one codon (DNA or RNA alphabet). Returns `None` for invalid bases.
pub fn translate_codon(codon: &[u8]) -> Option<u8> {
    if codon.len() != 3 {
        return None;
    }
    let i = base_index(codon[0])? * 16 + base_index(codon[1])? * 4 + base_index(codon[2])?;
    Some(TABLE1[i])
}

pub fn is_stop(codon: &[u8]) -> bool {
    translate_codon(codon) == Some(STOP)
}

/// Translate from `start` in steps of 3. If `to_stop`, stop *before* the first stop codon;
/// otherwise translate every complete codon, writing stops as `*`.
/// A trailing partial codon is ignored.
pub fn translate(seq: &str, start: usize, to_stop: bool) -> String {
    let bytes = seq.as_bytes();
    let mut out = String::new();
    let mut i = start;
    while i + 3 <= bytes.len() {
        let aa = translate_codon(&bytes[i..i + 3]).unwrap_or(b'X');
        if to_stop && aa == STOP {
            break;
        }
        out.push(aa as char);
        i += 3;
    }
    out
}

/// All 61 sense codons, as DNA, in TCAG order.
pub fn sense_codons() -> Vec<String> {
    let bases = b"TCAG";
    let mut out = Vec::with_capacity(61);
    for (i, &aa) in TABLE1.iter().enumerate() {
        if aa != STOP {
            let c = [bases[i / 16], bases[(i / 4) % 4], bases[i % 4]];
            out.push(String::from_utf8(c.to_vec()).expect("ascii"));
        }
    }
    out
}

pub const STOP_CODONS: [&str; 3] = ["TAA", "TAG", "TGA"];

pub fn random_sense_codon(rng: &mut Rng) -> String {
    let codons = sense_codons();
    rng.choose(&codons).clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn landmark_codons() {
        assert_eq!(translate_codon(b"ATG"), Some(b'M'));
        assert_eq!(translate_codon(b"AUG"), Some(b'M'));
        assert_eq!(translate_codon(b"TGG"), Some(b'W'));
        for s in STOP_CODONS {
            assert!(is_stop(s.as_bytes()), "{s}");
        }
        assert_eq!(translate_codon(b"GCC"), Some(b'A'));
        assert_eq!(translate_codon(b"AAA"), Some(b'K'));
        assert_eq!(translate_codon(b"TTT"), Some(b'F'));
        assert_eq!(translate_codon(b"GGG"), Some(b'G'));
        assert_eq!(translate_codon(b"CAT"), Some(b'H'));
    }

    #[test]
    fn sixty_one_sense_codons() {
        let s = sense_codons();
        assert_eq!(s.len(), 61);
        assert!(!s.iter().any(|c| STOP_CODONS.contains(&c.as_str())));
    }

    #[test]
    fn translate_to_stop_and_through() {
        assert_eq!(translate("ATGGCCTAAGGG", 0, true), "MA");
        assert_eq!(translate("ATGGCCTAAGGG", 0, false), "MA*G");
        assert_eq!(translate("CCATGAAATGA", 2, true), "MK");
    }
}
