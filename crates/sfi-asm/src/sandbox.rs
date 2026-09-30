//! Path-3 trading sandbox.
//!
//! A real daily price series plays forward. Each trader is a Santa Fe style
//! learner that is now a *price-taker*: it forecasts tomorrow's return from
//! technical signals using a population of condition->forecast rules, takes a
//! long/short position, compounds its wealth on the realized move, and evolves its
//! rules (keeping accurate ones, copying fit neighbors). Traders belong to
//! cohorts with different temperaments, so one run can mix e.g. calm + bubbly.
//!
//! There is no market clearing here — price is exogenous (the real stock). What
//! we study is behaviour: who profits, how the crowd leans, and whether herding
//! lines up with what the price actually does.
//!
//! What happens when the real data runs out is controlled by [`Replay`]. Replaying
//! the same history lets traders memorise it, so anything measured after the
//! first pass is in-sample; [`Replay::Bootstrap`] continues on a fresh
//! resampled path instead, and [`Replay::Stop`] ends the run.

use crate::network::{Network, Topology};
use crate::rng::Rng;

/// Technical state bits describing the market on a given day (no fundamental).
pub const SB_BITS: usize = 8;

/// Trading days per year, used to annualise Sharpe ratios.
pub const TRADING_DAYS: f64 = 252.0;

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

/// What to do when the price series reaches its last day.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Replay {
    /// Jump back to the start of the real history (after sitting out the seam
    /// day). Every pass after the first is in-sample: traders have already
    /// learned on exactly this path.
    Loop,
    /// Continue on a synthetic path built by a stationary block bootstrap of the
    /// real daily returns (mean block length `block` days). The path starts
    /// where the previous one ended, so there is no seam, and it keeps the real
    /// series' volatility and short-range structure without repeating it.
    Bootstrap { block: usize },
    /// End the run after one pass over the real data.
    Stop,
}

/// How a forecast is turned into a position.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Sizing {
    /// `risk * r_hat / sigma_price` — scale by recent market volatility only.
    Volatility,
    /// `risk * r_hat / (gamma * var_rule)` — mean-variance (SFI-style) sizing:
    /// a rule with a better forecast-error record bets more.
    MeanVariance { gamma: f64 },
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
    /// Days of history the signals need before anyone trades. The longest
    /// lookback is MA50, so trading starts at index 50 by default; before that
    /// the moving averages would be computed on partial windows.
    pub warmup: usize,
    pub replay: Replay,
    pub sizing: Sizing,
    /// A trader whose wealth falls below `bust_fraction * init_wealth` is
    /// bankrupt: it stops trading and is no longer copied by neighbours.
    pub bust_fraction: f64,
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
            warmup: 50,
            replay: Replay::Loop,
            sizing: Sizing::Volatility,
            bust_fraction: 0.01,
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
    /// Highest wealth reached so far (for drawdown).
    pub peak: f64,
    /// Worst peak-to-trough fall in wealth so far, as a fraction (0..1).
    pub max_drawdown: f64,
    /// Sum of daily `|Δposition|` — how much this trader churns.
    pub turnover: f64,
    /// Bankrupt: no longer trades or teaches neighbours.
    pub bust: bool,
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
            peak: p.init_wealth,
            max_drawdown: 0.0,
            turnover: 0.0,
            bust: false,
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

/// Running performance statistics for one return stream (a guild's pooled
/// wealth, or the buy-and-hold benchmark).
#[derive(Clone, Debug)]
struct Track {
    days: u64,
    mean: f64,
    m2: f64,
    level: f64,
    peak: f64,
    max_drawdown: f64,
}

impl Track {
    fn new(level: f64) -> Self {
        Self { days: 0, mean: 0.0, m2: 0.0, level, peak: level, max_drawdown: 0.0 }
    }

    /// Record one day's simple return (Welford update) and the new level.
    fn push(&mut self, ret: f64, level: f64) {
        self.days += 1;
        let d = ret - self.mean;
        self.mean += d / self.days as f64;
        self.m2 += d * (ret - self.mean);
        self.level = level;
        if level > self.peak {
            self.peak = level;
        } else if self.peak > 0.0 {
            self.max_drawdown = self.max_drawdown.max(1.0 - level / self.peak);
        }
    }

