# Trading Town — maturity review & improvement roadmap

A review of where the project stands and what would move it forward, with the
reasoning for each item. Items marked **✅ done in this PR** ship with the review;
the others are ranked by value versus effort.

---

## 1. How mature is it right now?

**Overall: a well-built research prototype (roughly "alpha").** The core idea is
clear and the architecture is sound. The gaps are in scientific validity,
reproducibility tooling, and release hygiene, not in the code's structure.

| Area | Rating | Notes |
|---|---|---|
| Architecture | 🟢 Strong | Pure, dependency-free Rust engine, a thin Tauri layer, and a UI that only renders snapshots. The boundaries are clear and documented in CONTRIBUTING. |
| Engine correctness | 🟡 Good, with bugs | Deterministic seeded RNG, drift-neutral mutation, bounded fitness, and a variance floor, so the classic SFI pitfalls are handled. The Path-3 sandbox had a loop-seam P&L bug (fixed here). |
| Tests | 🟡 Decent for the engine | 22 unit tests before this PR, covering invariants, determinism and REE recovery. There were no frontend tests and no CI (CI added here). |
| Scientific validity | 🟠 Weakest area | There was no baseline to compare against (added here) and no trading costs (added here). Traders relearn the same looping path, so results are in-sample, and each result comes from one seed. |
| UX | 🟢 Charming | The pixel town, the inspector and the insights panel work well. Errors were only logged to the console, and offline fallback silently switched the company (both fixed here). |
| Release hygiene | 🔴 Needs work | Stale `bun.lock`, two lockfiles, ~70 MB of unused art shipped in the bundle, leftover `remind` identifiers, `csp: null`, dead commands, and a README link to a missing file. |
| Docs | 🟢 Above average | README, CONCEPT and CONTRIBUTING are thoughtful, and the "honesty rules" are unusually good for a hobby project. |

---

## 2. Changes made in this PR

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

## 3. Recommended next steps (not in this PR)

### P0: scientific validity (these matter most for a research tool)

1. **Stop replaying the same path (in-sample learning).**
   After one loop, the rules have been selected on this exact price path, so
   loop 2 onward measures memorisation, not skill. Options, from cheapest up:
   (a) run a single pass and stop, clearly labelled; (b) hold out the last
   N % as an out-of-sample window and report the two periods separately;
   (c) after the first pass, feed a **stationary block bootstrap** of the
   returns so each loop is a fresh but statistically similar path.
   *Why:* without this, a guild "winning" can be pure overfitting.

2. **Multi-seed runs and confidence intervals.**
   Every number on screen is one seed. Add a headless `Sandbox` batch runner
   (an engine example plus a Tauri command) that runs K seeds and reports
   mean ± CI of per-guild excess return over buy-and-hold. Only differences
   that survive across seeds should be shown as findings.
   *Why:* this follows the project's own honesty rule ("if a finding is
   tentative, say so").

3. **Risk-adjusted metrics.**
   Report, per guild: excess return over the benchmark, annualised Sharpe,
   max drawdown and turnover. Use the **median** or geometric mean alongside
   the arithmetic mean, because a few lucky "Reckless" traders dominate an
   average of compounding wealth.

4. **Warm-up period.**
   On days 0–49 the MA20 and MA50 signals are computed from partial windows,
   yet traders trade on them from day 0. Start settlement at `idx = 50` (or
   the longest lookback) and let rules only observe before that.

5. **Position sizing should use the rule's own confidence.**
   The position is currently `risk · r̂ / σ_price`, and the active rule's
   forecast-error variance (`active_acc`) is ignored. The SFI-faithful choice
   is mean-variance sizing, `r̂ / (λ · var_rule)`. This makes accurate rules
   bet more, which is the mechanism behind the original paper's results.

6. **Bankruptcy handling.**
   Wealth is floored at `1e-6`, so ruined traders stay in the population
   forever, still teaching neighbours via the GA and dragging down averages.
   Either retire and respawn them (tracking the count as a metric), or exclude
   them from social learning.

### P1: product and UX

7. **Fast-forward / batch stepping.** At the maximum of 6 days/s, a 5-year
   series takes about 3.5 minutes per loop. `step_sandbox` already takes
   `batch`, so add a "⏩ ×20" mode that steps several days per frame.
