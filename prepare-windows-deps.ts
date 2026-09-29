import { mkdir, readFile, rm, copyFile, writeFile, readdir } from "node:fs/promises";
import { homedir, tmpdir } from "node:os";
import { basename, join } from "node:path";
import { PAWNIO } from "./deps/pawnio";

const cacheRoot = process.env.CCLOVER_MON_DEPS_CACHE ?? join(homedir(), ".cache", "cclover-mon", "deps");
const pawnioRoot = join(cacheRoot, "pawnio");
const sourceDeclaration = new URL("./deps/pawnio.ts", import.meta.url);
const declarationBytes = await readFile(sourceDeclaration);

await mkdir(pawnioRoot, { recursive: true });

async function sha256File(path: string): Promise<string> {
  const hasher = new Bun.CryptoHasher("sha256");
  const file = Bun.file(path);
  const reader = file.stream().getReader();
  for (;;) {
    const { done, value } = await reader.read();
    if (done) break;
    hasher.update(value);
  }
  return hasher.digest("hex");
}

async function verify(path: string, expected: string): Promise<void> {
  const actual = await sha256File(path);
  if (actual !== expected) throw new Error(`digest mismatch for ${path}: expected ${expected}, got ${actual}`);
}

async function download(url: string, expectedSha256: string, path: string): Promise<void> {
  try {
    await verify(path, expectedSha256);
    return;
  } catch {
    // Missing or stale cache entry: fetch the pinned artifact again.
  }
  const response = await fetch(url, { redirect: "follow" });
  if (!response.ok) throw new Error(`download failed ${response.status} ${response.statusText}: ${url}`);
  const bytes = new Uint8Array(await response.arrayBuffer());
  await writeFile(path, bytes);
  await verify(path, expectedSha256);
}

function run(command: string[], cwd: string): void {
  const result = Bun.spawnSync({ cmd: command, cwd, stdout: "pipe", stderr: "pipe" });
  if (result.exitCode !== 0) {
    throw new Error(`${command.join(" ")} failed:\n${result.stderr.toString()}${result.stdout.toString()}`);
  }
}

async function extractByDigest(
  directory: string,
  expected: { name: string; sha256: string },
  output: string,
): Promise<void> {
  for (const entry of await readdir(directory)) {
    const path = join(directory, entry);
    try {
      if ((await sha256File(path)) === expected.sha256) {
        await copyFile(path, output);
        await verify(output, expected.sha256);
        return;
      }
    } catch {
      // Ignore directories and unreadable entries.
    }
  }
  throw new Error(`verified ${expected.name} (${expected.sha256}) was not found in extracted artifact`);
}

const downloads = join(pawnioRoot, "downloads");
await mkdir(downloads, { recursive: true });
const installer = join(downloads, basename(new URL(PAWNIO.driver.installerUrl).pathname));
const modulesArchive = join(downloads, basename(new URL(PAWNIO.modules.archiveUrl).pathname));
await download(PAWNIO.driver.installerUrl, PAWNIO.driver.installerSha256, installer);
await download(PAWNIO.modules.archiveUrl, PAWNIO.modules.archiveSha256, modulesArchive);

const staging = await Bun.file(join(tmpdir(), `cclover-mon-pawnio-${process.pid}`)).exists()
  ? join(tmpdir(), `cclover-mon-pawnio-${process.pid}-${Date.now()}`)
  : join(tmpdir(), `cclover-mon-pawnio-${process.pid}`);
await mkdir(staging, { recursive: true });

try {
  const pe = join(staging, "pe");
  const cab = join(staging, "cab");
  const modules = join(staging, "modules");
  await mkdir(pe);
  await mkdir(cab);
  await mkdir(modules);

  run(["7z", "e", "-y", installer], pe);
  const cabPath = join(staging, "PawnIO-driver.cab");
  await extractByDigest(
    pe,
    { name: "embedded PawnIO driver cabinet", sha256: PAWNIO.driver.embeddedCabSha256 },
    cabPath,
  );
  run(["7z", "e", "-y", "-aou", cabPath], cab);
  run(["7z", "e", "-y", modulesArchive, PAWNIO.modules.required.intelMsr.name, "COPYING"], modules);

  const x64 = join(pawnioRoot, "x86_64");
  const x86 = join(pawnioRoot, "x86");
  const x64Driver = join(x64, "driver");
  await mkdir(x64Driver, { recursive: true });
  await mkdir(join(x64, "modules"), { recursive: true });
  await mkdir(join(x86, "modules"), { recursive: true });

  await extractByDigest(cab, PAWNIO.driver.productionAmd64.inf, join(x64Driver, "PawnIO.inf"));
  await extractByDigest(cab, PAWNIO.driver.productionAmd64.sys, join(x64Driver, "PawnIO.sys"));
  await extractByDigest(cab, PAWNIO.driver.productionAmd64.cat, join(x64Driver, "PawnIO.cat"));

  const intelMsr = join(modules, PAWNIO.modules.required.intelMsr.name);
  await verify(intelMsr, PAWNIO.modules.required.intelMsr.sha256);
  await copyFile(intelMsr, join(x64, "modules", "IntelMSR.bin"));
  await copyFile(intelMsr, join(x86, "modules", "IntelMSR.bin"));

  const copying = join(modules, "COPYING");
  await copyFile(copying, join(pawnioRoot, "PawnIO.Modules.COPYING"));
  await writeFile(join(pawnioRoot, "declaration.ts"), declarationBytes);
  await writeFile(
    join(pawnioRoot, "runtime.meta"),
    `driver_version=${PAWNIO.driver.version}\nmin_windows_build=${PAWNIO.driver.minWindowsBuild}\n`,
  );

  console.log(`prepared PawnIO ${PAWNIO.driver.version} + modules ${PAWNIO.modules.version} in ${pawnioRoot}`);
} finally {
  await rm(staging, { recursive: true, force: true });
}
