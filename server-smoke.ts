const SERVER_PACKAGE = "cclover-server";
const SERVER_BINARY = "target/release/cclover-mon-server";
const ADDRESS = "127.0.0.1:19847";

function run(command: string[]): string {
  const result = Bun.spawnSync({ cmd: command, stdout: "pipe", stderr: "inherit" });
  if (result.exitCode !== 0) process.exit(result.exitCode);
  return new TextDecoder().decode(result.stdout);
}

function assertAbsent(text: string, forbidden: readonly string[], context: string): void {
  for (const value of forbidden) {
    if (text.includes(value)) throw new Error(`${context} unexpectedly contains ${value}`);
  }
}

const dependencyTree = run(["cargo", "tree", "--locked", "-p", SERVER_PACKAGE]);
assertAbsent(dependencyTree, ["cclover-desktop", "cclover-tui"], "server dependency tree");

if (process.platform === "linux") {
  const dynamicLibraries = run(["ldd", SERVER_BINARY]);
  assertAbsent(
    dynamicLibraries,
    ["libcairo", "libX11", "libXext", "libXrender", "libwayland-client", "libgio-2.0"],
    "server dynamic dependencies",
  );
}

const server = Bun.spawn({
  cmd: [SERVER_BINARY, "--bind", ADDRESS],
  env: { ...process.env, DISPLAY: "", WAYLAND_DISPLAY: "" },
  stdout: "inherit",
  stderr: "inherit",
});

async function waitFor(path: string, expectedStatus: number): Promise<void> {
  const deadline = Date.now() + 10_000;
  let lastStatus: number | undefined;
  while (Date.now() < deadline) {
    try {
      const response = await fetch(`http://${ADDRESS}${path}`);
      lastStatus = response.status;
      if (response.status === expectedStatus) return;
    } catch {
      // Listener may not be ready yet.
    }
    await Bun.sleep(100);
  }
  throw new Error(`${path} did not reach HTTP ${expectedStatus}; last status=${lastStatus ?? "none"}`);
}

try {
  await waitFor("/healthz", 200);
  await waitFor("/readyz", 200);
  const state = await fetch(`http://${ADDRESS}/api/v1/state`);
  if (state.status !== 200) throw new Error(`/api/v1/state returned HTTP ${state.status}`);
} finally {
  server.kill("SIGTERM");
}

const exitCode = await server.exited;
if (exitCode !== 0) throw new Error(`server exited with code ${exitCode} after SIGTERM`);
