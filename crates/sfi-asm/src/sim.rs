//! The simulation: wires dividend, agents, clearing, learning, GA, and stats
//! into one deterministic period loop.

use std::collections::VecDeque;

use crate::agent::{Agent, Forecast};
use crate::classifier::Rule;
use crate::dividend::{Dividend, Ree};
use crate::market;
use crate::network::{Network, Topology};
use crate::params::Params;
use crate::rng::Rng;
use crate::stats::ReturnStats;

/// Window (periods) over which moving averages and return stats are computed.
const MA_MAX: usize = 500;
const RETURN_WINDOW: usize = 500;

/// Price/dividend ratio thresholds for fundamental bits 0..=5 (`p*r/d` vs these).
const PD_THRESHOLDS: [f64; 6] = [0.25, 0.5, 0.75, 0.875, 1.0, 1.125];
/// Moving-average windows for technical bits 6..=9.
const MA_WINDOWS: [usize; 4] = [5, 10, 100, 500];

pub struct Simulation {
    pub params: Params,
    pub rng: Rng,
    pub dividend: Dividend,
    pub ree: Ree,
    pub agents: Vec<Agent>,
    pub network: Network,

    /// Most recent cleared price (this is `p_{t-1}` while clearing period `t`).
    pub price: f64,
    pub t: u64,

    /// Recent cleared prices (newest at back), for moving averages.
    price_hist: VecDeque<f64>,
    /// REE benchmark aligned with `price_hist` (for the fair-value band in the UI).
    ree_hist: VecDeque<f64>,
    /// Recent log returns (newest at back), for rolling statistics.
    returns: VecDeque<f64>,
    /// Number of times `advance()` has been called (UI-driven stepping).
    steps_done: u64,

    /// Scratch buffer reused each period to avoid per-tick allocation.
    forecasts: Vec<Forecast>,
}

impl Simulation {
    pub fn new(params: Params) -> Self {
        let mut rng = Rng::new(params.seed);
        let dividend = Dividend::new(&params);
        let ree = Ree::new(&params);

        // Stagger first GA times across [0, ga_interval) so agents don't all
        // evolve on the same period (matches the paper's asynchronous GA).
        let agents: Vec<Agent> = (0..params.n_agents)
            .map(|_| {
                let first = rng.below(params.ga_interval.max(1) as usize) as u64;
                Agent::new(&mut rng, &params, first)
            })
            .collect();

        // Build the social network after agents so agent construction RNG is fixed.
        let network = Network::build(params.n_agents, params.topology, &mut rng);

        let price = ree.price(params.d_bar);
        let mut price_hist = VecDeque::with_capacity(MA_MAX);
        price_hist.push_back(price);
        let mut ree_hist = VecDeque::with_capacity(MA_MAX);
        ree_hist.push_back(ree.price(params.d_bar));

        Self {
            forecasts: Vec::with_capacity(params.n_agents),
            params,
            rng,
            dividend,
            ree,
            agents,
            network,
            price,
            t: 0,
            price_hist,
            ree_hist,
            returns: VecDeque::with_capacity(RETURN_WINDOW),
            steps_done: 0,
        }
    }

    /// Advance one period. `t` is the period index (caller-supplied, monotonically
    /// increasing) used for GA scheduling.
    pub fn step(&mut self, t: u64) {
        self.t = t;

        // 1. New dividend.
        let d = self.dividend.step(&mut self.rng);

        // 2. Market state from the *lagged* price (no circularity in rule selection).
        let state = self.market_state(d);

        // 3. Each agent selects its active rule -> forecast contribution.
        self.forecasts.clear();
        for agent in &mut self.agents {
            self.forecasts.push(agent.select(state, &self.params));
        }

        // 4. Clear the market for the new price.
        let prev_price = self.price;
        let clearing = market::clear(&self.forecasts, d, &self.params, prev_price);
        let price = clearing.price;
        let realized = price + d; // the quantity forecasts try to predict

        // 5. Learn: score last period's forecast and post this period's.
        for (i, agent) in self.agents.iter_mut().enumerate() {
            if agent.forecast_valid {
                if let Some(ri) = agent.prev_rule {
                    let err = realized - agent.prev_forecast;
                    agent.rules[ri].update_accuracy(err, &self.params);
                }
            }
            let (a, b, _) = self.forecasts[i];
            agent.prev_forecast = a * realized + b;
            agent.prev_rule = agent.active_rule;
            agent.forecast_valid = true;
            agent.demand = clearing.demands[i];
        }

        // 5b. Evolve: any agents due this period run their GA.
        self.run_ga_pass(t);

        // 6. Bookkeeping: returns + price history.
        if prev_price > 0.0 && price > 0.0 {
            let ret = (price / prev_price).ln();
            self.returns.push_back(ret);
            if self.returns.len() > RETURN_WINDOW {
                self.returns.pop_front();
            }
        }
        self.price = price;
        self.price_hist.push_back(price);
        self.ree_hist.push_back(self.ree.price(d));
        if self.price_hist.len() > MA_MAX {
            self.price_hist.pop_front();
            self.ree_hist.pop_front();
        }
    }

    /// Advance one period using an internal counter (convenient for UI-driven
    /// stepping where the caller doesn't track the period index).
    pub fn advance(&mut self) {
        self.step(self.steps_done);
        self.steps_done += 1;
    }

