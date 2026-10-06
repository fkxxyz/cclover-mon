import { describe, expect, test } from "bun:test";

import {
  ActiveWebSseCounter,
  parseActiveWebArgs,
  parseProductionListenUrl,
  runActiveWebWorkload,
} from "./perf-web";

describe("active Web performance workload", () => {
  test("parses explicit executable and fixed update count", () => {
    expect(
      parseActiveWebArgs([
        "--executable",
        "./target/release/cclover-mon",
        "--updates",
        "30",
      ]),
    ).toEqual({
      executable: "./target/release/cclover-mon",
      updates: 30,
    });
  });

  test("requires positive fixed work", () => {
    expect(() =>
      parseActiveWebArgs([
        "--executable",
        "./target/release/cclover-mon",
        "--updates",
        "0",
      ]),
    ).toThrow("--updates requires a positive integer");
    expect(() =>
      parseActiveWebArgs(["--executable", "./target/release/cclover-mon"]),
    ).toThrow("--updates is required");
  });

  test("rejects unrelated launcher policy", () => {
    expect(() =>
      parseActiveWebArgs([
        "--executable",
        "./target/release/cclover-mon",
        "--updates",
        "5",
        "--duration",
        "30",
      ]),
    ).toThrow("unexpected argument: --duration");
  });

  test("counts complete SSE events rather than data lines", () => {
    const counter = new ActiveWebSseCounter(2);

    expect(counter.consumeLine("data: activation-a")).toBeNull();
    expect(counter.consumeLine("data: activation-b")).toBeNull();
    expect(counter.consumeLine("")).toBe("active");

    expect(counter.consumeLine(": keepalive")).toBeNull();
    expect(counter.consumeLine("")).toBeNull();

    expect(counter.consumeLine("data:first-update-a")).toBeNull();
    expect(counter.consumeLine("data: first-update-b")).toBeNull();
    expect(counter.consumeLine("")).toBe("update");

    expect(counter.consumeLine("data: second-update")).toBeNull();
    expect(counter.consumeLine("")).toBe("done");
  });

  test("pins the production listen diagnostic used for address discovery", () => {
    expect(
      parseProductionListenUrl(
        "cclover-mon: HTTP monitor listening on http://127.0.0.1:41234",
      ),
    ).toBe("http://127.0.0.1:41234");
    expect(
      parseProductionListenUrl(
        "cclover-mon: HTTP listening on http://127.0.0.1:41234",
      ),
    ).toBeNull();
  });

  test("validates work count before launching a process", async () => {
    await expect(
      runActiveWebWorkload({
        executable: "/definitely/not/a/cclover-mon",
        updates: 0,
      }),
    ).rejects.toThrow("updates must be a positive integer");
  });
});
