import { mkdirSync } from "node:fs";
import { join } from "node:path";

function fail(message: string): never {
  console.error(`linux-ebpf-runtime-smoke: ${message}`);
  process.exit(1);
}

if (process.platform !== "linux") {
  fail("this validation must run on a real Linux host");
}

const root = process.cwd();
const executable = join(root, "target", "release", "cclover-mon");
const source = join(root, "tools", "linux-validation", "libbpf-batch-unsupported.c");
const outputDirectory = join(root, "target", "validation");
const shim = join(outputDirectory, "libbpf-batch-unsupported.so");
mkdirSync(outputDirectory, { recursive: true });

const compile = Bun.spawnSync({
  cmd: ["clang", "-shared", "-fPIC", "-O2", "-Wall", "-Wextra", "-Werror", source, "-o", shim],
  stdout: "pipe",
  stderr: "pipe",
});
if (compile.exitCode !== 0) {
  fail(new TextDecoder().decode(compile.stderr) || `shim compilation exited ${compile.exitCode}`);
}

const elevatedPrefix = (() => {
  if (typeof process.getuid === "function" && process.getuid() === 0) return [];
  if (!Bun.which("sudo")) fail("sudo is required when linux-ebpf-runtime is not run as root");
  return ["sudo"];
})();
const expectedDiagnostic = "BPF batch map lookup unsupported by kernel/map";

for (const collector of ["disk-attribution", "network-attribution"] as const) {
  const result = Bun.spawnSync({
    cmd: [
      ...elevatedPrefix,
      "env",
      `LD_PRELOAD=${shim}`,
      executable,
      "probe",
      collector,
      "--raw",
    ],
    stdout: "pipe",
    stderr: "pipe",
  });
  const stdout = new TextDecoder().decode(result.stdout);
  const stderr = new TextDecoder().decode(result.stderr);
  if (result.exitCode !== 0) {
    fail(`${collector} probe exited ${result.exitCode}\n${stderr || stdout}`);
  }
  if (!/^status: (ok|degraded)$/m.test(stdout)) {
    fail(`${collector} did not remain available through scalar fallback\n${stdout}${stderr}`);
  }
  if (!stdout.includes(expectedDiagnostic)) {
    fail(`${collector} did not report the forced scalar fallback\n${stdout}${stderr}`);
  }
  console.log(`linux-ebpf-runtime-smoke: ${collector} used production scalar fallback`);
}
