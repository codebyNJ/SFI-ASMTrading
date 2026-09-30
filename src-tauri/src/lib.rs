//! Tauri backend: owns one live `Simulation` and exposes commands to configure
//! it and pull snapshots for the dashboard. The heavy compute stays in Rust; the
//! frontend just drives stepping and renders snapshots.

mod market_data;

use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use sfi_asm::{Cohort, Params, Sandbox, SandboxParams, Simulation, Topology};
use tauri::State;

use market_data::MarketSeries;

/// Managed application state: the single live simulation (if initialized).
#[derive(Default)]
struct AppState {
    sim: Mutex<Option<Simulation>>,
}

/// Managed state for the Path-3 trading sandbox.
#[derive(Default)]
struct SandboxState(Mutex<Option<Sandbox>>);

/// Configuration sent from the UI to (re)create a simulation.
#[derive(Deserialize)]
struct SimConfig {
    n_agents: usize,
    ga_interval: u64,
    theta: f64,
    spec_cost: f64,
    social_prob: f64,
    /// "none" | "lattice" | "random" | "scalefree"
    topology: String,
    /// Target mean degree (k for lattice, mean_degree for random, 2*m for scale-free).
    degree: f64,
    seed: u64,
    /// Dividend-noise scale. When omitted, the default is used; calibration sets it.
    #[serde(default)]
    sigma_eps: Option<f64>,
    /// Risk aversion. When omitted, the default is used; calibration may lower it.
    #[serde(default)]
    lambda: Option<f64>,
}

#[derive(Serialize)]
struct NodeDto {
    degree: usize,
    /// Agent's mean forecast slope `a` (a crude "belief" indicator for coloring).
    belief: f64,
    /// Net demand at the cleared price; >1 = net buyer (bullish), <1 = net seller.
    demand: f64,
    /// Mean forecast-error variance across the agent's rules; lower = more skilled.
    accuracy: f64,
}

/// Everything the dashboard needs to render one frame.
#[derive(Serialize)]
struct Snapshot {
    period: u64,
    price: f64,
    ree: f64,
    rel_dev: f64,
    ret_std: f64,
    kurtosis: f64,
    ac1: f64,
    abs_ac1: f64,
    tech_bits: f64,
    prices: Vec<f64>,
    ree_series: Vec<f64>,
    a_bins: Vec<u32>,
    a_min: f64,
    a_max: f64,
    mean_degree: f64,
    max_degree: usize,
    nodes: Vec<NodeDto>,
    edges: Vec<[usize; 2]>,
}

fn build_params(c: &SimConfig) -> Params {
    let mut p = Params::classic();
    p.n_agents = c.n_agents.clamp(2, 2000);
    p.ga_interval = c.ga_interval.max(1);
    p.theta = c.theta.clamp(1e-4, 1.0);
    p.spec_cost = c.spec_cost.max(0.0);
    p.social_prob = c.social_prob.clamp(0.0, 1.0);
    p.seed = c.seed;
    if let Some(s) = c.sigma_eps {
        p.sigma_eps = s.clamp(1e-4, 100.0);
    }
    if let Some(l) = c.lambda {
        p.lambda = l.clamp(0.02, 10.0);
    }
    p.topology = match c.topology.as_str() {
        "lattice" => Topology::Lattice { k: (c.degree.round() as usize).max(2) },
        "random" => Topology::Random { mean_degree: c.degree.max(0.0) },
        "scalefree" => Topology::ScaleFree { m: ((c.degree / 2.0).round() as usize).max(1) },
        _ => Topology::None,
    };
    p
}

fn histogram(values: &[f64], lo: f64, hi: f64, bins: usize) -> Vec<u32> {
    let mut h = vec![0u32; bins];
    if hi <= lo || bins == 0 {
        return h;
    }
    for &v in values {
        let idx = (((v - lo) / (hi - lo)) * bins as f64).floor() as isize;
        let idx = idx.clamp(0, bins as isize - 1) as usize;
        h[idx] += 1;
    }
    h
}

