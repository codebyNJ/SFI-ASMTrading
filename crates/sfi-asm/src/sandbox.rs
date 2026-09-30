//! Path-3 trading sandbox.
//!
//! A real daily price series plays forward (looping). Each trader is a Santa Fe
//! style learner that is now a *price-taker*: it forecasts tomorrow's return from
//! technical signals using a population of condition->forecast rules, takes a
//! long/short position, compounds its wealth on the realized move, and evolves its
//! rules (keeping accurate ones, copying fit neighbors). Traders belong to
//! cohorts with different temperaments, so one run can mix e.g. calm + bubbly.
//!
//! There is no market clearing here — price is exogenous (the real stock). What
//! we study is behaviour: who profits, how the crowd leans, and whether herding
//! lines up with what the price actually does.

use crate::network::{Network, Topology};
use crate::rng::Rng;

/// Technical state bits describing the market on a given day (no fundamental).
pub const SB_BITS: usize = 8;

/// A group of traders that share a behavioural style.
#[derive(Clone, Debug)]
pub struct Cohort {
    pub name: String,
    pub count: usize,
    /// Learning memory (theta): low = patient/long memory, high = jumpy/reactive.
    pub memory: f64,
    /// Probability of copying a neighbour's rule when evolving (0..1).
    pub herding: f64,
    /// Position aggressiveness — scales how big a bet a forecast becomes.
    pub risk: f64,
}

impl Cohort {
    pub fn new(name: &str, count: usize, memory: f64, herding: f64, risk: f64) -> Self {
        Self { name: name.to_string(), count, memory, herding, risk }
    }
}

#[derive(Clone, Debug)]
pub struct SandboxParams {
    pub rules_per_trader: usize,
    pub ga_interval: u64,
    pub gen_fraction: f64,
    pub p_mutation: f64,
    pub crossover_prob: f64,
    pub bit_prob: f64,
    pub spec_cost: f64,
    pub max_leverage: f64,
    pub init_wealth: f64,
    pub init_var: f64,
    pub topology: Topology,
    pub seed: u64,
    /// Round-trip friction charged on turnover, in basis points of the traded
    /// notional (`|position_today - position_yesterday| * cost_bps / 1e4`).
    /// 0 keeps the classic frictionless behaviour.
    pub cost_bps: f64,
}

impl Default for SandboxParams {
    fn default() -> Self {
        Self {
            rules_per_trader: 60,
            ga_interval: 40,
            gen_fraction: 0.2,
            p_mutation: 0.04,
            crossover_prob: 0.1,
            bit_prob: 0.15,
            spec_cost: 1e-5,
            max_leverage: 1.0,
            init_wealth: 1000.0,
            init_var: 4e-4,
            topology: Topology::ScaleFree { m: 2 },
            seed: 42,
            cost_bps: 0.0,
        }
    }
}

/// A rule: a ternary condition over the state bits and a linear return forecast
/// `r_hat = a * momentum + b`.
#[derive(Clone, Debug)]
struct Rule {
    mask: u32,
    value: u32,
    a: f64,
    b: f64,
    /// Moving forecast-error variance — lower is better.
    accuracy: f64,
    fitness: f64,
}

impl Rule {
    fn random(rng: &mut Rng, p: &SandboxParams) -> Self {
        let mut mask = 0u32;
        let mut value = 0u32;
        for i in 0..SB_BITS {
            if rng.chance(p.bit_prob) {
                mask |= 1 << i;
                if rng.chance(0.5) {
                    value |= 1 << i;
                }
            }
        }
        let mut r = Self {
            mask,
            value,
            a: rng.uniform(-2.0, 2.0),
            b: rng.uniform(-0.005, 0.005),
            accuracy: p.init_var,
            fitness: 0.0,
        };
        r.refit(p);
        r
    }

    #[inline]
    fn matches(&self, state: u32) -> bool {
        (state & self.mask) == self.value
    }

    #[inline]
    fn specificity(&self) -> u32 {
        self.mask.count_ones()
    }

