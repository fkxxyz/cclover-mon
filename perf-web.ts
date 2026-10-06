export interface ActiveWebWorkloadConfig {
  executable: string;
  updates: number;
  onActive?: (info: ActiveWebWorkloadActive) => void;
}

export interface ActiveWebWorkloadActive {
  pid: number;
  url: string;
}

export interface ActiveWebCpuTime {
  userMicros: number;
  systemMicros: number;
  totalMicros: number;
}

export interface ActiveWebWorkloadResult {
  cpuTime: ActiveWebCpuTime;
  updates: number;
}

interface StartedProcess {
  child: ReturnType<typeof Bun.spawn>;
  ready: Promise<string>;
  stderrDone: Promise<string>;
}

const MAX_DIAGNOSTIC_STDERR = 32 * 1024;
const STARTUP_TIMEOUT_MS = 10_000;
const PROGRESS_TIMEOUT_MS = 10_000;

type SseProgress = "active" | "update" | "done";

export class ActiveWebSseCounter {
  private eventHasData = false;
  private activated = false;
  private remaining: number;

  constructor(updates: number) {
    this.remaining = updates;
  }

  consumeLine(line: string): SseProgress | null {
    if (line.startsWith("data:")) {
      this.eventHasData = true;
      return null;
    }
    if (line !== "" || !this.eventHasData) return null;

    this.eventHasData = false;
    if (!this.activated) {
      this.activated = true;
      return "active";
    }

    this.remaining -= 1;
    return this.remaining === 0 ? "done" : "update";
  }
}

export function parseProductionListenUrl(line: string): string | null {
  const match = line.match(
    /^cclover-mon: HTTP monitor listening on (http:\/\/127\.0\.0\.1:\d+)$/,
  );
  return match?.[1] ?? null;
}

async function withTimeout<T>(
  promise: Promise<T>,
  timeoutMs: number,
  message: string,
): Promise<T> {
  let timer: ReturnType<typeof setTimeout> | undefined;
  const timeout = new Promise<never>((_, reject) => {
    timer = setTimeout(() => reject(new Error(message)), timeoutMs);
  });
  try {
    return await Promise.race([promise, timeout]);
  } finally {
    if (timer !== undefined) clearTimeout(timer);
  }
}

function positiveInteger(option: string, value: string | undefined): number {
  const parsed = value === undefined ? NaN : Number(value);
  if (!Number.isSafeInteger(parsed) || parsed <= 0) {
    throw new Error(`${option} requires a positive integer`);
  }
  return parsed;
}

export function parseActiveWebArgs(args: readonly string[]): {
  executable: string;
  updates: number;
} {
  let executable: string | undefined;
  let updates: number | undefined;

  for (let index = 0; index < args.length; index += 1) {
    const argument = args[index]!;
    switch (argument) {
      case "--executable":
        executable = args[++index];
        if (!executable) throw new Error("--executable requires a path");
        break;
      case "--updates":
        updates = positiveInteger("--updates", args[++index]);
        break;
      default:
        throw new Error(`unexpected argument: ${argument}`);
    }
  }

  if (!executable) throw new Error("--executable is required");
  if (updates === undefined) throw new Error("--updates is required");
  return { executable, updates };
}

function safeMicros(value: bigint, field: string): number {
  const converted = Number(value);
  if (!Number.isSafeInteger(converted) || converted < 0) {
    throw new Error(`invalid active-Web child ${field} CPU time: ${value}`);
  }
  return converted;
}

function boundedDiagnostics(current: string, addition: string): string {
  const combined = current + addition;
  return combined.length <= MAX_DIAGNOSTIC_STDERR
    ? combined
    : combined.slice(combined.length - MAX_DIAGNOSTIC_STDERR);
}

function observeStderr(
  stderr: ReadableStream<Uint8Array>,
): { ready: Promise<string>; done: Promise<string> } {
  let resolveReady!: (url: string) => void;
  let rejectReady!: (error: Error) => void;
  let readySettled = false;
  const ready = new Promise<string>((resolve, reject) => {
    resolveReady = resolve;
    rejectReady = reject;
  });

  const done = (async () => {
    const reader = stderr.getReader();
    const decoder = new TextDecoder();
    let pending = "";
    let diagnostics = "";

    const consumeLine = (line: string) => {
      diagnostics = boundedDiagnostics(diagnostics, `${line}\n`);
      const url = parseProductionListenUrl(line);
      if (url && !readySettled) {
        readySettled = true;
        resolveReady(url);
      }
    };

    try {
      while (true) {
        const { value, done: streamDone } = await reader.read();
        if (streamDone) break;
        pending += decoder.decode(value, { stream: true });
        let newline: number;
        while ((newline = pending.indexOf("\n")) >= 0) {
          const line = pending.slice(0, newline).replace(/\r$/, "");
          pending = pending.slice(newline + 1);
          consumeLine(line);
        }
      }
      pending += decoder.decode();
      if (pending) consumeLine(pending.replace(/\r$/, ""));
      if (!readySettled) {
        readySettled = true;
        rejectReady(new Error("production HTTP server exited before reporting its listen address"));
      }
      return diagnostics;
    } finally {
      reader.releaseLock();
    }
  })();

  return { ready, done };
}

