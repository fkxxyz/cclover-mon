import { resolve } from "node:path";
import { createHash } from "node:crypto";

export type ComparisonSide = "baseline" | "candidate";

export type PerfWorkload =
  | { kind: "headless" }
  | { kind: "collector"; collector: string };

export interface ComparisonConfig {
  baseline: string;
  candidate: string;
  workload: PerfWorkload;
  samples: number;
  pairs: number;
}

export interface CpuTime {
  userMicros: number;
  systemMicros: number;
  totalMicros: number;
}

export interface ComparisonPair {
  baseline: CpuTime;
  candidate: CpuTime;
}

export type ComparisonDirection =
  | "candidate consistently lower"
  | "candidate consistently higher"
  | "inconclusive";

export interface ExecutableIdentity {
  path: string;
  sha256: string;
}

export interface MetricSummary {
  baselineMedianMicros: number;
  candidateMedianMicros: number;
  pairedDeltaMedian: number | null;
  pairedDeltaQ1: number | null;
  pairedDeltaQ3: number | null;
}

export interface ComparisonSummary {
  user: MetricSummary;
  system: MetricSummary;
  total: MetricSummary;
  direction: ComparisonDirection;
}

export interface ComparisonResult {
  config: ComparisonConfig;
  baselineIdentity: ExecutableIdentity;
  candidateIdentity: ExecutableIdentity;
  pairResults: readonly ComparisonPair[];
  summary: ComparisonSummary;
}

export interface ComparisonProgress {
  run: number;
  runs: number;
  pair: number;
  pairs: number;
  side: ComparisonSide;
}

const DEFAULT_PAIRS = 6;
const MIN_PAIRS = 6;

function positiveInteger(option: string, value: string | undefined): number {
  const parsed = value === undefined ? NaN : Number(value);
  if (!Number.isSafeInteger(parsed) || parsed <= 0) {
    throw new Error(`${option} requires a positive integer`);
  }
  return parsed;
}

export function parseComparisonArgs(args: readonly string[]): ComparisonConfig {
  let baseline: string | undefined;
  let candidate: string | undefined;
  let samples: number | undefined;
  let pairs = DEFAULT_PAIRS;
  const positional: string[] = [];

  for (let index = 0; index < args.length; index += 1) {
    const argument = args[index]!;
    switch (argument) {
      case "--baseline":
        baseline = args[++index];
        if (!baseline) throw new Error("--baseline requires an executable path");
        break;
      case "--candidate":
        candidate = args[++index];
        if (!candidate) throw new Error("--candidate requires an executable path");
        break;
      case "--samples":
        samples = positiveInteger("--samples", args[++index]);
        break;
      case "--pairs":
        pairs = positiveInteger("--pairs", args[++index]);
        break;
      default:
        if (argument.startsWith("--")) {
          throw new Error(`unexpected argument: ${argument}`);
        }
        positional.push(argument);
        break;
    }
  }

  if (!baseline) throw new Error("--baseline is required");
  if (!candidate) throw new Error("--candidate is required");
  if (samples === undefined) throw new Error("--samples is required for comparable fixed work");
  if (pairs < MIN_PAIRS || pairs % 2 !== 0) {
    throw new Error(`--pairs must be an even integer of at least ${MIN_PAIRS}`);
  }

  let workload: PerfWorkload;
  if (positional.length === 1 && positional[0] === "headless") {
    workload = { kind: "headless" };
  } else if (positional.length === 2 && positional[0] === "collector" && positional[1]) {
    workload = { kind: "collector", collector: positional[1] };
  } else {
    throw new Error("workload must be `headless` or `collector <name>`");
  }

  return {
    baseline: resolve(baseline),
    candidate: resolve(candidate),
    workload,
    samples,
    pairs,
  };
}

export function comparisonSchedule(pairs: number): readonly (readonly [ComparisonSide, ComparisonSide])[] {
  if (pairs < MIN_PAIRS || pairs % 2 !== 0) {
    throw new Error(`pairs must be an even integer of at least ${MIN_PAIRS}`);
  }
  return Array.from({ length: pairs }, (_, index) =>
    index % 2 === 0 ? (["baseline", "candidate"] as const) : (["candidate", "baseline"] as const),
  );
}

export function workloadCommand(
  executable: string,
  workload: PerfWorkload,
  samples: number,
): readonly string[] {
  const workloadArgs =
    workload.kind === "headless"
      ? ["headless"]
      : ["collector", workload.collector];
  return [executable, "perf", ...workloadArgs, "--samples", String(samples)];
}