    #[inline]
    fn refit(&mut self, p: &SandboxParams) {
        // Higher fitness = lower error, lightly penalizing over-specific rules.
        self.fitness = -self.accuracy - p.spec_cost * self.specificity() as f64;
    }

    #[inline]
    fn learn(&mut self, err: f64, memory: f64, p: &SandboxParams) {
        self.accuracy = (1.0 - memory) * self.accuracy + memory * err * err;
        self.refit(p);
    }
}

#[derive(Clone, Debug)]
pub struct Trader {
    pub cohort: usize,
    rules: Vec<Rule>,
    pub wealth: f64,
    /// Held fraction of wealth in the stock this day: >0 long, <0 short.
    pub position: f64,
    /// Last day's wealth change as a fraction (for mood/expression).
    pub last_pnl: f64,
    next_ga: u64,
    // --- read-only instrumentation: the rule that drove today's decision ---
    /// Momentum sensitivity `a` of the active rule (>0 trend, <0 contrarian).
    pub active_a: f64,
    pub active_b: f64,
    /// Active rule's forecast-error variance (lower = more confident).
    pub active_acc: f64,
    /// Forecast return `r_hat` the active rule produced today.
    pub active_forecast: f64,
    /// Condition mask of the active rule (which of the 8 signals it uses).
    pub active_mask: u32,
}

impl Trader {
    fn new(rng: &mut Rng, cohort: usize, p: &SandboxParams) -> Self {
        let rules = (0..p.rules_per_trader).map(|_| Rule::random(rng, p)).collect();
        Self {
            cohort,
            rules,
            wealth: p.init_wealth,
            position: 0.0,
            last_pnl: 0.0,
            next_ga: rng.below(p.ga_interval.max(1) as usize) as u64,
            active_a: 0.0,
            active_b: 0.0,
            active_acc: p.init_var,
            active_forecast: 0.0,
            active_mask: 0,
        }
    }

    /// Update *every* rule that matches the current state against the realized
    /// return (the classifier "match set"). This is what lets selection find the
    /// genuinely best forecaster instead of locking in the first one chosen.
    fn learn_all(&mut self, state: u32, momentum: f64, realized: f64, memory: f64, p: &SandboxParams) {
        for r in &mut self.rules {
            if r.matches(state) {
                let r_hat = r.a * momentum + r.b;
                r.learn(realized - r_hat, memory, p);
            }
        }
    }

    /// Best matching rule's forecast params + its accuracy. Falls back to a
    /// fitness-weighted average when nothing matches.
    fn forecast(&self, state: u32) -> (f64, f64, f64, Option<usize>) {
        let mut best: Option<usize> = None;
        let mut best_fit = f64::NEG_INFINITY;
        for (i, r) in self.rules.iter().enumerate() {
            if r.matches(state) && r.fitness > best_fit {
                best_fit = r.fitness;
                best = Some(i);
            }
        }
        match best {
            Some(i) => {
                let r = &self.rules[i];
                (r.a, r.b, r.accuracy, Some(i))
            }
            None => {
                let min_fit = self.rules.iter().map(|r| r.fitness).fold(f64::INFINITY, f64::min);
                let (mut wa, mut wb, mut wv, mut ws) = (0.0, 0.0, 0.0, 0.0);
                for r in &self.rules {
                    let w = (r.fitness - min_fit) + 1e-9;
                    ws += w;
                    wa += w * r.a;
                    wb += w * r.b;
                    wv += w * r.accuracy;
                }
                if ws <= 0.0 {
                    (0.0, 0.0, 1.0, None)
                } else {
                    (wa / ws, wb / ws, wv / ws, None)
                }
            }
        }
    }
}

/// Live results of a run.
#[derive(Clone, Debug, Default)]
pub struct Outcomes {
    pub day: u64,
    pub price: f64,
    /// Fraction of traders that are net long (bullish).
    pub sentiment: f64,
    /// How one-sided the crowd is: 0 = evenly split, 1 = everyone same way.
    pub herding: f64,
    pub avg_wealth: f64,
    /// Wealth of a passive buy-and-hold investor who started with `init_wealth`
    /// on the same days the town traded — the baseline any strategy must beat.
    pub benchmark_wealth: f64,
    pub cohort_names: Vec<String>,
    pub cohort_wealth: Vec<f64>,
}

