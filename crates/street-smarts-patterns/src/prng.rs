//! Tiny seedable PRNG. SplitMix64 → xoshiro256** style for short test runs.
//! No external crates; WASM-friendly.

pub struct Prng {
    state: [u64; 4],
}

impl Prng {
    pub fn new(seed: u64) -> Self {
        // Seed via SplitMix64 to spread bits.
        let mut sm = seed;
        let mut next = || {
            sm = sm.wrapping_add(0x9E3779B97F4A7C15);
            let mut z = sm;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
            z ^ (z >> 31)
        };
        Self { state: [next(), next(), next(), next()] }
    }

    /// xoshiro256**
    pub fn next_u64(&mut self) -> u64 {
        let result = self.state[1].wrapping_mul(5).rotate_left(7).wrapping_mul(9);
        let t = self.state[1] << 17;
        self.state[2] ^= self.state[0];
        self.state[3] ^= self.state[1];
        self.state[1] ^= self.state[2];
        self.state[0] ^= self.state[3];
        self.state[2] ^= t;
        self.state[3] = self.state[3].rotate_left(45);
        result
    }

    /// Uniform [0, 1)
    pub fn next_f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
    }

    /// Uniform [lo, hi)
    pub fn range(&mut self, lo: f64, hi: f64) -> f64 {
        lo + self.next_f64() * (hi - lo)
    }

    /// Standard normal via Box-Muller (one of two; we discard the other for simplicity).
    pub fn normal(&mut self) -> f64 {
        let u1 = self.next_f64().max(f64::MIN_POSITIVE);
        let u2 = self.next_f64();
        (-2.0 * u1.ln()).sqrt() * (std::f64::consts::TAU * u2).cos()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn deterministic_for_same_seed() {
        let mut a = Prng::new(42);
        let mut b = Prng::new(42);
        for _ in 0..20 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
    }
    #[test]
    fn different_seeds_diverge() {
        let mut a = Prng::new(1);
        let mut b = Prng::new(2);
        let mut diffs = 0;
        for _ in 0..20 {
            if a.next_u64() != b.next_u64() { diffs += 1; }
        }
        assert!(diffs > 15);
    }
    #[test]
    fn range_bounds() {
        let mut r = Prng::new(7);
        for _ in 0..100 {
            let x = r.range(-3.0, 5.0);
            assert!(x >= -3.0 && x < 5.0);
        }
    }
}