fn make_snapshot(sim: &Simulation) -> Snapshot {
    let s = sim.stats();
    let a_values = sim.forecast_a_distribution();
    let (a_min, a_max) = (sim.params.a_min, sim.params.a_max);

    let mut nodes = Vec::with_capacity(sim.agents.len());
    let mut edges = Vec::new();
    for (i, agent) in sim.agents.iter().enumerate() {
        let belief = if agent.rules.is_empty() {
            0.0
        } else {
            agent.rules.iter().map(|r| r.a).sum::<f64>() / agent.rules.len() as f64
        };
        nodes.push(NodeDto {
            degree: sim.network.neighbors(i).len(),
            belief,
            demand: agent.demand,
            accuracy: agent.mean_accuracy(),
        });
        for &j in sim.network.neighbors(i) {
            if i < j {
                edges.push([i, j]);
            }
        }
    }

    Snapshot {
        period: sim.period(),
        price: sim.price,
        ree: sim.ree_price(),
        rel_dev: sim.relative_deviation(),
        ret_std: s.std,
        kurtosis: s.excess_kurtosis,
        ac1: s.autocorr1,
        abs_ac1: s.abs_autocorr1,
        tech_bits: sim.mean_technical_bits(),
        prices: sim.price_history().iter().copied().collect(),
        ree_series: sim.ree_history().iter().copied().collect(),
        a_bins: histogram(&a_values, a_min, a_max, 40),
        a_min,
        a_max,
        mean_degree: sim.network.mean_degree(),
        max_degree: sim.network.max_degree(),
        nodes,
        edges,
    }
}

/// Create (or replace) the live simulation and return its initial snapshot.
#[tauri::command]
fn init_sim(config: SimConfig, state: State<AppState>) -> Snapshot {
    let sim = Simulation::new(build_params(&config));
    let snap = make_snapshot(&sim);
    *state.sim.lock().unwrap() = Some(sim);
    snap
}

/// Advance the live simulation by `batch` periods and return a fresh snapshot.
#[tauri::command]
fn step_sim(batch: u32, state: State<AppState>) -> Result<Snapshot, String> {
    let mut guard = state.sim.lock().unwrap();
    let sim = guard.as_mut().ok_or("simulation not initialized")?;
    for _ in 0..batch.max(1) {
        sim.advance();
    }
    Ok(make_snapshot(sim))
}

/// Fetch a real company's daily price history (live, with offline fallback).
#[tauri::command]
fn fetch_real_market(symbol: Option<String>) -> MarketSeries {
    let sym = symbol.unwrap_or_else(|| "AAPL".to_string());
    market_data::load(&sym)
}

// --- volatility calibration ---
// One simulation period := one trading day, so matching the sim's per-period
// log-return std to real BTC's *daily* return std calibrates volatility.

const CAL_WARMUP: u64 = 1200;
const CAL_WINDOW: u64 = 1500;

#[derive(Serialize)]
struct Calibration {
    sigma_eps: f64,
    lambda: f64,
    achieved_std: f64,
    target_std: f64,
    iterations: u32,
    ok: bool,
}

/// Measure the simulated per-period log-return std for a given parameter set.
fn measure_std(params: &Params) -> f64 {
    let mut sim = Simulation::new(params.clone());
    for t in 0..CAL_WARMUP {
        sim.step(t);
    }
    let mut prev = sim.price;
    let mut rets: Vec<f64> = Vec::with_capacity(CAL_WINDOW as usize);
    for t in CAL_WARMUP..(CAL_WARMUP + CAL_WINDOW) {
        sim.step(t);
        let p = sim.price;
        if prev > 0.0 && p > 0.0 {
            rets.push((p / prev).ln());
        }
        prev = p;
    }
    if rets.len() < 2 {
        return 0.0;
    }
    let n = rets.len() as f64;
    let mean = rets.iter().sum::<f64>() / n;
    let var = rets.iter().map(|r| (r - mean) * (r - mean)).sum::<f64>() / n;
    var.sqrt()
}