pub struct Sandbox {
    pub params: SandboxParams,
    pub cohorts: Vec<Cohort>,
    pub prices: Vec<f64>,
    pub idx: usize,
    pub day: u64,
    pub traders: Vec<Trader>,
    pub network: Network,
    /// Buy-and-hold baseline, compounded on exactly the days traders settle.
    pub benchmark_wealth: f64,
    rng: Rng,
}

impl Sandbox {
    pub fn new(prices: Vec<f64>, cohorts: Vec<Cohort>, params: SandboxParams) -> Self {
        let mut rng = Rng::new(params.seed);
        let mut traders = Vec::new();
        for (ci, c) in cohorts.iter().enumerate() {
            for _ in 0..c.count {
                traders.push(Trader::new(&mut rng, ci, &params));
            }
        }
        let network = Network::build(traders.len(), params.topology, &mut rng);
        let benchmark_wealth = params.init_wealth;
        Self { params, cohorts, prices, idx: 0, day: 0, traders, network, benchmark_wealth, rng }
    }

    /// Advance one trading day: forecast -> position -> realize -> learn -> evolve.
    pub fn step(&mut self) {
        let n = self.prices.len();
        if n < 2 {
            return;
        }
        let p = self.prices[self.idx];
        let wraps = self.idx + 1 >= n;
        let next_idx = if wraps { 0 } else { self.idx + 1 };
        let p_next = self.prices[next_idx];
        // The loop seam (last day -> first day) is not a real market move: the
        // "return" p[0]/p[n-1] is years of drift in one step. Settling or learning
        // on it would hand every trader a fake crash/boom, so the town sits out
        // that day and flattens its book.
        if wraps || p <= 0.0 || p_next <= 0.0 {
            for t in &mut self.traders {
                t.position = 0.0;
                t.last_pnl = 0.0;
            }
            self.idx = next_idx;
            self.day += 1;
            return;
        }
        let simple = p_next / p - 1.0;
        let realized = (p_next / p).ln();

        let state = state_bits(&self.prices, self.idx);
        let momentum = mean_return_k(&self.prices, self.idx, 5);
        let sigma = sigma_recent(&self.prices, self.idx, 20).max(0.005);
        let cost_rate = self.params.cost_bps.max(0.0) * 1e-4;
        self.benchmark_wealth *= 1.0 + simple;

        for trader in &mut self.traders {
            let cohort = &self.cohorts[trader.cohort];
            let (a, b, var, ridx) = trader.forecast(state);
            let r_hat = a * momentum + b;
            // Record the active rule's reasoning (read-only, for the UI inspector).
            trader.active_a = a;
            trader.active_b = b;
            trader.active_acc = var;
            trader.active_forecast = r_hat;
            trader.active_mask = ridx.map(|i| trader.rules[i].mask).unwrap_or(0);
            // Bet sized by forecast / recent volatility, scaled by cohort risk.
            let pos = (cohort.risk * r_hat / sigma).clamp(-self.params.max_leverage, self.params.max_leverage);
            let turnover = (pos - trader.position).abs();
            trader.position = pos;

            // Realize P&L on the day's actual move, net of trading friction.
            trader.last_pnl = pos * simple - cost_rate * turnover;
            trader.wealth = (trader.wealth * (1.0 + trader.last_pnl)).max(1e-6);

            // Learn: update every rule that matched today against the real move.
            trader.learn_all(state, momentum, realized, cohort.memory, &self.params);
        }

        self.evolve();

        self.idx = next_idx;
        self.day += 1;
    }

