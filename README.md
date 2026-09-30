# 🏘️ Trading Town

**A living, agent‑based stock market you can watch.** A little pixel town of
self‑taught traders reacts, day by day, to a **real company's share price** — and
you watch booms, herding, and crashes emerge from their behaviour. It's a modern,
playable take on the **Santa Fe Artificial Stock Market** (Arthur, Holland,
LeBaron, Palmer, Tayler 1996).

> Pick a company → compose the town (how many of each kind of trader) → press Run →
> watch (slowly) who gets rich, when the crowd panics, and click any trader to see
> *why* it's buying or selling.

See **[CONCEPT.md](CONCEPT.md)** for the story/metaphor
and **[docs/IMPROVEMENTS.md](docs/IMPROVEMENTS.md)** for a maturity review and roadmap.


---

## What it does

- **Real data, real ticker.** Loads a company's daily price history from Yahoo
  Finance (AAPL, MSFT, NVDA, TSLA, …). Every fetch is cached, so a ticker you
  have loaded before also works offline; otherwise a bundled AAPL sample is used
  and the title bar says so. The price is *exogenous* — the town reacts to it,
  it doesn't invent it.
- **Self‑taught traders.** Each trader carries a population of evolving
  rules‑of‑thumb (a classifier system), forecasts the next move, takes a long/short
  position, **wins or loses money**, and **learns** (a genetic algorithm weeds out
  bad rules and copies fit neighbours over a social network).
- **Composable guilds.** Mix behavioural cohorts in one run — Patient, Calm, Bubbly,
  Herders, Reckless — each with its own temperament, herding tendency, and risk.
- **See the "why".** Click a townsperson to inspect its active rule, forecast,
  conviction, and the market signals it watches. A live **Insights** column shows
  the price with the town's net long/short position, the strategy mix (trend vs
  contrarian), belief spread, signal usage, and a per‑guild leaderboard with
  wealth vs **buy & hold**, Sharpe ratio and max drawdown.
- **Is it luck?** One click re‑runs the town on 16 seeds and reports each
  guild's result against buy & hold with a 95% confidence interval.
- **Honest replays.** When the real history ends, the town continues on a fresh
  *synthetic* path (a block bootstrap of the real returns, clearly labelled),
  replays the same history (labelled *in‑sample*), or stops — your choice.
- **Fast‑forward & export.** Up to 60 days/sec, and a CSV export of the run.
- **A pixel world + macOS‑style UI.** A draggable tile town (Sunnyside World art),
  with translucent floating panels, a custom titlebar, and SF fonts.

> **Note on honesty:** because the price is real/exogenous (the "Path 3" design),
> there is no *fundamental value* in the model — so the strategy split is labelled
> **Trend vs Contrarian**, not "fundamentalists vs trend‑followers".

---

## How it works (in one minute)

Each trading **day**:

