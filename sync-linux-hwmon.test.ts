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
      const vendorPath = join(import.meta.dir, "vendor", "linux", file.path.replace(/^drivers\//, ""));
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
});