    /// Run the GA for every trader whose turn it is, with neighbour copying.
    fn evolve(&mut self) {
        let day = self.day;
        if !self.traders.iter().any(|t| day >= t.next_ga) {
            return;
        }
        // Snapshot rule pools so neighbours are read from one consistent state.
        // GA turns are staggered, so only a few traders evolve on any given day:
        // copy just their pools and their neighbours' instead of the whole town.
        // (Iteration order is unchanged, so results are bit-identical.)
        let mut snapshot: Vec<Option<Vec<Rule>>> = vec![None; self.traders.len()];
        for i in 0..self.traders.len() {
            if day < self.traders[i].next_ga {
                continue;
            }
            for &j in std::iter::once(&i).chain(self.network.neighbors(i)) {
                if snapshot[j].is_none() {
                    snapshot[j] = Some(self.traders[j].rules.clone());
                }
            }
        }
        for i in 0..self.traders.len() {
            if day < self.traders[i].next_ga {
                continue;
            }
            let herding = self.cohorts[self.traders[i].cohort].herding;
            let pools: Vec<&[Rule]> = self
                .network
                .neighbors(i)
                .iter()
                .filter_map(|&j| snapshot[j].as_deref())
                .collect();
            let own = snapshot[i].as_deref().unwrap_or(&[]);
            run_ga(&mut self.traders[i].rules, own, &pools, herding, &mut self.rng, &self.params);
            self.traders[i].next_ga = day + self.params.ga_interval;
        }
    }

    pub fn price(&self) -> f64 {
        self.prices.get(self.idx).copied().unwrap_or(0.0)
    }

    pub fn outcomes(&self) -> Outcomes {
        let n = self.traders.len().max(1) as f64;
        let longs = self.traders.iter().filter(|t| t.position > 0.0).count() as f64;
        let sign_sum: f64 = self.traders.iter().map(|t| t.position.signum()).sum();
        let avg_wealth = self.traders.iter().map(|t| t.wealth).sum::<f64>() / n;

        let mut cohort_wealth = vec![0.0; self.cohorts.len()];
        let mut cohort_count = vec![0.0; self.cohorts.len()];
        for t in &self.traders {
            cohort_wealth[t.cohort] += t.wealth;
            cohort_count[t.cohort] += 1.0;
        }
        for (w, c) in cohort_wealth.iter_mut().zip(cohort_count.iter()) {
            if *c > 0.0 {
                *w /= *c;
            }
        }

        Outcomes {
            day: self.day,
            price: self.price(),
            sentiment: longs / n,
            herding: (sign_sum / n).abs(),
            avg_wealth,
            benchmark_wealth: self.benchmark_wealth,
            cohort_names: self.cohorts.iter().map(|c| c.name.clone()).collect(),
            cohort_wealth,
        }
    }
}

// ---------- technical signals ----------

fn ma(prices: &[f64], idx: usize, win: usize) -> f64 {
    let lo = idx.saturating_sub(win - 1);
    let slice = &prices[lo..=idx];
    slice.iter().sum::<f64>() / slice.len() as f64
}

fn mean_return_k(prices: &[f64], idx: usize, k: usize) -> f64 {
    if idx < k || prices[idx - k] <= 0.0 || prices[idx] <= 0.0 {
        return 0.0;
    }
    (prices[idx] / prices[idx - k]).ln() / k as f64
}

fn sigma_recent(prices: &[f64], idx: usize, win: usize) -> f64 {
    if idx < 2 {
        return 0.02;
    }
    let lo = idx.saturating_sub(win);
    let mut rets = Vec::new();
    for i in (lo + 1)..=idx {
        if prices[i - 1] > 0.0 && prices[i] > 0.0 {
            rets.push((prices[i] / prices[i - 1]).ln());
        }
    }
    if rets.len() < 2 {
        return 0.02;
    }
    let m = rets.iter().sum::<f64>() / rets.len() as f64;
    let v = rets.iter().map(|r| (r - m) * (r - m)).sum::<f64>() / rets.len() as f64;
    v.sqrt()
}