    /// Annualised Sharpe ratio of the daily returns (zero risk-free rate).
    fn sharpe(&self) -> f64 {
        if self.days < 2 {
            return 0.0;
        }
        let sd = (self.m2 / (self.days - 1) as f64).sqrt();
        if sd <= 1e-12 {
            0.0
        } else {
            self.mean / sd * TRADING_DAYS.sqrt()
        }
    }
}

/// Risk-adjusted results for one guild.
#[derive(Clone, Debug, Default)]
pub struct CohortStats {
    pub name: String,
    pub mean_wealth: f64,
    /// Median is reported alongside the mean because a few lucky, highly
    /// levered traders can dominate an average of compounding wealth.
    pub median_wealth: f64,
    /// `ln(mean_wealth / benchmark_wealth)`: > 0 beat buy-and-hold.
    pub excess_log_return: f64,
    /// Annualised Sharpe of the guild's pooled daily returns.
    pub sharpe: f64,
    /// Worst peak-to-trough fall of the guild's pooled wealth (0..1).
    pub max_drawdown: f64,
    /// Average `|Δposition|` per trader per trading day.
    pub turnover: f64,
    pub busts: usize,
}

/// Live results of a run.
#[derive(Clone, Debug, Default)]
pub struct Outcomes {
    pub day: u64,
    /// Days on which positions were actually settled.
    pub trading_days: u64,
    pub price: f64,
    /// Fraction of traders that are net long (bullish).
    pub sentiment: f64,
    /// How one-sided the crowd is: 0 = evenly split, 1 = everyone same way.
    pub herding: f64,
    /// Mean position across traders (-1 all short .. +1 all long at max leverage 1).
    pub net_position: f64,
    pub avg_wealth: f64,
    /// Wealth of a passive buy-and-hold investor who started with `init_wealth`
    /// on the same days the town traded — the baseline any strategy must beat.
    pub benchmark_wealth: f64,
    pub benchmark_sharpe: f64,
    pub benchmark_max_drawdown: f64,
    /// 0 = first pass over the real history; each loop/bootstrap adds one.
    pub pass: u32,
    /// True when today's price comes from a bootstrapped (synthetic) path.
    pub synthetic: bool,
    /// True once a [`Replay::Stop`] run has used up its data.
    pub finished: bool,
    pub busts: usize,
    pub cohort_names: Vec<String>,
    pub cohort_wealth: Vec<f64>,
    pub cohorts: Vec<CohortStats>,
}

