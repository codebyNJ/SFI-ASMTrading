//! Real company market data from Yahoo Finance (daily closes).
//!
//! In the Path-3 sandbox this real price series *drives* the simulation: it plays
//! forward day by day and the agents react to it. We fetch live (no API key, just
//! a browser User-Agent), cache every successful fetch on disk, and when offline
//! fall back to that cache and then to a bundled snapshot.

use std::path::Path;
use std::time::Duration;

use serde::Serialize;
use serde_json::Value;

/// Offline fallback (Apple, ~5y daily) so the app always has a series to play.
const BUNDLED_AAPL: &str = include_str!("../resources/stock_aapl.json");
const UA: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120 Safari/537.36";

#[derive(Serialize, Clone)]
pub struct MarketSeries {
    pub symbol: String,
    pub name: String,
    /// "yahoo" | "cache" | "bundled"
    pub source: String,
    /// The ticker the user asked for. Differs from `symbol` when the live fetch
    /// failed and the bundled AAPL snapshot was substituted.
    pub requested: String,
    /// True when `symbol != requested` — the UI must say so rather than
    /// silently showing another company's prices.
    pub fallback: bool,
    pub days: usize,
    pub last_price: f64,
    /// Unix-second timestamps aligned with `prices`.
    pub timestamps: Vec<i64>,
    /// Daily closing prices — the series the sandbox plays forward.
    pub prices: Vec<f64>,
    pub returns: Vec<f64>,
    /// Daily log-return std.
    pub ret_std: f64,
    /// Annualized volatility (ret_std * sqrt(252) for trading days).
    pub annual_vol: f64,
    /// Excess kurtosis of daily returns (fat tails > 0).
    pub kurtosis: f64,
    /// Lag-1 autocorrelation of |returns| (volatility clustering).
    pub abs_ac1: f64,
    pub ret_hist: Vec<u32>,
    pub ret_hist_lo: f64,
    pub ret_hist_hi: f64,
}

fn http_get(url: &str) -> Option<String> {
    ureq::get(url).set("User-Agent", UA).timeout(Duration::from_secs(20)).call().ok()?.into_string().ok()
}

/// Parse Yahoo's chart JSON into (timestamps, closes), dropping null gaps.
fn parse_yahoo(body: &str) -> Option<(String, Vec<i64>, Vec<f64>)> {
    let v: Value = serde_json::from_str(body).ok()?;
    let result = v.get("chart")?.get("result")?.get(0)?;
    let symbol = result.get("meta").and_then(|m| m.get("symbol")).and_then(|s| s.as_str()).unwrap_or("").to_string();
    let ts = result.get("timestamp")?.as_array()?;
    let close = result.get("indicators")?.get("quote")?.get(0)?.get("close")?.as_array()?;

    let mut times = Vec::with_capacity(ts.len());
    let mut closes = Vec::with_capacity(ts.len());
    for (t, c) in ts.iter().zip(close.iter()) {
        if let (Some(ti), Some(ci)) = (t.as_i64(), c.as_f64()) {
            if ci.is_finite() && ci > 0.0 {
                times.push(ti);
                closes.push(ci);
            }
        }
    }
    (closes.len() >= 60).then_some((symbol, times, closes))
}

/// Parse the bundled `{symbol,name,t:[],c:[]}` snapshot.
fn parse_bundled() -> Option<(String, String, Vec<i64>, Vec<f64>)> {
    parse_snapshot(BUNDLED_AAPL)
}

/// Parse a `{symbol,name,t:[],c:[]}` snapshot (bundled or cached).
fn parse_snapshot(json: &str) -> Option<(String, String, Vec<i64>, Vec<f64>)> {
    let v: Value = serde_json::from_str(json).ok()?;
    let symbol = v.get("symbol")?.as_str()?.to_string();
    let name = v.get("name").and_then(|n| n.as_str()).unwrap_or("").to_string();
    let t: Vec<i64> = v.get("t")?.as_array()?.iter().filter_map(|x| x.as_i64()).collect();
    let c: Vec<f64> = v.get("c")?.as_array()?.iter().filter_map(|x| x.as_f64()).collect();
    Some((symbol, name, t, c))
}

