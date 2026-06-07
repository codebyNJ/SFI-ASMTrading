//! Genetic algorithm: each agent independently evolves its rule population.
//!
//! Every `tau` (`ga_interval`) periods an agent replaces its weakest rules with
//! offspring of fit parents (tournament selection + crossover + mutation).
//!
//! The mutation operator here is **drift-neutral**: when a condition trit mutates
//! it is re-drawn from the stationary symbol distribution (specified with
//! probability `bit_prob`, else #). This keeps the aggregate #-density stationary,
//! which is exactly what Ehrentreich (2002) showed the original SFI-ASM operator
//! failed to do — its bias injected technical bits over time and made "emergent
//! technical trading" partly an artifact of genetic drift rather than selection.

use crate::agent::Agent;
use crate::classifier::Rule;
use crate::params::Params;
use crate::rng::Rng;
use crate::NBITS;

// Mutation step sizes for the linear forecast parameters.
const A_MUT_SIGMA: f64 = 0.05;
const B_MUT_SIGMA: f64 = 0.50;

impl Agent {
    /// Run one purely-individual GA round (no network). Equivalent to social GA
    /// with no neighbors — kept as a convenience and for the baseline path.
    pub fn run_ga(&mut self, rng: &mut Rng, p: &Params) {
        let own = self.rules.clone();
        self.run_ga_social(rng, p, &own, &[]);
    }

    /// Run one GA round, optionally importing rules from network neighbors.
    ///
    /// `own_pool` and `neighbor_pools` are read-only snapshots taken *before* any
    /// agent evolved this period, so the whole population learns from a consistent
    /// state (and the result is order-independent and reproducible). Offspring
    /// replace this agent's weakest rules. With empty `neighbor_pools` this is the
    /// classic individual GA.
    pub fn run_ga_social(
        &mut self,
        rng: &mut Rng,
        p: &Params,
        own_pool: &[Rule],
        neighbor_pools: &[&[Rule]],
    ) {
        let m = self.rules.len();
        if m < 2 {
            return;
        }
        let n_new = ((p.gen_fraction * m as f64).round() as usize).clamp(1, m);
        let inherited_accuracy = if own_pool.is_empty() {
            1.0
        } else {
            own_pool.iter().map(|r| r.accuracy).sum::<f64>() / own_pool.len() as f64
        };

        let mut children: Vec<Rule> = Vec::with_capacity(n_new);
        while children.len() < n_new {
            let mut child = make_child(own_pool, neighbor_pools, rng, p);
            mutate(&mut child, rng, p);
            child.accuracy = inherited_accuracy;
            child.n_active = 0;
            child.recompute_fitness(p);
            children.push(child);
        }

        // Replace the weakest `n_new` rules with the offspring.
        let mut order: Vec<usize> = (0..m).collect();
        order.sort_by(|&i, &j| self.rules[i].fitness.partial_cmp(&self.rules[j].fitness).unwrap());
        for (k, child) in children.into_iter().enumerate() {
            self.rules[order[k]] = child;
        }
    }
}

/// Produce one offspring. With probability `social_prob` (and if neighbors exist)
/// one parent is imported from a random neighbor's population — this is the
/// cultural transmission channel the network experiment is built around.
fn make_child(own: &[Rule], neighbor_pools: &[&[Rule]], rng: &mut Rng, p: &Params) -> Rule {
    let use_social = !neighbor_pools.is_empty() && rng.chance(p.social_prob);
    if use_social {
        let npool = neighbor_pools[rng.below(neighbor_pools.len())];
        let parent_n = &npool[tournament(npool, rng)];
        let parent_self = &own[tournament(own, rng)];
        crossover(parent_n, parent_self, rng)
    } else if rng.chance(p.crossover_prob) {
        let a = &own[tournament(own, rng)];
        let b = &own[tournament(own, rng)];
        crossover(a, b, rng)
    } else {
        own[tournament(own, rng)].clone()
    }
}

