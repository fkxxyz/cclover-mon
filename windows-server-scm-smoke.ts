import { existsSync } from "node:fs";
import { join, resolve } from "node:path";

import { PAWNIO } from "./deps/pawnio";

function fail(message: string): never {
  console.error(`windows-server-scm-smoke: ${message}`);
  process.exit(1);
}

if (process.platform !== "win32") {
  fail("this validation must run on a real Windows host");
}

const executable = resolve(join("target", "release", "cclover-mon-server.exe"));
if (!existsSync(executable)) fail(`release executable not found: ${executable}`);

const script = resolve(join("tools", "windows-validation", "server-scm-smoke.ps1"));
const result = Bun.spawnSync({
  cmd: [
    "powershell.exe",
    "-NoLogo",
    "-NoProfile",
    "-NonInteractive",
    "-ExecutionPolicy",
    "Bypass",
    "-File",
    script,
    "-ExecutablePath",
    executable,
    "-MinimumPawnIoVersion",
    PAWNIO.driver.version,
  ],
  stdout: "inherit",
  stderr: "inherit",
});

process.exitCode = result.exitCode ?? 1;