function startProductionWeb(executable: string): StartedProcess {
  const child = Bun.spawn(
    [executable, "--http", "--http-bind", "127.0.0.1:0"],
    {
      stdin: "ignore",
      stdout: "ignore",
      stderr: "pipe",
    },
  );
  const observed = observeStderr(child.stderr);
  return {
    child,
    ready: observed.ready,
    stderrDone: observed.done,
  };
}

async function consumeSseUpdates(
  url: string,
  updates: number,
  signal: AbortSignal,
  onActive?: () => void,
): Promise<void> {
  const response = await withTimeout(
    fetch(`${url}/events`, {
      headers: { Accept: "text/event-stream" },
      signal,
    }),
    PROGRESS_TIMEOUT_MS,
    "production SSE endpoint did not respond before timeout",
  );
  if (!response.ok) {
    throw new Error(`production SSE endpoint returned HTTP ${response.status}`);
  }
  if (!response.body) {
    throw new Error("production SSE endpoint returned no body");
  }

  const reader = response.body.getReader();
  const decoder = new TextDecoder();
  const counter = new ActiveWebSseCounter(updates);
  let pending = "";
  let progressDeadline = Date.now() + PROGRESS_TIMEOUT_MS;

  try {
    while (true) {
      const remainingMs = Math.max(1, progressDeadline - Date.now());
      const { value, done } = await withTimeout(
        reader.read(),
        remainingMs,
        "active Web workload made no dashboard progress before timeout",
      );
      if (done) {
        throw new Error("production SSE stream closed before fixed Web work completed");
      }
      pending += decoder.decode(value, { stream: true });
      let newline: number;
      while ((newline = pending.indexOf("\n")) >= 0) {
        const line = pending.slice(0, newline).replace(/\r$/, "");
        pending = pending.slice(newline + 1);
        const progress = counter.consumeLine(line);
        if (progress === null) continue;
        progressDeadline = Date.now() + PROGRESS_TIMEOUT_MS;
        if (progress === "active") onActive?.();
        if (progress === "done") return;
      }
    }
  } finally {
    await reader.cancel().catch(() => {});
    reader.releaseLock();
  }
}

async function terminateProductionProcess(
  child: ReturnType<typeof Bun.spawn>,
): Promise<number> {
  try {
    child.kill();
  } catch {
    // Process may already have exited; child.exited remains the authority.
  }
  return await child.exited;
}

export async function runActiveWebWorkload(
  config: ActiveWebWorkloadConfig,
): Promise<ActiveWebWorkloadResult> {
  if (!Number.isSafeInteger(config.updates) || config.updates <= 0) {
    throw new Error("updates must be a positive integer");
  }

  const started = startProductionWeb(config.executable);
  const controller = new AbortController();
  let completed = false;

  try {
    const readiness = await withTimeout(
      Promise.race([
        started.ready.then((url) => ({ kind: "ready" as const, url })),
        started.child.exited.then((code) => ({ kind: "exit" as const, code })),
      ]),
      STARTUP_TIMEOUT_MS,
      "production Web process did not report its listen address before timeout",
    );
    if (readiness.kind === "exit") {
      const stderr = await started.stderrDone;
      const detail = stderr.trim() ? `\n${stderr.trim()}` : "";
      throw new Error(
        `production Web process exited with ${readiness.code} before readiness${detail}`,
      );
    }

    const work = consumeSseUpdates(
      readiness.url,
      config.updates,
      controller.signal,
      () => config.onActive?.({ pid: started.child.pid, url: readiness.url }),
    );
    const outcome = await Promise.race([
      work.then(() => ({ kind: "complete" as const })),
      started.child.exited.then((code) => ({ kind: "exit" as const, code })),
    ]);
    if (outcome.kind === "exit") {
      controller.abort();
      const stderr = await started.stderrDone;
      const detail = stderr.trim() ? `\n${stderr.trim()}` : "";
      throw new Error(
        `production Web process exited with ${outcome.code} before fixed work completed${detail}`,
      );
    }

    controller.abort();
    await terminateProductionProcess(started.child);
    await started.stderrDone;
    const usage = started.child.resourceUsage().cpuTime;
    completed = true;
    return {
      cpuTime: {
        userMicros: safeMicros(usage.user, "user"),
        systemMicros: safeMicros(usage.system, "system"),
        totalMicros: safeMicros(usage.total, "total"),
      },
      updates: config.updates,
    };
  } finally {
    controller.abort();
    if (!completed) {
      await terminateProductionProcess(started.child).catch(() => {});
      await started.stderrDone.catch(() => {});
    }
  }
}

function usage(): string {
  return [
    "usage:",
    "  bun perf-web.ts --executable <cclover-mon> --updates <count>",
  ].join("\n");
}

if (import.meta.main) {
  try {
    const config = parseActiveWebArgs(process.argv.slice(2));
    const result = await runActiveWebWorkload({
      ...config,
      onActive: ({ pid, url }) => {
        console.error(`active Web workload: pid ${pid}, ${url}/events`);
      },
    });
    console.log(
      `completed ${result.updates} active Web updates; child CPU ${(
        result.cpuTime.totalMicros / 1_000
      ).toFixed(3)} ms`,
    );
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    console.error(`\n${usage()}`);
    process.exitCode = 1;
  }
}
