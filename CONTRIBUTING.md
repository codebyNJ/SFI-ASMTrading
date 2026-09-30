# Contributing to Trading Town

Thanks for your interest! This is a research/learning sandbox — an agent‑based
stock‑market simulation (a "Path‑3" Santa Fe Artificial Stock Market) with a
pixel‑art town front end. This guide explains how the repo is organised, how to
run it, and the conventions that keep it clean and trustworthy.

Read **[README.md](README.md)** for the high‑level picture and **[CONCEPT.md](CONCEPT.md)**
for the story/metaphor before diving in.

---

## Architecture, in one breath

The **Rust engine is the source of truth**; the UI only renders snapshots.

```
crates/sfi-asm  (pure simulation, no GUI, deterministic, unit‑tested)
      │   Sandbox::step()  →  per‑trader state + Outcomes
      ▼
src-tauri       (Tauri commands: build a Sandbox, step it, serialise a Snapshot)
      │   init_sandbox / step_sandbox / fetch_real_market
      ▼
src             (React + PixiJS: draws the town & insights from each Snapshot)
```

Put **simulation logic in `crates/sfi-asm`** (so it's testable without a window),
**data/serialisation in `src-tauri`**, and **rendering/interaction in `src`**. If
you find yourself doing market logic in TypeScript, it probably belongs in the
engine instead.

---

## Dev setup

### Prerequisites
- **Rust** (stable) + Cargo
- **Node 18+** and **pnpm**
- Tauri OS prerequisites — <https://tauri.app/start/prerequisites/>
  (Windows: WebView2 + MSVC build tools)

The repo uses **pnpm** only: `pnpm-lock.yaml` is the single lockfile and
`tauri.conf.json` runs `pnpm dev` / `pnpm build`. Please don't add another lockfile.

### Run / build / test
```bash
pnpm install
pnpm tauri dev          # desktop app, hot reload

# checks (CI runs all of these on every PR)
cargo fmt --check --manifest-path crates/sfi-asm/Cargo.toml
cargo clippy --all-targets --manifest-path crates/sfi-asm/Cargo.toml -- -D warnings
cargo test  --manifest-path crates/sfi-asm/Cargo.toml     # engine unit tests
cargo fmt --check --manifest-path src-tauri/Cargo.toml
cargo clippy --all-targets --manifest-path src-tauri/Cargo.toml -- -D warnings
cargo test  --manifest-path src-tauri/Cargo.toml          # backend tests
pnpm test                                                 # frontend unit tests (Vitest)
pnpm build                                                # tsc + production build
```
`rustfmt.toml` at the repo root sets the style (120 columns).
Run the engine headless (fast iteration, no GUI):
```bash
cargo run --release --manifest-path crates/sfi-asm/Cargo.toml --example experiment
# examples: run · regime · experiment
```

---

## Definition of done (PR checklist)

- [ ] CI is green: fmt, clippy (`-D warnings`) and tests for both crates, `pnpm test`,
      and `pnpm build` (strict TypeScript; **no unused vars/params**).
- [ ] New engine behaviour has a unit test.
- [ ] A new sprite is added to the asset manifest in `SunnysideWorld.tsx` *and* its
      file is committed under `public/` (only used sprites are kept).
- [ ] New page resources work under the CSP in `tauri.conf.json` (no inline
      scripts, `eval`, or remote hosts).
- [ ] App launches (`pnpm tauri dev`) with no new console errors; include a
      screenshot/GIF for visual changes.
- [ ] Determinism preserved (same seed → same run) — see below.
- [ ] No assets added that violate their license (see *Assets*).

---

## Conventions

### Rust engine (`crates/sfi-asm`)
- **Keep it dependency‑free and GUI‑free.** No Tauri/serde imports here; it's a pure
  library. (Serialisation lives in `src-tauri`.)
- **Stay deterministic.** All randomness goes through the seeded `Rng` (`rng.rs`).
  Don't introduce `std::time`, thread RNG, or unordered iteration that affects
  results. A fixed `seed` must reproduce a run exactly — there's a test for this;
  keep it green.
- **Test new logic.** Add `#[cfg(test)]` tests next to the code (see `sandbox.rs`,
  `market.rs`). Prefer testing invariants (wealth finite/positive, ranges, "learns
  on a trend") over snapshots.
- Run `cargo fmt` and address `cargo clippy` warnings.
- Instrumentation for the UI (e.g. `Trader.active_*`) should be **read‑only** — it
  must not change the dynamics.

### Tauri backend (`src-tauri`)
- Commands return plain serialisable DTOs (`SandboxSnapshot`, `TraderView`, …).
  Keep per‑step payloads small.
- **New command?** register it in `tauri::generate_handler!`. **New window/OS API?**
  add the matching permission to `capabilities/default.json` (e.g.
  `core:window:allow-…`) or it will fail at runtime.
- Network access (e.g. stock data) must have a **fallback** (we bundle a snapshot in
  `resources/`) so the app works offline.

### Frontend (`src`)
- **Strict TypeScript** (`noUnusedLocals`/`noUnusedParameters` are on). Shared types
  live in `src/types.ts`; mirror any backend DTO change there.
- **Styling:** Tailwind v4 + the shadcn‑style primitives in `src/components/ui`.
  Reuse them; keep the light/macOS look (glass floating panels, SF fonts, tabular
  numbers). Use the `@/` import alias.
- **Rendering:** all PixiJS/world code stays in `SunnysideWorld.tsx`. Keep it cheap —
  cap the ticker (`maxFPS`), do heavy per‑trader updates only on a **new day**, and
  only redraw on change. The world reads from a `dataRef`; React passes snapshots in.
- Don't put market logic in React — request it from the engine via a command.

### Honesty rules (important for a research project)
- The price is **real & exogenous** (Path‑3): the town reacts, it doesn't set price.
  Don't add a "fundamental value" readout — we label strategy as **Trend vs
  Contrarian** for this reason.
- Don't overstate results in code comments or UI copy. If a metric is noisy or a
  finding is tentative, say so.

---

## How to add common things

- **A trader guild (cohort):** add an entry to `COHORT_PRESETS` in
  `src/data/presets.ts` (name, blurb, `memory`/`herding`/`risk`, colour).
- **A company:** add to `COMPANIES` in `src/data/presets.ts` (Yahoo ticker).
- **An insight panel:** add a component used by `src/components/Insights.tsx`; if it
  needs new numbers, add them to the engine/`SandboxSnapshot` first, then `types.ts`.
- **Engine parameter:** add to `SandboxParams`, thread it through
  `src-tauri` (`SandboxCfg` + `sandbox_params`) and the Controls panel if user‑facing.
- **A performance claim:** check it holds across seeds before putting it in the UI or
  docs, with the in‑app *Test 16 seeds* button or the `sandbox_sweep` example. Compare
  against the buy‑and‑hold benchmark, not the starting wealth.

---

## Commits & PRs
- Keep PRs focused; describe **what** and **why**, and the user‑visible effect.
- Conventional‑commit style is welcome (`feat:`, `fix:`, `refactor:`, `docs:`…).
- Don't bump model defaults or break determinism without calling it out explicitly.

## Assets
Bundled art is **Sunnyside World by Daniel Diggle**
(<https://danieldiggle.itch.io/sunnyside>). Keep attribution; per its license, don't
repackage/resell the raw assets or use them for AI training. Don't add third‑party
assets without a compatible license and credit in the README.

## Where to help next
See **[docs/IMPROVEMENTS.md](docs/IMPROVEMENTS.md)** for the maturity review and the
open roadmap items (e.g. splitting `SunnysideWorld.tsx`, generated DTO types, an
endogenous‑price mode).

---

Questions or ideas? Open an issue describing the behaviour/feature and how it fits
the "watch a town react to a real market" goal.