pub struct Sandbox {
    pub params: SandboxParams,
    pub cohorts: Vec<Cohort>,
    /// The real history, kept as the source for bootstrapped passes.
    pub real_prices: Vec<f64>,
    /// The path currently being played (the real history on pass 0).
    pub prices: Vec<f64>,
    pub idx: usize,
    pub day: u64,
    pub pass: u32,
    pub finished: bool,
    pub traders: Vec<Trader>,
    pub network: Network,
    /// Buy-and-hold baseline, compounded on exactly the days traders settle.
    pub benchmark_wealth: f64,
    bench: Track,
    cohort_tracks: Vec<Track>,
    rng: Rng,
    /// Separate stream for synthetic paths, so choosing a replay mode never
    /// perturbs the learning randomness of the first pass.
    path_rng: Rng,
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
        let path_rng = Rng::new(params.seed ^ 0x9E37_79B9_7F4A_7C15);
        let cohort_tracks = cohorts.iter().map(|c| Track::new(c.count as f64 * params.init_wealth)).collect();
        let benchmark_wealth = params.init_wealth;
        let mut s = Self {
            bench: Track::new(benchmark_wealth),
            cohort_tracks,
            real_prices: prices.clone(),
            prices,
            idx: 0,
            day: 0,
            pass: 0,
            finished: false,
            traders,
            network,
            benchmark_wealth,
            rng,
            path_rng,
            params,
            cohorts,
        };
        s.idx = s.warmup_idx();
        s
    }

    /// First tradable index: the warm-up, capped so short series still trade.
    fn warmup_idx(&self) -> usize {
        self.params.warmup.min(self.prices.len().saturating_sub(2))
    }

    /// Advance one trading day: forecast -> position -> realize -> learn -> evolve.
    pub fn step(&mut self) {
        let n = self.prices.len();
        if n < 2 || self.finished {
            return;
        }
        if self.idx + 1 >= n && !self.start_next_pass() {
            // Sat out the seam (Loop) or ran out of data (Stop).
            self.day += 1;
            return;
        }

        let p = self.prices[self.idx];
        let next_idx = self.idx + 1;
        let p_next = self.prices[next_idx];
        if p <= 0.0 || p_next <= 0.0 {
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
        let bust_level = self.params.bust_fraction.max(0.0) * self.params.init_wealth;

        self.benchmark_wealth *= 1.0 + simple;
        self.bench.push(simple, self.benchmark_wealth);

        let k = self.cohorts.len();
        let mut before = vec![0.0; k];
        let mut after = vec![0.0; k];

        for trader in &mut self.traders {
            before[trader.cohort] += trader.wealth;
            if trader.bust {
                after[trader.cohort] += trader.wealth;
                continue;
            }
            let cohort = &self.cohorts[trader.cohort];
            let (a, b, var, ridx) = trader.forecast(state);
            let r_hat = a * momentum + b;
            // Record the active rule's reasoning (read-only, for the UI inspector).
            trader.active_a = a;
            trader.active_b = b;
            trader.active_acc = var;
            trader.active_forecast = r_hat;
            trader.active_mask = ridx.map(|i| trader.rules[i].mask).unwrap_or(0);

            let raw = match self.params.sizing {
                Sizing::Volatility => cohort.risk * r_hat / sigma,
                Sizing::MeanVariance { gamma } => cohort.risk * r_hat / (gamma.max(1e-3) * var.max(1e-6)),
            };
            let lev = self.params.max_leverage;
            let pos = if raw.is_finite() { raw.clamp(-lev, lev) } else { 0.0 };
            let turnover = (pos - trader.position).abs();
            trader.position = pos;
            trader.turnover += turnover;

            // Realize P&L on the day's actual move, net of trading friction.
            trader.last_pnl = pos * simple - cost_rate * turnover;
            trader.wealth = (trader.wealth * (1.0 + trader.last_pnl)).max(1e-6);
            if trader.wealth > trader.peak {
                trader.peak = trader.wealth;
            } else {
                trader.max_drawdown = trader.max_drawdown.max(1.0 - trader.wealth / trader.peak);
            }
            if trader.wealth < bust_level {
                trader.bust = true;
                trader.position = 0.0;
            }
            after[trader.cohort] += trader.wealth;

            // Learn: update every rule that matched today against the real move.
            trader.learn_all(state, momentum, realized, cohort.memory, &self.params);
        }

        for (ci, track) in self.cohort_tracks.iter_mut().enumerate() {
            if before[ci] > 0.0 {
                track.push(after[ci] / before[ci] - 1.0, after[ci]);
            }
        }

        self.evolve();

        self.idx = next_idx;
        self.day += 1;
    }

    /// Called when the current path has no next day. Returns `true` if trading
    /// can continue today on a fresh path, `false` if today is not traded.
    fn start_next_pass(&mut self) -> bool {
        let flatten = |traders: &mut [Trader]| {
            for t in traders {
                t.position = 0.0;
                t.last_pnl = 0.0;
            }
        };
        match self.params.replay {
            Replay::Stop => {
                flatten(&mut self.traders);
                self.finished = true;
                false
            }
            Replay::Loop => {
                // The seam p[n-1] -> p[0] is not a real market move: it is
                // years of drift in one step. Settling or learning on it would
                // hand every trader a fake crash/boom, so the town sits out
                // that day and flattens its book.
                flatten(&mut self.traders);
                self.idx = self.warmup_idx();
                self.pass += 1;
                false
            }
            Replay::Bootstrap { block } => {
                let n = self.prices.len();
                let keep = self.params.warmup.max(1).min(n - 1);
                self.prices = bootstrap_path(&self.real_prices, &self.prices[n - keep..], n, block, &mut self.path_rng);
                // The last `keep` prices of the old path seed the new one, so
                // signals stay continuous and trading resumes immediately.
                self.idx = keep - 1;
                self.pass += 1;
                true
            }
        }
    }

    /// Run the GA for every trader whose turn it is, with neighbour copying.
    fn evolve(&mut self) {
        let day = self.day;
        let due = |t: &Trader| !t.bust && day >= t.next_ga;
        if !self.traders.iter().any(due) {
            return;
        }
        // Snapshot rule pools so neighbours are read from one consistent state.
        // GA turns are staggered, so only a few traders evolve on any given day:
        // copy just their pools and their neighbours' instead of the whole town.
        let mut snapshot: Vec<Option<Vec<Rule>>> = vec![None; self.traders.len()];
        for i in 0..self.traders.len() {
            if !due(&self.traders[i]) {
                continue;
            }
            for &j in std::iter::once(&i).chain(self.network.neighbors(i)) {
                if snapshot[j].is_none() && !self.traders[j].bust {
                    snapshot[j] = Some(self.traders[j].rules.clone());
                }
            }
        }
        for i in 0..self.traders.len() {
            if !due(&self.traders[i]) {
                continue;
            }
            let herding = self.cohorts[self.traders[i].cohort].herding;
            // Bankrupt neighbours have no snapshot, so they are never copied.
            let pools: Vec<&[Rule]> =
                self.network.neighbors(i).iter().filter_map(|&j| snapshot[j].as_deref()).collect();
            let own = snapshot[i].as_deref().unwrap_or(&[]);
            run_ga(&mut self.traders[i].rules, own, &pools, herding, &mut self.rng, &self.params);
            self.traders[i].next_ga = day + self.params.ga_interval;
        }
    }

    pub fn price(&self) -> f64 {
        self.prices.get(self.idx).copied().unwrap_or(0.0)
    }

    /// Simple return from the previous day on the current path (0 on day one).
    pub fn change(&self) -> f64 {
        if self.idx == 0 {
            return 0.0;
        }
        let prev = self.prices[self.idx - 1];
        if prev > 0.0 {
            self.price() / prev - 1.0
        } else {
            0.0
        }
    }

    pub fn outcomes(&self) -> Outcomes {
        let n = self.traders.len().max(1) as f64;
        let longs = self.traders.iter().filter(|t| t.position > 0.0).count() as f64;
        // Not `f64::signum`: it maps 0.0 to 1.0, which counted every flat
        // trader as a bull and showed "Herding 1.00" before anyone had traded.
        let sign = |x: f64| {
            if x > 0.0 {
                1.0
            } else if x < 0.0 {
                -1.0
            } else {
                0.0
            }
        };
        let sign_sum: f64 = self.traders.iter().map(|t| sign(t.position)).sum();
        let avg_wealth = self.traders.iter().map(|t| t.wealth).sum::<f64>() / n;
        let net_position = self.traders.iter().map(|t| t.position).sum::<f64>() / n;
        let days = self.bench.days.max(1) as f64;

        let cohorts: Vec<CohortStats> = self
            .cohorts
            .iter()
            .enumerate()
            .map(|(ci, c)| {
                let members: Vec<&Trader> = self.traders.iter().filter(|t| t.cohort == ci).collect();
                let mut w: Vec<f64> = members.iter().map(|t| t.wealth).collect();
                w.sort_by(f64::total_cmp);
                let mean_wealth = if w.is_empty() { 0.0 } else { w.iter().sum::<f64>() / w.len() as f64 };
                let track = &self.cohort_tracks[ci];
                CohortStats {
                    name: c.name.clone(),
                    mean_wealth,
                    median_wealth: median_sorted(&w),
                    excess_log_return: if mean_wealth > 0.0 && self.benchmark_wealth > 0.0 {
                        (mean_wealth / self.benchmark_wealth).ln()
                    } else {
                        0.0
                    },
                    sharpe: track.sharpe(),
                    max_drawdown: track.max_drawdown,
                    turnover: members.iter().map(|t| t.turnover).sum::<f64>() / members.len().max(1) as f64 / days,
                    busts: members.iter().filter(|t| t.bust).count(),
                }
            })
            .collect();

        Outcomes {
            day: self.day,
            trading_days: self.bench.days,
            price: self.price(),
            sentiment: longs / n,
            herding: (sign_sum / n).abs(),
            net_position,
            avg_wealth,
            benchmark_wealth: self.benchmark_wealth,
            benchmark_sharpe: self.bench.sharpe(),
            benchmark_max_drawdown: self.bench.max_drawdown,
            pass: self.pass,
            synthetic: self.pass > 0 && matches!(self.params.replay, Replay::Bootstrap { .. }),
            finished: self.finished,
            busts: self.traders.iter().filter(|t| t.bust).count(),
            cohort_names: self.cohorts.iter().map(|c| c.name.clone()).collect(),
            cohort_wealth: cohorts.iter().map(|c| c.mean_wealth).collect(),
            cohorts,
        }
    }
}

