import { describe, expect, test } from "bun:test";

import {
  LIBBPF_BASELINE,
  parseLibbpfImports,
  parseNeededLibraries,
  validateLibbpfElf,
} from "./libbpf-compat";

const DYNAMIC = `
 0x0000000000000001 (NEEDED)             Shared library: [libbpf.so.1]
 0x0000000000000001 (NEEDED)             Shared library: [libc.so.6]
`;

const SYMBOLS = `
   11: 0000000000000000     0 FUNC    GLOBAL DEFAULT  UND bpf_object__open_mem@LIBBPF_0.0.6 (13)
   12: 0000000000000000     0 FUNC    GLOBAL DEFAULT  UND bpf_object__next_program@LIBBPF_0.6.0 (14)
   13: 0000000000000000     0 FUNC    GLOBAL DEFAULT  UND bpf_map_lookup_batch@LIBBPF_0.0.7 (8)
`;

describe("libbpf ELF compatibility", () => {
  test("baseline is explicit and stable", () => {
    expect(LIBBPF_BASELINE).toEqual({ version: "1.0.0", soname: "libbpf.so.1" });
  });

  test("parses runtime libraries and versioned undefined imports", () => {
    expect(parseNeededLibraries(DYNAMIC)).toEqual(["libbpf.so.1", "libc.so.6"]);
    expect(parseLibbpfImports(SYMBOLS)).toEqual([
      { symbol: "bpf_object__open_mem", version: "0.0.6" },
      { symbol: "bpf_object__next_program", version: "0.6.0" },
      { symbol: "bpf_map_lookup_batch", version: "0.0.7" },
    ]);
  });

  test("accepts current baseline-compatible imports", () => {
    expect(validateLibbpfElf(DYNAMIC, SYMBOLS).imports).toHaveLength(3);
  });

  test("rejects a different libbpf SONAME", () => {
    expect(() =>
      validateLibbpfElf(DYNAMIC.replace("libbpf.so.1", "libbpf.so.2"), SYMBOLS),
    ).toThrow("expected libbpf.so.1 runtime dependency");
  });

  test("rejects direct imports introduced after the declared baseline", () => {
    const newer = `${SYMBOLS}\n   14: 0 0 FUNC GLOBAL DEFAULT UND bpf_new_api@LIBBPF_1.1.0 (20)`;
    expect(() => validateLibbpfElf(DYNAMIC, newer)).toThrow(
      "libbpf ABI baseline 1.0.0 exceeded by direct imports: bpf_new_api@LIBBPF_1.1.0",
    );
  });

  test("requires evidence that the inspected executable is eBPF-enabled", () => {
    expect(() => validateLibbpfElf(DYNAMIC, "no matching symbols")).toThrow(
      "no versioned libbpf imports found",
    );
  });
});
