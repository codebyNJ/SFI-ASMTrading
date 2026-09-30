import { useEffect, useRef } from "react";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { type MarketHistory, type Snapshot, type Sweep, SIGNAL_LABELS, money } from "@/types";
import { ciVerdict, excessPct } from "@/lib/format";

type Guild = { name: string; color: string };

export function Insights(props: { snap: Snapshot | null; guilds: Guild[]; history: number[][]; market: MarketHistory; sweep: Sweep | null }) {
  const { snap, guilds, history, market, sweep } = props;
  return (
    <div className="flex h-full flex-col gap-2 overflow-y-auto pr-0.5">
      <TownMood snap={snap} />
      <MarketChart market={market} />
      <Leaderboard snap={snap} guilds={guilds} history={history} />
      {sweep && <Robustness sweep={sweep} guilds={guilds} />}
      <StrategyMix snap={snap} />
      <BeliefSpread snap={snap} />
      <SignalUsage snap={snap} />
    </div>
  );
}

function MarketChart(props: { market: MarketHistory }) {
  const ref = useRef<HTMLCanvasElement>(null);
  const { price, net, synthetic } = props.market;
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
    if (price.length < 2) return;
    const n = price.length;
    const x = (i: number) => (i / (n - 1)) * w;
    // Town positioning as bars around the midline: green = net long, red = net short.
    const mid = h / 2;
    const bw = Math.max(1, w / n);
    net.forEach((v, i) => {
      ctx.fillStyle = v >= 0 ? "rgba(34,197,94,0.28)" : "rgba(239,68,68,0.28)";
      const bh = Math.min(1, Math.abs(v)) * (h / 2);
      ctx.fillRect(x(i) - bw / 2, v >= 0 ? mid - bh : mid, bw, bh);
    });
    let lo = Infinity, hi = -Infinity;
    for (const p of price) { lo = Math.min(lo, p); hi = Math.max(hi, p); }
    const pad = (hi - lo) * 0.08 || 1; lo -= pad; hi += pad;
    const y = (p: number) => h - ((p - lo) / (hi - lo)) * h;
    ctx.lineWidth = 1.5;
    for (let i = 1; i < n; i++) {
      ctx.strokeStyle = synthetic[i] ? "#7c3aed" : "#0f172a";
      ctx.beginPath();
      ctx.moveTo(x(i - 1), y(price[i - 1]));
      ctx.lineTo(x(i), y(price[i]));
      ctx.stroke();
    }
  }, [price, net, synthetic]);
  return (
    <Card>
      <CardHeader className="pb-1">
        <CardTitle>Market &amp; the crowd</CardTitle>
        <span className="text-[11px] text-muted-foreground">price · bars = town net long / short{synthetic.some(Boolean) ? " · purple = synthetic" : ""}</span>
      </CardHeader>
      <CardContent>
        <canvas ref={ref} className="h-24 w-full" />
      </CardContent>
    </Card>
  );
}

