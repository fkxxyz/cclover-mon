import { describe, expect, test } from "bun:test";

import { staticArtifactViolations } from "./server-static-check";

const staticHeader = `ELF Header:
  Class:                             ELF64
  Machine:                           AArch64
  Type:                              EXEC (Executable file)
`;

const staticProgramHeaders = `Program Headers:
  Type           Offset   VirtAddr           PhysAddr           FileSiz  MemSiz   Flg Align
  LOAD           0x000000 0x0000000000400000 0x0000000000400000 0x1a2b3c 0x1a2b3c R E 0x10000
  GNU_STACK      0x000000 0x0000000000000000 0x0000000000000000 0x000000 0x000000 RW  0x10
`;

const staticDynamic = "There is no dynamic section in this file.\n";

describe("static server artifact check", () => {
  test("accepts a self-contained ELF for the expected machine", () => {
    expect(
      staticArtifactViolations("AArch64", staticHeader, staticProgramHeaders, staticDynamic),
    ).toEqual([]);
  });

  test("the expected machine is a parameter, not a built-in architecture", () => {
    const x86 = staticHeader.replace("AArch64", "Advanced Micro Devices X86-64");
    expect(
      staticArtifactViolations(
        "Advanced Micro Devices X86-64",
        x86,
        staticProgramHeaders,
        staticDynamic,
      ),
    ).toEqual([]);
  });

  test("rejects a program interpreter", () => {
    const withInterp = staticProgramHeaders.replace(
      "  GNU_STACK",
      "  INTERP         0x000238 0x0000000000400238 0x0000000000400238 0x00001b 0x00001b R   0x1\n  GNU_STACK",
    );
    expect(staticArtifactViolations("AArch64", staticHeader, withInterp, staticDynamic)).toEqual([
      "artifact declares a program interpreter (dynamic loader)",
    ]);
  });

  test("rejects dynamic library dependencies", () => {
    const dynamic = [
      "Dynamic section at offset 0x1000 contains 3 entries:",
      " 0x0000000000000001 (NEEDED)             Shared library: [libbpf.so.1]",
      " 0x0000000000000001 (NEEDED)             Shared library: [libc.so.6]",
    ].join("\n");
    const violations = staticArtifactViolations(
      "AArch64",
      staticHeader,
      staticProgramHeaders,
      dynamic,
    );
    expect(violations).toHaveLength(1);
    expect(violations[0]).toContain("libbpf.so.1");
    expect(violations[0]).toContain("libc.so.6");
  });

  test("rejects a different machine or a non-ELF64 artifact", () => {
    const x86 = staticHeader.replace("AArch64", "Advanced Micro Devices X86-64");
    expect(staticArtifactViolations("AArch64", x86, staticProgramHeaders, staticDynamic)).toEqual([
      "artifact machine is Advanced Micro Devices X86-64, expected AArch64",
    ]);
    const elf32 = staticHeader.replace("ELF64", "ELF32");
    expect(
      staticArtifactViolations("AArch64", elf32, staticProgramHeaders, staticDynamic),
    ).toContain("artifact is not a 64-bit ELF");
  });
});