fn median_sorted(v: &[f64]) -> f64 {
    match v.len() {
        0 => 0.0,
        n if n % 2 == 1 => v[n / 2],
        n => 0.5 * (v[n / 2 - 1] + v[n / 2]),
    }
}

/// Stationary block bootstrap (Politis & Romano 1994) of the real daily log
/// returns. The new path begins with `seed_prices` (the tail of the previous
/// path) and is extended to `len` prices by resampled returns: each day either
/// continues the current block or, with probability `1/block`, jumps to a
/// random new block start.
fn bootstrap_path(real: &[f64], seed_prices: &[f64], len: usize, block: usize, rng: &mut Rng) -> Vec<f64> {
    let rets: Vec<f64> = real.windows(2).filter(|w| w[0] > 0.0 && w[1] > 0.0).map(|w| (w[1] / w[0]).ln()).collect();
    let mut path = seed_prices.to_vec();
    let mut price = path.last().copied().unwrap_or(1.0);
    if rets.is_empty() {
        path.resize(len.max(path.len()), price);
        return path;
    }
    let restart = 1.0 / block.max(1) as f64;
    let mut pos = rng.below(rets.len());
    while path.len() < len {
        if rng.chance(restart) {
            pos = rng.below(rets.len());
        } else {
            pos = (pos + 1) % rets.len();
        }
        price *= rets[pos].exp();
        path.push(price);
    }
    path
}

