import { describe, expect, test } from "bun:test";

import {
  VALIDATION_PROFILES,
  XWIN_ARCH,
  formatValidationStep,
  validationSteps,
} from "./validate";

describe("validation profiles", () => {
  test("fast profile keeps architecture enforcement cheap and deterministic", () => {
    expect(validationSteps("fast").map((step) => step.command)).toEqual([
      ["bun", "archgate.ts"],
      ["bun", "test", "archgate.test.ts", "validate.test.ts"],
      ["cargo", "fmt", "--all", "--check"],
      ["bun", "archdoc.ts", "check"],
    ]);
  });

  test("every Windows build pins the shared xwin architecture set per invocation", () => {
    for (const step of validationSteps("windows")) {
      expect(step.command.slice(0, 3)).toEqual(["cargo", "xwin", "build"]);
      expect(step.env).toEqual({ XWIN_ARCH: "x86,x86_64" });
      expect(formatValidationStep(step)).toStartWith(`XWIN_ARCH=${XWIN_ARCH} cargo xwin build`);
    }
  });

  test("all profile is exactly the three focused profiles in order", () => {
    expect(validationSteps("all")).toEqual([
      ...VALIDATION_PROFILES.fast,
      ...VALIDATION_PROFILES.linux,
      ...VALIDATION_PROFILES.windows,
    ]);
  });
});