1. The real price moves; the town reads it off the notice board.
2. Every trader picks its best‑matching rule and **forecasts** tomorrow's return.
3. It takes a **position** (buy / short), sized by its forecast ÷ recent volatility
   (or, optionally, by mean‑variance sizing on the rule's own track record).
4. Money is settled on the actual move, net of an optional trading cost — right
   calls grow wealth, wrong calls shrink it. A trader that loses 99% goes bankrupt
   and stops trading.
5. Every rule that fired **learns** from its error; periodically a **genetic
   algorithm** breeds the fittest rules and **copies** successful neighbours (herding).

Trading starts after a 50‑day warm‑up so every moving average has full history.
Same seed → same run (it's deterministic, not random noise). The interesting
behaviour — herding, strategy swings, who profits — *emerges*.

> **What the model finds so far:** on the bundled 5‑year AAPL history, every
> default guild ends well behind simply holding the stock, and that result holds
> across seeds (`cargo run --release --example sandbox_sweep`). The town is a
> lens on crowd behaviour, not a strategy that beats the market.

---

## Project structure

```
SFI-ASMTrading/
├─ crates/sfi-asm/        # Pure Rust simulation engine (no GUI) — unit‑tested
│  ├─ src/                #   dividend/REE, classifier agents, GA, network, sandbox (Path‑3)
│  └─ examples/           #   headless runs, incl. sandbox_sweep (multi‑seed robustness)
├─ src-tauri/             # Tauri shell: commands, stock-data fetch, app config
│  ├─ src/lib.rs          #   init_sandbox / step_sandbox / sweep_seeds / export_run
│  └─ src/market_data.rs  #   Yahoo Finance loader + disk cache + bundled fallback
├─ src/                   # Frontend (Vite + React + TS + Tailwind/shadcn + PixiJS)
│  ├─ App.tsx             #   two‑column layout, controls, stepping loop
│  ├─ SunnysideWorld.tsx  #   the pixel town (PixiJS)
│  ├─ components/         #   TitleBar, FloatingPanel, Insights, TraderInspector, ui/*
│  └─ lib/                #   pacing + formatting helpers (Vitest‑tested)
├─ public/                # Only the Sunnyside sprites the town uses + extracted tiles
├─ docs/IMPROVEMENTS.md   # maturity review & roadmap
└─ CONCEPT.md             # the story / metaphor
```

---

## Getting started

### Prerequisites
- **Rust** (stable) + **Cargo**
- **Node 18+** and a package manager
- Tauri OS prerequisites (on Windows: WebView2, MSVC build tools) — see
  <https://tauri.app/start/prerequisites/>

The project uses **pnpm** (`pnpm-lock.yaml` is the only lockfile, and
`tauri.conf.json` runs `pnpm dev` / `pnpm build`).

### Run (desktop app)
```bash
pnpm install
pnpm tauri dev
```

### Build a release
```bash
pnpm tauri build
```

### Run the engine on its own (fast, no GUI)
```bash
cargo test  --manifest-path crates/sfi-asm/Cargo.toml          # unit tests
cargo run   --release --manifest-path crates/sfi-asm/Cargo.toml --example sandbox_sweep
```
`sandbox_sweep` runs the default town on the bundled AAPL history across 32 seeds
and prints each guild's excess return over buy & hold with a 95% CI (pass a JSON
file, seed count and cost in bps as arguments to change them). Examples for the
classic endogenous‑price SFI model: `run` (regime demo), `regime` (knob sweep),
`experiment` (network comparison).

---

## Tech stack

| Layer | Choice | Why |
|---|---|---|
| Simulation engine | **Rust** (`crates/sfi-asm`) | Fast, deterministic, dependency‑free, unit‑tested |
| Desktop shell | **Tauri 2** | Native window, small footprint, Rust ↔ web bridge |
| Frontend | **Vite + React + TypeScript** | Standard, fast, type‑safe |
| Styling | **Tailwind v4 + shadcn‑style components** | Clean, consistent, macOS‑leaning |
| World rendering | **PixiJS** | High‑performance 2D pixel rendering |
| Stock data | **Yahoo Finance** (daily), bundled fallback | No API key required |

---

## Assets & credits

- **Sunnyside World** pixel‑art pack by **Daniel Diggle** —
  <https://danieldiggle.itch.io/sunnyside>. Used for the town tiles and characters.
  Please keep attribution; per its license, do not repackage/resell the raw assets
  or use them for AI training. (Asset files live under `public/`.)

Only the ~30 Sunnyside sprites the town actually draws are included (listed in
the asset manifest at the top of `src/SunnysideWorld.tsx`); the full pack and the
unused "Simple Summer" pack were removed, so the app ships only what it uses and
the bundle stays ~1 MB. To use a new sprite, add that file from your
own copy of the pack under the same path.

---

## Status & caveats

- This is a **research / learning sandbox**, not a trading or forecasting tool. It
  shows *how crowd behaviour reacts to* a real market — it does **not** predict it.


## License

- **Code:** MIT — see [LICENSE](LICENSE).
- **Bundled assets:** retain their own licenses (see *Assets & credits*).
