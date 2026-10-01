import { closeSync, mkdtempSync, openSync, rmSync, writeSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

function fail(message: string): never {
  console.error(`windows-etw-disk-smoke: ${message}`);
  process.exit(1);
}

if (process.platform !== "win32") {
  fail("this validation must run on a real Windows host");
}

const executable = join(process.cwd(), "target", "release", "cclover-mon.exe");
const directory = mkdtempSync(join(tmpdir(), "cclover-etw-disk-"));
const path = join(directory, "workload.bin");
const file = openSync(path, "w");
const payload = Buffer.alloc(1024 * 1024, 0x5a);

try {
  // Keep this handle open before the probe starts. A passing smoke therefore exercises the
  // FileIo rundown seed path used for files that predate cclover-mon's production ETW session.
  writeSync(file, payload);

  const probe = Bun.spawn([executable, "probe", "disk-attribution", "--raw"], {
    stdout: "pipe",
    stderr: "pipe",
  });

  const deadline = Date.now() + 2500;
  while (Date.now() < deadline) {
    writeSync(file, payload);
    await Bun.sleep(10);
  }

  const [exitCode, stdout, stderr] = await Promise.all([
    probe.exited,
    new Response(probe.stdout).text(),
    new Response(probe.stderr).text(),
  ]);

  if (exitCode !== 0) {
    fail(`probe exited with ${exitCode}\n${stderr || stdout}`);
  }

  const rows = stdout
    .split(/\r?\n/)
    .filter((line) => line.includes(`pid=${process.pid} `));
  const attributed = rows.some((line) => {
    const match = /write_bytes=(\d+)/.exec(line);
    return match !== null && BigInt(match[1]) > 0n;
  });

  if (!attributed) {
    fail(
      `production probe did not report nonzero disk writes for workload PID ${process.pid}\n${stdout}${stderr}`,
    );
  }

  console.log(`windows-etw-disk-smoke: attributed nonzero writes to PID ${process.pid}`);
} finally {
  closeSync(file);
  rmSync(directory, { recursive: true, force: true });
}
