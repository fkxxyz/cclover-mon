import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";

import {
  LINUX_RUNTIME_ABI_POLICY,
  isAuthoritativeLinuxReleaseUserspace,
  parseLinuxInterpreter,
  parseLinuxVersionedImports,
  validateLinuxReleaseBuildHost,
  validateLinuxReleasePublicationEligibility,
  validateLinuxRuntimeAbi,
} from "./tools/release/linux-runtime-abi";
import { releaseArtifact } from "./tools/release/plan";

const ARTIFACT = releaseArtifact("cclover-mon-x86_64-unknown-linux-gnu");
const PROGRAM_HEADERS = `
Elf file type is DYN (Position-Independent Executable file)
      [Requesting program interpreter: /lib64/ld-linux-x86-64.so.2]
`;
const DYNAMIC = `
 0x0000000000000001 (NEEDED)             Shared library: [libbpf.so.1]
 0x0000000000000001 (NEEDED)             Shared library: [libgcc_s.so.1]
 0x0000000000000001 (NEEDED)             Shared library: [libc.so.6]
`;
const SYMBOLS = `
   10: 0000000000000000     0 FUNC    GLOBAL DEFAULT  UND __libc_start_main@GLIBC_2.34 (2)
   11: 0000000000000000     0 FUNC    GLOBAL DEFAULT  UND hypot@GLIBC_2.35 (3)
   12: 0000000000000000     0 FUNC    GLOBAL DEFAULT  UND _Unwind_Resume@GCC_3.0 (4)
   13: 0000000000000000     0 FUNC    GLOBAL DEFAULT  UND __register_frame_info@GCC_4.2.0 (5)
   14: 0000000000000000     0 FUNC    GLOBAL DEFAULT  UND bpf_object__open_mem@LIBBPF_0.0.6 (6)
   15: 0000000000000000     0 FUNC    GLOBAL DEFAULT  UND bpf_object__next_program@LIBBPF_0.6.0 (7)
`;

