import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Play, Pause, RotateCcw, ChevronDown, ChevronRight, Download, FlaskConical } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Slider } from "@/components/ui/slider";
import { Select } from "@/components/ui/select";
import { Badge } from "@/components/ui/badge";
import { COHORT_PRESETS, COMPANIES, REPLAY_MODES, SIZING_MODES, TOPOLOGIES, type CohortPreset } from "@/data/presets";
import SunnysideWorld, { type CohortView } from "@/SunnysideWorld";
import { TitleBar } from "@/components/TitleBar";
import { FloatingPanel } from "@/components/FloatingPanel";
import { Insights } from "@/components/Insights";
import { TraderInspector } from "@/components/TraderInspector";
import { pushCapped, stepPlan } from "@/lib/pacing";
import type { Snapshot, Init, MarketHistory, Sweep } from "@/types";

const DEFAULT_COUNTS: Record<string, number> = { patient: 8, calm: 10, bubbly: 10, herder: 8, reckless: 4 };
const HIST_CAP = 400;
const SWEEP_SEEDS = 16;
const EMPTY_MARKET: MarketHistory = { price: [], net: [], synthetic: [] };

function App() {
  const [symbol, setSymbol] = useState("AAPL");
  const [counts, setCounts] = useState<Record<string, number>>(DEFAULT_COUNTS);
  const [topology, setTopology] = useState("scalefree");
  const [replay, setReplay] = useState("bootstrap");
  const [sizing, setSizing] = useState("volatility");
  const [costBps, setCostBps] = useState(5);
  const [speed, setSpeed] = useState(1.5);
  const [seed, setSeed] = useState(1000);
  const [advanced, setAdvanced] = useState(false);

  const [init, setInit] = useState<Init | null>(null);
  const [snap, setSnap] = useState<Snapshot | null>(null);
  const [running, setRunning] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [selected, setSelected] = useState<number | null>(null);
  const [history, setHistory] = useState<number[][]>([]);
  const [market, setMarket] = useState<MarketHistory>(EMPTY_MARKET);
  const [sweep, setSweep] = useState<Sweep | null>(null);
  const [sweeping, setSweeping] = useState(false);

  const speedRef = useRef(speed);
  speedRef.current = speed;
  const lastHistDay = useRef(-1);

  const active = COHORT_PRESETS.filter((p) => (counts[p.key] ?? 0) > 0);
  const total = active.reduce((s, p) => s + counts[p.key], 0);
  const cohortViews: CohortView[] = active.map((p) => ({ name: p.name, color: p.color, herding: p.herding }));
  const guilds = active.map((p) => ({ name: p.name, color: p.color }));

  const change = snap?.change ?? 0;

  async function build() {
    setRunning(false);
    setBusy(true);
    setSelected(null);
    setHistory(active.map(() => []));
    setMarket(EMPTY_MARKET);
    setSweep(null);
    lastHistDay.current = -1;
    setError(null);
    setNotice(null);
    try {
      const cohorts = active.map((p) => ({ name: p.name, count: counts[p.key], memory: p.memory, herding: p.herding, risk: p.risk }));
      const config = { symbol, cohorts, topology, degree: 6, seed, replay, sizing, cost_bps: costBps };
      const res = await invoke<Init>("init_sandbox", { config });
      setInit(res);
      setSnap(res.snapshot);
    } catch (e) {
      console.error("init_sandbox:", e);
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  async function runSweep() {
    setSweeping(true);
    setError(null);
    try {
      setSweep(await invoke<Sweep>("sweep_seeds", { seeds: SWEEP_SEEDS }));
    } catch (e) {
      setError(String(e));
    } finally {
      setSweeping(false);
    }
  }

  async function exportRun() {
    setError(null);
    try {
      const path = await invoke<string>("export_run");
      setNotice(`Saved ${path}`);
    } catch (e) {
      setError(String(e));
    }
  }

  useEffect(() => {
    build();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    if (!running) return;
    let on = true;
    let timer = 0;
    const tick = async () => {
      if (!on) return;
      const { batch, intervalMs } = stepPlan(speedRef.current);
      try {
        const s = await invoke<Snapshot>("step_sandbox", { batch });
        if (!on) return;
        setSnap(s);
        if (s.finished) {
          setRunning(false);
          setNotice("Reached the end of the real data. Build again, or pick another replay mode under Advanced.");
          return;
        }
      } catch (e) {
        console.error(e);
        setError(String(e));
        setRunning(false);
        return;
      }
      timer = window.setTimeout(tick, intervalMs);
    };
    tick();
    return () => {
      on = false;
      window.clearTimeout(timer);
    };
  }, [running]);

  // accumulate per-guild wealth and market history on each new day
  useEffect(() => {
    if (!snap || snap.day === lastHistDay.current) return;
    lastHistDay.current = snap.day;
    setHistory((h) => snap.cohort_wealth.map((w, i) => pushCapped(h[i] ?? [], w, HIST_CAP)));
    setMarket((m) => ({
      price: pushCapped(m.price, snap.price, HIST_CAP),
      net: pushCapped(m.net, snap.net_position, HIST_CAP),
      synthetic: m.synthetic.concat(snap.synthetic).slice(-HIST_CAP),
    }));
  }, [snap]);

  const selTrader = selected != null && snap && selected < snap.traders.length ? snap.traders[selected] : null;
  const selGuild = selTrader ? guilds[selTrader.cohort] : null;

  return (
    <div className="flex h-screen flex-col">
      <TitleBar
        name={init?.name}
        symbol={init?.symbol}
        day={snap ? snap.idx + 1 : 0}
        days={init?.days ?? 0}
        price={snap?.price ?? 0}
        change={change}
        fallbackFor={init?.fallback ? init.requested : undefined}
        cached={init?.source === "cache"}
        pass={snap?.pass ?? 0}
        synthetic={snap?.synthetic ?? false}
      />

      <div className="flex min-h-0 flex-1">
        {/* Town */}
        <div className="relative min-w-0 flex-1">
          <SunnysideWorld
            traders={snap?.traders ?? []}
            cohorts={cohortViews}
            price={snap?.price ?? 0}
            day={snap?.day ?? 0}
            change={change}
            selected={selected}
            onSelect={setSelected}
          />

          {/* Floating: Controls */}
          <FloatingPanel title="Controls" x={18} y={48} width={300}>
            <div className="space-y-3">
              <div className="space-y-1">
                <span className="text-[11px] text-muted-foreground">Company</span>
                <Select value={symbol} onChange={(e) => setSymbol(e.target.value)}>
                  {COMPANIES.map((c) => (
                    <option key={c.symbol} value={c.symbol}>
                      {c.name} ({c.symbol})
                    </option>
                  ))}
                </Select>
              </div>

              <div className="flex gap-2">
                <Button variant="secondary" className="flex-1" onClick={build} disabled={busy || total < 1}>
                  <RotateCcw className="h-4 w-4" /> Build
                </Button>
                <Button className="flex-1" onClick={() => setRunning((r) => !r)} disabled={!snap || snap.finished}>
                  {running ? <Pause className="h-4 w-4" /> : <Play className="h-4 w-4" />}
                  {running ? "Pause" : "Run"}
                </Button>
              </div>

              {error && <p className="rounded-md bg-red-50 px-2 py-1 text-[11px] text-red-700">{error}</p>}
              {notice && <p className="break-all rounded-md bg-sky-50 px-2 py-1 text-[11px] text-sky-800">{notice}</p>}

              <LabeledSlider
                label={speed > 6 ? "Speed (days / sec) · fast-forward" : "Speed (days / sec)"}
                value={speed}
                min={0.3}
                max={60}
                step={0.1}
                onChange={setSpeed}
              />

              <div className="flex gap-2">
                <Button variant="secondary" className="flex-1" onClick={runSweep} disabled={!snap || sweeping || running}>
                  <FlaskConical className="h-4 w-4" /> {sweeping ? "Testing…" : `Test ${SWEEP_SEEDS} seeds`}
                </Button>
                <Button variant="secondary" className="flex-1" onClick={exportRun} disabled={!snap || snap.trading_days === 0}>
                  <Download className="h-4 w-4" /> Export CSV
                </Button>
              </div>

              <div className="border-t pt-2">
                <div className="mb-1 text-[11px] text-muted-foreground">Compose the town · {total} traders</div>
                <div className="space-y-3">
                  {COHORT_PRESETS.map((p) => (
                    <CohortRow key={p.key} p={p} value={counts[p.key] ?? 0} onChange={(v) => setCounts((c) => ({ ...c, [p.key]: v }))} />
                  ))}
                </div>
              </div>

              <button className="flex items-center gap-1 text-xs text-muted-foreground" onClick={() => setAdvanced((a) => !a)}>
                {advanced ? <ChevronDown className="h-3 w-3" /> : <ChevronRight className="h-3 w-3" />} Advanced
              </button>
              {advanced && (
                <div className="space-y-3 border-t pt-2">
                  <LabeledSelect label="When the real data runs out" value={replay} onChange={setReplay} options={REPLAY_MODES} />
                  <LabeledSelect label="Bet sizing" value={sizing} onChange={setSizing} options={SIZING_MODES} />
                  <LabeledSlider label="Trading cost (bps of turnover)" value={costBps} min={0} max={50} step={1} onChange={setCostBps} />
                  <LabeledSelect label="Social structure" value={topology} onChange={setTopology} options={TOPOLOGIES} />
                  <LabeledSlider label="Random seed" value={seed} min={1} max={9999} step={1} onChange={setSeed} />
                  <p className="text-[10px] leading-snug text-muted-foreground">Press Build after changing guilds, company, or any setting here.</p>
                </div>
              )}
            </div>
          </FloatingPanel>

          {/* Floating: Trader inspector */}
          {selTrader && selGuild && (
            <FloatingPanel title="Trader" x={18} y={430} width={260} onClose={() => setSelected(null)}>
              <TraderInspector trader={selTrader} cohortName={selGuild.name} color={selGuild.color} />
            </FloatingPanel>
          )}
        </div>

        {/* Insights */}
        <div className="w-[344px] shrink-0 overflow-hidden border-l bg-background p-2">
          <Insights snap={snap} guilds={guilds} history={history} market={market} sweep={sweep} />
        </div>
      </div>
    </div>
  );
}

function CohortRow(props: { p: CohortPreset; value: number; onChange: (v: number) => void }) {
  const { p, value, onChange } = props;
  return (
    <div className="space-y-1">
      <div className="flex items-center justify-between">
        <span className="flex items-center gap-1.5 text-[13px]" title={p.blurb}>
          <span className="inline-block h-2.5 w-2.5 rounded-full" style={{ background: p.color }} />
          {p.name}
        </span>
        <Badge variant="secondary">{value}</Badge>
      </div>
      <Slider value={[value]} min={0} max={40} step={1} onValueChange={(v) => onChange(v[0])} />
    </div>
  );
}

function LabeledSlider(props: { label: string; value: number; min: number; max: number; step: number; onChange: (v: number) => void }) {
  return (
    <div className="space-y-1">
      <div className="flex justify-between text-[11px]">
        <span className="text-muted-foreground">{props.label}</span>
        <span className="tabular-nums">{props.value}</span>
      </div>
      <Slider value={[props.value]} min={props.min} max={props.max} step={props.step} onValueChange={(v) => props.onChange(v[0])} />
    </div>
  );
}

function LabeledSelect(props: { label: string; value: string; onChange: (v: string) => void; options: { key: string; label: string }[] }) {
  return (
    <div className="space-y-1">
      <span className="text-[11px] text-muted-foreground">{props.label}</span>
      <Select value={props.value} onChange={(e) => props.onChange(e.target.value)}>
        {props.options.map((o) => (
          <option key={o.key} value={o.key}>
            {o.label}
          </option>
        ))}
      </Select>
    </div>
  );
}

export default App;
