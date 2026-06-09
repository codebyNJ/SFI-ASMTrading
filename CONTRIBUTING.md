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
- **Node 18+** and a package manager
- Tauri OS prerequisites — <https://tauri.app/start/prerequisites/>
  (Windows: WebView2 + MSVC build tools)

> ⚠️ `src-tauri/tauri.conf.json` currently runs the frontend via **bun**
> (`bun run dev` / `bun run build`). Install [bun](https://bun.sh) **or** switch those
> two commands to your package manager. Please **pick one package manager** and use
> it consistently (don't commit two lockfiles — see *Known cleanups* below).

### Run / build / test
```bash
pnpm install            # or bun install
pnpm tauri dev          # desktop app, hot reload

# checks (run these before every PR)
cargo test  --manifest-path crates/sfi-asm/Cargo.toml     # engine unit tests
cargo check --manifest-path src-tauri/Cargo.toml          # backend type/compile + capabilities
npx tsc --noEmit                                          # frontend type check
pnpm build                                                # frontend production build
```
Run the engine headless (fast iteration, no GUI):
```bash
cargo run --release --manifest-path crates/sfi-asm/Cargo.toml --example experiment
# examples: run · regime · experiment
```

---

## Definition of done (PR checklist)

- [ ] `cargo test` (engine) passes, and new engine behaviour has a unit test.
- [ ] `cargo check` (src-tauri) passes.
- [ ] `npx tsc --noEmit` passes (strict mode; **no unused vars/params**).
- [ ] `pnpm build` succeeds.
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

## Known cleanups (good first contributions)
- Consolidate to **one package manager / lockfile** (both `bun.lock` and a pnpm
  lockfile exist).
- Remove the unused legacy "Simple Summer" pack (`public/PNG`, `public/EPS`).
- Add a code **LICENSE** (the README has a placeholder).

---

Questions or ideas? Open an issue describing the behaviour/feature and how it fits
the "watch a town react to a real market" goal.
