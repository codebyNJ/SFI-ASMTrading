import { getCurrentWindow } from "@tauri-apps/api/window";
import { Minus, Square, X, ArrowUp, ArrowDown } from "lucide-react";

export function TitleBar(props: {
  name?: string;
  symbol?: string;
  day: number;
  days: number;
  price: number;
  change: number;
  /** Set when live data failed and a bundled series was substituted. */
  fallbackFor?: string;
  /** Live data failed but an earlier download of this ticker was used. */
  cached?: boolean;
  /** 0 = first pass over the real history. */
  pass?: number;
  /** Today's price comes from a bootstrapped path, not the real history. */
  synthetic?: boolean;
}) {
  const { name, symbol, day, days, price, change, fallbackFor, cached, pass = 0, synthetic } = props;
  const up = change >= 0;
  return (
    <div data-tauri-drag-region className="titlebar flex h-9 shrink-0 items-center justify-between px-3 select-none">
      <div data-tauri-drag-region className="flex items-center gap-2 text-[13px] font-semibold tracking-tight">
        <span>🏘️ Trading Town</span>
      </div>
      <div data-tauri-drag-region className="flex items-center gap-2 text-[12px] text-muted-foreground">
        {name && (
          <span className="text-foreground/80">
            {name} ({symbol})
          </span>
        )}
        {fallbackFor && (
          <span
            className="rounded-md bg-amber-100 px-1.5 py-0.5 text-[11px] font-medium text-amber-800"
            title={`Couldn't load ${fallbackFor} (offline or unknown ticker) — showing the bundled ${symbol} sample instead.`}
          >
            ⚠ {fallbackFor} unavailable · offline sample
          </span>
        )}
        {cached && !fallbackFor && (
          <span className="rounded-md bg-secondary px-1.5 py-0.5 text-[11px]" title="Offline — using the last downloaded copy of this ticker.">
            cached data
          </span>
        )}
        {pass > 0 && (
          <span
            className={`rounded-md px-1.5 py-0.5 text-[11px] font-medium ${synthetic ? "bg-violet-100 text-violet-800" : "bg-amber-100 text-amber-800"}`}
            title={
              synthetic
                ? "The real history has ended. Prices now come from a resampled (block-bootstrapped) path with the same volatility, not real data."
                : "Replaying the same history: traders have already learned on it, so results from here are in-sample."
            }
          >
            {synthetic ? `synthetic pass ${pass}` : `replay ${pass} · in-sample`}
          </span>
        )}
        <span>· Day {day} / {days} ·</span>
        <span className="font-medium tabular-nums text-foreground">${price > 0 ? price.toFixed(2) : "—"}</span>
        <span className={`flex items-center gap-0.5 tabular-nums ${up ? "text-emerald-600" : "text-red-600"}`}>
          {up ? <ArrowUp className="h-3 w-3" /> : <ArrowDown className="h-3 w-3" />}
          {Math.abs(change * 100).toFixed(2)}%
        </span>
      </div>
      <div className="flex items-center gap-1">
        <button className="grid h-6 w-6 place-items-center rounded-md text-muted-foreground hover:bg-black/5" onClick={() => getCurrentWindow().minimize()}>
          <Minus className="h-3.5 w-3.5" />
        </button>
        <button className="grid h-6 w-6 place-items-center rounded-md text-muted-foreground hover:bg-black/5" onClick={() => getCurrentWindow().toggleMaximize()}>
          <Square className="h-3 w-3" />
        </button>
        <button className="grid h-6 w-6 place-items-center rounded-md text-muted-foreground hover:bg-red-500 hover:text-white" onClick={() => getCurrentWindow().close()}>
          <X className="h-3.5 w-3.5" />
        </button>
      </div>
    </div>
  );
}
