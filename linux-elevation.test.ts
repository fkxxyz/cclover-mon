import { describe, expect, test } from "bun:test";

import {
  localElevatedCommand,
  type LinuxElevationEnvironment,
} from "./tools/validation/linux-elevation";

function environment(overrides: Partial<LinuxElevationEnvironment> = {}): LinuxElevationEnvironment {
  return {
    platform: "linux",
    uid: 1000,
    sudo: "/usr/bin/sudo",
    ...overrides,
  };
}

describe("Linux validation elevation", () => {
  test("profile metadata is the privilege authority", () => {
    expect(() =>
      localElevatedCommand("linux", ["true"], {}, environment()),
    ).toThrow("linux does not require local elevation");
    expect(() =>
      localElevatedCommand("windows-etw-runtime", ["true"], {}, environment()),
    ).toThrow("windows-etw-runtime does not require local elevation");
  });

  test("root executes the production child directly", () => {
    expect(
      localElevatedCommand(
        "linux-ebpf-runtime",
        ["target/release/cclover-mon", "perf", "collector", "disk-attribution"],
        {},
        environment({ uid: 0, sudo: undefined }),
      ),
    ).toEqual(["target/release/cclover-mon", "perf", "collector", "disk-attribution"]);
  });

  test("non-root execution uses the single sudo authority", () => {
    expect(
      localElevatedCommand(
        "linux-ebpf-runtime",
        ["target/release/cclover-mon", "probe", "disk-attribution"],
        {},
        environment(),
      ),
    ).toEqual([
      "/usr/bin/sudo",
      "--",
      "target/release/cclover-mon",
      "probe",
      "disk-attribution",
    ]);
  });

  test("injects only explicitly requested validation environment", () => {
    expect(
      localElevatedCommand(
        "linux-ebpf-runtime",
        ["target/release/cclover-mon", "perf", "collector", "disk-attribution"],
        {
          LD_PRELOAD: "/tmp/observer.so",
          CCLOVER_MON_EBPF_VALIDATION_TRACE: "/tmp/trace",
        },
        environment(),
      ),
    ).toEqual([
      "/usr/bin/sudo",
      "--",
      "env",
      "LD_PRELOAD=/tmp/observer.so",
      "CCLOVER_MON_EBPF_VALIDATION_TRACE=/tmp/trace",
      "target/release/cclover-mon",
      "perf",
      "collector",
      "disk-attribution",
    ]);
  });

  test("missing sudo fails with the validation contract", () => {
    expect(() =>
      localElevatedCommand(
        "linux-ebpf-runtime",
        ["target/release/cclover-mon"],
        {},
        environment({ sudo: undefined }),
      ),
    ).toThrow("linux-ebpf-runtime requires local elevation, but sudo is unavailable");
  });
});
