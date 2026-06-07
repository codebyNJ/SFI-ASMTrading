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


---

## What it does

- **Real data, real ticker.** Loads a company's daily price history from Yahoo
  Finance (AAPL, MSFT, NVDA, TSLA, …), with an offline fallback. The price is
  *exogenous* — the town reacts to it, it doesn't invent it.
- **Self‑taught traders.** Each trader carries a population of evolving
  rules‑of‑thumb (a classifier system), forecasts the next move, takes a long/short
  position, **wins or loses money**, and **learns** (a genetic algorithm weeds out
  bad rules and copies fit neighbours over a social network).
- **Composable guilds.** Mix behavioural cohorts in one run — Value, Calm, Bubbly,
  Herders, Reckless — each with its own temperament, herding tendency, and risk.
- **See the "why".** Click a townsperson to inspect its active rule, forecast,
  conviction, and the market signals it watches. A live **Insights** column shows
  the town's strategy mix (trend vs contrarian), belief spread, signal usage, and a
  per‑guild wealth leaderboard over the whole run.
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
3. It takes a **position** (buy / short), sized by conviction ÷ recent volatility.
4. Money is settled on the actual move — right calls grow wealth, wrong calls shrink it.
5. Every rule that fired **learns** from its error; periodically a **genetic
   algorithm** breeds the fittest rules and **copies** successful neighbours (herding).

Same seed → same run (it's deterministic, not random noise). The interesting
behaviour — bubbles, volatility clustering, who profits — *emerges*.

---

## Project structure

```
remind/
├─ crates/sfi-asm/        # Pure Rust simulation engine (no GUI) — unit‑tested
│  └─ src/                #   dividend/REE, classifier agents, GA, network, sandbox (Path‑3)
├─ src-tauri/             # Tauri shell: commands, stock-data fetch, app config
│  ├─ src/lib.rs          #   init_sandbox / step_sandbox / fetch_real_market / calibrate
│  └─ src/market_data.rs  #   Yahoo Finance loader + bundled fallback
├─ src/                   # Frontend (Vite + React + TS + Tailwind/shadcn + PixiJS)
│  ├─ App.tsx             #   two‑column layout, controls, stepping loop
│  ├─ SunnysideWorld.tsx  #   the pixel town (PixiJS)
│  └─ components/         #   TitleBar, FloatingPanel, Insights, TraderInspector, ui/*
├─ public/                # Pixel‑art assets (Sunnyside World) + extracted tiles
├─ CONCEPT.md             # the story / metaphor
└─ PRD_PHASE_2.md         # current phase spec
```

---

## Getting started

### Prerequisites
- **Rust** (stable) + **Cargo**
- **Node 18+** and a package manager
- Tauri OS prerequisites (on Windows: WebView2, MSVC build tools) — see
  <https://tauri.app/start/prerequisites/>

> ⚠️ This repo's `src-tauri/tauri.conf.json` runs the frontend via **bun**
> (`bun run dev` / `bun run build`). Either install [bun](https://bun.sh), **or**
> change `beforeDevCommand` / `beforeBuildCommand` there to your manager
> (`pnpm dev` / `pnpm build`).

### Run (desktop app)
```bash
pnpm install          # or: bun install
pnpm tauri dev        # or: bun tauri dev
```

### Build a release
```bash
pnpm tauri build
```

### Run the engine on its own (fast, no GUI)
```bash
cargo test  --manifest-path crates/sfi-asm/Cargo.toml          # unit tests
cargo run   --release --manifest-path crates/sfi-asm/Cargo.toml --example experiment
```
Other examples: `run` (regime demo), `regime` (knob sweep), `experiment` (network comparison).

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

The legacy "Simple Summer" top‑down pack (`public/PNG`, `public/EPS`) is **unused**
(kept only for reference) — the world is built from Sunnyside.

---

## Status & caveats

- This is a **research / learning sandbox**, not a trading or forecasting tool. It
  shows *how crowd behaviour reacts to* a real market — it does **not** predict it.


## License

- **Code:** add your preferred license here (e.g. MIT) — currently unspecified.
- **Bundled assets:** retain their own licenses (see *Assets & credits*).
