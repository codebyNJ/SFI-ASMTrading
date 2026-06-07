//! Classifier rules: a ternary condition over the market state plus a linear forecast.

use crate::params::Params;
use crate::rng::Rng;
use crate::NBITS;

/// A single classifier rule.
///
/// The condition is a ternary string over `NBITS` positions, each 0, 1, or #
/// (don't-care). We store it as two bitmasks: `mask` marks the *specified*
/// positions and `value` holds the required bit there. A rule matches state `s`
/// iff `s & mask == value` (with `value` already masked).
///
/// The forecast is linear: `E[p_{t+1}+d_{t+1}] = a*(p_t+d_t) + b`.
#[derive(Clone, Debug)]
pub struct Rule {
    pub mask: u64,
    pub value: u64,
    pub a: f64,
    pub b: f64,
    /// Moving average of squared forecast error — lower is better. Doubles as the
    /// agent's forecast variance `sigma^2` when this rule is the active one.
    pub accuracy: f64,
    /// Bounded strength used for selection: `base - accuracy - cost*specificity`.
    pub fitness: f64,
    /// How many times this rule has been the active (selected) rule.
    pub n_active: u64,
}

impl Rule {
    /// A random rule. Condition bits are specified with probability `bit_prob`
    /// (so low `bit_prob` => mostly-# general rules), forecast params drawn near
    /// the classic SFI-ASM ranges (which happen to bracket the REE values).
    pub fn random(rng: &mut Rng, p: &Params) -> Self {
        let mut mask = 0u64;
        let mut value = 0u64;
        for i in 0..NBITS {
            if rng.chance(p.bit_prob) {
                mask |= 1 << i;
                if rng.chance(0.5) {
                    value |= 1 << i;
                }
            }
        }
        let a = rng.uniform(p.a_min, p.a_max);
        let b = rng.uniform(-10.0, 19.0);
        let mut rule = Self { mask, value, a, b, accuracy: p.init_variance, fitness: 0.0, n_active: 0 };
        rule.recompute_fitness(p);
        rule
    }

    #[inline]
    pub fn matches(&self, state: u64) -> bool {
        (state & self.mask) == self.value
    }

    /// Number of specified (non-#) condition bits.
    #[inline]
    pub fn specificity(&self) -> u32 {
        self.mask.count_ones()
    }

    /// Count of specified *technical* bits (indices in `TECHNICAL_BITS`). Used as
    /// the observable for "is technical trading being used".
    pub fn technical_bits(&self) -> u32 {
        let mut tech_mask = 0u64;
        for i in crate::TECHNICAL_BITS {
            tech_mask |= 1 << i;
        }
        (self.mask & tech_mask).count_ones()
    }

    #[inline]
    pub fn recompute_fitness(&mut self, p: &Params) {
        self.fitness = p.fitness_base - self.accuracy - p.spec_cost * self.specificity() as f64;
    }

    /// Bayesian-style exponential update of the forecast-error variance, then refit.
    #[inline]
    pub fn update_accuracy(&mut self, err: f64, p: &Params) {
        self.accuracy = (1.0 - p.theta) * self.accuracy + p.theta * err * err;
        self.recompute_fitness(p);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wildcards_match_anything() {
        let p = Params::classic();
        // All-# rule (mask = 0) matches every state.
        let rule = Rule { mask: 0, value: 0, a: 1.0, b: 0.0, accuracy: 1.0, fitness: 0.0, n_active: 0 };
        for s in [0u64, 0xFFF, 0b1010_1010, 1] {
            assert!(rule.matches(s));
        }
        let _ = p;
    }

    #[test]
    fn specified_bits_must_match() {
        // Require bit0 = 1 and bit3 = 0.
        let mask = (1 << 0) | (1 << 3);
        let value = 1 << 0;
        let rule = Rule { mask, value, a: 1.0, b: 0.0, accuracy: 1.0, fitness: 0.0, n_active: 0 };
        assert!(rule.matches(0b0001));
        assert!(rule.matches(0b0101)); // bit2 set is irrelevant (it's #)
        assert!(!rule.matches(0b1001)); // bit3 set -> fails
        assert!(!rule.matches(0b0000)); // bit0 clear -> fails
    }
}
