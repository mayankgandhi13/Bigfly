//! Restriction enzyme panel.
//!
//! Recognition sites and cut positions are from REBASE (http://rebase.neb.com).
//! Positions are offsets into the site written 5'→3' on the top strand:
//! `cut_top` is the number of site bases left of the top-strand cut;
//! `cut_bottom` is the same, measured in top-strand coordinates, for the bottom-strand cut.
//!
//! e.g. EcoRI G^AATTC: cut_top = 1, cut_bottom = 5 → 5' overhang AATT.

use serde::{Deserialize, Serialize};

use crate::bio::seq::reverse_complement;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Enzyme {
    pub name: &'static str,
    pub site: &'static str,
    pub cut_top: u8,
    pub cut_bottom: u8,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum EndKind {
    Blunt,
    FivePrime,
    ThreePrime,
}

impl EndKind {
    pub const ALL: [EndKind; 3] = [EndKind::Blunt, EndKind::FivePrime, EndKind::ThreePrime];
}

pub const PANEL: [Enzyme; 8] = [
    Enzyme {
        name: "EcoRI",
        site: "GAATTC",
        cut_top: 1,
        cut_bottom: 5,
    },
    Enzyme {
        name: "BamHI",
        site: "GGATCC",
        cut_top: 1,
        cut_bottom: 5,
    },
    Enzyme {
        name: "HindIII",
        site: "AAGCTT",
        cut_top: 1,
        cut_bottom: 5,
    },
    Enzyme {
        name: "HaeIII",
        site: "GGCC",
        cut_top: 2,
        cut_bottom: 2,
    },
    Enzyme {
        name: "PstI",
        site: "CTGCAG",
        cut_top: 5,
        cut_bottom: 1,
    },
    Enzyme {
        name: "SalI",
        site: "GTCGAC",
        cut_top: 1,
        cut_bottom: 5,
    },
    Enzyme {
        name: "XhoI",
        site: "CTCGAG",
        cut_top: 1,
        cut_bottom: 5,
    },
    Enzyme {
        name: "BglII",
        site: "AGATCT",
        cut_top: 1,
        cut_bottom: 5,
    },
];

pub fn find(name: &str) -> Option<&'static Enzyme> {
    PANEL.iter().find(|e| e.name == name)
}

impl Enzyme {
    pub fn end_kind(&self) -> EndKind {
        use std::cmp::Ordering::*;
        match self.cut_top.cmp(&self.cut_bottom) {
            Equal => EndKind::Blunt,
            Less => EndKind::FivePrime,
            Greater => EndKind::ThreePrime,
        }
    }

    /// Single-stranded overhang, 5'→3' on the protruding strand ("" if blunt).
    /// For the palindromic sites in this panel, both strands give the same string.
    pub fn overhang(&self) -> &'static str {
        let (a, b) = if self.cut_top <= self.cut_bottom {
            (self.cut_top, self.cut_bottom)
        } else {
            (self.cut_bottom, self.cut_top)
        };
        &self.site[a as usize..b as usize]
    }

    /// Cut notation, e.g. `G^AATT_C` (top-strand cut ^, bottom-strand cut _).
    pub fn notation(&self) -> String {
        let mut out = String::new();
        for (i, c) in self.site.chars().enumerate() {
            if i == self.cut_top as usize {
                out.push('^');
            }
            if i == self.cut_bottom as usize && self.cut_bottom != self.cut_top {
                out.push('_');
            }
            out.push(c);
        }
        out
    }
}

/// Can ends produced by `a` and `b` be joined directly by DNA ligase?
/// Blunt + blunt: yes (T4 ligase joins blunt ends, less efficiently).
/// Sticky + sticky: only if same polarity and complementary overhangs.
/// Sticky + blunt: no (without end modification such as fill-in).
pub fn compatible(a: &Enzyme, b: &Enzyme) -> bool {
    match (a.end_kind(), b.end_kind()) {
        (EndKind::Blunt, EndKind::Blunt) => true,
        (x, y) if x == y => reverse_complement(a.overhang()) == b.overhang(),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sites_are_palindromic_with_symmetric_cuts() {
        for e in PANEL {
            assert_eq!(reverse_complement(e.site), e.site, "{}", e.name);
            assert_eq!(
                e.cut_bottom as usize,
                e.site.len() - e.cut_top as usize,
                "{}",
                e.name
            );
        }
    }

    #[test]
    fn end_types_match_rebase() {
        let kind = |n| find(n).unwrap().end_kind();
        assert_eq!(kind("EcoRI"), EndKind::FivePrime);
        assert_eq!(kind("HaeIII"), EndKind::Blunt);
        assert_eq!(kind("PstI"), EndKind::ThreePrime);
        assert_eq!(find("EcoRI").unwrap().overhang(), "AATT");
        assert_eq!(find("PstI").unwrap().overhang(), "TGCA");
        assert_eq!(find("BamHI").unwrap().overhang(), "GATC");
        assert_eq!(find("SalI").unwrap().overhang(), "TCGA");
        assert_eq!(find("EcoRI").unwrap().notation(), "G^AATT_C");
        assert_eq!(find("PstI").unwrap().notation(), "C_TGCA^G");
        assert_eq!(find("HaeIII").unwrap().notation(), "GG^CC");
    }

    #[test]
    fn ligation_compatibility() {
        let f = |n| find(n).unwrap();
        assert!(compatible(f("BamHI"), f("BglII")));
        assert!(compatible(f("SalI"), f("XhoI")));
        assert!(compatible(f("EcoRI"), f("EcoRI")));
        assert!(compatible(f("HaeIII"), f("HaeIII")));
        assert!(!compatible(f("EcoRI"), f("BamHI")));
        assert!(!compatible(f("EcoRI"), f("HaeIII")));
        assert!(!compatible(f("PstI"), f("EcoRI")));
    }
}
