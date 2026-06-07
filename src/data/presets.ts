// Plain-language trader guilds and the companies you can load.

export type HouseColor = "blue" | "green" | "orange" | "purple" | "red";

export type CohortPreset = {
  key: string;
  name: string;
  blurb: string;
  memory: number; // learning reactivity (theta)
  herding: number; // 0..1 copy neighbours
  risk: number; // position aggressiveness
  color: string; // accent color
  house: HouseColor; // which house sprite this guild lives in
};

export const COHORT_PRESETS: CohortPreset[] = [
  { key: "value", name: "Value", blurb: "patient, ignores the crowd", memory: 0.015, herding: 0.05, risk: 0.6, color: "#3b82f6", house: "blue" },
  { key: "calm", name: "Calm", blurb: "steady, mild follower", memory: 0.03, herding: 0.15, risk: 0.9, color: "#22c55e", house: "green" },
  { key: "bubbly", name: "Bubbly", blurb: "excitable, chases trends", memory: 0.08, herding: 0.55, risk: 1.5, color: "#f59e0b", house: "orange" },
  { key: "herder", name: "Herders", blurb: "copies neighbours hard", memory: 0.06, herding: 0.9, risk: 1.2, color: "#a855f7", house: "purple" },
  { key: "reckless", name: "Reckless", blurb: "jumpy, oversized bets", memory: 0.12, herding: 0.5, risk: 2.2, color: "#ef4444", house: "red" },
];

export type Company = { symbol: string; name: string };

export const COMPANIES: Company[] = [
  { symbol: "AAPL", name: "Apple" },
  { symbol: "MSFT", name: "Microsoft" },
  { symbol: "NVDA", name: "Nvidia" },
  { symbol: "TSLA", name: "Tesla" },
  { symbol: "AMZN", name: "Amazon" },
  { symbol: "GOOGL", name: "Alphabet" },
  { symbol: "META", name: "Meta" },
  { symbol: "SPY", name: "S&P 500 ETF" },
  { symbol: "KO", name: "Coca-Cola" },
  { symbol: "JPM", name: "JPMorgan" },
];

export const TOPOLOGIES = [
  { key: "scalefree", label: "Influencers (hubs)" },
  { key: "random", label: "Random friendships" },
  { key: "lattice", label: "Neighbours (local)" },
  { key: "none", label: "Isolated (no copying)" },
];