/// Binary tournament over a rule pool: draw two, return the index of the fitter.
fn tournament(pool: &[Rule], rng: &mut Rng) -> usize {
    let i = rng.below(pool.len());
    let j = rng.below(pool.len());
    if pool[i].fitness >= pool[j].fitness {
        i
    } else {
        j
    }
}

/// Uniform crossover on the condition (per bit) and forecast params (per param).
fn crossover(a: &Rule, b: &Rule, rng: &mut Rng) -> Rule {
    let mut mask = 0u64;
    let mut value = 0u64;
    for i in 0..NBITS {
        let src = if rng.chance(0.5) { a } else { b };
        let bit = 1u64 << i;
        if src.mask & bit != 0 {
            mask |= bit;
            value |= src.value & bit;
        }
    }
    let fa = if rng.chance(0.5) { a.a } else { b.a };
    let fb = if rng.chance(0.5) { a.b } else { b.b };
    Rule { mask, value, a: fa, b: fb, accuracy: a.accuracy.max(b.accuracy), fitness: 0.0, n_active: 0 }
}

/// Drift-neutral mutation (see module docs).
fn mutate(rule: &mut Rule, rng: &mut Rng, p: &Params) {
    for i in 0..NBITS {
        if rng.chance(p.p_mutation) {
            let bit = 1u64 << i;
            if rng.chance(p.bit_prob) {
                // Re-specify: set as specified, value 0/1 each with prob 1/2.
                rule.mask |= bit;
                if rng.chance(0.5) {
                    rule.value |= bit;
                } else {
                    rule.value &= !bit;
                }
            } else {
                // Generalize to #.
                rule.mask &= !bit;
                rule.value &= !bit;
            }
        }
    }
    if rng.chance(p.p_mutation) {
        rule.a = (rule.a + rng.gaussian() * A_MUT_SIGMA).clamp(p.a_min, p.a_max);
    }
    if rng.chance(p.p_mutation) {
        rule.b = (rule.b + rng.gaussian() * B_MUT_SIGMA).clamp(-50.0, 50.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ga_keeps_population_size_and_invariants() {
        let p = Params::classic();
        let mut rng = Rng::new(3);
        let mut agent = Agent::new(&mut rng, &p, 0);
        let before = agent.rules.len();
        for _ in 0..50 {
            agent.run_ga(&mut rng, &p);
        }
        assert_eq!(agent.rules.len(), before);
        for r in &agent.rules {
            // value must never specify a bit outside the mask.
            assert_eq!(r.value & !r.mask, 0);
            assert!(r.a.is_finite() && r.b.is_finite());
        }
    }

    // Specified-bit density across a slice of rules.
    fn density(rules: &[Rule]) -> f64 {
        rules.iter().map(|r| r.specificity() as f64).sum::<f64>() / (rules.len() as f64 * NBITS as f64)
    }

    #[test]
    fn mutation_operator_is_drift_neutral() {
        // Isolate the mutation operator from selection/replacement: applying it
        // repeatedly must drive the specified-bit density toward `bit_prob` from
        // *either* extreme. This is exactly the Ehrentreich (2002) property the
        // original SFI-ASM operator lacked.
        let mut p = Params::classic();
        p.p_mutation = 0.1;
        let mut rng = Rng::new(11);

        let make = |mask: u64| Rule { mask, value: 0, a: 1.0, b: 0.0, accuracy: 1.0, fitness: 0.0, n_active: 0 };
        // Start one population all-# (density 0) and one all-specified (density 1).
        let mut all_wild = vec![make(0); 400];
        let full = (1u64 << NBITS) - 1;
        let mut all_spec = vec![make(full); 400];

        for _ in 0..600 {
            for r in all_wild.iter_mut().chain(all_spec.iter_mut()) {
                mutate(r, &mut rng, &p);
            }
        }
        // Both converge to the stationary density `bit_prob`, regardless of start.
        assert!((density(&all_wild) - p.bit_prob).abs() < 0.03, "from-0 -> {}", density(&all_wild));
        assert!((density(&all_spec) - p.bit_prob).abs() < 0.03, "from-1 -> {}", density(&all_spec));
    }
}
