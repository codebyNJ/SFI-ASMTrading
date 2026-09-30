import { type Trader, SIGNAL_LABELS, money } from "@/types";

const KIND_LABEL: Record<string, string> = { trend: "Trend-follower", contrarian: "Contrarian", drifter: "Drifter" };

export function TraderInspector(props: { trader: Trader; cohortName: string; color: string }) {
  const t = props.trader;
  const long = t.position >= 0;
  const conviction = Math.max(0.04, Math.min(0.99, 1 - t.accuracy / 0.0009));
  return (
    <div className="space-y-3 text-[13px]">
      <div className="flex items-center gap-2">
        <span className="inline-block h-3 w-3 rounded-full" style={{ background: props.color }} />
        <span className="font-semibold">{props.cohortName} trader</span>
        <span className="ml-auto rounded-full bg-secondary px-2 py-0.5 text-[11px] font-medium">{KIND_LABEL[t.kind] ?? t.kind}</span>
      </div>

      {t.bust && (
        <div className="rounded-md bg-red-50 px-2 py-1 text-[11px] text-red-700">Bankrupt — no longer trades, and neighbours no longer copy its rules.</div>
      )}

      <div className="grid grid-cols-3 gap-2">
        <Field label="Wealth" value={money(t.wealth)} tone={t.wealth >= 1000 ? "up" : "down"} />
        <Field label="Today" value={`${(t.pnl * 100).toFixed(2)}%`} tone={t.pnl >= 0 ? "up" : "down"} />
        <Field label="Max drawdown" value={`${Math.round(t.max_drawdown * 100)}%`} tone={t.max_drawdown > 0.3 ? "down" : undefined} />
      </div>

      <div>
        <div className="mb-1 flex justify-between text-[11px] text-muted-foreground">
          <span>{long ? "Buying" : "Selling"}</span>
          <span className="tabular-nums">{Math.round(Math.abs(t.position) * 100)}%</span>
        </div>
        <div className="h-2 w-full overflow-hidden rounded-full bg-secondary">
          <div className="h-full rounded-full" style={{ width: `${Math.min(100, Math.abs(t.position) * 100)}%`, background: long ? "#22c55e" : "#ef4444" }} />
        </div>
      </div>

      <div className="grid grid-cols-2 gap-2">
        <Field label="Forecast (next day)" value={`${(t.forecast * 100).toFixed(2)}%`} tone={t.forecast >= 0 ? "up" : "down"} />
        <div>
          <div className="text-[11px] text-muted-foreground">Conviction</div>
          <div className="mt-1 h-2 w-full overflow-hidden rounded-full bg-secondary">
            <div className="h-full rounded-full bg-sky-500" style={{ width: `${conviction * 100}%` }} />
          </div>
        </div>
      </div>

      <div>
        <div className="mb-1 text-[11px] text-muted-foreground">Signals this rule watches</div>
        <div className="flex flex-wrap gap-1">
          {SIGNAL_LABELS.map((s, i) => {
            const on = (t.signals & (1 << i)) !== 0;
            return (
              <span key={i} className={`rounded-md px-1.5 py-0.5 text-[10px] ${on ? "bg-sky-100 text-sky-700" : "bg-secondary text-muted-foreground/60"}`}>
                {s}
              </span>
            );
          })}
        </div>
        {t.signals === 0 && <div className="mt-1 text-[10px] text-muted-foreground">No specific rule matched — using a blended view.</div>}
      </div>
    </div>
  );
}

function Field(props: { label: string; value: string; tone?: "up" | "down" }) {
  return (
    <div>
      <div className="text-[11px] text-muted-foreground">{props.label}</div>
      <div className={`font-semibold tabular-nums ${props.tone === "up" ? "text-emerald-600" : props.tone === "down" ? "text-red-600" : ""}`}>{props.value}</div>
    </div>
  );
}
