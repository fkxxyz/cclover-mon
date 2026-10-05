import { createHash } from "node:crypto";
import { mkdir, mkdtemp, readdir, rename, rm, stat } from "node:fs/promises";
import { homedir, tmpdir } from "node:os";
import { dirname, join, posix, resolve, sep } from "node:path";

import type { ReleaseCompanionAssetPlan } from "./plan";

export type SourceAcquisitionPolicy = "allow-network" | "cache-only";

export interface RetainedSourceSubmodule {
  readonly path: string;
  readonly repository: string;
  readonly commit: string;
}

export interface MaterializedRetainedSource {
  readonly resolvedSourceCommit: string;
  readonly submodules: readonly RetainedSourceSubmodule[];
}

export interface SourceRetentionOptions {
  readonly repoRoot: string;
  readonly policy?: SourceAcquisitionPolicy;
  readonly cacheRoot?: string;
}

export interface SourceRetentionCacheContract {
  readonly path: string;
  readonly key: string;
  readonly restoreKeyPrefix: string;
}

const CACHE_NAMESPACE = "release-sources";
const CACHE_SCHEMA = "v1";
const RETAINED_REF = "refs/heads/retained-source";

function run(command: readonly string[], cwd: string): void {
  const result = Bun.spawnSync({
    cmd: [...command],
    cwd,
    env: process.env,
    stdout: "pipe",
    stderr: "pipe",
  });
  if (result.exitCode !== 0) {
    throw new Error(
      `${command.join(" ")} failed with exit code ${result.exitCode}: ${result.stderr.toString().trim()}${result.stdout.toString().trim()}`,
    );
  }
}

function capture(command: readonly string[], cwd: string): string {
  const result = Bun.spawnSync({
    cmd: [...command],
    cwd,
    env: process.env,
    stdout: "pipe",
    stderr: "pipe",
  });
  if (result.exitCode !== 0) {
    throw new Error(
      `${command.join(" ")} failed with exit code ${result.exitCode}: ${result.stderr.toString().trim()}`,
    );
  }
  return result.stdout.toString().trim();
}

function captureRawAllowNoMatch(command: readonly string[], cwd: string): string {
  const result = Bun.spawnSync({
    cmd: [...command],
    cwd,
    env: process.env,
    stdout: "pipe",
    stderr: "pipe",
  });
  if (result.exitCode === 1) return "";
  if (result.exitCode !== 0) {
    throw new Error(
      `${command.join(" ")} failed with exit code ${result.exitCode}: ${result.stderr.toString().trim()}`,
    );
  }
  return result.stdout.toString();
}

function sha256Text(value: string): string {
  return createHash("sha256").update(value).digest("hex");
}

function defaultCacheRoot(): string {
  const depsCache = process.env.CCLOVER_MON_DEPS_CACHE ?? join(homedir(), ".cache", "cclover-mon", "deps");
  return join(depsCache, CACHE_NAMESPACE, CACHE_SCHEMA);
}

function retainedSourceIdentity(plan: ReleaseCompanionAssetPlan): string {
  assertCommit(plan.source.commit, `pinned source commit for ${plan.id}`);
  return sha256Text(`${plan.source.repository}\0${plan.source.commit}`);
}

function resolveCacheRoot(cacheRoot?: string): string {
  return resolve(cacheRoot ?? defaultCacheRoot());
}

function currentRetainedSourceIdentities(plans: readonly ReleaseCompanionAssetPlan[]): readonly string[] {
  return [...new Set(plans.map(retainedSourceIdentity))].sort();
}

export function sourceRetentionCacheContract(
  plans: readonly ReleaseCompanionAssetPlan[],
  options: { readonly cacheRoot?: string } = {},
): SourceRetentionCacheContract {
  const restoreKeyPrefix = `${CACHE_NAMESPACE}-${CACHE_SCHEMA}-`;
  const fingerprint = sha256Text(currentRetainedSourceIdentities(plans).join("\n"));
  return {
    path: resolveCacheRoot(options.cacheRoot),
    key: `${restoreKeyPrefix}${fingerprint}`,
    restoreKeyPrefix,
  };
}

export async function pruneSourceRetentionCache(
  plans: readonly ReleaseCompanionAssetPlan[],
  options: { readonly cacheRoot?: string } = {},
): Promise<void> {
  const root = resolveCacheRoot(options.cacheRoot);
  const retained = new Set(currentRetainedSourceIdentities(plans));
  const entries = await readdir(root, { withFileTypes: true }).catch((error) => {
    if ((error as NodeJS.ErrnoException).code === "ENOENT") return [];
    throw error;
  });
  for (const entry of entries) {
    if (!retained.has(entry.name)) await rm(join(root, entry.name), { recursive: true, force: true });
  }
}

