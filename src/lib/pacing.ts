/** Above this many days per second the UI stops rendering every day and steps in batches. */
export const MAX_FRAMES_PER_SEC = 6;

/**
 * How to reach `daysPerSec`: step `batch` days per engine call, one call every
 * `intervalMs`. Up to 6 days/s every day is shown; beyond that ("fast-forward")
 * several days are simulated per frame so the town doesn't redraw too often.
 */
export function stepPlan(daysPerSec: number): { batch: number; intervalMs: number } {
  const speed = Math.max(0.2, daysPerSec);
  if (speed <= MAX_FRAMES_PER_SEC) return { batch: 1, intervalMs: 1000 / speed };
  const batch = Math.ceil(speed / MAX_FRAMES_PER_SEC);
  return { batch, intervalMs: (1000 * batch) / speed };
}

/** Append to a rolling series, keeping at most `cap` points. */
export function pushCapped(series: number[], value: number, cap: number): number[] {
  const next = series.concat(value);
  return next.length > cap ? next.slice(next.length - cap) : next;
}
