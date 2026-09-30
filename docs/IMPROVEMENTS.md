# Trading Town — maturity review & improvement roadmap

A review of where the project stood, what was changed, and what is still open,
with the reasoning for each item.

---

## 1. Maturity: before and after

**Before: a well-built research prototype (alpha).** The architecture was sound;
the gaps were in scientific validity, reproducibility tooling and release hygiene.
**After: a credible research beta.** Results are now benchmarked, risk-adjusted,
out-of-sample by default and checked across seeds, and the release is lean and
guarded by CI. The main open work is structural (splitting the renderer,
generated types) and new research modes.

| Area | Before | After | What changed |
|---|---|---|---|
| Architecture | 🟢 Strong | 🟢 Strong | Dead classic-SFI commands removed from the app; engine stays pure. |
| Engine correctness | 🟡 Bugs | 🟢 Good | Loop-seam P&L, NaN panic, and "herding 1.00 when flat" fixed; warm-up period. |
| Tests | 🟡 Engine only, no CI | 🟢 Good | 34 engine, 7 backend, 7 frontend tests; CI with fmt + clippy gates. |
| Scientific validity | 🟠 Weakest | 🟢 Good | Benchmark, costs, bootstrap replays, Sharpe/drawdown, 95% CIs across seeds. |
| UX | 🟢 Charming | 🟢 Richer | Market chart, risk table, robustness card, fast-forward, CSV export, honest badges. |
| Release hygiene | 🔴 Needs work | 🟢 Good | One lockfile, 115 MB → 1.2 MB bundle, strict CSP, renamed from `remind`. |
| Docs | 🟢 Good | 🟢 Good | README/CONTRIBUTING updated; this roadmap. |

### The headline finding
With the new tools the model can answer "does any guild beat the market?". On the
bundled 5-year AAPL history (1,205 trading days after warm-up), **every default
guild ends about 55–60% behind buy & hold with zero trading costs, on every one
of 32 seeds**, with 95% CIs of about ±1–4 points. At 10 bps the gap widens to
about 57–70%. This is the result the
project's honesty rules ask for: the town is a lens on crowd behaviour, not a
strategy. Reproduce with:

```bash
cargo run --release --manifest-path crates/sfi-asm/Cargo.toml --example sandbox_sweep            # 0 bps
cargo run --release --manifest-path crates/sfi-asm/Cargo.toml --example sandbox_sweep -- \
  src-tauri/resources/stock_aapl.json 32 10                                                       # 10 bps
```

---

## 2. Round 1: bug fixes and honesty basics

### ✅ Loop-seam bug: fake crash every time the data loops (engine)
`Sandbox::step` wraps from the last day back to day 0 and settled P&L on
`p[0] / p[n-1] - 1`. For a stock that rose 3× over five years, that is a
**−67 % one-day "crash"** every loop. Every trader was wiped out or enriched by
a move that never happened, and every matching rule learned from it. This
distorted the wealth leaderboard and the learned strategies after the first
loop.
**Fix:** the town sits out the seam day. Positions are flattened, no P&L is
settled and no learning happens. Test: `loop_seam_is_not_traded`.

### ✅ Buy-and-hold benchmark (engine → UI)
A guild ending at $1,400 means nothing on its own. If the stock doubled, $1,400
is a loss against doing nothing. `Outcomes.benchmark_wealth` compounds the
passive return on exactly the days traders settle. It is shown as a
"Buy & hold (baseline)" row above the leaderboard. This is the single most
important honesty addition for a research sandbox. Test:
`benchmark_tracks_buy_and_hold`.

### ✅ Transaction costs (engine, opt-in)
Before this change, trading had no friction, which strongly flatters
high-turnover guilds ("Reckless", "Bubbly"). `SandboxParams.cost_bps` charges
`|Δposition| × cost_bps / 10⁴` of wealth per day, and `SandboxCfg.cost_bps`
exposes it through Tauri. **The default is 0**, so existing runs are unchanged
(per CONTRIBUTING, defaults are not bumped silently). Test:
`transaction_costs_only_reduce_wealth`.