export async function executableIdentity(path: string): Promise<ExecutableIdentity> {
  const file = Bun.file(path);
  if (!(await file.exists())) {
    throw new Error(`executable does not exist: ${path}`);
  }
  const bytes = await file.arrayBuffer();
  const sha256 = createHash("sha256").update(Buffer.from(bytes)).digest("hex");
  return { path, sha256 };
}

export function verifyExecutableIdentity(
  before: ExecutableIdentity,
  after: ExecutableIdentity,
  side: ComparisonSide,
): void {
  if (before.path !== after.path || before.sha256 !== after.sha256) {
    throw new Error(
      `${side} executable changed during comparison: ${before.path} (${before.sha256} -> ${after.sha256})`,
    );
  }
}

function safeMicros(value: bigint, field: string): number {
  const converted = Number(value);
  if (!Number.isSafeInteger(converted) || converted < 0) {
    throw new Error(`invalid child ${field} CPU time: ${value}`);
  }
  return converted;
}

export async function measureCommand(command: readonly string[]): Promise<CpuTime> {
  const child = Bun.spawn([...command], {
    stdout: "ignore",
    stderr: "pipe",
  });
  const stderrPromise = new Response(child.stderr).text();
  const exitCode = await child.exited;
  const stderr = (await stderrPromise).trim();

  if (exitCode !== 0) {
    const detail = stderr ? `\n${stderr}` : "";
    throw new Error(`command failed with exit ${exitCode}: ${command.join(" ")}${detail}`);
  }

  const cpuTime = child.resourceUsage().cpuTime;
  return {
    userMicros: safeMicros(cpuTime.user, "user"),
    systemMicros: safeMicros(cpuTime.system, "system"),
    totalMicros: safeMicros(cpuTime.total, "total"),
  };
}

function quantile(values: readonly number[], fraction: number): number {
  if (values.length === 0) throw new Error("cannot summarize an empty sample");
  const sorted = [...values].sort((left, right) => left - right);
  const position = (sorted.length - 1) * fraction;
  const lower = Math.floor(position);
  const upper = Math.ceil(position);
  if (lower === upper) return sorted[lower]!;
  const weight = position - lower;
  return sorted[lower]! * (1 - weight) + sorted[upper]! * weight;
}

export function median(values: readonly number[]): number {
  return quantile(values, 0.5);
}

function pairedRelativeDeltas(
  pairs: readonly ComparisonPair[],
  select: (cpu: CpuTime) => number,
): number[] {
  const deltas: number[] = [];
  for (const pair of pairs) {
    const baseline = select(pair.baseline);
    if (baseline === 0) continue;
    deltas.push(select(pair.candidate) / baseline - 1);
  }
  return deltas;
}

function summarizeMetric(
  pairs: readonly ComparisonPair[],
  select: (cpu: CpuTime) => number,
): MetricSummary {
  const deltas = pairedRelativeDeltas(pairs, select);
  return {
    baselineMedianMicros: median(pairs.map((pair) => select(pair.baseline))),
    candidateMedianMicros: median(pairs.map((pair) => select(pair.candidate))),
    pairedDeltaMedian: deltas.length === 0 ? null : quantile(deltas, 0.5),
    pairedDeltaQ1: deltas.length === 0 ? null : quantile(deltas, 0.25),
    pairedDeltaQ3: deltas.length === 0 ? null : quantile(deltas, 0.75),
  };
}

export function summarizePairs(pairs: readonly ComparisonPair[]): ComparisonSummary {
  if (pairs.length < MIN_PAIRS || pairs.length % 2 !== 0) {
    throw new Error(`comparison requires an even number of at least ${MIN_PAIRS} pairs`);
  }
  if (pairs.some((pair) => pair.baseline.totalMicros === 0)) {
    throw new Error("baseline total CPU time must be nonzero for paired comparison");
  }

  const user = summarizeMetric(pairs, (cpu) => cpu.userMicros);
  const system = summarizeMetric(pairs, (cpu) => cpu.systemMicros);
  const total = summarizeMetric(pairs, (cpu) => cpu.totalMicros);
  const totalDeltas = pairedRelativeDeltas(pairs, (cpu) => cpu.totalMicros);
  const direction: ComparisonDirection =
    totalDeltas.every((delta) => delta < 0)
      ? "candidate consistently lower"
      : totalDeltas.every((delta) => delta > 0)
        ? "candidate consistently higher"
        : "inconclusive";

  return { user, system, total, direction };
}

