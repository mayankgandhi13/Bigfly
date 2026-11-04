//! Deterministic PRNG owned by this crate (SplitMix64 seeding → xoshiro256**).
//!
//! We deliberately do not depend on `rand`: its sampling algorithms have changed
//! between versions, which would silently change scenarios for a stored seed.
//! Every algorithm here is pinned by tests; changing any output is a breaking
//! engine change.

/// One SplitMix64 step. Used for seeding and seed derivation.
fn splitmix64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// FNV-1a 64-bit hash of a label (stable, dependency-free).
fn fnv1a(label: &str) -> u64 {
    let mut h: u64 = 0xCBF2_9CE4_8422_2325;
    for b in label.bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0000_0100_0000_01B3);
    }
    h
}

/// Derive an independent sub-seed from a root seed and a label, e.g.
/// `derive_seed(seed, "dna_chopper")` or `derive_seed(seed, "distractors")`.
/// Independent streams mean adding a draw in one place doesn't shift others.
pub fn derive_seed(root: u64, label: &str) -> u64 {
    let mut state = root ^ fnv1a(label);
    splitmix64(&mut state)
}

/// Seed for the daily question, from a date string `YYYY-MM-DD`.
/// Reduced to the JS-safe integer range so it round-trips through JSON.
pub fn daily_seed(date: &str) -> u64 {
    derive_seed(fnv1a(date), "daily") & crate::core::types::MAX_SEED
}

/// xoshiro256** generator.
#[derive(Clone, Debug)]
pub struct Rng {
    s: [u64; 4],
}

impl Rng {
    pub fn from_seed(seed: u64) -> Self {
        let mut sm = seed;
        let s = [
            splitmix64(&mut sm),
            splitmix64(&mut sm),
            splitmix64(&mut sm),
            splitmix64(&mut sm),
        ];
        Self { s }
    }

    #[cfg(test)]
    fn from_state(s: [u64; 4]) -> Self {
        Self { s }
    }

    pub fn next_u64(&mut self) -> u64 {
        let result = self.s[1].wrapping_mul(5).rotate_left(7).wrapping_mul(9);
        let t = self.s[1] << 17;
        self.s[2] ^= self.s[0];
        self.s[3] ^= self.s[1];
        self.s[1] ^= self.s[2];
        self.s[0] ^= self.s[3];
        self.s[2] ^= t;
        self.s[3] = self.s[3].rotate_left(45);
        result
    }

    pub fn next_u32(&mut self) -> u32 {
        (self.next_u64() >> 32) as u32
    }

    /// Uniform integer in `0..n` (Lemire's unbiased method). Panics if `n == 0`.
    pub fn below(&mut self, n: u32) -> u32 {
        assert!(n > 0, "Rng::below(0)");
        let mut m = u64::from(self.next_u32()) * u64::from(n);
        let mut low = m as u32;
        if low < n {
            let threshold = n.wrapping_neg() % n;
            while low < threshold {
                m = u64::from(self.next_u32()) * u64::from(n);
                low = m as u32;
            }
        }
        (m >> 32) as u32
    }

    /// Uniform integer in `lo..=hi`.
    pub fn range(&mut self, lo: u32, hi: u32) -> u32 {
        assert!(lo <= hi, "Rng::range: lo > hi");
        lo + self.below(hi - lo + 1)
    }

    /// Uniform index into a slice of length `len`.
    pub fn index(&mut self, len: usize) -> usize {
        let n = u32::try_from(len).expect("slice too long for Rng::index");
        self.below(n) as usize
    }

    /// True with probability `num / den`.
    pub fn chance(&mut self, num: u32, den: u32) -> bool {
        self.below(den) < num
    }

    pub fn choose<'a, T>(&mut self, xs: &'a [T]) -> &'a T {
        &xs[self.index(xs.len())]
    }

    /// Fisher–Yates shuffle.
    pub fn shuffle<T>(&mut self, xs: &mut [T]) {
        for i in (1..xs.len()).rev() {
            let j = self.index(i + 1);
            xs.swap(i, j);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splitmix64_reference_value() {
        // Reference: first output of SplitMix64 seeded with 0.
        let mut s = 0;
        assert_eq!(splitmix64(&mut s), 0xE220_A839_7B1D_CDAF);
    }

    #[test]
    fn xoshiro_reference_first_output() {
        // rotl(2 * 5, 7) * 9 = 11520 for state [1, 2, 3, 4].
        let mut r = Rng::from_state([1, 2, 3, 4]);
        assert_eq!(r.next_u64(), 11520);
    }

    #[test]
    fn same_seed_same_stream() {
        let mut a = Rng::from_seed(42);
        let mut b = Rng::from_seed(42);
        for _ in 0..1000 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
    }

    #[test]
    fn pinned_stream_for_seed_42() {
        // Golden values: if this changes, every stored scenario changes.
        let mut r = Rng::from_seed(42);
        let got: Vec<u32> = (0..5).map(|_| r.below(100)).collect();
        assert_eq!(got, PINNED_42);
    }
    const PINNED_42: [u32; 5] = [8, 37, 68, 92, 99];

    #[test]
    fn below_stays_in_range_and_covers_it() {
        let mut r = Rng::from_seed(7);
        let mut seen = [false; 6];
        for _ in 0..1000 {
            let v = r.below(6);
            assert!(v < 6);
            seen[v as usize] = true;
        }
        assert!(seen.iter().all(|&s| s));
    }

    #[test]
    fn derive_seed_separates_labels() {
        assert_ne!(derive_seed(1, "a"), derive_seed(1, "b"));
        assert_eq!(derive_seed(1, "a"), derive_seed(1, "a"));
    }

    #[test]
    fn daily_seed_is_js_safe_and_stable() {
        let s = daily_seed("2026-11-03");
        assert!(s <= crate::core::types::MAX_SEED);
        assert_eq!(s, daily_seed("2026-11-03"));
        assert_ne!(s, daily_seed("2026-11-04"));
    }

    #[test]
    fn shuffle_is_permutation() {
        let mut r = Rng::from_seed(3);
        let mut v: Vec<u32> = (0..20).collect();
        r.shuffle(&mut v);
        let mut sorted = v.clone();
        sorted.sort();
        assert_eq!(sorted, (0..20).collect::<Vec<_>>());
    }
}
