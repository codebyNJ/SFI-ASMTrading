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
};

export type Snapshot = {
  day: number;
  idx: number;
  price: number;
  sentiment: number;
  herding: number;
  avg_wealth: number;
  cohort_names: string[];
  cohort_wealth: number[];
  traders: Trader[];
  trend_count: number;
  contrarian_count: number;
  drifter_count: number;
  belief_hist: number[];
  signal_usage: number[];
};

export type Init = { symbol: string; name: string; source: string; days: number; prices: number[]; snapshot: Snapshot };

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