/// Normalise a user-supplied ticker, rejecting anything that isn't a plain
/// Yahoo symbol (letters, digits, `.`, `-`, `^`, `=`), so it can't smuggle path
/// segments or query parameters into the request URL.
pub fn sanitize_symbol(symbol: &str) -> Option<String> {
    let sym = symbol.trim().to_uppercase();
    let ok = !sym.is_empty()
        && sym.len() <= 15
        && sym.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '^' | '='));
    ok.then_some(sym)
}

fn cache_file(dir: &Path, sym: &str) -> std::path::PathBuf {
    dir.join(format!("{sym}.json"))
}

/// Best-effort write of a fetched series to the on-disk cache.
fn write_cache(dir: &Path, sym: &str, name: &str, t: &[i64], c: &[f64]) {
    let body = serde_json::json!({ "symbol": sym, "name": name, "t": t, "c": c });
    if std::fs::create_dir_all(dir).is_ok() {
        let _ = std::fs::write(cache_file(dir, sym), body.to_string());
    }
}

fn read_cache(dir: &Path, sym: &str) -> Option<(String, String, Vec<i64>, Vec<f64>)> {
    let json = std::fs::read_to_string(cache_file(dir, sym)).ok()?;
    parse_snapshot(&json).filter(|(s, _, _, c)| s == sym && c.len() >= 60)
}

/// Load a daily series for `symbol`: live from Yahoo, else the last cached
/// fetch of that symbol (when `cache_dir` is given), else the bundled AAPL.
pub fn load(symbol: &str, cache_dir: Option<&Path>) -> MarketSeries {
    let requested = symbol.trim().to_uppercase();
    if let Some(sym) = sanitize_symbol(symbol) {
        let url = format!("https://query1.finance.yahoo.com/v8/finance/chart/{sym}?range=5y&interval=1d");
        if let Some((ysym, t, c)) = http_get(&url).and_then(|b| parse_yahoo(&b)) {
            let name = if ysym.is_empty() { sym.clone() } else { ysym };
            if let Some(dir) = cache_dir {
                write_cache(dir, &sym, &name, &t, &c);
            }
            return compute(&sym, &name, "yahoo", &requested, t, c);
        }
        if let Some((csym, cname, t, c)) = cache_dir.and_then(|d| read_cache(d, &sym)) {
            return compute(&csym, &cname, "cache", &requested, t, c);
        }
    }
    if let Some((bsym, bname, t, c)) = parse_bundled() {
        return compute(&bsym, &bname, "bundled", &requested, t, c);
    }
    compute(&requested, &requested, "bundled", &requested, Vec::new(), Vec::new())
}

fn compute(
    symbol: &str,
    name: &str,
    source: &str,
    requested: &str,
    timestamps: Vec<i64>,
    prices: Vec<f64>,
) -> MarketSeries {
    let mut returns = Vec::new();
    for w in prices.windows(2) {
        returns.push((w[1] / w[0]).ln());
    }

    let n = returns.len().max(1) as f64;
    let mean = returns.iter().sum::<f64>() / n;
    let (mut m2, mut m4) = (0.0, 0.0);
    for &r in &returns {
        let d = r - mean;
        m2 += d * d;
        m4 += d * d * d * d;
    }
    let var = m2 / n;
    let std = var.sqrt();
    let kurtosis = if var > 1e-300 { (m4 / n) / (var * var) - 3.0 } else { 0.0 };

    let abs_ac1 = abs_lag1(&returns);

    let bins = 41usize;
    let hi = returns.iter().fold(0.0_f64, |a, &b| a.max(b.abs())).max(1e-6);
    let lo = -hi;
    let mut ret_hist = vec![0u32; bins];
    for &r in &returns {
        let idx = (((r - lo) / (hi - lo)) * bins as f64).floor() as isize;
        ret_hist[idx.clamp(0, bins as isize - 1) as usize] += 1;
    }

    MarketSeries {
        symbol: symbol.to_string(),
        name: name.to_string(),
        source: source.to_string(),
        requested: requested.to_string(),
        fallback: !symbol.eq_ignore_ascii_case(requested),
        days: prices.len(),
        last_price: prices.last().copied().unwrap_or(0.0),
        timestamps,
        prices,
        returns,
        ret_std: std,
        annual_vol: std * 252.0_f64.sqrt(),
        kurtosis,
        abs_ac1,
        ret_hist,
        ret_hist_lo: lo,
        ret_hist_hi: hi,
    }
}

