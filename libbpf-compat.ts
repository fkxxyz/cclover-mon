export const LIBBPF_BASELINE = {
  version: "1.0.0",
  soname: "libbpf.so.1",
} as const;

export interface LibbpfImport {
  symbol: string;
  version: string;
}

export interface LibbpfElfCompatibility {
  needed: readonly string[];
  imports: readonly LibbpfImport[];
}

function parseVersion(version: string): readonly [number, number, number] {
  const parts = version.split(".").map(Number);
  if (parts.length !== 3 || parts.some((part) => !Number.isInteger(part) || part < 0)) {
    throw new Error(`invalid libbpf symbol version: ${version}`);
  }
  return [parts[0]!, parts[1]!, parts[2]!];
}

function compareVersion(left: string, right: string): number {
  const a = parseVersion(left);
  const b = parseVersion(right);
  for (let index = 0; index < a.length; index += 1) {
    if (a[index] !== b[index]) return a[index]! - b[index]!;
  }
  return 0;
}

export function parseNeededLibraries(output: string): string[] {
  return [...output.matchAll(/Shared library: \[([^\]]+)\]/g)].map((match) => match[1]!);
}

export function parseLibbpfImports(output: string): LibbpfImport[] {
  const imports: LibbpfImport[] = [];
  for (const line of output.split("\n")) {
    if (!/\bUND\b/.test(line)) continue;
    const match = line.match(/\b([A-Za-z_][A-Za-z0-9_]*)@{1,2}LIBBPF_([0-9]+\.[0-9]+\.[0-9]+)\b/);
    if (!match) continue;
    imports.push({ symbol: match[1]!, version: match[2]! });
  }
  return imports;
}

export function validateLibbpfElf(
  dynamicSection: string,
  dynamicSymbols: string,
): LibbpfElfCompatibility {
  const needed = parseNeededLibraries(dynamicSection);
  const libbpfNeeded = needed.filter((library) => library.startsWith("libbpf.so"));
  if (!libbpfNeeded.includes(LIBBPF_BASELINE.soname)) {
    const observed = libbpfNeeded.length === 0 ? "none" : libbpfNeeded.join(", ");
    throw new Error(
      `expected ${LIBBPF_BASELINE.soname} runtime dependency; observed libbpf dependencies: ${observed}`,
    );
  }
  if (libbpfNeeded.some((library) => library !== LIBBPF_BASELINE.soname)) {
    throw new Error(`unexpected additional libbpf runtime dependency: ${libbpfNeeded.join(", ")}`);
  }

  const imports = parseLibbpfImports(dynamicSymbols);
  if (imports.length === 0) {
    throw new Error("no versioned libbpf imports found in eBPF-enabled executable");
  }

  const violations = imports.filter(
    (entry) => compareVersion(entry.version, LIBBPF_BASELINE.version) > 0,
  );
  if (violations.length > 0) {
    const details = violations
      .map((entry) => `${entry.symbol}@LIBBPF_${entry.version}`)
      .sort()
      .join(", ");
    throw new Error(
      `libbpf ABI baseline ${LIBBPF_BASELINE.version} exceeded by direct imports: ${details}`,
    );
  }

  return { needed, imports };
}

function runReadelf(executable: string, args: readonly string[]): string {
  const result = Bun.spawnSync({
    cmd: [executable, ...args],
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

export function checkLibbpfElf(path: string): LibbpfElfCompatibility {
  const readelf = Bun.which("readelf");
  if (!readelf) {
    throw new Error("readelf is required for libbpf ABI validation; install binutils");
  }
  return validateLibbpfElf(
    runReadelf(readelf, ["-d", path]),
    runReadelf(readelf, ["--dyn-syms", "--wide", path]),
  );
}

function usage(): never {
  console.error("usage: bun libbpf-compat.ts <eBPF-enabled ELF> [<eBPF-enabled ELF> ...]");
  process.exit(2);
}

if (import.meta.main) {
  const paths = process.argv.slice(2);
  if (paths.length === 0) usage();
  try {
    for (const path of paths) {
      const result = checkLibbpfElf(path);
      const highest = result.imports.reduce((current, entry) =>
        compareVersion(entry.version, current.version) > 0 ? entry : current,
      );
      console.log(
        `${path}: ${LIBBPF_BASELINE.soname}, highest direct import ${highest.symbol}@LIBBPF_${highest.version} (baseline ${LIBBPF_BASELINE.version})`,
      );
    }
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    process.exit(1);
  }
}