// ---------- multi-seed robustness ----------

/// Across-seed summary for one guild.
#[derive(Clone, Debug, Default)]
pub struct CohortSweep {
    pub name: String,
    /// Mean of `ln(guild mean wealth / buy-and-hold)` across seeds.
    pub excess_mean: f64,
    /// Half-width of the ~95% confidence interval of `excess_mean`
    /// (Student-t on the across-seed spread).
    pub excess_ci95: f64,
    /// Fraction of seeds in which the guild ended ahead of buy-and-hold.
    pub beat_fraction: f64,
    pub sharpe_mean: f64,
    pub max_drawdown_mean: f64,
}

#[derive(Clone, Debug, Default)]
pub struct SweepSummary {
    pub seeds: usize,
    pub days: u64,
    /// Mean buy-and-hold log return across seeds (identical across seeds unless
    /// the run crosses into bootstrapped passes).
    pub benchmark_log_return: f64,
    pub cohorts: Vec<CohortSweep>,
}

/// Run the same town on `seeds.len()` seeds for `days` steps each and summarise
/// how robust each guild's result is. Runs are spread over threads but each is
/// independent and deterministic, so the summary is reproducible.
pub fn sweep(prices: &[f64], cohorts: &[Cohort], params: &SandboxParams, seeds: &[u64], days: u64) -> SweepSummary {
    let run_one = |seed: u64| -> Outcomes {
        let p = SandboxParams { seed, ..params.clone() };
        let mut s = Sandbox::new(prices.to_vec(), cohorts.to_vec(), p);
        for _ in 0..days {
            s.step();
        }
        s.outcomes()
    };

    let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1).min(seeds.len().max(1));
    let chunk = seeds.len().div_ceil(threads.max(1)).max(1);
    let results: Vec<Outcomes> = std::thread::scope(|scope| {
        let handles: Vec<_> = seeds
            .chunks(chunk)
            .map(|part| scope.spawn(move || part.iter().map(|&s| run_one(s)).collect::<Vec<_>>()))
            .collect();
        handles.into_iter().flat_map(|h| h.join().expect("sweep worker panicked")).collect()
    });

    let k = results.len();
    let mean = |xs: &[f64]| if xs.is_empty() { 0.0 } else { xs.iter().sum::<f64>() / xs.len() as f64 };
    let bench: Vec<f64> = results.iter().map(|o| (o.benchmark_wealth / params.init_wealth).max(1e-300).ln()).collect();

    let summaries = cohorts
        .iter()
        .enumerate()
        .map(|(ci, c)| {
            let ex: Vec<f64> = results.iter().map(|o| o.cohorts[ci].excess_log_return).collect();
            let m = mean(&ex);
            let ci95 = if k > 1 {
                let var = ex.iter().map(|x| (x - m) * (x - m)).sum::<f64>() / (k - 1) as f64;
                t95(k - 1) * (var / k as f64).sqrt()
            } else {
                0.0
            };
            CohortSweep {
                name: c.name.clone(),
                excess_mean: m,
                excess_ci95: ci95,
                beat_fraction: ex.iter().filter(|&&x| x > 0.0).count() as f64 / k.max(1) as f64,
                sharpe_mean: mean(&results.iter().map(|o| o.cohorts[ci].sharpe).collect::<Vec<_>>()),
                max_drawdown_mean: mean(&results.iter().map(|o| o.cohorts[ci].max_drawdown).collect::<Vec<_>>()),
            }
        })
        .collect();

    SweepSummary { seeds: k, days, benchmark_log_return: mean(&bench), cohorts: summaries }
}

