//! Tauri backend: owns the live Path-3 trading sandbox and exposes commands to
//! build it, step it, stress-test it across seeds, and export a run. The heavy
//! compute stays in Rust; the frontend drives stepping and renders snapshots.

mod market_data;

use std::fmt::Write as _;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use sfi_asm::{sweep, Cohort, Outcomes, Replay, Sandbox, SandboxParams, Sizing, Topology};
use tauri::{AppHandle, Manager, State};

/// Cap on logged days kept for CSV export (~40 years of daily data).
const LOG_CAP: usize = 10_000;

/// Everything needed to re-run or export the current town.
struct Live {
    sandbox: Sandbox,
    symbol: String,
    /// The real history the run started from (sweeps always use this).
    prices: Vec<f64>,
    cohorts: Vec<Cohort>,
    params: SandboxParams,
    log: Vec<LogRow>,
}

/// Managed state for the Path-3 trading sandbox.
#[derive(Default)]
struct SandboxState(Mutex<Option<Live>>);

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
    /// "loop" | "bootstrap" | "stop" — what happens when the real data ends.
    #[serde(default)]
    replay: Option<String>,
    /// Mean block length (days) for the bootstrap replay.
    #[serde(default)]
    block: Option<usize>,
    /// "volatility" | "meanvariance"
    #[serde(default)]
    sizing: Option<String>,
    /// Risk aversion for mean-variance sizing.
    #[serde(default)]
    gamma: Option<f64>,
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
    max_drawdown: f64,
    bust: bool,
}

#[derive(Serialize)]
struct CohortStatsView {
    name: String,
    mean_wealth: f64,
    median_wealth: f64,
    excess_log_return: f64,
    sharpe: f64,
    max_drawdown: f64,
    turnover: f64,
    busts: usize,
}

#[derive(Serialize)]
struct SandboxSnapshot {
    day: u64,
    idx: usize,
    trading_days: u64,
    price: f64,
    /// Simple return from the previous day on the path being played.
    change: f64,
    /// 0 = first pass over the real history.
    pass: u32,
    /// Today's price comes from a bootstrapped (synthetic) path.
    synthetic: bool,
    /// A "stop" run has used up its data.
    finished: bool,
    sentiment: f64,
    herding: f64,
    /// Mean position across traders (−1 all short … +1 all long).
    net_position: f64,
    avg_wealth: f64,
    /// Passive buy-and-hold wealth over the same trading days (the baseline).
    benchmark_wealth: f64,
    benchmark_sharpe: f64,
    benchmark_max_drawdown: f64,
    busts: usize,
    cohort_names: Vec<String>,
    cohort_wealth: Vec<f64>,
    cohort_stats: Vec<CohortStatsView>,
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
    p.replay = match cfg.replay.as_deref() {
        Some("bootstrap") => Replay::Bootstrap { block: cfg.block.unwrap_or(20).clamp(1, 250) },
        Some("stop") => Replay::Stop,
        _ => Replay::Loop,
    };
    p.sizing = match cfg.sizing.as_deref() {
        Some("meanvariance") => Sizing::MeanVariance { gamma: cfg.gamma.unwrap_or(3.0).clamp(0.1, 100.0) },
        _ => Sizing::Volatility,
    };
    p
}

fn cohorts_from(cfg: &SandboxCfg) -> Vec<Cohort> {
    cfg.cohorts
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
        .collect()
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
            max_drawdown: t.max_drawdown,
            bust: t.bust,
        });
    }

    let cohort_stats = o
        .cohorts
        .iter()
        .map(|c| CohortStatsView {
            name: c.name.clone(),
            mean_wealth: c.mean_wealth,
            median_wealth: c.median_wealth,
            excess_log_return: c.excess_log_return,
            sharpe: c.sharpe,
            max_drawdown: c.max_drawdown,
            turnover: c.turnover,
            busts: c.busts,
        })
        .collect();

    SandboxSnapshot {
        day: o.day,
        idx: s.idx,
        trading_days: o.trading_days,
        price: o.price,
        change: s.change(),
        pass: o.pass,
        synthetic: o.synthetic,
        finished: o.finished,
        sentiment: o.sentiment,
        herding: o.herding,
        net_position: o.net_position,
        avg_wealth: o.avg_wealth,
        benchmark_wealth: o.benchmark_wealth,
        benchmark_sharpe: o.benchmark_sharpe,
        benchmark_max_drawdown: o.benchmark_max_drawdown,
        busts: o.busts,
        cohort_names: o.cohort_names,
        cohort_wealth: o.cohort_wealth,
        cohort_stats,
        traders,
        trend_count: trend,
        contrarian_count: contra,
        drifter_count: drift,
        belief_hist,
        signal_usage,
    }
}

