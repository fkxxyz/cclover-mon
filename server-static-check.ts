// Verifies that a release artifact is a self-contained ELF for the expected machine: no program
// interpreter and no DT_NEEDED dynamic-library dependencies.
//
// Usage: bun server-static-check.ts --machine <readelf Machine name> <elf-path>

export function staticArtifactViolations(
  machine: string,
  header: string,
  programHeaders: string,
  dynamicSection: string,
): string[] {
  const violations: string[] = [];
  if (!/Class:\s+ELF64\b/.test(header)) violations.push("artifact is not a 64-bit ELF");
  const actualMachine = /Machine:\s+(.+)/.exec(header)?.[1]?.trim();
  if (actualMachine !== machine) {
    violations.push(`artifact machine is ${actualMachine ?? "unknown"}, expected ${machine}`);
  }
  if (/^\s*INTERP\b/m.test(programHeaders)) {
    violations.push("artifact declares a program interpreter (dynamic loader)");
  }
  const needed = dynamicSection
    .split("\n")
    .map((line) => line.trim())
    .filter((line) => line.includes("(NEEDED)"));
  if (needed.length > 0) {
    violations.push(`artifact has dynamic library dependencies: ${needed.join("; ")}`);
  }
  return violations;
}

function readelf(flags: readonly string[], path: string): string {
  const result = Bun.spawnSync({
    cmd: ["readelf", ...flags, path],
    // readelf output is parsed as English text.
    env: { ...process.env, LC_ALL: "C" },
    stdout: "pipe",
    stderr: "pipe",
  });
  if (result.exitCode !== 0) {
    throw new Error(`readelf ${flags.join(" ")} failed: ${new TextDecoder().decode(result.stderr)}`);
  }
  return new TextDecoder().decode(result.stdout);
}

if (import.meta.main) {
  const [flag, machine, path] = process.argv.slice(2);
  if (flag !== "--machine" || !machine || !path) {
    console.error("usage: bun server-static-check.ts --machine <readelf Machine name> <elf-path>");
    process.exit(2);
  }
  const violations = staticArtifactViolations(
    machine,
    readelf(["-hW"], path),
    readelf(["-lW"], path),
    readelf(["-dW"], path),
  );
  if (violations.length > 0) {
    for (const violation of violations) console.error(`server-static-check: ${violation}`);
    process.exit(1);
  }
  console.log(`server-static-check: ${path} is a self-contained ${machine} ELF`);
}