/// Two-sided 95% Student-t critical value for `df` degrees of freedom.
fn t95(df: usize) -> f64 {
    const T: [f64; 30] = [
        12.706, 4.303, 3.182, 2.776, 2.571, 2.447, 2.365, 2.306, 2.262, 2.228, 2.201, 2.179, 2.160, 2.145, 2.131,
        2.120, 2.110, 2.101, 2.093, 2.086, 2.080, 2.074, 2.069, 2.064, 2.060, 2.056, 2.052, 2.048, 2.045, 2.042,
    ];
    match df {
        0 => f64::INFINITY,
        d if d <= 30 => T[d - 1],
        _ => 1.96,
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

    /// A wavy series with a trend, so returns have both signs.
    fn wavy(n: usize) -> Vec<f64> {
        (0..n).map(|i| 100.0 * (1.0 + 0.3 * (i as f64 * 0.05).sin() + 0.0005 * i as f64)).collect()
    }

    fn default_cohorts() -> Vec<Cohort> {
        vec![Cohort::new("calm", 15, 0.02, 0.1, 0.8), Cohort::new("bubbly", 15, 0.08, 0.6, 1.5)]
    }

    /// Steps needed to trade every real day once (warm-up .. last day).
    fn one_pass(s: &Sandbox) -> usize {
        s.prices.len() - 1 - s.idx
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
        assert!(o.pass > 0);
        for t in &s.traders {
            assert!(t.wealth.is_finite() && t.wealth > 0.0);
            assert!(t.position.abs() <= s.params.max_leverage + 1e-9);
            assert!((0.0..=1.0).contains(&t.max_drawdown));
        }
        assert_eq!(o.cohort_wealth.len(), 2);
        for c in &o.cohorts {
            assert!(c.sharpe.is_finite());
            assert!((0.0..=1.0).contains(&c.max_drawdown));
            assert!(c.turnover >= 0.0);
        }
    }

    #[test]
    fn traders_profit_on_a_steady_uptrend() {
        // A long single-pass uptrend (no loop boundary, so no artificial crash):
        // learners should figure out "go long" and end up net long and richer.
        let mut s = Sandbox::new(uptrend(3000), default_cohorts(), SandboxParams::default());
        for _ in 0..one_pass(&s) {
            s.step();
        }
        let o = s.outcomes();
        assert!(o.avg_wealth > s.params.init_wealth, "avg wealth {} did not grow", o.avg_wealth);
        assert!(o.sentiment > 0.5, "crowd should lean long on an uptrend, got {}", o.sentiment);
    }

    #[test]
    fn a_flat_town_shows_no_herding() {
        let s = Sandbox::new(uptrend(300), default_cohorts(), SandboxParams::default());
        let o = s.outcomes();
        assert_eq!(o.herding, 0.0);
        assert_eq!(o.sentiment, 0.0);
        assert_eq!(o.net_position, 0.0);
    }

    #[test]
    fn trading_starts_after_warmup() {
        let s = Sandbox::new(uptrend(300), default_cohorts(), SandboxParams::default());
        assert_eq!(s.idx, 50);
        let short = Sandbox::new(uptrend(20), default_cohorts(), SandboxParams::default());
        assert_eq!(short.idx, 18, "short series still get at least one tradable day");
    }

    #[test]
    fn loop_seam_is_not_traded() {
        // Price triples over the series, so wrapping p[n-1] -> p[0] would be a
        // ~-67% one-day "crash" if it were settled.
        let n = 200;
        let prices: Vec<f64> = (0..n).map(|i| 100.0 * (1.0 + 2.0 * i as f64 / n as f64)).collect();
        let mut s = Sandbox::new(prices, default_cohorts(), SandboxParams::default());
        for _ in 0..one_pass(&s) {
            s.step();
        }
        assert_eq!(s.idx, n - 1);
        let before: Vec<f64> = s.traders.iter().map(|t| t.wealth).collect();
        let bench_before = s.benchmark_wealth;
        s.step(); // the seam
        assert_eq!(s.idx, s.params.warmup);
        assert_eq!(s.pass, 1);
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
        let start = s.idx;
        for _ in 0..one_pass(&s) {
            s.step();
        }
        let o = s.outcomes();
        let expected = s.params.init_wealth * prices[n - 1] / prices[start];
        assert!(
            (o.benchmark_wealth / expected - 1.0).abs() < 1e-9,
            "benchmark {} vs buy&hold {expected}",
            o.benchmark_wealth
        );
        assert_eq!(o.benchmark_max_drawdown, 0.0, "a monotone uptrend has no drawdown");
        assert!(o.benchmark_sharpe > 0.0);
        assert_eq!(o.trading_days as usize, n - 1 - start);
    }

    #[test]
    fn transaction_costs_only_reduce_wealth() {
        let run = |cost_bps: f64| {
            let params = SandboxParams { cost_bps, ..SandboxParams::default() };
            let mut s = Sandbox::new(uptrend(800), default_cohorts(), params);
            for _ in 0..one_pass(&s) {
                s.step();
            }
            s.outcomes().avg_wealth
        };
        let free = run(0.0);
        let costly = run(25.0);
        assert!(costly.is_finite() && costly > 0.0);
        assert!(costly < free, "costs should bite: {costly} vs frictionless {free}");
    }

    #[test]
    fn stop_mode_ends_after_one_pass() {
        let params = SandboxParams { replay: Replay::Stop, ..SandboxParams::default() };
        let mut s = Sandbox::new(wavy(300), default_cohorts(), params);
        for _ in 0..one_pass(&s) {
            s.step();
        }
        assert!(!s.finished);
        s.step();
        assert!(s.finished && s.outcomes().finished);
        let w: Vec<f64> = s.traders.iter().map(|t| t.wealth).collect();
        let idx = s.idx;
        for _ in 0..10 {
            s.step();
        }
        assert_eq!(s.idx, idx);
        assert_eq!(w, s.traders.iter().map(|t| t.wealth).collect::<Vec<_>>());
    }

    #[test]
    fn bootstrap_continues_on_a_seamless_synthetic_path() {
        let params = SandboxParams { replay: Replay::Bootstrap { block: 20 }, ..SandboxParams::default() };
        let real = wavy(300);
        let mut s = Sandbox::new(real.clone(), default_cohorts(), params.clone());
        for _ in 0..one_pass(&s) {
            s.step();
        }
        let last_real = s.price();
        let tail: Vec<f64> = s.prices[s.prices.len() - 50..].to_vec();
        s.step(); // crosses into pass 1 and trades immediately
        assert_eq!(s.pass, 1);
        assert!(s.outcomes().synthetic);
        assert_eq!(s.prices.len(), real.len());
        assert_eq!(&s.prices[..50], tail.as_slice(), "new path starts from the old tail");
        assert_eq!(s.prices[49], last_real);
        assert_eq!(s.idx, 50, "traded the first synthetic day");
        assert_ne!(s.prices, real, "the second pass must not replay history");
        assert!(s.prices.iter().all(|p| p.is_finite() && *p > 0.0));

        // Deterministic for the same seed, and the same across runs.
        let run = || {
            let mut s = Sandbox::new(real.clone(), default_cohorts(), params.clone());
            for _ in 0..2000 {
                s.step();
            }
            (s.outcomes().avg_wealth.to_bits(), s.pass)
        };
        let (a, pa) = run();
        let (b, _) = run();
        assert_eq!(a, b);
        assert!(pa >= 7);
    }

    #[test]
    fn first_pass_is_unaffected_by_replay_mode() {
        let run = |replay| {
            let params = SandboxParams { replay, ..SandboxParams::default() };
            let mut s = Sandbox::new(wavy(400), default_cohorts(), params);
            for _ in 0..one_pass(&s) {
                s.step();
            }
            s.outcomes().avg_wealth.to_bits()
        };
        let base = run(Replay::Loop);
        assert_eq!(base, run(Replay::Stop));
        assert_eq!(base, run(Replay::Bootstrap { block: 10 }));
    }

    #[test]
    fn mean_variance_sizing_respects_leverage() {
        let params = SandboxParams { sizing: Sizing::MeanVariance { gamma: 3.0 }, ..SandboxParams::default() };
        let mut s = Sandbox::new(wavy(600), default_cohorts(), params);
        let mut saw_position = false;
        for _ in 0..one_pass(&s) {
            s.step();
            for t in &s.traders {
                assert!(t.position.abs() <= s.params.max_leverage + 1e-9);
                assert!(t.wealth.is_finite() && t.wealth > 0.0);
                saw_position |= t.position != 0.0;
            }
        }
        assert!(saw_position);
    }

    #[test]
    fn bankrupt_traders_stop_trading() {
        // Heavy leverage on a violently oscillating price wipes traders out.
        let prices: Vec<f64> = (0..600).map(|i| if (i / 3) % 2 == 0 { 100.0 } else { 40.0 }).collect();
        let params = SandboxParams { max_leverage: 5.0, bust_fraction: 0.2, ..SandboxParams::default() };
        let cohorts = vec![Cohort::new("reckless", 30, 0.1, 0.5, 5.0)];
        let mut s = Sandbox::new(prices, cohorts, params);
        for _ in 0..one_pass(&s) {
            s.step();
        }
        let o = s.outcomes();
        assert!(o.busts > 0, "expected some bankruptcies");
        assert_eq!(o.busts, o.cohorts[0].busts);
        let frozen: Vec<(usize, f64)> =
            s.traders.iter().enumerate().filter(|(_, t)| t.bust).map(|(i, t)| (i, t.wealth)).collect();
        for _ in 0..30 {
            s.step();
        }
        for (i, w) in frozen {
            assert!(s.traders[i].bust);
            assert_eq!(s.traders[i].wealth, w, "a bankrupt trader must not keep trading");
            assert_eq!(s.traders[i].position, 0.0);
        }
    }

    #[test]
    fn sweep_is_deterministic_and_bounded() {
        let prices = wavy(400);
        let params = SandboxParams::default();
        let seeds = [1, 2, 3, 4, 5, 6];
        let a = sweep(&prices, &default_cohorts(), &params, &seeds, 349);
        let b = sweep(&prices, &default_cohorts(), &params, &seeds, 349);
        assert_eq!(a.seeds, 6);
        assert_eq!(a.cohorts.len(), 2);
        for (x, y) in a.cohorts.iter().zip(&b.cohorts) {
            assert_eq!(x.excess_mean.to_bits(), y.excess_mean.to_bits());
            assert!(x.excess_ci95 >= 0.0 && x.excess_ci95.is_finite());
            assert!((0.0..=1.0).contains(&x.beat_fraction));
        }
        // The summary for a seed matches a direct run with that seed.
        let single = sweep(&prices, &default_cohorts(), &params, &[3], 349);
        let mut s = Sandbox::new(prices.clone(), default_cohorts(), SandboxParams { seed: 3, ..params });
        for _ in 0..349 {
            s.step();
        }
        assert_eq!(single.cohorts[0].excess_mean.to_bits(), s.outcomes().cohorts[0].excess_log_return.to_bits());
        assert_eq!(single.cohorts[0].excess_ci95, 0.0);
    }

    #[test]
    fn median_handles_even_and_odd() {
        assert_eq!(median_sorted(&[]), 0.0);
        assert_eq!(median_sorted(&[1.0, 2.0, 9.0]), 2.0);
        assert_eq!(median_sorted(&[1.0, 2.0, 4.0, 9.0]), 3.0);
    }
}
