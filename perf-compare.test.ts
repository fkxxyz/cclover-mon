import { describe, expect, test } from "bun:test";
import { resolve } from "node:path";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";

import {
  comparisonSchedule,
  executableIdentity,
  measureCommand,
  parseComparisonArgs,
  summarizePairs,
  verifyExecutableIdentity,
  workloadCommand,
} from "./perf-compare";

describe("performance comparison", () => {
  test("parses an explicit fixed-work collector comparison", () => {
    expect(
      parseComparisonArgs([
        "--baseline",
        "./old",
        "--candidate",
        "./new",
        "collector",
        "processes",
        "--samples",
        "30",
      ]),
    ).toEqual({
      baseline: resolve("./old"),
      candidate: resolve("./new"),
      workload: { kind: "collector", collector: "processes" },
      count: 30,
      pairs: 6,
    });
  });

  test("parses an explicit fixed-work Web comparison", () => {
    expect(
      parseComparisonArgs([
        "--baseline",
        "./old",
        "--candidate",
        "./new",
        "web",
        "--updates",
        "30",
      ]),
    ).toEqual({
      baseline: resolve("./old"),
      candidate: resolve("./new"),
      workload: { kind: "web" },
      count: 30,
      pairs: 6,
    });
  });

  test("requires fixed sample count rather than duration", () => {
    expect(() =>
      parseComparisonArgs([
        "--baseline",
        "./old",
        "--candidate",
        "./new",
        "headless",
        "--duration",
        "30",
      ]),
    ).toThrow("unexpected argument: --duration");
  });

  test("keeps workload-specific fixed-work units explicit", () => {
    expect(() =>
      parseComparisonArgs([
        "--baseline",
        "./old",
        "--candidate",
        "./new",
        "web",
        "--samples",
        "5",
      ]),
    ).toThrow("--samples is not valid for web workload; use --updates");
    expect(() =>
      parseComparisonArgs([
        "--baseline",
        "./old",
        "--candidate",
        "./new",
        "headless",
        "--updates",
        "5",
      ]),
    ).toThrow("--updates is only valid for web workload");
  });

  test("requires enough even pairs for balanced order", () => {
    for (const pairs of [1, 3, 4, 5]) {
      expect(() =>
        parseComparisonArgs([
          "--baseline",
          "./old",
          "--candidate",
          "./new",
          "headless",
          "--samples",
          "5",
          "--pairs",
          String(pairs),
        ]),
      ).toThrow("--pairs must be an even integer of at least 6");
    }
  });

  test("alternates and balances run order", () => {
    expect(comparisonSchedule(6)).toEqual([
      ["baseline", "candidate"],
      ["candidate", "baseline"],
      ["baseline", "candidate"],
      ["candidate", "baseline"],
      ["baseline", "candidate"],
      ["candidate", "baseline"],
    ]);
  });

  test("composes only the existing production perf workload", () => {
    expect(workloadCommand("/tmp/cclover-mon", { kind: "headless" }, 10)).toEqual([
      "/tmp/cclover-mon",
      "perf",
      "headless",
      "--samples",
      "10",
    ]);
    expect(
      workloadCommand("/tmp/cclover-mon", { kind: "collector", collector: "network" }, 10),
    ).toEqual([
      "/tmp/cclover-mon",
      "perf",
      "collector",
      "network",
      "--samples",
      "10",
    ]);
    expect(() => workloadCommand("/tmp/cclover-mon", { kind: "web" }, 10)).toThrow(
      "web workload is executed by the active-Web workload launcher",
    );
  });

  test("classifies consistent paired reductions and regressions", () => {
    const lower = summarizePairs([
      pair(100, 90),
      pair(110, 95),
      pair(95, 90),
      pair(105, 90),
      pair(102, 91),
      pair(108, 94),
    ]);
    expect(lower.direction).toBe("candidate consistently lower");

    const higher = summarizePairs([
      pair(100, 110),
      pair(110, 125),
      pair(95, 102),
      pair(105, 120),
      pair(102, 111),
      pair(108, 118),
    ]);
    expect(higher.direction).toBe("candidate consistently higher");
  });

  test("reports overlapping paired noise as inconclusive", () => {
    const summary = summarizePairs([
      pair(100, 95),
      pair(100, 104),
      pair(100, 96),
      pair(100, 105),
      pair(100, 98),
      pair(100, 103),
    ]);
    expect(summary.direction).toBe("inconclusive");
    expect(summary.total.pairedDeltaQ1).toBeLessThan(0);
    expect(summary.total.pairedDeltaQ3).toBeGreaterThan(0);
  });

  test("does not force a direction when one pair disagrees outside the IQR", () => {
    const summary = summarizePairs([
      pair(100, 90),
      pair(100, 90),
      pair(100, 90),
      pair(100, 90),
      pair(100, 90),
      pair(100, 101),
    ]);
    expect(summary.total.pairedDeltaQ3).toBeLessThan(0);
    expect(summary.direction).toBe("inconclusive");
  });

  test("summary enforces the comparison pair-count invariant", () => {
    expect(() =>
      summarizePairs([
        pair(100, 90),
        pair(100, 90),
        pair(100, 90),
        pair(100, 90),
      ]),
    ).toThrow("comparison requires an even number of at least 6 pairs");
  });

  test("executable identity is stable for unchanged bytes", async () => {
    const directory = await mkdtemp(join(tmpdir(), "cclover-perf-identity-"));
    try {
      const path = join(directory, "candidate");
      await writeFile(path, "same bytes");
      const before = await executableIdentity(path);
      const after = await executableIdentity(path);
      expect(after).toEqual(before);
      expect(() => verifyExecutableIdentity(before, after, "candidate")).not.toThrow();
    } finally {
      await rm(directory, { recursive: true, force: true });
    }
  });

  test("executable identity fails closed when bytes change", async () => {
    const directory = await mkdtemp(join(tmpdir(), "cclover-perf-identity-"));
    try {
      const path = join(directory, "baseline");
      await writeFile(path, "before");
      const before = await executableIdentity(path);
      await writeFile(path, "after");
      const after = await executableIdentity(path);
      expect(() => verifyExecutableIdentity(before, after, "baseline")).toThrow(
        "baseline executable changed during comparison",
      );
    } finally {
      await rm(directory, { recursive: true, force: true });
    }
  });

  test("fails closed when a measured child exits unsuccessfully", async () => {
    await expect(measureCommand([process.execPath, "-e", "process.exit(7)"])).rejects.toThrow(
      "command failed with exit 7",
    );
  });
});

function pair(baselineTotal: number, candidateTotal: number) {
  return {
    baseline: {
      userMicros: baselineTotal,
      systemMicros: baselineTotal / 2,
      totalMicros: baselineTotal,
    },
    candidate: {
      userMicros: candidateTotal,
      systemMicros: candidateTotal / 2,
      totalMicros: candidateTotal,
    },
  };
}