// --- run log (CSV export) ---

struct LogRow {
    day: u64,
    pass: u32,
    synthetic: bool,
    price: f64,
    sentiment: f64,
    herding: f64,
    net_position: f64,
    avg_wealth: f64,
    benchmark: f64,
    cohort_wealth: Vec<f64>,
}

impl LogRow {
    fn from(o: &Outcomes) -> Self {
        Self {
            day: o.day,
            pass: o.pass,
            synthetic: o.synthetic,
            price: o.price,
            sentiment: o.sentiment,
            herding: o.herding,
            net_position: o.net_position,
            avg_wealth: o.avg_wealth,
            benchmark: o.benchmark_wealth,
            cohort_wealth: o.cohort_wealth.clone(),
        }
    }
}

fn csv_field(s: &str) -> String {
    if s.contains([',', '"', '\n']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

fn log_csv(cohorts: &[Cohort], rows: &[LogRow]) -> String {
    let mut out = String::from("day,pass,synthetic,price,sentiment,herding,net_position,avg_wealth,buy_and_hold");
    for c in cohorts {
        out.push(',');
        out.push_str(&csv_field(&format!("wealth_{}", c.name)));
    }
    out.push('\n');
    for r in rows {
        let _ = write!(
            out,
            "{},{},{},{:.6},{:.4},{:.4},{:.4},{:.4},{:.4}",
            r.day, r.pass, r.synthetic, r.price, r.sentiment, r.herding, r.net_position, r.avg_wealth, r.benchmark
        );
        for w in &r.cohort_wealth {
            let _ = write!(out, ",{w:.4}");
        }
        out.push('\n');
    }
    out
}

// --- commands ---

/// Start a sandbox run on a real company's price history with a cohort mix.
///
/// `async` so the (up to 20 s) Yahoo fetch runs off the main thread instead of
/// freezing the window; the blocking HTTP call is moved onto a blocking worker.
#[tauri::command]
async fn init_sandbox(
    app: AppHandle,
    config: SandboxCfg,
    state: State<'_, SandboxState>,
) -> Result<SandboxInit, String> {
    let symbol = config.symbol.clone();
    let cache = app.path().app_cache_dir().ok().map(|d| d.join("market"));
    let series = tauri::async_runtime::spawn_blocking(move || market_data::load(&symbol, cache.as_deref()))
        .await
        .map_err(|e| format!("market data task failed: {e}"))?;
    if series.prices.len() < 60 {
        return Err(format!("not enough price data for {}", config.symbol));
    }
    if config.cohorts.is_empty() {
        return Err("at least one cohort is required".into());
    }
    let cohorts = cohorts_from(&config);
    let params = sandbox_params(&config);
    let sandbox = Sandbox::new(series.prices.clone(), cohorts.clone(), params.clone());
    let snapshot = sandbox_snapshot(&sandbox);
    *state.0.lock().map_err(|e| e.to_string())? = Some(Live {
        sandbox,
        symbol: series.symbol.clone(),
        prices: series.prices.clone(),
        cohorts,
        params,
        log: Vec::new(),
    });
    Ok(SandboxInit {
        symbol: series.symbol,
        name: series.name,
        source: series.source,
        requested: series.requested,
        fallback: series.fallback,
        days: series.days,
        prices: series.prices,
        timestamps: series.timestamps,
        snapshot,
    })
}

/// Advance the sandbox by `batch` trading days and return a fresh snapshot.
#[tauri::command]
fn step_sandbox(batch: u32, state: State<SandboxState>) -> Result<SandboxSnapshot, String> {
    let mut guard = state.0.lock().map_err(|e| e.to_string())?;
    let live = guard.as_mut().ok_or("sandbox not initialized")?;
    for _ in 0..batch.clamp(1, 1000) {
        if live.sandbox.finished {
            break;
        }
        live.sandbox.step();
        if live.log.len() < LOG_CAP {
            live.log.push(LogRow::from(&live.sandbox.outcomes()));
        }
    }
    Ok(sandbox_snapshot(&live.sandbox))
}

#[derive(Serialize)]
struct CohortSweepView {
    name: String,
    excess_mean: f64,
    excess_ci95: f64,
    beat_fraction: f64,
    sharpe_mean: f64,
    max_drawdown_mean: f64,
}

#[derive(Serialize)]
struct SweepView {
    seeds: usize,
    days: u64,
    benchmark_log_return: f64,
    cohorts: Vec<CohortSweepView>,
}

/// Re-run the current town (same data, guilds and settings) on `seeds`
/// different seeds for one pass over the real history, and summarise how
/// robust each guild's result is against buy-and-hold.
#[tauri::command]
async fn sweep_seeds(seeds: u32, state: State<'_, SandboxState>) -> Result<SweepView, String> {
    let (prices, cohorts, params) = {
        let guard = state.0.lock().map_err(|e| e.to_string())?;
        let live = guard.as_ref().ok_or("sandbox not initialized")?;
        (live.prices.clone(), live.cohorts.clone(), live.params.clone())
    };
    let n = seeds.clamp(2, 256) as u64;
    let days = prices.len().saturating_sub(1 + params.warmup).max(1) as u64;
    // Seeds are offset from the live one so the sweep is reproducible per build.
    let seed_list: Vec<u64> = (0..n).map(|i| params.seed.wrapping_add(i + 1)).collect();
    let summary = tauri::async_runtime::spawn_blocking(move || {
        let p = SandboxParams { replay: Replay::Stop, ..params };
        sweep(&prices, &cohorts, &p, &seed_list, days)
    })
    .await
    .map_err(|e| format!("sweep failed: {e}"))?;
    Ok(SweepView {
        seeds: summary.seeds,
        days: summary.days,
        benchmark_log_return: summary.benchmark_log_return,
        cohorts: summary
            .cohorts
            .into_iter()
            .map(|c| CohortSweepView {
                name: c.name,
                excess_mean: c.excess_mean,
                excess_ci95: c.excess_ci95,
                beat_fraction: c.beat_fraction,
                sharpe_mean: c.sharpe_mean,
                max_drawdown_mean: c.max_drawdown_mean,
            })
            .collect(),
    })
}

/// Write the run so far as CSV into the user's Downloads folder (or the app
/// data folder if there is none) and return the file path.
#[tauri::command]
fn export_run(app: AppHandle, state: State<SandboxState>) -> Result<String, String> {
    let guard = state.0.lock().map_err(|e| e.to_string())?;
    let live = guard.as_ref().ok_or("sandbox not initialized")?;
    if live.log.is_empty() {
        return Err("nothing to export yet — run the town first".into());
    }
    let dir = app.path().download_dir().or_else(|_| app.path().app_data_dir()).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let safe_symbol: String = live.symbol.chars().filter(|c| c.is_ascii_alphanumeric()).collect();
    let path = dir.join(format!("trading-town_{}_seed{}_day{}.csv", safe_symbol, live.params.seed, live.sandbox.day));
    std::fs::write(&path, log_csv(&live.cohorts, &live.log)).map_err(|e| e.to_string())?;
    Ok(path.display().to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(SandboxState::default())
        .invoke_handler(tauri::generate_handler![init_sandbox, step_sandbox, sweep_seeds, export_run])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(json: &str) -> SandboxCfg {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn params_parse_replay_and_sizing() {
        let p = sandbox_params(&cfg(
            r#"{"symbol":"AAPL","cohorts":[],"replay":"bootstrap","block":10,"sizing":"meanvariance","gamma":5,"cost_bps":9999}"#,
        ));
        assert_eq!(p.replay, Replay::Bootstrap { block: 10 });
        assert_eq!(p.sizing, Sizing::MeanVariance { gamma: 5.0 });
        assert_eq!(p.cost_bps, 500.0, "cost is clamped");
        let d = sandbox_params(&cfg(r#"{"symbol":"AAPL","cohorts":[]}"#));
        assert_eq!(d.replay, Replay::Loop);
        assert_eq!(d.sizing, Sizing::Volatility);
    }

    #[test]
    fn cohorts_are_clamped() {
        let c = cohorts_from(&cfg(
            r#"{"symbol":"AAPL","cohorts":[{"name":"x","count":99999,"memory":5,"herding":-1,"risk":50}]}"#,
        ));
        assert_eq!(c[0].count, 500);
        assert_eq!((c[0].memory, c[0].herding, c[0].risk), (1.0, 0.0, 10.0));
    }

    #[test]
    fn csv_has_header_and_one_row_per_day() {
        let cohorts = vec![Cohort::new("Calm", 5, 0.03, 0.1, 1.0), Cohort::new("A, \"B\"", 5, 0.03, 0.1, 1.0)];
        let prices: Vec<f64> = (0..120).map(|i| 100.0 + i as f64).collect();
        let mut s = Sandbox::new(prices, cohorts.clone(), SandboxParams::default());
        let mut rows = Vec::new();
        for _ in 0..10 {
            s.step();
            rows.push(LogRow::from(&s.outcomes()));
        }
        let csv = log_csv(&cohorts, &rows);
        let lines: Vec<&str> = csv.lines().collect();
        assert_eq!(lines.len(), 11);
        assert!(lines[0].ends_with(",wealth_Calm,\"wealth_A, \"\"B\"\"\""));
        assert_eq!(lines[1].split(',').count(), 11);
    }
}
