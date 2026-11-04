//! Restriction digest of linear or circular DNA.
//!
//! Cut positions are 0-based indices of the first top-strand base *after* the cut
//! (Biopython's `search()` reports the same position 1-based). Fragment lengths are
//! measured between top-strand cuts, the usual convention for digest maps.

use serde::{Deserialize, Serialize};

use crate::bio::enzymes::Enzyme;

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Topology {
    Linear,
    Circular,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cut {
    pub position: u32,
    pub enzyme: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fragment {
    pub start: u32,
    pub length: u32,
    /// `None` = original molecule end (linear only); otherwise the cutting enzyme.
    pub left: Option<&'static str>,
    pub right: Option<&'static str>,
}

/// Start positions of every recognition site (sites may wrap the origin if circular).
pub fn find_sites(seq: &str, enzyme: &Enzyme, topology: Topology) -> Vec<u32> {
    let s = seq.as_bytes();
    let site = enzyme.site.as_bytes();
    let (n, k) = (s.len(), site.len());
    if n < k {
        return Vec::new();
    }
    let starts = match topology {
        Topology::Linear => n - k + 1,
        Topology::Circular => n,
    };
    (0..starts)
        .filter(|&i| (0..k).all(|j| s[(i + j) % n] == site[j]))
        .map(|i| i as u32)
        .collect()
}

/// All top-strand cut positions, sorted and de-duplicated (first enzyme wins on ties).
pub fn cuts(seq: &str, enzymes: &[&'static Enzyme], topology: Topology) -> Vec<Cut> {
    let n = seq.len() as u32;
    let mut out: Vec<Cut> = Vec::new();
    for e in enzymes {
        for start in find_sites(seq, e, topology) {
            let position = (start + u32::from(e.cut_top)) % n.max(1);
            if !out.iter().any(|c| c.position == position) {
                out.push(Cut {
                    position,
                    enzyme: e.name,
                });
            }
        }
    }
    // A linear molecule can't be "cut" at its very ends.
    if topology == Topology::Linear {
        out.retain(|c| c.position > 0 && c.position < n);
    }
    out.sort_by_key(|c| c.position);
    out
}

pub fn digest(seq: &str, enzymes: &[&'static Enzyme], topology: Topology) -> Vec<Fragment> {
    let n = seq.len() as u32;
    let cs = cuts(seq, enzymes, topology);
    match topology {
        Topology::Linear => {
            let mut frags = Vec::with_capacity(cs.len() + 1);
            let mut prev_pos = 0;
            let mut prev_enz = None;
            for c in &cs {
                frags.push(Fragment {
                    start: prev_pos,
                    length: c.position - prev_pos,
                    left: prev_enz,
                    right: Some(c.enzyme),
                });
                prev_pos = c.position;
                prev_enz = Some(c.enzyme);
            }
            frags.push(Fragment {
                start: prev_pos,
                length: n - prev_pos,
                left: prev_enz,
                right: None,
            });
            frags
        }
        Topology::Circular => {
            if cs.is_empty() {
                return vec![Fragment {
                    start: 0,
                    length: n,
                    left: None,
                    right: None,
                }];
            }
            (0..cs.len())
                .map(|i| {
                    let a = cs[i];
                    let b = cs[(i + 1) % cs.len()];
                    let length = if cs.len() == 1 {
                        n
                    } else {
                        (b.position + n - a.position) % n
                    };
                    Fragment {
                        start: a.position,
                        length,
                        left: Some(a.enzyme),
                        right: Some(b.enzyme),
                    }
                })
                .collect()
        }
    }
}

/// Fragment sizes, largest first (the order they appear from the top of a gel).
pub fn fragment_sizes(seq: &str, enzymes: &[&'static Enzyme], topology: Topology) -> Vec<u32> {
    let mut sizes: Vec<u32> = digest(seq, enzymes, topology)
        .iter()
        .map(|f| f.length)
        .collect();
    sizes.sort_unstable_by(|a, b| b.cmp(a));
    sizes
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bio::enzymes::find;

    fn e(n: &str) -> &'static Enzyme {
        find(n).unwrap()
    }

    #[test]
    fn linear_single_cut() {
        //           0123456789
        let seq = "AAAGAATTCAAA"; // EcoRI site at 3, cut after G → position 4
        let cs = cuts(seq, &[e("EcoRI")], Topology::Linear);
        assert_eq!(cs.iter().map(|c| c.position).collect::<Vec<_>>(), vec![4]);
        assert_eq!(
            fragment_sizes(seq, &[e("EcoRI")], Topology::Linear),
            vec![8, 4]
        );
    }

    #[test]
    fn circular_n_cuts_give_n_fragments() {
        let seq = "GAATTCAAAAAAGAATTCAAAAAAAAAA"; // two EcoRI sites, length 28
        let linear = fragment_sizes(seq, &[e("EcoRI")], Topology::Linear);
        let circular = fragment_sizes(seq, &[e("EcoRI")], Topology::Circular);
        assert_eq!(linear.len(), 3);
        assert_eq!(circular.len(), 2);
        assert_eq!(circular.iter().sum::<u32>(), 28);
        assert_eq!(circular, vec![16, 12]);
    }

    #[test]
    fn circular_site_spanning_origin() {
        let seq = "ATTCAAAAAAAAGA"; // "GA" + "ATTC" wraps the origin
        let cs = cuts(seq, &[e("EcoRI")], Topology::Circular);
        assert_eq!(cs.len(), 1);
        assert_eq!(cs[0].position, 13); // site starts at 12, cut after first base
        assert!(cuts(seq, &[e("EcoRI")], Topology::Linear).is_empty());
    }

    #[test]
    fn pst1_three_prime_cut_position() {
        let seq = "AAACTGCAGAAA"; // site at 3, top cut after CTGCA → 8
        assert_eq!(cuts(seq, &[e("PstI")], Topology::Linear)[0].position, 8);
    }

    #[test]
    fn sizes_sum_to_length() {
        let seq = "GGCCAAGAATTCTTGGCCAGGATCCAAA";
        for topo in [Topology::Linear, Topology::Circular] {
            let s = fragment_sizes(seq, &[e("HaeIII"), e("EcoRI"), e("BamHI")], topo);
            assert_eq!(s.iter().sum::<u32>(), seq.len() as u32);
        }
    }
}