function cacheEntry(plan: ReleaseCompanionAssetPlan, options: SourceRetentionOptions): string {
  return join(resolveCacheRoot(options.cacheRoot), retainedSourceIdentity(plan));
}

function retainedRepositoryPath(entry: string, displayPath: string): string {
  if (displayPath.length === 0) return join(entry, "root.git");
  return join(entry, "submodules", `${sha256Text(displayPath)}.git`);
}

async function directoryState(path: string): Promise<"missing" | "directory" | "invalid"> {
  try {
    return (await stat(path)).isDirectory() ? "directory" : "invalid";
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code === "ENOENT") return "missing";
    throw error;
  }
}

function assertCommit(value: string, description: string): void {
  if (!/^[0-9a-f]{40}$/.test(value)) throw new Error(`invalid ${description}: ${value}`);
}

interface ImmediateSubmodule {
  readonly name: string;
  readonly path: string;
  readonly repository: string;
  readonly commit: string;
}

function parseGitmodulePaths(output: string): readonly { name: string; path: string }[] {
  if (output.length === 0) return [];
  const result: { name: string; path: string }[] = [];
  for (const record of output.split("\0")) {
    if (!record) continue;
    const separator = record.indexOf("\n");
    if (separator === -1) throw new Error(`invalid .gitmodules config record: ${record}`);
    const key = record.slice(0, separator);
    const path = record.slice(separator + 1);
    const match = /^submodule\.(.+)\.path$/.exec(key);
    if (!match?.[1] || path.length === 0) throw new Error(`invalid .gitmodules config record: ${record}`);
    result.push({ name: match[1], path });
  }
  return result;
}

function gitlinks(repository: string, cwd: string): ReadonlyMap<string, string> {
  const output = captureRawAllowNoMatch(["git", "-C", repository, "ls-tree", "-rz", "HEAD"], cwd);
  const result = new Map<string, string>();
  for (const record of output.split("\0")) {
    if (!record) continue;
    const separator = record.indexOf("\t");
    if (separator === -1) throw new Error(`invalid git tree record in ${repository}: ${record}`);
    const metadata = record.slice(0, separator).split(" ");
    const path = record.slice(separator + 1);
    const [mode, type, object, ...extra] = metadata;
    if (mode !== "160000") continue;
    if (type !== "commit" || !object || extra.length > 0) {
      throw new Error(`invalid gitlink record in ${repository}: ${record}`);
    }
    assertCommit(object, `gitlink commit for ${path}`);
    if (result.has(path)) throw new Error(`duplicate gitlink path in ${repository}: ${path}`);
    result.set(path, object);
  }
  return result;
}

function assertRelativeSubmodulePath(repository: string, path: string): void {
  const absolute = resolve(repository, path);
  const prefix = repository.endsWith(sep) ? repository : `${repository}${sep}`;
  if (absolute === repository || !absolute.startsWith(prefix)) {
    throw new Error(`invalid submodule path outside source checkout: ${path}`);
  }
}

function immediateSubmodules(repository: string, cwd: string): readonly ImmediateSubmodule[] {
  const modulesFile = join(repository, ".gitmodules");
  const pathsText = captureRawAllowNoMatch(
    ["git", "-C", repository, "config", "-z", "-f", modulesFile, "--get-regexp", "^submodule\\..*\\.path$"],
    cwd,
  );
  const configured = parseGitmodulePaths(pathsText);
  const links = gitlinks(repository, cwd);
  if (configured.length === 0 && links.size === 0) return [];
  if (configured.length !== links.size) {
    throw new Error(
      `submodule metadata/gitlink count mismatch in ${repository}: ${configured.length} configured, ${links.size} gitlinks`,
    );
  }

  run(["git", "-C", repository, "submodule", "init"], cwd);
  const seen = new Set<string>();
  return configured
    .map(({ name, path }) => {
      if (seen.has(path)) throw new Error(`duplicate submodule path in ${repository}: ${path}`);
      seen.add(path);
      assertRelativeSubmodulePath(repository, path);
      const commit = links.get(path);
      if (!commit) throw new Error(`submodule ${path} in ${repository} has no matching gitlink`);
      const submoduleRepository = capture(
        ["git", "-C", repository, "config", "--get", `submodule.${name}.url`],
        cwd,
      );
      if (submoduleRepository.length === 0) {
        throw new Error(`submodule ${path} in ${repository} has no resolved repository URL`);
      }
      return { name, path, repository: submoduleRepository, commit };
    })
    .sort((left, right) => left.path.localeCompare(right.path));
}

