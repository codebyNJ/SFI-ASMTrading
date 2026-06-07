//! Deterministic pseudo-random generator (splitmix64) with Gaussian draws.
//!
//! We roll our own so the whole simulation is reproducible from a single `u64`
//! seed with no dependency on external crate versions. splitmix64 is fast and
//! statistically fine for Monte-Carlo simulation (it is not cryptographic).

use std::f64::consts::PI;

#[derive(Clone, Debug)]
pub struct Rng {
    state: u64,
    gauss_cache: Option<f64>,
}

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self { state: seed, gauss_cache: None }
    }

    #[inline]
    fn next_u64(&mut self) -> u64 {
        // splitmix64
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform in [0, 1).
    #[inline]
    pub fn next_f64(&mut self) -> f64 {
        // Use the top 53 bits for a full-precision double in [0,1).
        (self.next_u64() >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
    }

    /// Uniform in [lo, hi).
    #[inline]
    pub fn uniform(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * self.next_f64()
    }

    /// Uniform integer in [0, n).
    #[inline]
    pub fn below(&mut self, n: usize) -> usize {
        debug_assert!(n > 0);
        (self.next_u64() % n as u64) as usize
    }

    /// Bernoulli(p).
    #[inline]
    pub fn chance(&mut self, p: f64) -> bool {
        self.next_f64() < p
    }

    /// Standard normal via Box-Muller, caching the paired draw.
    pub fn gaussian(&mut self) -> f64 {
        if let Some(g) = self.gauss_cache.take() {
            return g;
        }
        let u1 = self.next_f64().max(1e-12);
        let u2 = self.next_f64();
        let r = (-2.0 * u1.ln()).sqrt();
        let (s, c) = (2.0 * PI * u2).sin_cos();
        self.gauss_cache = Some(r * s);
        r * c
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_for_same_seed() {
        let mut a = Rng::new(42);
        let mut b = Rng::new(42);
        for _ in 0..1000 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
    }

    #[test]
    fn gaussian_is_roughly_standard() {
        let mut r = Rng::new(7);
        let n = 200_000;
        let mut mean = 0.0;
        let mut m2 = 0.0;
        for i in 1..=n {
            let x = r.gaussian();
            let delta = x - mean;
            mean += delta / i as f64;
            m2 += delta * (x - mean);
        }
        let var = m2 / n as f64;
        assert!(mean.abs() < 0.02, "mean was {mean}");
        assert!((var - 1.0).abs() < 0.03, "var was {var}");
    }
}
