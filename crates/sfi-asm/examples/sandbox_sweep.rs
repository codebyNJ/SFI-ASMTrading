//! Headless robustness check for the Path-3 sandbox: run the default town on
//! the bundled AAPL history across many seeds and report, per guild, the
//! excess log return over buy-and-hold with a ~95% confidence interval.
//!
//! ```text
//! cargo run --release --example sandbox_sweep                     # bundled AAPL, 32 seeds
//! cargo run --release --example sandbox_sweep -- path.json 64 10  # file, seeds, cost bps
//! ```
//! The JSON file uses the bundled snapshot format: `{"c": [close, ...], ...}`.

use sfi_asm::{sweep, Cohort, Replay, SandboxParams};

const DEFAULT_DATA: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../src-tauri/resources/stock_aapl.json");

/// Minimal extraction of the `"c": [...]` closes array (the engine crate stays
/// dependency-free, so no serde here).
fn closes(json: &str) -> Vec<f64> {
    let start = json.find("\"c\"").and_then(|i| json[i..].find('[').map(|j| i + j + 1));
    let Some(start) = start else { return Vec::new() };
    let end = json[start..].find(']').map(|j| start + j).unwrap_or(json.len());
    json[start..end].split(',').filter_map(|x| x.trim().parse::<f64>().ok()).collect()
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let path = args.first().map(String::as_str).unwrap_or(DEFAULT_DATA);
    let n_seeds: u64 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(32);
    let cost_bps: f64 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(0.0);

    let json = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("reading {path}: {e}"));
    let prices = closes(&json);
    assert!(prices.len() > 100, "need a longer series, got {}", prices.len());

    // Mirrors the app's default guilds (src/data/presets.ts).
    let cohorts = vec![
        Cohort::new("Patient", 8, 0.015, 0.05, 0.6),
        Cohort::new("Calm", 10, 0.03, 0.15, 0.9),
        Cohort::new("Bubbly", 10, 0.08, 0.55, 1.5),
        Cohort::new("Herders", 8, 0.06, 0.9, 1.2),
        Cohort::new("Reckless", 4, 0.12, 0.5, 2.2),
    ];
    let params = SandboxParams { replay: Replay::Stop, cost_bps, ..SandboxParams::default() };
    let days = (prices.len() - 1 - params.warmup) as u64;
    let seeds: Vec<u64> = (1..=n_seeds).collect();

    let t0 = std::time::Instant::now();
    let s = sweep(&prices, &cohorts, &params, &seeds, days);
    println!(
        "{} seeds x {} days, cost {cost_bps} bps  ({:.1}s)\nbuy & hold log return: {:+.3}\n",
        s.seeds,
        s.days,
        t0.elapsed().as_secs_f64(),
        s.benchmark_log_return
    );
    println!("{:<10} {:>18} {:>10} {:>8} {:>8}", "guild", "excess vs B&H", "beats B&H", "Sharpe", "maxDD");
    for c in &s.cohorts {
        println!(
            "{:<10} {:>+8.3} ± {:<7.3} {:>9.0}% {:>8.2} {:>7.0}%",
            c.name,
            c.excess_mean,
            c.excess_ci95,
            c.beat_fraction * 100.0,
            c.sharpe_mean,
            c.max_drawdown_mean * 100.0
        );
    }
    println!("\nAn interval that straddles 0 means the guild did not reliably beat (or lose to) buy & hold.");
}