function verifyRetainedRepository(retained: string, expectedCommit: string, cwd: string): void {
  const actualCommit = capture(
    ["git", `--git-dir=${retained}`, "rev-parse", RETAINED_REF],
    cwd,
  );
  if (actualCommit !== expectedCommit) {
    throw new Error(`retained repository ${retained} resolves to ${actualCommit}, expected ${expectedCommit}`);
  }
  run(["git", `--git-dir=${retained}`, "cat-file", "-e", `${expectedCommit}^{commit}`], cwd);
  run(["git", `--git-dir=${retained}`, "fsck", "--full", "--no-dangling"], cwd);
}

function checkoutRetainedRepository(
  retained: string,
  expectedCommit: string,
  upstreamRepository: string,
  destination: string,
  cwd: string,
): void {
  run(["git", "init", "-q", destination], cwd);
  run(["git", "-C", destination, "remote", "add", "origin", upstreamRepository], cwd);
  verifyRetainedRepository(retained, expectedCommit, cwd);
  run(
    ["git", "-C", destination, "fetch", "--quiet", "--depth", "1", retained, RETAINED_REF],
    cwd,
  );
  const fetched = capture(["git", "-C", destination, "rev-parse", "FETCH_HEAD"], cwd);
  if (fetched !== expectedCommit) {
    throw new Error(`retained repository ${retained} resolved to ${fetched}, expected ${expectedCommit}`);
  }
  run(["git", "-C", destination, "checkout", "--quiet", "--detach", expectedCommit], cwd);
  run(["git", "-C", destination, "fsck", "--full", "--no-dangling"], cwd);
  run(["git", "-C", destination, "cat-file", "-e", `${expectedCommit}^{commit}`], cwd);
}

async function materializeRepository(
  entry: string,
  repository: string,
  expectedCommit: string,
  upstreamRepository: string,
  displayPath: string,
  cwd: string,
  submodules: RetainedSourceSubmodule[],
): Promise<void> {
  const retained = retainedRepositoryPath(entry, displayPath);
  await rm(repository, { recursive: true, force: true });
  await mkdir(repository, { recursive: true });
  checkoutRetainedRepository(retained, expectedCommit, upstreamRepository, repository, cwd);

  for (const submodule of immediateSubmodules(repository, cwd)) {
    const childDisplayPath = displayPath.length === 0 ? submodule.path : posix.join(displayPath, submodule.path);
    submodules.push({
      path: childDisplayPath,
      repository: submodule.repository,
      commit: submodule.commit,
    });
    await materializeRepository(
      entry,
      join(repository, submodule.path),
      submodule.commit,
      submodule.repository,
      childDisplayPath,
      cwd,
      submodules,
    );
  }
}

async function materializeEntry(
  plan: ReleaseCompanionAssetPlan,
  entry: string,
  destination: string,
  cwd: string,
): Promise<MaterializedRetainedSource> {
  const submodules: RetainedSourceSubmodule[] = [];
  await materializeRepository(
    entry,
    destination,
    plan.source.commit,
    plan.source.repository,
    "",
    cwd,
    submodules,
  );
  return {
    resolvedSourceCommit: plan.source.commit,
    submodules: submodules.sort((left, right) => left.path.localeCompare(right.path)),
  };
}

async function retainRepository(
  entry: string,
  repository: string,
  expectedCommit: string,
  displayPath: string,
  cwd: string,
): Promise<void> {
  const actualCommit = capture(["git", "-C", repository, "rev-parse", "HEAD"], cwd);
  assertCommit(actualCommit, `source commit for ${displayPath || "root"}`);
  if (actualCommit !== expectedCommit) {
    throw new Error(
      `source ${displayPath || "root"} resolved to ${actualCommit}, expected pinned commit ${expectedCommit}`,
    );
  }

  const retained = retainedRepositoryPath(entry, displayPath);
  await mkdir(dirname(retained), { recursive: true });
  run(["git", "init", "-q", "--bare", retained], cwd);
  run(
    [
      "git",
      `--git-dir=${retained}`,
      "fetch",
      "--quiet",
      "--depth",
      "1",
      "--no-tags",
      repository,
      expectedCommit,
    ],
    cwd,
  );
  const fetched = capture(["git", `--git-dir=${retained}`, "rev-parse", "FETCH_HEAD"], cwd);
  if (fetched !== expectedCommit) {
    throw new Error(`retained repository fetch resolved to ${fetched}, expected ${expectedCommit}`);
  }
  run(["git", `--git-dir=${retained}`, "update-ref", RETAINED_REF, expectedCommit], cwd);
  run(["git", `--git-dir=${retained}`, "symbolic-ref", "HEAD", RETAINED_REF], cwd);
  verifyRetainedRepository(retained, expectedCommit, cwd);

  for (const submodule of immediateSubmodules(repository, cwd)) {
    const childDisplayPath = displayPath.length === 0 ? submodule.path : posix.join(displayPath, submodule.path);
    await retainRepository(
      entry,
      join(repository, submodule.path),
      submodule.commit,
      childDisplayPath,
      cwd,
    );
  }
}

