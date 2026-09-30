export type Trader = {
  cohort: number;
  wealth: number;
  position: number;
  pnl: number;
  a: number;
  b: number;
  forecast: number;
  accuracy: number;
  signals: number;
  kind: string; // "trend" | "contrarian" | "drifter"
  max_drawdown: number; // worst peak-to-trough fall so far (0..1)
  bust: boolean; // bankrupt: no longer trades or is copied
};

export type CohortStats = {
  name: string;
  mean_wealth: number;
  median_wealth: number;
  excess_log_return: number; // ln(mean wealth / buy & hold)
  sharpe: number; // annualised
  max_drawdown: number; // 0..1
  turnover: number; // avg |Δposition| per trader per day
  busts: number;
};

export type Snapshot = {
  day: number;
  idx: number;
  trading_days: number;
  price: number;
  change: number; // simple return vs the previous day on the path being played
  pass: number; // 0 = first pass over the real history
  synthetic: boolean; // price comes from a bootstrapped path
  finished: boolean; // a "stop" run has used up its data
  sentiment: number;
  herding: number;
  net_position: number; // mean position, −1 all short … +1 all long
  avg_wealth: number;
  benchmark_wealth: number; // passive buy & hold over the same days
  benchmark_sharpe: number;
  benchmark_max_drawdown: number;
  busts: number;
  cohort_names: string[];
  cohort_wealth: number[];
  cohort_stats: CohortStats[];
  traders: Trader[];
  trend_count: number;
  contrarian_count: number;
  drifter_count: number;
  belief_hist: number[];
  signal_usage: number[];
};

export type Init = {
  symbol: string;
  name: string;
  source: string; // "yahoo" | "cache" | "bundled"
  requested: string; // ticker the user picked
  fallback: boolean; // true when `symbol` was substituted for `requested`
  days: number;
  prices: number[];
  snapshot: Snapshot;
};

export type CohortSweep = {
  name: string;
  excess_mean: number; // mean ln(guild wealth / buy & hold) across seeds
  excess_ci95: number; // half-width of the ~95% CI
  beat_fraction: number; // share of seeds ahead of buy & hold
  sharpe_mean: number;
  max_drawdown_mean: number;
};

export type Sweep = { seeds: number; days: number; benchmark_log_return: number; cohorts: CohortSweep[] };

/** Rolling series the UI keeps for charts (one point per snapshot). */
export type MarketHistory = { price: number[]; net: number[]; synthetic: boolean[] };

export const SIGNAL_LABELS = [
  "Price > MA5",
  "Price > MA20",
  "Price > MA50",
  "MA5 > MA20",
  "MA20 > MA50",
  "Up yesterday",
  "Big move",
  "5-day uptrend",
];

export const money = (x: number) => "$" + Math.round(x).toLocaleString();