describe("Linux release runtime ABI authority", () => {
  test("declares one explicit x86_64 Linux compatibility baseline", () => {
    expect(LINUX_RUNTIME_ABI_POLICY).toEqual({
      target: "x86_64-unknown-linux-gnu",
      interpreter: "/lib64/ld-linux-x86-64.so.2",
      glibcMax: "2.35",
      gccMax: "4.2.0",
      githubRunnerImage: "ubuntu22",
      githubRunnerLabel: "ubuntu-22.04",
      allowedNeeded: {
        "cclover-mon": [
          "libcairo.so.2",
          "libXext.so.6",
          "libXrender.so.1",
          "libX11.so.6",
          "libwayland-client.so.0",
          "libm.so.6",
          "libgio-2.0.so.0",
          "libgobject-2.0.so.0",
          "libglib-2.0.so.0",
          "libbpf.so.1",
          "libgcc_s.so.1",
          "libc.so.6",
          "ld-linux-x86-64.so.2",
        ],
        "cclover-mon-server": [
          "libbpf.so.1",
          "libgcc_s.so.1",
          "libc.so.6",
          "ld-linux-x86-64.so.2",
        ],
      },
      forbiddenNeeded: ["libnvidia-ml.so.1"],
    });
  });

  test("parses the loader and governed direct symbol imports", () => {
    expect(parseLinuxInterpreter(PROGRAM_HEADERS)).toBe("/lib64/ld-linux-x86-64.so.2");
    expect(parseLinuxVersionedImports(SYMBOLS)).toEqual([
      { symbol: "__libc_start_main", namespace: "GLIBC", version: "2.34" },
      { symbol: "hypot", namespace: "GLIBC", version: "2.35" },
      { symbol: "_Unwind_Resume", namespace: "GCC", version: "3.0" },
      { symbol: "__register_frame_info", namespace: "GCC", version: "4.2.0" },
    ]);
  });

  test("accepts the declared baseline and delegates libbpf policy", () => {
    const report = validateLinuxRuntimeAbi(ARTIFACT, PROGRAM_HEADERS, DYNAMIC, SYMBOLS);
    expect(report.highestGlibc).toBe("2.35");
    expect(report.libbpf.imports).toHaveLength(2);
  });

  test("rejects a changed ELF interpreter", () => {
    expect(() =>
      validateLinuxRuntimeAbi(
        ARTIFACT,
        PROGRAM_HEADERS.replace(
          "/lib64/ld-linux-x86-64.so.2",
          "/lib/ld-musl-x86_64.so.1",
        ),
        DYNAMIC,
        SYMBOLS,
      ),
    ).toThrow("Linux runtime interpreter mismatch");
  });

  test("rejects direct imports above the glibc ceiling", () => {
    const newer = `${SYMBOLS}\n16: 0 0 FUNC GLOBAL DEFAULT UND future@GLIBC_2.36 (8)`;
    expect(() => validateLinuxRuntimeAbi(ARTIFACT, PROGRAM_HEADERS, DYNAMIC, newer)).toThrow(
      "GLIBC ABI baseline 2.35 exceeded by direct imports: future@GLIBC_2.36",
    );
  });

  test("rejects GLIBC_PRIVATE and newer libgcc ABI imports", () => {
    const privateImport = `${SYMBOLS}\n16: 0 0 FUNC GLOBAL DEFAULT UND private_api@GLIBC_PRIVATE (8)`;
    expect(() =>
      validateLinuxRuntimeAbi(ARTIFACT, PROGRAM_HEADERS, DYNAMIC, privateImport),
    ).toThrow("direct GLIBC_PRIVATE import is not allowed");

    const newerGcc = `${SYMBOLS}\n16: 0 0 FUNC GLOBAL DEFAULT UND unwind_new@GCC_4.3.0 (8)`;
    expect(() => validateLinuxRuntimeAbi(ARTIFACT, PROGRAM_HEADERS, DYNAMIC, newerGcc)).toThrow(
      "GCC ABI baseline 4.2.0 exceeded by direct imports: unwind_new@GCC_4.3.0",
    );
  });

  test("allows dependency removal but rejects undeclared mandatory runtime libraries", () => {
    expect(() => validateLinuxRuntimeAbi(ARTIFACT, PROGRAM_HEADERS, DYNAMIC, SYMBOLS)).not.toThrow();

    const ordinary = `${DYNAMIC}\n0x1 (NEEDED) Shared library: [libexample.so.1]`;
    expect(() => validateLinuxRuntimeAbi(ARTIFACT, PROGRAM_HEADERS, ordinary, SYMBOLS)).toThrow(
      "Linux runtime dependency policy does not allow mandatory libraries for cclover-mon: libexample.so.1",
    );
  });

  test("forbids optional NVML from becoming mandatory", () => {
    const nvml = `${DYNAMIC}\n0x1 (NEEDED) Shared library: [libnvidia-ml.so.1]`;
    expect(() => validateLinuxRuntimeAbi(ARTIFACT, PROGRAM_HEADERS, nvml, SYMBOLS)).toThrow(
      "optional runtime library became a mandatory ELF dependency: libnvidia-ml.so.1",
    );
  });

  test("keeps the existing libbpf ceiling authoritative", () => {
    const newer = `${SYMBOLS}\n16: 0 0 FUNC GLOBAL DEFAULT UND bpf_new@LIBBPF_1.1.0 (8)`;
    expect(() => validateLinuxRuntimeAbi(ARTIFACT, PROGRAM_HEADERS, DYNAMIC, newer)).toThrow(
      "libbpf ABI baseline 1.0.0 exceeded by direct imports: bpf_new@LIBBPF_1.1.0",
    );
  });

  test("pins GitHub-hosted Linux release builds to the declared userspace generation", () => {
    expect(() =>
      validateLinuxReleaseBuildHost(ARTIFACT, {
        host: {
          os: "linux",
          arch: "x64",
          runnerEnvironment: "github-hosted",
          runnerImage: "ubuntu22",
          runnerImageVersion: "20261001.1",
        },
      }),
    ).not.toThrow();
    expect(() =>
      validateLinuxReleaseBuildHost(ARTIFACT, {
        host: {
          os: "linux",
          arch: "x64",
          runnerEnvironment: "github-hosted",
          runnerImage: "ubuntu24",
          runnerImageVersion: "20261001.1",
        },
      }),
    ).toThrow("Linux GitHub release build requires authoritative ubuntu22 runner provenance");
  });

  test("derives publication userspace authority only from controlled runner provenance", () => {
    const authoritative = {
      host: {
        os: "linux",
        arch: "x64",
        osRelease: "Ubuntu 22.04 fixture",
        runnerEnvironment: "github-hosted",
        runnerImage: "ubuntu22",
        runnerImageVersion: "20261001.1",
      },
    };
    expect(isAuthoritativeLinuxReleaseUserspace(authoritative)).toBe(true);
    expect(() => validateLinuxReleasePublicationEligibility(ARTIFACT, authoritative)).not.toThrow();

    for (const uncontrolled of [
      {
        host: {
          os: "linux",
          arch: "x64",
          osRelease: "Arch Linux",
        },
      },
      {
        host: {
          os: "linux",
          arch: "x64",
          osRelease: "Ubuntu 22.04 fixture",
        },
      },
      {
        host: {
          os: "linux",
          arch: "x64",
          osRelease: "Ubuntu 22.04 fixture",
          runnerEnvironment: "self-hosted",
          runnerImage: "ubuntu22",
          runnerImageVersion: "20261001.1",
        },
      },
      {
        host: {
          os: "linux",
          arch: "x64",
          runnerEnvironment: "github-hosted",
          runnerImage: "ubuntu22",
        },
      },
    ]) {
      expect(isAuthoritativeLinuxReleaseUserspace(uncontrolled)).toBe(false);
      expect(() => validateLinuxReleasePublicationEligibility(ARTIFACT, uncontrolled)).toThrow(
        "was built from uncontrolled userspace and is not eligible for official publication",
      );
    }
  });

  test("release workflow consumes the declared Linux runner label", () => {
    const workflow = readFileSync(".github/workflows/release.yml", "utf8");
    expect(workflow).toContain(
      "runs-on: ${{ matrix.platform == 'linux' && '" +
        LINUX_RUNTIME_ABI_POLICY.githubRunnerLabel +
        "' || 'ubuntu-latest' }}",
    );
  });
});