function Robustness(props: { sweep: Sweep; guilds: Guild[] }) {
  const { sweep, guilds } = props;
  const label = { beats: "beats B&H", lags: "lags B&H", unclear: "no clear edge" } as const;
  const tone = { beats: "text-emerald-600", lags: "text-red-600", unclear: "text-muted-foreground" } as const;
  return (
    <Card>
      <CardHeader className="pb-1">
        <CardTitle>Is it luck? · {sweep.seeds} seeds</CardTitle>
        <span className="text-[11px] text-muted-foreground">
          one pass over the real {sweep.days} days · wealth vs buy &amp; hold, mean ± 95% CI
        </span>
      </CardHeader>
      <CardContent className="space-y-1.5">
        {sweep.cohorts.map((c, i) => {
          const v = ciVerdict(c.excess_mean, c.excess_ci95);
          return (
            <div key={c.name} className="flex items-center justify-between gap-2 text-[12px]">
              <span className="flex items-center gap-1.5">
                <span className="inline-block h-2.5 w-2.5 rounded-full" style={{ background: guilds[i]?.color ?? "#888" }} />
                {c.name}
              </span>
              <span className="tabular-nums text-muted-foreground" title="Final wealth relative to buy & hold (95% CI across seeds)">
                {excessPct(c.excess_mean)} <span className="text-[10px]">[{excessPct(c.excess_mean - c.excess_ci95)}, {excessPct(c.excess_mean + c.excess_ci95)}]</span>
              </span>
              <span className={`w-20 text-right text-[11px] ${tone[v]}`} title={`${Math.round(c.beat_fraction * 100)}% of seeds ahead of buy & hold`}>
                {label[v]}
              </span>
            </div>
          );
        })}
        <p className="pt-1 text-[10px] leading-snug text-muted-foreground">
          "No clear edge" means the interval includes zero: the difference could be down to the random seed.
        </p>
      </CardContent>
    </Card>
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
  const bench = snap?.benchmark_wealth ?? 1000;
  const ranked = guilds
    .map((g, i) => ({ ...g, wealth: snap?.cohort_wealth[i] ?? 1000, stats: snap?.cohort_stats[i], i }))
    .sort((a, b) => b.wealth - a.wealth);
  return (
    <Card className="flex-1">
      <CardHeader className="pb-1">
        <CardTitle>Who's winning</CardTitle>
        <span className="text-[11px] text-muted-foreground">guild wealth over the run, vs simply holding the stock</span>
      </CardHeader>
      <CardContent className="space-y-2">
        <Sparklines history={history} colors={guilds.map((g) => g.color)} />
        <div className="grid grid-cols-[1fr_auto_auto_auto_auto] items-center gap-x-2 gap-y-1 text-[12px]">
          <span />
          <span className="text-right text-[10px] text-muted-foreground" title="Mean wealth (median in brackets)">wealth</span>
          <span className="text-right text-[10px] text-muted-foreground" title="Mean wealth relative to buy & hold">vs B&amp;H</span>
          <span className="text-right text-[10px] text-muted-foreground" title="Annualised Sharpe ratio of the guild's pooled daily returns">Sharpe</span>
          <span className="text-right text-[10px] text-muted-foreground" title="Worst peak-to-trough fall of the guild's pooled wealth">max DD</span>
          {ranked.map((g) => (
            <GuildRow key={g.i} g={g} bench={bench} />
          ))}
          {snap && (
            <>
              <span className="text-muted-foreground" title="What you'd have by simply holding the stock over the same days">Buy &amp; hold</span>
              <span className="text-right tabular-nums text-muted-foreground">{money(snap.benchmark_wealth)}</span>
              <span />
              <span className="text-right tabular-nums text-muted-foreground">{snap.benchmark_sharpe.toFixed(2)}</span>
              <span className="text-right tabular-nums text-muted-foreground">{Math.round(snap.benchmark_max_drawdown * 100)}%</span>
            </>
          )}
        </div>
        {snap && snap.busts > 0 && <p className="text-[10px] text-red-600">{snap.busts} trader{snap.busts === 1 ? "" : "s"} bankrupt</p>}
      </CardContent>
    </Card>
  );
}

function GuildRow(props: {
  g: Guild & { wealth: number; stats?: Snapshot["cohort_stats"][number] };
  bench: number;
}) {
  const { g, bench } = props;
  const s = g.stats;
  const ahead = g.wealth >= bench;
  return (
    <>
      <span className="flex items-center gap-1.5 truncate">
        <span className="inline-block h-2.5 w-2.5 shrink-0 rounded-full" style={{ background: g.color }} />
        {g.name}
        {s && s.busts > 0 && <span className="text-[10px] text-red-600">†{s.busts}</span>}
      </span>
      <span className="text-right tabular-nums" title={s ? `median ${money(s.median_wealth)}` : undefined}>{money(g.wealth)}</span>
      <span className={`text-right tabular-nums ${ahead ? "text-emerald-600" : "text-red-600"}`}>{s ? excessPct(s.excess_log_return) : "—"}</span>
      <span className="text-right tabular-nums">{s ? s.sharpe.toFixed(2) : "—"}</span>
      <span className="text-right tabular-nums">{s ? `${Math.round(s.max_drawdown * 100)}%` : "—"}</span>
    </>
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