    /// Run the genetic algorithm for every agent whose `next_ga` is due at `t`.
    ///
    /// When social learning is active we snapshot all rule pools first so neighbors
    /// are read from a single pre-GA state (order-independent, reproducible). With
    /// no network we take the cheap individual path and skip the snapshot.
    fn run_ga_pass(&mut self, t: u64) {
        let any_due = self.agents.iter().any(|a| t >= a.next_ga);
        if !any_due {
            return;
        }
        let social = self.params.social_prob > 0.0 && self.params.topology != Topology::None;

        if social {
            let snapshot: Vec<Vec<Rule>> = self.agents.iter().map(|a| a.rules.clone()).collect();
            for i in 0..self.agents.len() {
                if t >= self.agents[i].next_ga {
                    let nbrs: Vec<usize> = self.network.neighbors(i).to_vec();
                    let pools: Vec<&[Rule]> = nbrs.iter().map(|&j| snapshot[j].as_slice()).collect();
                    self.agents[i].run_ga_social(&mut self.rng, &self.params, &snapshot[i], &pools);
                    self.agents[i].next_ga = t + self.params.ga_interval;
                }
            }
        } else {
            for i in 0..self.agents.len() {
                if t >= self.agents[i].next_ga {
                    self.agents[i].run_ga(&mut self.rng, &self.params);
                    self.agents[i].next_ga = t + self.params.ga_interval;
                }
            }
        }
    }

    /// Build the `NBITS`-wide market-state descriptor from the lagged price,
    /// current dividend, and price history.
    fn market_state(&self, d: f64) -> u64 {
        let p = self.price;
        let mut s = 0u64;

        // Fundamental bits 0..=5: price/dividend ratio vs thresholds.
        let pd = if d.abs() > 1e-12 { p * self.params.r / d } else { 0.0 };
        for (i, &th) in PD_THRESHOLDS.iter().enumerate() {
            if pd > th {
                s |= 1 << i;
            }
        }

        // Technical bits 6..=9: price above moving averages.
        for (k, &w) in MA_WINDOWS.iter().enumerate() {
            if p > self.moving_average(w) {
                s |= 1 << (6 + k);
            }
        }

        // Control bits: 10 always on, 11 always off (drift detectors).
        s |= 1 << 10;

        s
    }

    /// Mean of the last `window` prices (or all available if fewer).
    fn moving_average(&self, window: usize) -> f64 {
        let n = self.price_hist.len().min(window);
        if n == 0 {
            return self.price;
        }
        let sum: f64 = self.price_hist.iter().rev().take(n).sum();
        sum / n as f64
    }

    // --- accessors / observables ---

    pub fn current_dividend(&self) -> f64 {
        self.dividend.current
    }

    /// REE price at the current dividend (the rational-regime benchmark).
    pub fn ree_price(&self) -> f64 {
        self.ree.price(self.dividend.current)
    }

    /// Signed relative deviation of price from the REE benchmark.
    pub fn relative_deviation(&self) -> f64 {
        let r = self.ree_price();
        if r.abs() < 1e-12 {
            0.0
        } else {
            (self.price - r) / r
        }
    }

    pub fn stats(&self) -> ReturnStats {
        ReturnStats::from_returns(&self.returns)
    }

    /// Population-average count of specified technical bits (the headline observable).
    pub fn mean_technical_bits(&self) -> f64 {
        if self.agents.is_empty() {
            return 0.0;
        }
        self.agents.iter().map(|a| a.mean_technical_bits()).sum::<f64>() / self.agents.len() as f64
    }

    /// Snapshot of the current `a` forecast parameter across all rules (for the
    /// belief-distribution panel later).
    pub fn forecast_a_distribution(&self) -> Vec<f64> {
        let mut v = Vec::with_capacity(self.agents.len() * self.params.n_rules);
        for agent in &self.agents {
            for rule in &agent.rules {
                v.push(rule.a);
            }
        }
        v
    }

    pub fn price_history(&self) -> &VecDeque<f64> {
        &self.price_hist
    }

    pub fn ree_history(&self) -> &VecDeque<f64> {
        &self.ree_hist
    }

    /// Period index of the most recent completed step.
    pub fn period(&self) -> u64 {
        self.steps_done
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nbits_layout_is_consistent() {
        // Sanity: our state layout uses exactly the documented bit positions.
        assert_eq!(PD_THRESHOLDS.len() + MA_WINDOWS.len() + 2, crate::NBITS);
    }

    #[test]
    fn runs_deterministically_for_same_seed() {
        let run = || {
            let mut sim = Simulation::new(Params::rational());
            for t in 0..2000 {
                sim.step(t);
            }
            (sim.price, sim.stats().excess_kurtosis)
        };
        let (p1, k1) = run();
        let (p2, k2) = run();
        assert_eq!(p1.to_bits(), p2.to_bits());
        assert_eq!(k1.to_bits(), k2.to_bits());
    }

    #[test]
    fn rational_regime_tracks_ree_on_average() {
        // Slow GA: price should stay reasonably close to the REE benchmark and
        // returns should be light-tailed (small excess kurtosis).
        let mut sim = Simulation::new(Params::rational());
        let burn = 2000u64;
        let total = 20_000u64;
        let mut dev_sum = 0.0;
        let mut cnt = 0.0;
        for t in 0..total {
            sim.step(t);
            if t >= burn {
                dev_sum += sim.relative_deviation().abs();
                cnt += 1.0;
            }
        }
        let mean_abs_dev = dev_sum / cnt;
        assert!(sim.price.is_finite() && sim.price > 0.0);
        assert!(mean_abs_dev < 0.25, "mean |deviation from REE| too large: {mean_abs_dev}");
    }
}