### ✅ GA robustness and performance (engine)
- `partial_cmp().unwrap()` became `total_cmp`, so a single NaN fitness can no
  longer panic the app mid-run.
- `evolve()` used to clone **every trader's full rule pool every day**. GA turns
  are staggered, so some trader is almost always due. It now clones only the
  pools of traders that are due and their neighbours. I checked that the
  results are **bit-identical** to the previous engine on a non-wrapping run.

### ✅ Honest offline fallback (backend → UI)
If a Yahoo fetch failed, `market_data::load("MSFT")` silently returned
**Apple's** bundled prices. The UI then played Apple while the user believed
they were watching Microsoft. `MarketSeries` now carries `requested` and
`fallback`, and the title bar shows "⚠ MSFT unavailable · offline sample".

### ✅ Input hardening (backend)
- Ticker symbols are validated (`[A-Z0-9.^=-]`, ≤ 15 chars) before they are
  put into the Yahoo URL, so a crafted symbol can't inject path or query
  segments.
- Cohort inputs are clamped (`memory ∈ [1e-4, 1]`, `herding ∈ [0, 1]`,
  `risk ∈ [0, 10]`, `count ≤ 500`). A `memory > 1` made the accuracy EMA
  negative.

### ✅ No UI freeze on Build (backend)
`init_sandbox` was a synchronous command doing up to 20 s of blocking HTTP, so
the window could freeze. It is now `async`, and the fetch runs on a blocking
worker.

### ✅ Visible errors (frontend)
Init and step failures were only logged with `console.error`, so the app looked
dead. They now appear in the Controls panel, and a failing step pauses the run.

### ✅ CI (GitHub Actions)
`.github/workflows/ci.yml` runs three jobs: engine tests and example builds,
src-tauri tests (with the Tauri system dependencies), and frontend `tsc` +
`vite build` via pnpm. This covers the "Definition of done" checklist from
CONTRIBUTING, which was previously manual only.

---


## 3. Round 2: the roadmap, implemented

### P0: scientific validity

1. ✅ **No more in-sample replays by default.** New `Replay` mode in the engine:
   - `Bootstrap { block }` continues on a stationary block bootstrap (Politis &
     Romano) of the real returns. The new path starts from the tail of the old
     one, so there is no seam and signals stay continuous. It keeps the real
     volatility and short-range structure without repeating the path.
   - `Loop` (the old behaviour) is kept, and the UI labels it "replay N · in-sample".
   - `Stop` ends the run after one pass.

   The app now defaults to **Bootstrap**, and the title bar shows "synthetic pass N"
   while the chart draws synthetic prices in purple. A separate RNG stream is used
   for paths, so the first pass is bit-identical whatever the mode (tested).
2. ✅ **Multi-seed robustness.** `sfi_asm::sweep` runs K seeds in parallel threads
   (deterministic, tested) and reports per guild the mean excess log return over
   buy & hold with a Student-t 95% CI, the share of seeds that beat it, and mean
   Sharpe and drawdown. It is available as the `sweep_seeds` command ("Test 16
   seeds" button → "Is it luck?" card, which only says "beats"/"lags" when the CI
   clears zero) and as the `sandbox_sweep` example.
3. ✅ **Risk-adjusted metrics.** Per guild: mean and median wealth, excess vs buy
   & hold, annualised Sharpe of pooled daily returns, max drawdown, turnover and
   bankruptcies. Buy & hold gets its own Sharpe and drawdown for comparison.
   Per trader: max drawdown (in the inspector).
4. ✅ **Warm-up.** Trading starts at index 50 (`SandboxParams.warmup`), so MA50
   is always computed on a full window; Loop replays restart there too.
5. ✅ **Mean-variance sizing** (opt-in): `Sizing::MeanVariance { gamma }` sizes by
   `r̂ / (γ · rule variance)`, so rules with a better track record bet more.
   The default stays volatility sizing so existing results are comparable.
6. ✅ **Bankruptcy.** Below `bust_fraction` (1%) of starting wealth a trader stops
   trading and is excluded from neighbours' social learning; counts are shown
   per guild.

### P1: product and UX

