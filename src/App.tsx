import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Play, Pause, RotateCcw, ChevronDown, ChevronRight } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Slider } from "@/components/ui/slider";
import { Select } from "@/components/ui/select";
import { Badge } from "@/components/ui/badge";
import { COHORT_PRESETS, COMPANIES, TOPOLOGIES, type CohortPreset } from "@/data/presets";
import SunnysideWorld, { type CohortView } from "@/SunnysideWorld";
import { TitleBar } from "@/components/TitleBar";
import { FloatingPanel } from "@/components/FloatingPanel";
import { Insights } from "@/components/Insights";
import { TraderInspector } from "@/components/TraderInspector";
import type { Snapshot, Init } from "@/types";

const DEFAULT_COUNTS: Record<string, number> = { value: 8, calm: 10, bubbly: 10, herder: 8, reckless: 4 };
const HIST_CAP = 400;

function App() {
  const [symbol, setSymbol] = useState("AAPL");
  const [counts, setCounts] = useState<Record<string, number>>(DEFAULT_COUNTS);
  const [topology, setTopology] = useState("scalefree");
  const [speed, setSpeed] = useState(1.5);
  const [seed, setSeed] = useState(1000);
  const [advanced, setAdvanced] = useState(false);

  const [init, setInit] = useState<Init | null>(null);
  const [snap, setSnap] = useState<Snapshot | null>(null);
  const [running, setRunning] = useState(false);
  const [busy, setBusy] = useState(false);
  const [selected, setSelected] = useState<number | null>(null);
  const [history, setHistory] = useState<number[][]>([]);

  const speedRef = useRef(speed);
  speedRef.current = speed;
  const lastHistDay = useRef(-1);

  const active = COHORT_PRESETS.filter((p) => (counts[p.key] ?? 0) > 0);
  const total = active.reduce((s, p) => s + counts[p.key], 0);
  const cohortViews: CohortView[] = active.map((p) => ({ name: p.name, color: p.color, herding: p.herding }));
  const guilds = active.map((p) => ({ name: p.name, color: p.color }));

  const idx = snap?.idx ?? 0;
  const change = init && idx > 0 && idx < init.prices.length ? (init.prices[idx] - init.prices[idx - 1]) / init.prices[idx - 1] : 0;

  async function build() {
    setRunning(false);
    setBusy(true);
    setSelected(null);
    setHistory(active.map(() => []));
    lastHistDay.current = -1;
    try {
      const cohorts = active.map((p) => ({ name: p.name, count: counts[p.key], memory: p.memory, herding: p.herding, risk: p.risk }));
      const res = await invoke<Init>("init_sandbox", { config: { symbol, cohorts, topology, degree: 6, seed } });
      setInit(res);
      setSnap(res.snapshot);
    } catch (e) {
      console.error("init_sandbox:", e);
    } finally {
      setBusy(false);
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
      try {
        const s = await invoke<Snapshot>("step_sandbox", { batch: 1 });
        if (!on) return;
        setSnap(s);
      } catch (e) {
        console.error(e);
        return;
      }
      timer = window.setTimeout(tick, 1000 / Math.max(0.2, speedRef.current));
    };
    tick();
    return () => {
      on = false;
      window.clearTimeout(timer);
    };
  }, [running]);

  // accumulate per-guild wealth history on each new day
  useEffect(() => {
    if (!snap || snap.day === lastHistDay.current) return;
    lastHistDay.current = snap.day;
    setHistory((h) => {
      const next = snap.cohort_wealth.map((w, i) => {
        const s = (h[i] ?? []).concat(w);
        return s.length > HIST_CAP ? s.slice(s.length - HIST_CAP) : s;
      });
      return next;
    });
  }, [snap]);

  const selTrader = selected != null && snap && selected < snap.traders.length ? snap.traders[selected] : null;
  const selGuild = selTrader ? guilds[selTrader.cohort] : null;

  return (
    <div className="flex h-screen flex-col">
      <TitleBar name={init?.name} symbol={init?.symbol} day={snap ? snap.idx + 1 : 0} days={init?.days ?? 0} price={snap?.price ?? 0} change={change} />

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
                <Button className="flex-1" onClick={() => setRunning((r) => !r)} disabled={!snap}>
                  {running ? <Pause className="h-4 w-4" /> : <Play className="h-4 w-4" />}
                  {running ? "Pause" : "Run"}
                </Button>
              </div>

              <LabeledSlider label="Speed (days / sec)" value={speed} min={0.3} max={6} step={0.1} onChange={setSpeed} />

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
                  <div className="space-y-1">
                    <span className="text-[11px] text-muted-foreground">Social structure</span>
                    <Select value={topology} onChange={(e) => setTopology(e.target.value)}>
                      {TOPOLOGIES.map((t) => (
                        <option key={t.key} value={t.key}>
                          {t.label}
                        </option>
                      ))}
                    </Select>
                  </div>
                  <LabeledSlider label="Random seed" value={seed} min={1} max={9999} step={1} onChange={setSeed} />
                  <p className="text-[10px] leading-snug text-muted-foreground">Press Build after changing guilds, company, or structure.</p>
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
          <Insights snap={snap} guilds={guilds} history={history} />
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
        <span className="flex items-center gap-1.5 text-[13px]">
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

export default App;