/// Largest `sigma_eps` that keeps the REE fundamental price comfortably positive
/// (the risk-premium term grows as `sigma_eps^2` and otherwise drags it negative).
fn safe_sigma_max(params: &Params) -> f64 {
    let f = params.rho / (1.0 + params.r - params.rho);
    // Keep premium <= 60% of the dividend term so the fundamental stays well above 0.
    let num = 0.6 * (f + 1.0) * (1.0 - params.rho) * params.d_bar;
    let den = (params.lambda * (f + 1.0) * (f + 1.0)).max(1e-9);
    (num / den).max(1e-10).sqrt()
}

/// Calibrate volatility to `target_std` in two phases:
///   1. Relax risk aversion `lambda` only until the target is reachable within
///      the *stable* sigma range (bigger positions => more volatility headroom).
///   2. Bisect `sigma_eps` in that range to hit the target precisely (volatility
///      is monotonic in sigma there, and deterministic for a fixed seed).
fn calibrate_core(mut params: Params, target_std: f64) -> Calibration {
    let target = target_std.max(1e-6);
    let mut iters = 0u32;

    // Phase 1: secure enough headroom.
    let mut smax = safe_sigma_max(&params);
    for _ in 0..6 {
        params.sigma_eps = smax;
        iters += 1;
        if measure_std(&params) >= target || params.lambda <= 0.031 {
            break;
        }
        params.lambda = (params.lambda * 0.5).max(0.03);
        smax = safe_sigma_max(&params);
    }

    // Phase 2: bisection on sigma_eps in (0, smax].
    let mut lo = 1e-4_f64;
    let mut hi = smax;
    let mut achieved = 0.0;
    for _ in 0..16 {
        iters += 1;
        let mid = 0.5 * (lo + hi);
        params.sigma_eps = mid;
        achieved = measure_std(&params);
        if (achieved / target - 1.0).abs() < 0.05 {
            break;
        }
        if achieved < target {
            lo = mid;
        } else {
            hi = mid;
        }
    }

    Calibration {
        sigma_eps: params.sigma_eps,
        lambda: params.lambda,
        achieved_std: achieved,
        target_std: target,
        iterations: iters,
        ok: (achieved / target - 1.0).abs() < 0.2,
    }
}

/// Calibrate the simulated market's volatility to a target daily return std.
#[tauri::command]
fn calibrate(config: SimConfig, target_std: f64) -> Calibration {
    calibrate_core(build_params(&config), target_std)
}

// --- Path-3 trading sandbox commands ---

#[derive(Deserialize)]
struct CohortCfg {
    name: String,
    count: usize,
    /// 0.01 patient .. 0.2 jumpy
    memory: f64,
    /// 0..1 copying
    herding: f64,
    /// position aggressiveness
    risk: f64,
}

#[derive(Deserialize)]
struct SandboxCfg {
    symbol: String,
    cohorts: Vec<CohortCfg>,
    #[serde(default)]
    rules_per_trader: Option<usize>,
    #[serde(default)]
    ga_interval: Option<u64>,
    #[serde(default)]
    topology: Option<String>,
    #[serde(default)]
    degree: Option<f64>,
    #[serde(default)]
    seed: Option<u64>,
    /// Trading friction in basis points of turnover (default 0 = frictionless).
    #[serde(default)]
    cost_bps: Option<f64>,
}