export async function compare(
  config: ComparisonConfig,
  onRunStart?: (progress: ComparisonProgress) => void,
): Promise<ComparisonResult> {
  const pairResults: ComparisonPair[] = [];
  const schedule = comparisonSchedule(config.pairs);
  const baselineIdentity = await executableIdentity(config.baseline);
  const candidateIdentity = await executableIdentity(config.candidate);

  for (const [pairIndex, [first, second]] of schedule.entries()) {
    const measured: Partial<Record<ComparisonSide, CpuTime>> = {};
    for (const [orderIndex, side] of [first, second].entries()) {
      onRunStart?.({
        run: pairIndex * 2 + orderIndex + 1,
        runs: config.pairs * 2,
        pair: pairIndex + 1,
        pairs: config.pairs,
        side,
      });
      const executable = side === "baseline" ? config.baseline : config.candidate;
      measured[side] = await measureCommand(workloadCommand(executable, config.workload, config.samples));
    }
    pairResults.push({
      baseline: measured.baseline!,
      candidate: measured.candidate!,
    });
  }

  verifyExecutableIdentity(baselineIdentity, await executableIdentity(config.baseline), "baseline");
  verifyExecutableIdentity(candidateIdentity, await executableIdentity(config.candidate), "candidate");

  return {
    config,
    baselineIdentity,
    candidateIdentity,
    pairResults,
    summary: summarizePairs(pairResults),
  };
}

function formatMicros(value: number): string {
  return `${(value / 1_000).toFixed(3)} ms`;
}

function formatDelta(value: number | null): string {
  if (value === null) return "n/a";
  const percent = value * 100;
  return `${percent >= 0 ? "+" : ""}${percent.toFixed(2)}%`;
}

function workloadLabel(config: ComparisonConfig): string {
  const workload =
    config.workload.kind === "headless"
      ? "headless"
      : `collector ${config.workload.collector}`;
  return `${workload} --samples ${config.samples}`;
}

export function formatReport(result: ComparisonResult): string {
  const { config, baselineIdentity, candidateIdentity, summary } = result;
  const rows: readonly [string, MetricSummary][] = [
    ["user", summary.user],
    ["system", summary.system],
    ["total", summary.total],
  ];
  const metricLines = rows.map(
    ([name, metric]) =>
      `${name.padEnd(7)} ${formatMicros(metric.baselineMedianMicros).padStart(12)} ${formatMicros(metric.candidateMedianMicros).padStart(12)} ${formatDelta(metric.pairedDeltaMedian).padStart(10)}`,
  );
  const reason =
    summary.direction === "inconclusive"
      ? "\nreason: paired total-CPU deltas do not all agree in direction"
      : "";

  return [
    "cclover performance comparison",
    "",
    `platform:  ${process.platform}`,
    `workload:  ${workloadLabel(config)}`,
    `pairs:     ${config.pairs}`,
    `baseline:  ${config.baseline}`,
    `            sha256 ${baselineIdentity.sha256}`,
    `candidate: ${config.candidate}`,
    `            sha256 ${candidateIdentity.sha256}`,
    "",
    "CPU time          baseline    candidate paired delta",
    ...metricLines,
    "",
    `total paired delta IQR: [${formatDelta(summary.total.pairedDeltaQ1)}, ${formatDelta(summary.total.pairedDeltaQ3)}]`,
    "",
    `result: ${summary.direction}${reason}`,
  ].join("\n");
}

function usage(): string {
  return [
    "usage:",
    "  bun perf-compare.ts --baseline <executable> --candidate <executable> headless --samples <count> [--pairs <even-count>]",
    "  bun perf-compare.ts --baseline <executable> --candidate <executable> collector <name> --samples <count> [--pairs <even-count>]",
  ].join("\n");
}

if (import.meta.main) {
  try {
    const config = parseComparisonArgs(process.argv.slice(2));
    const result = await compare(config, (progress) => {
      console.error(
        `[${progress.run}/${progress.runs}] pair ${progress.pair}/${progress.pairs} ${progress.side}`,
      );
    });
    console.log(formatReport(result));
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    console.error(`\n${usage()}`);
    process.exitCode = 1;
  }
}