async function populateEntry(
  plan: ReleaseCompanionAssetPlan,
  entry: string,
  options: SourceRetentionOptions,
): Promise<void> {
  const cacheRoot = resolveCacheRoot(options.cacheRoot);
  await mkdir(cacheRoot, { recursive: true });
  const stagingEntry = await mkdtemp(join(cacheRoot, `.${plan.id}-`));
  const acquisitionRoot = await mkdtemp(join(tmpdir(), `cclover-${plan.id}-acquire-`));
  const upstreamCheckout = join(acquisitionRoot, plan.sourceRoot);
  const verificationRoot = await mkdtemp(join(tmpdir(), `cclover-${plan.id}-verify-`));
  const verificationCheckout = join(verificationRoot, plan.sourceRoot);

  try {
    run(
      [
        "git",
        "clone",
        "--quiet",
        "--depth",
        "1",
        "--branch",
        plan.source.ref,
        "--single-branch",
        plan.source.repository,
        upstreamCheckout,
      ],
      options.repoRoot,
    );
    const resolvedSourceCommit = capture(["git", "-C", upstreamCheckout, "rev-parse", "HEAD"], options.repoRoot);
    assertCommit(resolvedSourceCommit, `resolved source commit for ${plan.id}`);
    if (resolvedSourceCommit !== plan.source.commit) {
      throw new Error(
        `source ref ${plan.source.ref} for ${plan.id} resolved to ${resolvedSourceCommit}, expected pinned commit ${plan.source.commit}`,
      );
    }
    const submoduleUpdate = [
      "git",
      ...(plan.source.repository.startsWith("file:") ? ["-c", "protocol.file.allow=always"] : []),
      "-C",
      upstreamCheckout,
      "submodule",
      "update",
      "--init",
      "--recursive",
      "--depth",
      "1",
    ];
    run(submoduleUpdate, options.repoRoot);
    await retainRepository(stagingEntry, upstreamCheckout, plan.source.commit, "", options.repoRoot);
    await materializeEntry(plan, stagingEntry, verificationCheckout, options.repoRoot);

    try {
      await rename(stagingEntry, entry);
    } catch (error) {
      if ((await directoryState(entry)) !== "directory") throw error;
    }
  } finally {
    await rm(stagingEntry, { recursive: true, force: true });
    await rm(acquisitionRoot, { recursive: true, force: true });
    await rm(verificationRoot, { recursive: true, force: true });
  }
}

export async function materializeRetainedSource(
  plan: ReleaseCompanionAssetPlan,
  destination: string,
  options: SourceRetentionOptions,
): Promise<MaterializedRetainedSource> {
  const policy = options.policy ?? "allow-network";
  const entry = cacheEntry(plan, options);
  const state = await directoryState(entry);
  if (state === "invalid") {
    console.error(`source retention ${plan.id}: corrupt`);
    throw new Error(`retained source cache corrupt for ${plan.id}: cache entry is not a directory: ${entry}`);
  }
  if (state === "missing") {
    if (policy === "cache-only") {
      console.error(`source retention ${plan.id}: miss (cache-only)`);
      throw new Error(`retained source cache miss for ${plan.id}: ${entry}`);
    }
    console.error(`source retention ${plan.id}: miss; acquiring upstream`);
    await populateEntry(plan, entry, options);
  }

  try {
    const materialized = await materializeEntry(plan, entry, destination, options.repoRoot);
    if (state === "directory") console.error(`source retention ${plan.id}: hit`);
    return materialized;
  } catch (error) {
    console.error(`source retention ${plan.id}: corrupt`);
    const message = error instanceof Error ? error.message : String(error);
    throw new Error(`retained source cache corrupt for ${plan.id}: ${message}`);
  }
}