#[derive(Serialize)]
struct TraderView {
    cohort: usize,
    wealth: f64,
    position: f64,
    pnl: f64,
    /// Active rule's momentum sensitivity (>0 trend, <0 contrarian).
    a: f64,
    b: f64,
    /// Forecast return r_hat for the day (fraction).
    forecast: f64,
    /// Forecast-error variance of the active rule (lower = more confident).
    accuracy: f64,
    /// Bitmask of which of the 8 market signals the active rule uses.
    signals: u32,
    /// "trend" | "contrarian" | "drifter".
    kind: String,
}

#[derive(Serialize)]
struct SandboxSnapshot {
    day: u64,
    idx: usize,
    price: f64,
    sentiment: f64,
    herding: f64,
    avg_wealth: f64,
    /// Passive buy-and-hold wealth over the same trading days (the baseline).
    benchmark_wealth: f64,
    cohort_names: Vec<String>,
    cohort_wealth: Vec<f64>,
    traders: Vec<TraderView>,
    // strategy mix
    trend_count: u32,
    contrarian_count: u32,
    drifter_count: u32,
    /// Histogram of active `a` over [-2, 2] (24 bins) — the belief spread.
    belief_hist: Vec<u32>,
    /// How many active rules use each of the 8 market signals.
    signal_usage: Vec<u32>,
}

/// Classify a trader's strategy from its active rule's momentum coefficient.
fn strategy_kind(a: f64) -> &'static str {
    const EPS: f64 = 0.1;
    if a > EPS {
        "trend"
    } else if a < -EPS {
        "contrarian"
    } else {
        "drifter"
    }
}

#[derive(Serialize)]
struct SandboxInit {
    symbol: String,
    name: String,
    source: String,
    /// Ticker the user asked for; differs from `symbol` on offline fallback.
    requested: String,
    fallback: bool,
    days: usize,
    prices: Vec<f64>,
    timestamps: Vec<i64>,
    snapshot: SandboxSnapshot,
}

fn sandbox_params(cfg: &SandboxCfg) -> SandboxParams {
    let mut p = SandboxParams::default();
    if let Some(m) = cfg.rules_per_trader {
        p.rules_per_trader = m.clamp(10, 200);
    }
    if let Some(t) = cfg.ga_interval {
        p.ga_interval = t.max(1);
    }
    if let Some(s) = cfg.seed {
        p.seed = s;
    }
    if let Some(c) = cfg.cost_bps {
        p.cost_bps = c.clamp(0.0, 500.0);
    }
    let deg = cfg.degree.unwrap_or(6.0);
    p.topology = match cfg.topology.as_deref() {
        Some("none") => Topology::None,
        Some("lattice") => Topology::Lattice { k: (deg.round() as usize).max(2) },
        Some("random") => Topology::Random { mean_degree: deg.max(0.0) },
        _ => Topology::ScaleFree { m: ((deg / 2.0).round() as usize).max(1) },
    };
    p
}

fn sandbox_snapshot(s: &Sandbox) -> SandboxSnapshot {
    let o = s.outcomes();

    let mut traders = Vec::with_capacity(s.traders.len());
    let (mut trend, mut contra, mut drift) = (0u32, 0u32, 0u32);
    const BINS: usize = 24;
    const A_LO: f64 = -2.0;
    const A_HI: f64 = 2.0;
    let mut belief_hist = vec![0u32; BINS];
    let mut signal_usage = vec![0u32; 8];

    for t in &s.traders {
        let kind = strategy_kind(t.active_a);
        match kind {
            "trend" => trend += 1,
            "contrarian" => contra += 1,
            _ => drift += 1,
        }
        let idx = (((t.active_a - A_LO) / (A_HI - A_LO)) * BINS as f64).floor() as isize;
        belief_hist[idx.clamp(0, BINS as isize - 1) as usize] += 1;
        for (bit, slot) in signal_usage.iter_mut().enumerate() {
            if t.active_mask & (1 << bit) != 0 {
                *slot += 1;
            }
        }
        traders.push(TraderView {
            cohort: t.cohort,
            wealth: t.wealth,
            position: t.position,
            pnl: t.last_pnl,
            a: t.active_a,
            b: t.active_b,
            forecast: t.active_forecast,
            accuracy: t.active_acc,
            signals: t.active_mask,
            kind: kind.to_string(),
        });
    }

    SandboxSnapshot {
        day: o.day,
        idx: s.idx,
        price: o.price,
        sentiment: o.sentiment,
        herding: o.herding,
        avg_wealth: o.avg_wealth,
        benchmark_wealth: o.benchmark_wealth,
        cohort_names: o.cohort_names,
        cohort_wealth: o.cohort_wealth,
        traders,
        trend_count: trend,
        contrarian_count: contra,
        drifter_count: drift,
        belief_hist,
        signal_usage,
    }
}

