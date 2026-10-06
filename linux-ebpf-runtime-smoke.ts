import { mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { createServer } from "node:net";
import { join } from "node:path";

import { localElevatedCommand } from "./tools/validation/linux-elevation";

function fail(message: string): never {
  console.error(`linux-ebpf-runtime-smoke: ${message}`);
  process.exit(1);
}

if (process.platform !== "linux") {
  fail("this validation must run on a real Linux host");
}

const root = process.cwd();
const executable = join(root, "target", "release", "cclover-mon");
const validationSources = join(root, "tools", "linux-validation");
const outputDirectory = join(root, "target", "validation");
const batchShim = join(outputDirectory, "libbpf-batch-unsupported.so");
const lifecycleObserver = join(outputDirectory, "libbpf-attribution-lifecycle-observer.so");
const printPolicyObserver = join(outputDirectory, "libbpf-print-policy-observer.so");
mkdirSync(outputDirectory, { recursive: true });

function compileSharedObject(sourceName: string, output: string, extraArgs: string[] = []): void {
  const source = join(validationSources, sourceName);
  const compile = Bun.spawnSync({
    cmd: [
      "clang",
      "-shared",
      "-fPIC",
      "-O2",
      "-Wall",
      "-Wextra",
      "-Werror",
      source,
      "-o",
      output,
      ...extraArgs,
    ],
    stdout: "pipe",
    stderr: "pipe",
  });
  if (compile.exitCode !== 0) {
    fail(new TextDecoder().decode(compile.stderr) || `${sourceName} compilation exited ${compile.exitCode}`);
  }
}

compileSharedObject("libbpf-batch-unsupported.c", batchShim);
compileSharedObject("libbpf-attribution-lifecycle-observer.c", lifecycleObserver, ["-ldl"]);
compileSharedObject("libbpf-print-policy-observer.c", printPolicyObserver, ["-ldl"]);

function readTrace(path: string): string {
  try {
    return readFileSync(path, "utf8");
  } catch {
    return "";
  }
}

function hasTraceEvent(trace: string, event: string, mapName: string, pid?: number): boolean {
  const expected = pid === undefined ? `${event}\t${mapName}` : `${event}\t${mapName}\t${pid}`;
  return trace.split("\n").some((line) => line === expected);
}

async function waitForTrace(
  path: string,
  predicate: (trace: string) => boolean,
  timeoutMs: number,
): Promise<string> {
  const deadline = Date.now() + timeoutMs;
  let trace = readTrace(path);
  while (!predicate(trace) && Date.now() < deadline) {
    await Bun.sleep(25);
    trace = readTrace(path);
  }
  return trace;
}

async function runChild(script: string): Promise<number> {
  const child = Bun.spawn({
    cmd: [process.execPath, "-e", script],
    stdout: "ignore",
    stderr: "pipe",
  });
  const pid = child.pid;
  const exitCode = await child.exited;
  const stderr = await new Response(child.stderr).text();
  if (exitCode !== 0) {
    throw new Error(`traffic helper PID ${pid} exited ${exitCode}${stderr ? `: ${stderr.trim()}` : ""}`);
  }
  return pid;
}

async function generateDiskTraffic(): Promise<number> {
  const path = join(outputDirectory, `ebpf-lifecycle-${process.pid}.bin`);
  const script = `
    const fs = require("node:fs");
    const fd = fs.openSync(${JSON.stringify(path)}, "w");
    const payload = Buffer.alloc(1024 * 1024, 0x5a);
    fs.writeSync(fd, payload);
    fs.fsyncSync(fd);
    fs.closeSync(fd);
  `;
  try {
    return await runChild(script);
  } finally {
    rmSync(path, { force: true });
  }
}

async function generateNetworkTraffic(): Promise<number> {
  const server = createServer((socket) => {
    socket.on("data", () => {});
  });
  await new Promise<void>((resolve, reject) => {
    server.once("error", reject);
    server.listen(0, "127.0.0.1", resolve);
  });

  const address = server.address();
  if (address === null || typeof address === "string") {
    server.close();
    throw new Error("loopback traffic server did not expose an IPv4 port");
  }

  const script = `
    const net = require("node:net");
    const socket = net.createConnection({ host: "127.0.0.1", port: ${address.port} }, () => {
      socket.end(Buffer.alloc(1024 * 1024, 0x5a));
    });
    socket.on("error", (error) => {
      console.error(error);
      process.exitCode = 1;
    });
  `;

  try {
    return await runChild(script);
  } finally {
    await new Promise<void>((resolve) => server.close(() => resolve()));
  }
}

function runDiagnosticOwnershipProof(): void {
  const marker = "libbpf-policy-validation:";
  const run = (debug: string, libbpfLogLevel: string) => {
    const result = Bun.spawnSync({
      cmd: [executable, "probe", "disk-attribution", "--raw"],
      env: {
        ...process.env,
        CCLOVER_MON_DEBUG: debug,
        CCLOVER_MON_DISABLE_EBPF_IO: "1",
        LIBBPF_LOG_LEVEL: libbpfLogLevel,
        LD_PRELOAD: printPolicyObserver,
      },
      stdout: "pipe",
      stderr: "pipe",
    });
    const stdout = new TextDecoder().decode(result.stdout);
    const stderr = new TextDecoder().decode(result.stderr);
    if (result.exitCode !== 0) {
      throw new Error(`diagnostic policy probe exited ${result.exitCode}\n${stderr || stdout}`);
    }
    if (/^status: unavailable$/m.test(stdout) && !/^diagnostic: .+/m.test(stdout)) {
      throw new Error(`diagnostic policy probe lost its typed diagnostic while unavailable\n${stdout}`);
    }
    return stderr;
  };

  const ordinary = run("0", "info");
  if (ordinary.includes(marker)) {
    throw new Error(`ordinary runtime exposed libbpf callback output\n${ordinary}`);
  }

  const development = run("1", "info");
  if (!development.includes(`${marker} warn`) || !development.includes(`${marker} info`)) {
    throw new Error(`development logging did not expose actionable libbpf detail\n${development}`);
  }
  if (development.includes(`${marker} debug`)) {
    throw new Error(`development logging exposed deep libbpf tracing without explicit opt-in\n${development}`);
  }

  const deepDebug = run("1", "debug");
  if (!deepDebug.includes(`${marker} debug`)) {
    throw new Error(`explicit deep libbpf debugging did not expose debug-level detail\n${deepDebug}`);
  }

  console.log("linux-ebpf-runtime-smoke: libbpf diagnostics policy preserves quiet, development, and deep-debug modes");
}

async function runLifecycleProof(
  collector: "disk-attribution" | "network-attribution",
  mapName: "disk_bytes" | "network_bytes",
  generateTraffic: () => Promise<number>,
): Promise<void> {
  const tracePath = join(outputDirectory, `${collector}-lifecycle.trace`);
  writeFileSync(tracePath, "");

  const perf = Bun.spawn({
    cmd: localElevatedCommand(
      "linux-ebpf-runtime",
      [
        executable,
        "perf",
        "collector",
        collector,
        "--samples",
        "4",
      ],
      {
        LD_PRELOAD: lifecycleObserver,
        CCLOVER_MON_EBPF_VALIDATION_TRACE: tracePath,
      },
    ),
    stdout: "pipe",
    stderr: "pipe",
  });

  const readyTrace = await waitForTrace(
    tracePath,
    (trace) => hasTraceEvent(trace, "scan", mapName),
    2_500,
  );

  let helperPid: number | undefined;
  let helperFailure: unknown;
  if (hasTraceEvent(readyTrace, "scan", mapName)) {
    try {
      helperPid = await generateTraffic();
    } catch (error) {
      helperFailure = error;
    }
  }

  let proofTrace = readTrace(tracePath);
  if (helperPid !== undefined) {
    proofTrace = await waitForTrace(
      tracePath,
      (trace) =>
        hasTraceEvent(trace, "seen", mapName, helperPid) &&
        hasTraceEvent(trace, "delete", mapName, helperPid),
      2_500,
    );
  }

  const exitCode = await perf.exited;
  const stdout = await new Response(perf.stdout).text();
  const stderr = await new Response(perf.stderr).text();
  if (exitCode !== 0) {
    throw new Error(`${collector} perf workload exited ${exitCode}\n${stderr || stdout}`);
  }
  if (!hasTraceEvent(readyTrace, "scan", mapName)) {
    throw new Error(`${collector} never reached a real ${mapName} map scan\n${stderr || stdout}`);
  }
  if (helperFailure !== undefined) {
    throw helperFailure;
  }
  if (helperPid === undefined) {
    throw new Error(`${collector} traffic helper did not start`);
  }
  if (!hasTraceEvent(proofTrace, "seen", mapName, helperPid)) {
    throw new Error(
      `${collector} never observed attribution state for short-lived PID ${helperPid}; ` +
        "the controlled workload may not have crossed the expected eBPF hook",
    );
  }
  if (!hasTraceEvent(proofTrace, "delete", mapName, helperPid)) {
    throw new Error(
      `${collector} observed attribution state for short-lived PID ${helperPid} but did not retire it`,
    );
  }

  console.log(
    `linux-ebpf-runtime-smoke: ${collector} retired production map state for short-lived PID ${helperPid}`,
  );
}

async function main(): Promise<void> {
  runDiagnosticOwnershipProof();
  await runLifecycleProof("disk-attribution", "disk_bytes", generateDiskTraffic);
  await runLifecycleProof("network-attribution", "network_bytes", generateNetworkTraffic);

  const expectedDiagnostic = "BPF batch map lookup unsupported by kernel/map";
  for (const collector of ["disk-attribution", "network-attribution"] as const) {
    const result = Bun.spawnSync({
      cmd: localElevatedCommand(
        "linux-ebpf-runtime",
        [
          executable,
          "probe",
          collector,
          "--raw",
        ],
        { LD_PRELOAD: batchShim },
      ),
      stdout: "pipe",
      stderr: "pipe",
    });
    const stdout = new TextDecoder().decode(result.stdout);
    const stderr = new TextDecoder().decode(result.stderr);
    if (result.exitCode !== 0) {
      throw new Error(`${collector} probe exited ${result.exitCode}\n${stderr || stdout}`);
    }
    if (!/^status: (ok|degraded)$/m.test(stdout)) {
      throw new Error(`${collector} did not remain available through scalar fallback\n${stdout}${stderr}`);
    }
    if (!stdout.includes(expectedDiagnostic)) {
      throw new Error(`${collector} did not report the forced scalar fallback\n${stdout}${stderr}`);
    }
    console.log(`linux-ebpf-runtime-smoke: ${collector} used production scalar fallback`);
  }
}

main().catch((error) => fail(error instanceof Error ? error.message : String(error)));
