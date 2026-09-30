import { mkdir, readFile, rm, writeFile } from "node:fs/promises";
import { homedir } from "node:os";
import { dirname, join } from "node:path";
import { LINUX_HWMON } from "./deps/linux-hwmon";

const cacheRoot = process.env.CCLOVER_MON_DEPS_CACHE ?? join(homedir(), ".cache", "cclover-mon", "deps");
const cache = join(cacheRoot, "linux-kernel");
const vendorRoot = join(import.meta.dir, "vendor", "linux");

function run(command: string[], cwd?: string): string {
  const result = Bun.spawnSync({ cmd: command, cwd, stdout: "pipe", stderr: "pipe" });
  if (result.exitCode !== 0) {
    throw new Error(`${command.join(" ")} failed:\n${result.stderr.toString()}${result.stdout.toString()}`);
  }
  return result.stdout.toString().trim();
}

async function sha256(path: string): Promise<string> {
  const hasher = new Bun.CryptoHasher("sha256");
  hasher.update(await readFile(path));
  return hasher.digest("hex");
}

await mkdir(dirname(cache), { recursive: true });
if (!(await Bun.file(join(cache, ".git", "HEAD")).exists())) {
  await rm(cache, { recursive: true, force: true });
  run([
    "git",
    "clone",
    "--filter=blob:none",
    "--no-checkout",
    "--depth=1",
    "--single-branch",
    "--branch",
    LINUX_HWMON.ref,
    LINUX_HWMON.remote,
    cache,
  ]);
}
run(["git", "remote", "set-url", "origin", LINUX_HWMON.remote], cache);
run(["git", "sparse-checkout", "init", "--no-cone"], cache);
run(["git", "sparse-checkout", "set", ...LINUX_HWMON.files.map((file) => file.path)], cache);
run(["git", "fetch", "--depth=1", "origin", LINUX_HWMON.commit], cache);
run(["git", "checkout", "--detach", LINUX_HWMON.commit], cache);
const head = run(["git", "rev-parse", "HEAD"], cache);
if (head !== LINUX_HWMON.commit) throw new Error(`expected ${LINUX_HWMON.commit}, got ${head}`);

for (const file of LINUX_HWMON.files) {
  const source = join(cache, file.path);
  const actual = await sha256(source);
  if (actual !== file.sha256) throw new Error(`digest mismatch for ${file.path}: expected ${file.sha256}, got ${actual}`);
  const destination = join(vendorRoot, file.path.replace(/^drivers\//, ""));
  await mkdir(dirname(destination), { recursive: true });
  await writeFile(destination, await readFile(source));
}
console.log(`synced Linux hwmon ${LINUX_HWMON.ref} (${LINUX_HWMON.commit}) into ${vendorRoot}`);