/// Start a sandbox run on a real company's price history with a cohort mix.
///
/// `async` so the (up to 20 s) Yahoo fetch runs off the main thread instead of
/// freezing the window; the blocking HTTP call is moved onto a blocking worker.
#[tauri::command]
async fn init_sandbox(config: SandboxCfg, state: State<'_, SandboxState>) -> Result<SandboxInit, String> {
    let symbol = config.symbol.clone();
    let series = tauri::async_runtime::spawn_blocking(move || market_data::load(&symbol))
        .await
        .map_err(|e| format!("market data task failed: {e}"))?;
    if series.prices.len() < 60 {
        return Err(format!("not enough price data for {}", config.symbol));
    }
    if config.cohorts.is_empty() {
        return Err("at least one cohort is required".into());
    }
    let cohorts: Vec<Cohort> = config
        .cohorts
        .iter()
        // Clamp to ranges the engine is stable in: memory > 1 would make the
        // accuracy EMA negative, and an unbounded count can exhaust memory.
        .map(|c| {
            Cohort::new(
                &c.name,
                c.count.min(500),
                c.memory.clamp(1e-4, 1.0),
                c.herding.clamp(0.0, 1.0),
                c.risk.clamp(0.0, 10.0),
            )
        })
        .collect();
    let sandbox = Sandbox::new(series.prices.clone(), cohorts, sandbox_params(&config));
    let snapshot = sandbox_snapshot(&sandbox);
    let init = SandboxInit {
        symbol: series.symbol,
        name: series.name,
        source: series.source,
        requested: series.requested,
        fallback: series.fallback,
        days: series.days,
        prices: series.prices,
        timestamps: series.timestamps,
        snapshot,
    };
    *state.0.lock().unwrap() = Some(sandbox);
    Ok(init)
}

/// Advance the sandbox by `batch` trading days and return a fresh snapshot.
#[tauri::command]
fn step_sandbox(batch: u32, state: State<SandboxState>) -> Result<SandboxSnapshot, String> {
    let mut guard = state.0.lock().unwrap();
    let s = guard.as_mut().ok_or("sandbox not initialized")?;
    for _ in 0..batch.max(1) {
        s.step();
    }
    Ok(sandbox_snapshot(s))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calibration_hits_btc_like_volatility() {
        // Complex regime, modest size for test speed.
        let mut p = Params::complex();
        p.n_agents = 40;
        p.theta = 0.05;
        p.spec_cost = 0.0;
        p.social_prob = 0.4;
        p.topology = Topology::Random { mean_degree: 6.0 };
        p.seed = 1000;

        let target = 0.035; // ~3.5% daily, BTC-like
        let cal = calibrate_core(p, target);
        assert!(cal.sigma_eps.is_finite() && cal.sigma_eps > 0.0);
        let err = (cal.achieved_std / target - 1.0).abs();
        assert!(err < 0.20, "calibration off: achieved {} vs target {target}", cal.achieved_std);
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(AppState::default())
        .manage(SandboxState::default())
        .invoke_handler(tauri::generate_handler![
            init_sim,
            step_sim,
            fetch_real_market,
            calibrate,
            init_sandbox,
            step_sandbox
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
