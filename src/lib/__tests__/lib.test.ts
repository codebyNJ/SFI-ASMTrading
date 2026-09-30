import { describe, expect, it } from "vitest";
import { MAX_FRAMES_PER_SEC, pushCapped, stepPlan } from "../pacing";
import { ciVerdict, excessPct, pct } from "../format";

describe("stepPlan", () => {
  it("shows every day at normal speeds", () => {
    expect(stepPlan(2)).toEqual({ batch: 1, intervalMs: 500 });
    expect(stepPlan(MAX_FRAMES_PER_SEC).batch).toBe(1);
  });

  it("batches days when fast-forwarding but keeps the requested rate", () => {
    for (const speed of [7, 12, 30, 60]) {
      const { batch, intervalMs } = stepPlan(speed);
      expect(batch).toBeGreaterThan(1);
      expect((batch * 1000) / intervalMs).toBeCloseTo(speed, 6);
      expect(1000 / intervalMs).toBeLessThanOrEqual(MAX_FRAMES_PER_SEC + 1e-9);
    }
  });

  it("guards against zero or negative speeds", () => {
    expect(stepPlan(0).intervalMs).toBe(5000);
    expect(stepPlan(-3).batch).toBe(1);
  });
});

describe("pushCapped", () => {
  it("keeps only the newest points", () => {
    expect(pushCapped([1, 2, 3], 4, 3)).toEqual([2, 3, 4]);
    expect(pushCapped([], 1, 3)).toEqual([1]);
  });
});

describe("format", () => {
  it("formats signed percentages", () => {
    expect(pct(0.1234)).toBe("+12.3%");
    expect(pct(-0.05, 0)).toBe("-5%");
  });

  it("turns log excess into a wealth difference", () => {
    expect(excessPct(Math.log(0.5))).toBe("-50%");
    expect(excessPct(0)).toBe("+0%");
  });

  it("only calls a result when the CI clears zero", () => {
    expect(ciVerdict(0.2, 0.1)).toBe("beats");
    expect(ciVerdict(-0.2, 0.1)).toBe("lags");
    expect(ciVerdict(0.05, 0.1)).toBe("unclear");
  });
});
