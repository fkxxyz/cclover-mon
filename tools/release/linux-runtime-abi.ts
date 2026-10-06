import {
  parseNeededLibraries,
  validateLibbpfElf,
  type LibbpfElfCompatibility,
} from "../../libbpf-compat";
import type { BuildProvenance, ReleaseArtifact } from "./plan";

export const LINUX_RUNTIME_ABI_POLICY = {
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
} as const;

export type LinuxVersionNamespace = "GLIBC" | "GCC";

export interface LinuxVersionedImport {
  symbol: string;
  namespace: LinuxVersionNamespace;
  version: string;
}

export interface LinuxRuntimeAbiReport {
  interpreter: string;
  needed: readonly string[];
  imports: readonly LinuxVersionedImport[];
  highestGlibc: string;
  highestGcc?: string;
  libbpf: LibbpfElfCompatibility;
}

function parseNumericVersion(version: string): readonly number[] {
  const parts = version.split(".").map(Number);
  if (parts.length < 2 || parts.some((part) => !Number.isInteger(part) || part < 0)) {
    throw new Error(`invalid ABI symbol version: ${version}`);
  }
  return parts;
}

export function compareNumericVersion(left: string, right: string): number {
  const a = parseNumericVersion(left);
  const b = parseNumericVersion(right);
  const length = Math.max(a.length, b.length);
  for (let index = 0; index < length; index += 1) {
    const difference = (a[index] ?? 0) - (b[index] ?? 0);
    if (difference !== 0) return difference;
  }
  return 0;
}

export function parseLinuxInterpreter(programHeaders: string): string {
  const match = programHeaders.match(/Requesting program interpreter:\s*([^\]]+)\]/);
  if (!match) throw new Error("ELF program headers do not declare a program interpreter");
  return match[1]!.trim();
}

export function parseLinuxVersionedImports(dynamicSymbols: string): LinuxVersionedImport[] {
  const imports: LinuxVersionedImport[] = [];
  for (const line of dynamicSymbols.split("\n")) {
    if (!/\bUND\b/.test(line)) continue;
    const match = line.match(
      /\b([A-Za-z_][A-Za-z0-9_]*)@{1,2}(GLIBC|GCC)_([0-9]+(?:\.[0-9]+)+)\b/,
    );
    if (!match) continue;
    imports.push({
      symbol: match[1]!,
      namespace: match[2]! as LinuxVersionNamespace,
      version: match[3]!,
    });
  }
  return imports;
}

function highestVersion(
  imports: readonly LinuxVersionedImport[],
  namespace: LinuxVersionNamespace,
): string | undefined {
  return imports
    .filter((entry) => entry.namespace === namespace)
    .map((entry) => entry.version)
    .reduce<string | undefined>(
      (highest, version) =>
        highest === undefined || compareNumericVersion(version, highest) > 0 ? version : highest,
      undefined,
    );
}

function assertVersionCeiling(
  imports: readonly LinuxVersionedImport[],
  namespace: LinuxVersionNamespace,
  ceiling: string,
): void {
  const violations = imports.filter(
    (entry) =>
      entry.namespace === namespace && compareNumericVersion(entry.version, ceiling) > 0,
  );
  if (violations.length === 0) return;
  const details = violations
    .map((entry) => `${entry.symbol}@${entry.namespace}_${entry.version}`)
    .sort()
    .join(", ");
  throw new Error(
    `${namespace} ABI baseline ${ceiling} exceeded by direct imports: ${details}`,
  );
}

