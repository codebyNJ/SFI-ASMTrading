import { useEffect, useRef } from "react";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { type Snapshot, SIGNAL_LABELS, money } from "@/types";

type Guild = { name: string; color: string };

export function Insights(props: { snap: Snapshot | null; guilds: Guild[]; history: number[][] }) {
  const { snap, guilds, history } = props;
  return (
    <div className="flex h-full flex-col gap-2 overflow-y-auto pr-0.5">
      <TownMood snap={snap} />
      <StrategyMix snap={snap} />
      <BeliefSpread snap={snap} />
      <SignalUsage snap={snap} />
      <Leaderboard snap={snap} guilds={guilds} history={history} />
    </div>
  );
}

function TownMood(props: { snap: Snapshot | null }) {
  const s = props.snap;
  return (
    <div className="grid grid-cols-3 gap-2">
      <MiniStat label="Bullish" value={s ? `${Math.round(s.sentiment * 100)}%` : "—"} />
      <MiniStat label="Herding" value={s ? s.herding.toFixed(2) : "—"} />
      <MiniStat label="Avg wealth" value={s ? money(s.avg_wealth) : "—"} />
    </div>
  );
}

function StrategyMix(props: { snap: Snapshot | null }) {
  const s = props.snap;
  const total = s ? Math.max(1, s.trend_count + s.contrarian_count + s.drifter_count) : 1;
  const seg = [
    { label: "Trend", n: s?.trend_count ?? 0, c: "#2563eb" },
    { label: "Contrarian", n: s?.contrarian_count ?? 0, c: "#f59e0b" },
    { label: "Drifter", n: s?.drifter_count ?? 0, c: "#94a3b8" },
  ];
  return (
    <Card>
      <CardHeader className="pb-1">
        <CardTitle>Strategy mix</CardTitle>
        <span className="text-[11px] text-muted-foreground">trend‑followers vs contrarians right now</span>
      </CardHeader>
      <CardContent className="space-y-2">
        <div className="flex h-3 w-full overflow-hidden rounded-full">
          {seg.map((x) => (
            <div key={x.label} style={{ width: `${(x.n / total) * 100}%`, background: x.c }} />
          ))}
        </div>
        <div className="flex justify-between text-[11px]">
          {seg.map((x) => (
            <span key={x.label} className="flex items-center gap-1">
              <span className="inline-block h-2 w-2 rounded-full" style={{ background: x.c }} />
              {x.label} {Math.round((x.n / total) * 100)}%
            </span>
          ))}
        </div>
      </CardContent>
    </Card>
  );
}

function BeliefSpread(props: { snap: Snapshot | null }) {
  const bins = props.snap?.belief_hist ?? [];
  const max = Math.max(1, ...bins);
  const mid = bins.length / 2;
  return (
    <Card>
      <CardHeader className="pb-1">
        <CardTitle>Belief spread</CardTitle>
        <span className="text-[11px] text-muted-foreground">bearish ◂ momentum view ▸ bullish</span>
      </CardHeader>
      <CardContent>
        <div className="flex h-16 items-end gap-[2px]">
          {bins.map((v, i) => (
            <div
              key={i}
              className="flex-1 rounded-sm"
              style={{ height: `${(v / max) * 100}%`, minHeight: 2, background: i < mid ? "#f59e0b" : "#2563eb", opacity: 0.85 }}
            />
          ))}
        </div>
      </CardContent>
    </Card>
  );
}

function SignalUsage(props: { snap: Snapshot | null }) {
  const usage = props.snap?.signal_usage ?? [];
  const total = props.snap ? Math.max(1, props.snap.traders.length) : 1;
  return (
    <Card>
      <CardHeader className="pb-1">
        <CardTitle>Signals the town watches</CardTitle>
      </CardHeader>
      <CardContent className="space-y-1">
        {SIGNAL_LABELS.map((label, i) => {
          const v = usage[i] ?? 0;
          return (
            <div key={i} className="flex items-center gap-2 text-[11px]">
              <span className="w-24 shrink-0 text-muted-foreground">{label}</span>
              <div className="h-1.5 flex-1 overflow-hidden rounded-full bg-secondary">
                <div className="h-full rounded-full bg-slate-500" style={{ width: `${(v / total) * 100}%` }} />
              </div>
            </div>
          );
        })}
      </CardContent>
    </Card>
  );
}

function Leaderboard(props: { snap: Snapshot | null; guilds: Guild[]; history: number[][] }) {
  const { snap, guilds, history } = props;
  const ranked = guilds
    .map((g, i) => ({ ...g, wealth: snap?.cohort_wealth[i] ?? 1000, i }))
    .sort((a, b) => b.wealth - a.wealth);
  return (
    <Card className="flex-1">
      <CardHeader className="pb-1">
        <CardTitle>Who's winning</CardTitle>
        <span className="text-[11px] text-muted-foreground">guild wealth over the run</span>
      </CardHeader>
      <CardContent className="space-y-2">
        <Sparklines history={history} colors={guilds.map((g) => g.color)} />
        {snap && (
          <div className="flex items-center justify-between border-b border-dashed pb-1.5 text-[12px] text-muted-foreground">
            <span title="What you'd have by simply holding the stock over the same days">Buy &amp; hold (baseline)</span>
            <span className="tabular-nums">{money(snap.benchmark_wealth)}</span>
          </div>
        )}
        {ranked.map((g) => (
          <div key={g.i} className="flex items-center justify-between text-[12px]">
            <span className="flex items-center gap-1.5">
              <span className="inline-block h-2.5 w-2.5 rounded-full" style={{ background: g.color }} />
              {g.name}
            </span>
            <span className={`tabular-nums ${g.wealth >= 1000 ? "text-emerald-600" : "text-red-600"}`}>{money(g.wealth)}</span>
          </div>
        ))}
      </CardContent>
    </Card>
  );
}

function MiniStat(props: { label: string; value: string }) {
  return (
    <div className="rounded-lg border bg-card px-2.5 py-2">
      <div className="text-[15px] font-semibold tabular-nums">{props.value}</div>
      <div className="text-[10px] text-muted-foreground">{props.label}</div>
    </div>
  );
}

function Sparklines(props: { history: number[][]; colors: string[] }) {
  const ref = useRef<HTMLCanvasElement>(null);
  useEffect(() => {
    const c = ref.current;
    if (!c) return;
    const w = c.clientWidth, h = c.clientHeight;
    if (w === 0 || h === 0) return;
    const dpr = window.devicePixelRatio || 1;
    c.width = w * dpr; c.height = h * dpr;
    const ctx = c.getContext("2d")!;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.clearRect(0, 0, w, h);
    const series = props.history.filter((s) => s.length > 1);
    if (series.length === 0) return;
    let lo = Infinity, hi = -Infinity, len = 0;
    for (const s of series) { len = Math.max(len, s.length); for (const v of s) { lo = Math.min(lo, v); hi = Math.max(hi, v); } }
    const pad = (hi - lo) * 0.1 || 1; lo -= pad; hi += pad;
    props.history.forEach((s, gi) => {
      if (s.length < 2) return;
      ctx.strokeStyle = props.colors[gi] ?? "#888";
      ctx.lineWidth = 1.5;
      ctx.beginPath();
      s.forEach((v, i) => {
        const x = (i / (len - 1)) * w;
        const y = h - ((v - lo) / (hi - lo)) * h;
        i ? ctx.lineTo(x, y) : ctx.moveTo(x, y);
      });
      ctx.stroke();
    });
  }, [props.history, props.colors]);
  return <canvas ref={ref} className="h-20 w-full" />;
}
