import { LIBRE_HARDWARE_MONITOR as LHM } from "./deps/librehardwaremonitor";

function usage(): never {
  console.error("usage: bun lhm-sync.ts status <LibreHardwareMonitor checkout>");
  process.exit(2);
}

function git(cwd: string, args: string[]): string {
  const result = Bun.spawnSync({ cmd: ["git", ...args], cwd, stdout: "pipe", stderr: "pipe" });
  if (result.exitCode !== 0) {
    throw new Error(result.stderr.toString().trim() || `git ${args.join(" ")} failed`);
  }
  return result.stdout.toString().trim();
}

const [, , command, checkout] = process.argv;
if (command === "--help" || command === "-h") usage();
if (command !== "status" || !checkout) usage();

const head = git(checkout, ["rev-parse", "HEAD"]);
git(checkout, ["cat-file", "-e", `${LHM.reviewedCommit}^{commit}`]);

console.log(`reviewed: ${LHM.reviewedCommit}`);
console.log(`checkout: ${head}`);
if (head === LHM.reviewedCommit) {
  console.log("status: reviewed revision");
  process.exit(0);
}

const changed = git(checkout, [
  "diff",
  "--name-only",
  `${LHM.reviewedCommit}..${head}`,
  "--",
  ...LHM.relevantSources,
]);

if (!changed) {
  console.log("status: no relevant hardware-source changes");
  process.exit(0);
}

console.log("status: review required");
for (const path of changed.split("\n")) console.log(`changed: ${path}`);