export function validateLinuxRuntimeAbi(
  artifact: ReleaseArtifact,
  programHeaders: string,
  dynamicSection: string,
  dynamicSymbols: string,
): LinuxRuntimeAbiReport {
  if (artifact.platform !== "linux") {
    throw new Error(`Linux runtime ABI policy does not apply to ${artifact.id}`);
  }
  if (artifact.target !== LINUX_RUNTIME_ABI_POLICY.target) {
    throw new Error(
      `Linux runtime ABI policy target ${LINUX_RUNTIME_ABI_POLICY.target} does not cover ${artifact.target}`,
    );
  }

  const interpreter = parseLinuxInterpreter(programHeaders);
  if (interpreter !== LINUX_RUNTIME_ABI_POLICY.interpreter) {
    throw new Error(
      `Linux runtime interpreter mismatch: expected ${LINUX_RUNTIME_ABI_POLICY.interpreter}, observed ${interpreter}`,
    );
  }

  const needed = parseNeededLibraries(dynamicSection);
  const forbidden = needed.filter((library) =>
    LINUX_RUNTIME_ABI_POLICY.forbiddenNeeded.some((candidate) => candidate === library),
  );
  if (forbidden.length > 0) {
    throw new Error(
      `optional runtime library became a mandatory ELF dependency: ${forbidden.join(", ")}`,
    );
  }
  const allowedNeeded = LINUX_RUNTIME_ABI_POLICY.allowedNeeded[artifact.product];
  const undeclared = needed.filter(
    (library) => !allowedNeeded.some((candidate) => candidate === library),
  );
  if (undeclared.length > 0) {
    throw new Error(
      `Linux runtime dependency policy does not allow mandatory libraries for ${artifact.product}: ${undeclared.join(", ")}`,
    );
  }

  const undefinedLines = dynamicSymbols
    .split("\n")
    .filter((line) => /\bUND\b/.test(line));
  if (undefinedLines.some((line) => /@{1,2}GLIBC_PRIVATE\b/.test(line))) {
    throw new Error("direct GLIBC_PRIVATE import is not allowed");
  }

  const imports = parseLinuxVersionedImports(dynamicSymbols);
  const highestGlibc = highestVersion(imports, "GLIBC");
  if (!highestGlibc) {
    throw new Error("no versioned GLIBC imports found in Linux release executable");
  }
  assertVersionCeiling(imports, "GLIBC", LINUX_RUNTIME_ABI_POLICY.glibcMax);
  assertVersionCeiling(imports, "GCC", LINUX_RUNTIME_ABI_POLICY.gccMax);
  const highestGcc = highestVersion(imports, "GCC");

  return {
    interpreter,
    needed,
    imports,
    highestGlibc,
    ...(highestGcc ? { highestGcc } : {}),
    libbpf: validateLibbpfElf(dynamicSection, dynamicSymbols),
  };
}

export function validateLinuxReleaseBuildHost(
  artifact: ReleaseArtifact,
  provenance: Pick<BuildProvenance, "host">,
): void {
  if (artifact.platform !== "linux") return;
  if (
    provenance.host.runnerEnvironment === "github-hosted" &&
    provenance.host.runnerImage !== LINUX_RUNTIME_ABI_POLICY.githubRunnerImage
  ) {
    throw new Error(
      `Linux GitHub release build requires ${LINUX_RUNTIME_ABI_POLICY.githubRunnerImage}; observed ${provenance.host.runnerImage ?? "unknown"}`,
    );
  }
}

function runReadelf(readelf: string, args: readonly string[]): string {
  const result = Bun.spawnSync({
    cmd: [readelf, ...args],
    env: { ...process.env, LC_ALL: "C" },
    stdout: "pipe",
    stderr: "pipe",
  });
  if (result.exitCode !== 0) {
    const stderr = new TextDecoder().decode(result.stderr).trim();
    throw new Error(`readelf ${args.join(" ")} failed: ${stderr || `exit ${result.exitCode}`}`);
  }
  return new TextDecoder().decode(result.stdout);
}

export function checkLinuxRuntimeAbi(
  artifact: ReleaseArtifact,
  path: string,
): LinuxRuntimeAbiReport {
  const readelf = Bun.which("readelf");
  if (!readelf) {
    throw new Error("readelf is required for Linux runtime ABI validation; install binutils");
  }
  return validateLinuxRuntimeAbi(
    artifact,
    runReadelf(readelf, ["-l", path]),
    runReadelf(readelf, ["-d", path]),
    runReadelf(readelf, ["--dyn-syms", "--wide", path]),
  );
}