fn state_bits(prices: &[f64], idx: usize) -> u32 {
    let p = prices[idx];
    let ma5 = ma(prices, idx, 5);
    let ma20 = ma(prices, idx, 20);
    let ma50 = ma(prices, idx, 50);
    let last_ret = if idx >= 1 && prices[idx - 1] > 0.0 { (p / prices[idx - 1]).ln() } else { 0.0 };
    let sig = sigma_recent(prices, idx, 20);
    let mom5 = mean_return_k(prices, idx, 5);

    let mut s = 0u32;
    if p > ma5 {
        s |= 1 << 0;
    }
    if p > ma20 {
        s |= 1 << 1;
    }
    if p > ma50 {
        s |= 1 << 2;
    }
    if ma5 > ma20 {
        s |= 1 << 3;
    }
    if ma20 > ma50 {
        s |= 1 << 4;
    }
    if last_ret > 0.0 {
        s |= 1 << 5;
    }
    if last_ret.abs() > sig {
        s |= 1 << 6;
    }
    if mom5 > 0.0 {
        s |= 1 << 7;
    }
    s
}

// ---------- genetic algorithm ----------

fn run_ga(
    rules: &mut [Rule],
    own: &[Rule],
    neighbor_pools: &[&[Rule]],
    herding: f64,
    rng: &mut Rng,
    p: &SandboxParams,
) {
    let m = rules.len();
    if m < 2 {
        return;
    }
    let n_new = ((p.gen_fraction * m as f64).round() as usize).clamp(1, m);
    let inherited = own.iter().map(|r| r.accuracy).sum::<f64>() / own.len().max(1) as f64;

    let mut children: Vec<Rule> = Vec::with_capacity(n_new);
    while children.len() < n_new {
        let social = !neighbor_pools.is_empty() && rng.chance(herding);
        let mut child = if social {
            let pool = neighbor_pools[rng.below(neighbor_pools.len())];
            crossover(&pool[tournament(pool, rng)], &own[tournament(own, rng)], rng)
        } else if rng.chance(p.crossover_prob) {
            crossover(&own[tournament(own, rng)], &own[tournament(own, rng)], rng)
        } else {
            own[tournament(own, rng)].clone()
        };
        mutate(&mut child, rng, p);
        child.accuracy = inherited;
        child.refit(p);
        children.push(child);
    }

    let mut order: Vec<usize> = (0..m).collect();
    // total_cmp: a NaN fitness must not panic the whole run mid-GA.
    order.sort_by(|&i, &j| rules[i].fitness.total_cmp(&rules[j].fitness));
    for (k, child) in children.into_iter().enumerate() {
        rules[order[k]] = child;
    }
}

fn tournament(pool: &[Rule], rng: &mut Rng) -> usize {
    let i = rng.below(pool.len());
    let j = rng.below(pool.len());
    if pool[i].fitness >= pool[j].fitness {
        i
    } else {
        j
    }
}

fn crossover(a: &Rule, b: &Rule, rng: &mut Rng) -> Rule {
    let mut mask = 0u32;
    let mut value = 0u32;
    for i in 0..SB_BITS {
        let bit = 1u32 << i;
        let src = if rng.chance(0.5) { a } else { b };
        if src.mask & bit != 0 {
            mask |= bit;
            value |= src.value & bit;
        }
    }
    Rule {
        mask,
        value,
        a: if rng.chance(0.5) { a.a } else { b.a },
        b: if rng.chance(0.5) { a.b } else { b.b },
        accuracy: a.accuracy.max(b.accuracy),
        fitness: 0.0,
    }
}

