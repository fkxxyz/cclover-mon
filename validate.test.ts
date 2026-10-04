import { describe, expect, test } from "bun:test";

import {
  VALIDATION_PROFILES,
  XWIN_ARCH,
  formatValidationStep,
  validationExecution,
  validationSteps,
} from "./validate";

describe("validation profiles", () => {
  test("fast profile keeps architecture enforcement cheap and deterministic", () => {
    expect(validationSteps("fast").map((step) => step.command)).toEqual([
      ["bun", "archgate.ts"],
      [
        "bun",
        "test",
        "archgate.test.ts",
        "release.test.ts",
        "release-publication.test.ts",
        "validate.test.ts",
        "windows-validate.test.ts",
        "sync-linux-hwmon.test.ts",
      ],
      ["cargo", "fmt", "--all", "--check"],
      ["bun", "archdoc.ts", "check"],
    ]);
  });

  test("every Windows cross command pins the shared xwin architecture set per invocation", () => {
    const [prepare, ...commands] = validationSteps("windows");
    expect(prepare?.command).toEqual(["bun", "prepare-windows-deps.ts"]);
    for (const step of commands) {
      expect(step.command.slice(0, 2)).toEqual(["cargo", "xwin"]);
      expect(["build", "test"]).toContain(step.command[2]);
      expect(step.env).toEqual({ XWIN_ARCH: "x86,x86_64" });
      expect(formatValidationStep(step)).toStartWith(`XWIN_ARCH=${XWIN_ARCH} cargo xwin`);
    }
  });

  test("Windows cross validation compiles target-specific test code", () => {
    const testCompiles = validationSteps("windows").filter((step) => step.command[2] === "test");
    expect(testCompiles).toHaveLength(2);
    for (const step of testCompiles) {
      expect(step.command).toContain("--no-run");
      expect(step.command).toContain("--no-default-features");
    }
  });

  test("Windows cross validation builds the headless server for both targets", () => {
    const serverBuilds = validationSteps("windows").filter((step) =>
      step.command.includes("cclover-server"),
    );
    expect(serverBuilds).toHaveLength(2);
    expect(serverBuilds.map((step) => step.command.at(-1))).toEqual([
      "x86_64-pc-windows-msvc",
      "i686-pc-windows-msvc",
    ]);
  });

  test("Windows native validation executes deterministic tests on a Windows host", () => {
    expect(validationSteps("windows-native").map((step) => step.command)).toEqual([
      ["bun", "prepare-windows-deps.ts"],
      ["cargo", "test", "--locked", "-p", "cclover-mon", "--no-default-features"],
    ]);
    expect(validationExecution("windows-native")).toEqual({
      host: "windows",
      privilege: "ordinary",
    });
  });

  test("Windows ETW runtime validation is isolated and covers probe plus exact semantics", () => {
    expect(validationSteps("windows-etw-runtime").map((step) => step.command)).toEqual([
      ["bun", "prepare-windows-deps.ts"],
      [
        "cargo",
        "build",
        "--locked",
        "--release",
        "-p",
        "cclover-mon",
        "--no-default-features",
      ],
      ["bun", "windows-etw-disk-smoke.ts"],
      [
        "cargo",
        "run",
        "--locked",
        "-p",
        "cclover-platform",
        "--bin",
        "windows-etw-semantic",
        "--features",
        "windows-etw-validation",
      ],
    ]);
    expect(
      validationSteps("windows-native").some((step) =>
        step.command.includes("windows-etw-disk-smoke.ts"),
      ),
    ).toBe(false);
    expect(
      validationSteps("portable").some((step) =>
        step.command.includes("windows-etw-disk-smoke.ts"),
      ),
    ).toBe(false);
    expect(validationExecution("windows-etw-runtime")).toEqual({
      host: "windows",
      privilege: "elevated",
    });
  });

  test("web browser validation is isolated from normal development profiles", () => {
    expect(validationSteps("web-browser").map((step) => step.command)).toEqual([
      ["cargo", "build", "--locked", "--release"],
      ["bun", "web-browser-smoke.ts"],
    ]);
    expect(validationSteps("fast").some((step) => step.command.includes("web-browser-smoke.ts"))).toBe(false);
    expect(validationSteps("linux").some((step) => step.command.includes("web-browser-smoke.ts"))).toBe(false);
  });

  test("server validation owns the headless product smoke", () => {
    expect(validationSteps("server").map((step) => step.command)).toEqual([
      ["cargo", "test", "--locked", "-p", "cclover-server"],
      ["cargo", "build", "--locked", "--release", "-p", "cclover-server"],
      ["bun", "server-smoke.ts"],
    ]);
  });

  test("portable profile is exactly the three host-portable profiles in order", () => {
    expect(validationSteps("portable")).toEqual([
      ...VALIDATION_PROFILES.fast.steps,
      ...VALIDATION_PROFILES.linux.steps,
      ...VALIDATION_PROFILES.windows.steps,
    ]);
  });
});