8. **Export a run.** A CSV/JSON of daily price, sentiment, herding, guild
   wealth and benchmark lets users analyse results outside the app, and makes
   results citable.
9. **Expose `cost_bps`** as an Advanced slider (the backend is ready), with
   0, 5 and 20 bps presets.
10. **Cache fetched series on disk** (the app data dir) with a timestamp, so
    the second run of a ticker is instant and offline-capable for *that*
    ticker rather than falling back to AAPL.
11. **Price chart with trade markers** under the town. The notice board shows
    today's price, but there is no view of where the town was long or short
    against the path.
12. **Rename the "Value" guild** (e.g. "Patient"). The project's own honesty
    rules say there is no fundamental value in Path-3. The name suggests
    value investing, but the guild's parameters are just a long memory and
    low herding.

### P2: engineering hygiene

13. **One package manager.** `bun.lock` is **stale**: it is missing pixi.js,
    radix, tailwind and others, while `tauri.conf.json` runs `bun run dev`.
    `pnpm-lock.yaml` is in sync (CI uses it). Either regenerate `bun.lock`
    and delete the pnpm lockfile, or switch `tauri.conf.json` to pnpm and
    delete `bun.lock`.
14. **Ship only the art you use.** `vite build` copies all of `public/` into
    the app bundle, which comes to about **115 MB**. That includes the unused
    "Simple Summer" pack (`public/EPS` at 67 MB and `public/PNG` at 2 MB) and
    the whole Sunnyside pack with its GameMaker project. Only a handful of
    sprites are used. Besides the install size, shipping the *raw* pack inside
    a distributed binary is at odds with the Sunnyside licence ("don't
    repackage the raw assets"). Copy the used sprites into `public/world/`
    and delete the rest.
15. **Remove dead surface.** `init_sim`, `step_sim`, `calibrate` and
    `fetch_real_market` are registered but unused by the UI, and
    `resources/btc_daily.json` is not loaded anywhere. Either put the classic
    SFI mode behind a UI toggle or move it to examples only.
16. **Rename leftovers.** The package and product are still `remind`, with
    identifier `com.remind.app`. The README also links to a missing
    `PRD_PHASE_2.md`.
17. **Set a CSP.** `"csp": null` disables Tauri's content-security policy.
    The app loads only local assets, so a strict
    `default-src 'self'; img-src 'self' data: blob:` is cheap
    defence-in-depth.
18. **`cargo fmt` + clippy gate.** The engine isn't `rustfmt`-clean today, and
    clippy reports 2 warnings. Run `cargo fmt` once in a dedicated commit,
    then add `cargo fmt --check` and `clippy -D warnings` to CI.
19. **Frontend tests.** Add a Vitest smoke test for `Insights` and
    `TraderInspector` with a fixture snapshot. The DTO ↔ `types.ts` mirror
    is currently checked only by hand; consider generating the TS types with
    `ts-rs` or `specta`.
20. **Split `SunnysideWorld.tsx`** (1,200 lines) into tile map, sprites and
    trader layer. It is the one file that is getting hard to change safely.

### P3: research extensions (after the P0 items)

- **An endogenous-price mode** that reuses the classic `Simulation` (already
  in the crate), so users can compare "town reacts to a real market" with
  "town *makes* the market" (bubbles that the traders cause themselves).
- **Network experiments in the sandbox**: does a scale-free (hub) topology
  measurably increase herding or drawdowns across seeds, compared with a
  lattice? Item 2 makes this answerable.
- **Richer signals**: volume, a volatility regime, and a cross-asset signal
  (e.g. SPY for single stocks).

---

## 4. Verification for this PR

- `cargo test --release` in `crates/sfi-asm`: 25 passed (3 new).
- `cargo test` in `src-tauri`: 4 passed (2 new).
- `pnpm exec tsc --noEmit` and `pnpm build`: clean.
- Determinism: with `cost_bps = 0`, a non-wrapping run matches the previous
  engine bit for bit (the evolve optimisation doesn't change results). Runs
  that cross the loop seam change, as intended.