fn mutate(rule: &mut Rule, rng: &mut Rng, p: &SandboxParams) {
    for i in 0..SB_BITS {
        if rng.chance(p.p_mutation) {
            let bit = 1u32 << i;
            if rng.chance(p.bit_prob) {
                rule.mask |= bit;
                if rng.chance(0.5) {
                    rule.value |= bit;
                } else {
                    rule.value &= !bit;
                }
            } else {
                rule.mask &= !bit;
                rule.value &= !bit;
            }
        }
    }
    if rng.chance(p.p_mutation) {
        rule.a = (rule.a + rng.gaussian() * 0.2).clamp(-3.0, 3.0);
    }
    if rng.chance(p.p_mutation) {
        rule.b = (rule.b + rng.gaussian() * 0.002).clamp(-0.05, 0.05);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn uptrend(n: usize) -> Vec<f64> {
        (0..n).map(|i| 100.0 * (1.0 + 0.001 * i as f64)).collect()
    }

    fn default_cohorts() -> Vec<Cohort> {
        vec![
            Cohort::new("calm", 15, 0.02, 0.1, 0.8),
            Cohort::new("bubbly", 15, 0.08, 0.6, 1.5),
        ]
    }

    #[test]
    fn deterministic_for_same_seed() {
        let run = || {
            let mut s = Sandbox::new(uptrend(400), default_cohorts(), SandboxParams::default());
            for _ in 0..3000 {
                s.step();
            }
            s.outcomes().avg_wealth
        };
        assert_eq!(run().to_bits(), run().to_bits());
    }

    #[test]
    fn invariants_and_loop_safety() {
        // Run far past the data length to exercise looping.
        let mut s = Sandbox::new(uptrend(300), default_cohorts(), SandboxParams::default());
        for _ in 0..5000 {
            s.step();
        }
        let o = s.outcomes();
        assert!(o.sentiment >= 0.0 && o.sentiment <= 1.0);
        assert!(o.herding >= 0.0 && o.herding <= 1.0);
        for t in &s.traders {
            assert!(t.wealth.is_finite() && t.wealth > 0.0);
            assert!(t.position.abs() <= s.params.max_leverage + 1e-9);
        }
        assert_eq!(o.cohort_wealth.len(), 2);
    }

    #[test]
    fn traders_profit_on_a_steady_uptrend() {
        // A long single-pass uptrend (no loop boundary, so no artificial crash):
        // learners should figure out "go long" and end up net long and richer.
        let n = 3000;
        let mut s = Sandbox::new(uptrend(n), default_cohorts(), SandboxParams::default());
        for _ in 0..(n - 1) {
            s.step();
        }
        
        let o = s.outcomes();
        assert!(o.avg_wealth > s.params.init_wealth, "avg wealth {} did not grow", o.avg_wealth);
        assert!(o.sentiment > 0.5, "crowd should lean long on an uptrend, got {}", o.sentiment);
    }

    #[test]
    fn loop_seam_is_not_traded() {
        // Price triples over the series, so wrapping p[n-1] -> p[0] would be a
        // ~-67% one-day "crash" if it were settled.
        let n = 200;
        let prices: Vec<f64> = (0..n).map(|i| 100.0 * (1.0 + 2.0 * i as f64 / n as f64)).collect();
        let mut s = Sandbox::new(prices, default_cohorts(), SandboxParams::default());
        for _ in 0..(n - 1) {
            s.step();
        }
        assert_eq!(s.idx, n - 1);
        let before: Vec<f64> = s.traders.iter().map(|t| t.wealth).collect();
        let bench_before = s.benchmark_wealth;
        s.step(); // the seam
        assert_eq!(s.idx, 0);
        for (t, w) in s.traders.iter().zip(before) {
            assert_eq!(t.wealth, w, "wealth must not move on the loop seam");
            assert_eq!(t.position, 0.0);
            assert_eq!(t.last_pnl, 0.0);
        }
        assert_eq!(s.benchmark_wealth, bench_before);
    }

    #[test]
    fn benchmark_tracks_buy_and_hold() {
        let n = 500;
        let prices = uptrend(n);
        let mut s = Sandbox::new(prices.clone(), default_cohorts(), SandboxParams::default());
        for _ in 0..(n - 1) {
            s.step();
        }
        let expected = s.params.init_wealth * prices[n - 1] / prices[0];
        let got = s.outcomes().benchmark_wealth;
        assert!((got / expected - 1.0).abs() < 1e-9, "benchmark {got} vs buy&hold {expected}");
    }

    #[test]
    fn transaction_costs_only_reduce_wealth() {
        let run = |cost_bps: f64| {
            let params = SandboxParams { cost_bps, ..SandboxParams::default() };
            let mut s = Sandbox::new(uptrend(800), default_cohorts(), params);
            for _ in 0..799 {
                s.step();
            }
            s.outcomes().avg_wealth
        };
        let free = run(0.0);
        let costly = run(25.0);
        assert!(costly.is_finite() && costly > 0.0);
        assert!(costly < free, "costs should bite: {costly} vs frictionless {free}");
    }
}
