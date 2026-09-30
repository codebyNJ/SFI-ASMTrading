/** "+12.3%" style percentage from a fraction. */
export const pct = (x: number, digits = 1) => `${x >= 0 ? "+" : ""}${(x * 100).toFixed(digits)}%`;

/**
 * Excess log return shown as the equivalent % difference in final wealth vs
 * buy & hold: exp(x) − 1. A log excess of −0.69 reads as "−50%".
 */
export const excessPct = (logExcess: number, digits = 0) => pct(Math.expm1(logExcess), digits);

/**
 * A mean ± 95% CI verdict: whether the interval clears zero. Keeps the UI from
 * presenting noise as a finding.
 */
export function ciVerdict(mean: number, halfWidth: number): "beats" | "lags" | "unclear" {
  if (mean - halfWidth > 0) return "beats";
  if (mean + halfWidth < 0) return "lags";
  return "unclear";
}