fn abs_lag1(returns: &[f64]) -> f64 {
    if returns.len() < 2 {
        return 0.0;
    }
    let n = returns.len() as f64;
    let mean = returns.iter().map(|r| r.abs()).sum::<f64>() / n;
    let mut var = 0.0;
    for &r in returns {
        let d = r.abs() - mean;
        var += d * d;
    }
    var /= n;
    if var <= 1e-300 {
        return 0.0;
    }
    let mut cov = 0.0;
    for i in 1..returns.len() {
        cov += (returns[i].abs() - mean) * (returns[i - 1].abs() - mean);
    }
    cov / n / var
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_series_parses_and_has_sensible_stats() {
        let (sym, _name, t, c) = parse_bundled().expect("bundled snapshot should parse");
        assert_eq!(sym, "AAPL");
        assert!(c.len() > 500, "expected a long history, got {}", c.len());
        assert_eq!(t.len(), c.len());
        let m = compute("AAPL", "Apple Inc.", "bundled", "AAPL", t, c);
        assert!(!m.fallback);
        assert!(m.ret_std > 0.001 && m.ret_std < 0.2, "daily vol {}", m.ret_std);
        assert!(m.kurtosis.is_finite());
        assert_eq!(m.prices.len(), m.days);
        assert!(m.last_price > 0.0);
    }

    #[test]
    fn fallback_is_flagged_when_another_symbol_is_substituted() {
        let (_, _, t, c) = parse_bundled().unwrap();
        let m = compute("AAPL", "Apple Inc.", "bundled", "MSFT", t, c);
        assert!(m.fallback);
        assert_eq!(m.requested, "MSFT");
    }

    #[test]
    fn cache_round_trips_and_rejects_other_symbols() {
        let dir = std::env::temp_dir().join(format!("tt-cache-test-{}", std::process::id()));
        let (_, _, t, c) = parse_bundled().unwrap();
        write_cache(&dir, "MSFT", "Microsoft", &t, &c);
        let (sym, name, t2, c2) = read_cache(&dir, "MSFT").expect("cached series should load");
        assert_eq!((sym.as_str(), name.as_str()), ("MSFT", "Microsoft"));
        assert_eq!((t2, c2), (t, c));
        assert!(read_cache(&dir, "NVDA").is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn symbols_are_sanitized() {
        assert_eq!(sanitize_symbol(" aapl ").as_deref(), Some("AAPL"));
        assert_eq!(sanitize_symbol("BRK-B").as_deref(), Some("BRK-B"));
        assert_eq!(sanitize_symbol("^GSPC").as_deref(), Some("^GSPC"));
        assert_eq!(sanitize_symbol("BTC-USD").as_deref(), Some("BTC-USD"));
        assert!(sanitize_symbol("").is_none());
        assert!(sanitize_symbol("../v7/finance").is_none());
        assert!(sanitize_symbol("AAPL?range=max").is_none());
        assert!(sanitize_symbol("A&B").is_none());
    }
}
