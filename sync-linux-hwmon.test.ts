import { describe, expect, test } from "bun:test";
import { readFile } from "node:fs/promises";
import { join } from "node:path";

import { LINUX_HWMON } from "./deps/linux-hwmon";

async function sha256(path: string): Promise<string> {
  const hasher = new Bun.CryptoHasher("sha256");
  hasher.update(await readFile(path));
  return hasher.digest("hex");
}

describe("vendored Linux hwmon snapshot", () => {
  test("pins an exact upstream commit", () => {
    expect(LINUX_HWMON.commit).toMatch(/^[0-9a-f]{40}$/);
    expect(LINUX_HWMON.ref.length).toBeGreaterThan(0);
    expect(LINUX_HWMON.files.length).toBeGreaterThan(0);
  });

  for (const file of LINUX_HWMON.files) {
    test(`${file.path} matches the declared upstream digest`, async () => {
      const vendorPath = join(import.meta.dir, "crates", "cclover-platform", "vendor", "linux", file.path.replace(/^drivers\//, ""));
      expect(await sha256(vendorPath)).toBe(file.sha256);
      const source = await readFile(vendorPath, "utf8");
      expect(source.slice(0, 256)).toContain("SPDX-License-Identifier:");
    });
  }

  test("representative Intel, AMD, and Super-I/O upstream sources are selected", () => {
    const paths = LINUX_HWMON.files.map((file) => file.path);
    expect(paths).toContain("drivers/hwmon/coretemp.c");
    expect(paths).toContain("drivers/hwmon/k8temp.c");
    expect(paths).toContain("drivers/hwmon/k10temp.c");
    expect(paths).toContain("drivers/hwmon/nct6775-core.c");
  });

  test("runtime coverage documentation names each active upstream CPU driver", async () => {
    const coverage = await readFile(join(import.meta.dir, "docs", "maintenance", "linux-hwmon-coverage.md"), "utf8");
    for (const driver of ["coretemp.c", "k8temp.c", "k10temp.c"]) {
      expect(coverage).toContain(driver);
    }
  });

  test("vendor warning isolation cannot weaken project-owned bridge diagnostics", async () => {
    const platformRoot = join(import.meta.dir, "crates", "cclover-platform");
    const buildSource = await readFile(join(platformRoot, "build.rs"), "utf8");
    const hwmonBuild = buildSource
      .split("fn build_windows_hwmon_compat() {")[1]
      ?.split("\n}\n\nfn configure_windows_resources")[0];

    expect(hwmonBuild).toBeDefined();
    expect(hwmonBuild).toContain(".warnings(true)");
    expect(hwmonBuild).not.toMatch(/-Wno-|\.warnings\(false\)|(?:^|[\s\"'])-w(?:[\s\"']|$)/);

    const diagnosticScope = await readFile(
      join(platformRoot, "native", "windows", "hwmon", "vendor_diagnostics.h"),
      "utf8",
    );
    expect(diagnosticScope).toContain('_Pragma("clang diagnostic push")');
    expect(diagnosticScope).toContain('_Pragma("clang diagnostic pop")');

    for (const [bridge, vendor] of [
      ["coretemp_bridge.c", "coretemp.c"],
      ["k8temp_bridge.c", "k8temp.c"],
      ["k10temp_bridge.c", "k10temp.c"],
    ] as const) {
      const source = await readFile(join(platformRoot, "native", "windows", "hwmon", bridge), "utf8");
      expect(source).toContain(
        `CCLOVER_HWMON_VENDOR_WARNINGS_BEGIN\n#include "${vendor}"\nCCLOVER_HWMON_VENDOR_WARNINGS_END`,
      );
    }
  });
});
