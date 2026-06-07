//! An agent: a population of classifier rules plus the bookkeeping needed to
//! select, act, and learn each period.

use crate::classifier::Rule;
use crate::params::Params;
use crate::rng::Rng;

/// The forecast an agent contributes to market clearing this period:
/// `(a, b, variance)` of its active rule.
pub type Forecast = (f64, f64, f64);

#[derive(Clone, Debug)]
pub struct Agent {
    pub rules: Vec<Rule>,
    /// Index of the rule selected this period (None => no rule matched; fallback used).
    pub active_rule: Option<usize>,
    /// The rule that produced `prev_forecast` (selected last period).
    pub prev_rule: Option<usize>,
    /// Forecast of the *current* period's `p+d`, made last period. Scored once known.
    pub prev_forecast: f64,
    pub forecast_valid: bool,
    /// Next period index at which this agent runs its GA.
    pub next_ga: u64,
    /// This agent's most recent share demand at the cleared price (diagnostics).
    pub demand: f64,
}

impl Agent {
    pub fn new(rng: &mut Rng, p: &Params, first_ga: u64) -> Self {
        let rules = (0..p.n_rules).map(|_| Rule::random(rng, p)).collect();
        Self {
            rules,
            active_rule: None,
            prev_rule: None,
            prev_forecast: 0.0,
            forecast_valid: false,
            next_ga: first_ga,
            demand: 0.0,
        }
    }

    /// Pick the highest-fitness rule whose condition matches `state` and return its
    /// forecast parameters. If nothing matches, fall back to a fitness-weighted
    /// average over the whole population (classic SFI-ASM behavior).
    pub fn select(&mut self, state: u64, p: &Params) -> Forecast {
        let mut best: Option<usize> = None;
        let mut best_fit = f64::NEG_INFINITY;
        for (i, rule) in self.rules.iter().enumerate() {
            if rule.matches(state) && rule.fitness > best_fit {
                best_fit = rule.fitness;
                best = Some(i);
            }
        }
        self.active_rule = best;
        match best {
            Some(i) => {
                self.rules[i].n_active += 1;
                let r = &self.rules[i];
                (r.a, r.b, r.accuracy.max(p.min_variance))
            }
            None => {
                let (a, b, v) = self.weighted_average();
                (a, b, v.max(p.min_variance))
            }
        }
    }

    /// Fitness-weighted mean of forecast params over all rules (fallback forecast).
    fn weighted_average(&self) -> Forecast {
        let min_fit = self.rules.iter().map(|r| r.fitness).fold(f64::INFINITY, f64::min);
        let mut wsum = 0.0;
        let (mut a, mut b, mut v) = (0.0, 0.0, 0.0);
        for r in &self.rules {
            let w = (r.fitness - min_fit) + 1e-6;
            wsum += w;
            a += w * r.a;
            b += w * r.b;
            v += w * r.accuracy;
        }
        if wsum <= 0.0 {
            return (1.0, 0.0, 1.0);
        }
        (a / wsum, b / wsum, v / wsum)
    }

    /// Mean accuracy across the population (new GA children inherit this).
    pub fn mean_accuracy(&self) -> f64 {
        if self.rules.is_empty() {
            return 1.0;
        }
        self.rules.iter().map(|r| r.accuracy).sum::<f64>() / self.rules.len() as f64
    }

    /// Average number of specified technical bits across active rules — the
    /// experiment's headline observable.
    pub fn mean_technical_bits(&self) -> f64 {
        if self.rules.is_empty() {
            return 0.0;
        }
        self.rules.iter().map(|r| r.technical_bits() as f64).sum::<f64>() / self.rules.len() as f64
    }
}