7. ✅ **Fast-forward** to 60 days/sec; above 6 days/sec the UI steps in batches
   (`src/lib/pacing.ts`, tested) so the town isn't redrawn every day.
8. ✅ **CSV export** of the run (day, pass, synthetic flag, price, sentiment,
   herding, net position, average wealth, buy & hold, per-guild wealth), written to
   Downloads.
9. ✅ **Cost slider** (0–50 bps; the app default is 5 bps), plus replay-mode and
   sizing selectors under Advanced.
10. ✅ **Disk cache** for fetched tickers (app cache dir). Offline, a ticker you
    loaded before is used ("cached data" badge) before falling back to AAPL.
11. ✅ **"Market & the crowd" chart:** the price with the town's net long/short
    position as bars.
12. ✅ **"Value" guild renamed to "Patient"** (there is no fundamental value in Path-3).

### P2: engineering hygiene

13. ✅ **pnpm only.** The stale `bun.lock` is deleted and `tauri.conf.json` runs
    `pnpm dev`/`pnpm build`. The unused `@sprite-foundry/townsfolk-48`
    dependency was removed.
14. ✅ **Only used art ships.** 31 Sunnyside sprites (the manifest in
    `SunnysideWorld.tsx`) plus the extracted tiles are kept. The rest of the
    pack, its GameMaker project, and the unused "Simple Summer" pack were removed.
    `dist` went from 115 MB to 1.2 MB.
15. ✅ **Dead surface removed:** `init_sim`, `step_sim`, `calibrate`,
    `fetch_real_market` and `btc_daily.json`. The classic SFI engine stays in
    the crate with its examples.
16. ✅ **Renamed** `remind` → `trading-town` (package, crate, product
    "Trading Town", identifier `com.tradingtown.desktop`). The broken
    `PRD_PHASE_2.md` link was removed, and the README license section now
    points at the MIT LICENSE.
    *Note:* the new identifier means a fresh app-data folder on existing installs.
17. ✅ **Strict CSP** in production (`default-src 'self'`, no remote hosts, no
    eval). PixiJS v8 needs `import "pixi.js/unsafe-eval"` to render without
    `eval`. The production build was verified in Chromium under this exact
    policy with no violations. `devCsp` is left open for Vite HMR.
18. ✅ **`cargo fmt` + `clippy -D warnings`** are clean in both crates and
    enforced in CI (`rustfmt.toml`: 120 columns).
19. 🟡 **Frontend tests: partly done.** Vitest is set up, with tests for pacing
    and formatting helpers, and runs in CI. Component tests and generated DTO
    types (`ts-rs`/`specta`) are still open.
20. ⬜ **Split `SunnysideWorld.tsx`: not done.** A 1,200-line renderer refactor
    needs visual regression checks in the real app; it should be its own PR.

### P3: research extensions (open)

- **An endogenous-price mode** that reuses the classic `Simulation`, to compare
  "town reacts to a market" with "town makes the market".
- **Network experiments across seeds:** does a scale-free topology raise
  herding or drawdowns relative to a lattice? `sweep` makes this a short
  experiment now.
- **Richer signals:** volume, volatility regime, cross-asset (e.g. SPY).

---

## 4. Behaviour changes to be aware of

- **Engine defaults:** 50-day warm-up (runs start at day 51); bankruptcy at 1%.
  `replay`, `sizing` and `cost_bps` keep their old behaviour by default
  (Loop / volatility / 0).
- **App defaults:** bootstrap replay and a 5 bps trading cost.
- **"Herding"** is 0 (not 1) when nobody holds a position.
- **Identifier change:** `com.remind.app` → `com.tradingtown.desktop`.

## 5. Verification

- Engine: 34 tests (`cargo test --release`), fmt and clippy clean.
- Backend: 7 tests, fmt and clippy clean.
- Frontend: 7 Vitest tests, `tsc` and `vite build` clean.
- UI: the production build was driven in headless Chromium under the production
  CSP, with a mocked Tauri bridge serving snapshots the real Rust backend
  produced. Build, run, fast-forward into a synthetic pass, the seed test,
  export, and Advanced all worked, and there were no CSP violations or missing
  sprites.
- Not verified: the packaged desktop app itself (no display in this environment).
