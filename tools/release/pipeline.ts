import { createHash } from "node:crypto";
import {
  copyFile,
  mkdir,
  readFile,
  readdir,
  rm,
  stat,
  utimes,
  writeFile,
} from "node:fs/promises";
import { dirname, join, resolve } from "node:path";

import { createArchive, verifyArchiveContents } from "./archive";
import { resolveBuildContext, validateBuildProvenance, type BuildInvocation } from "./build-context";
import {
  checkLinuxRuntimeAbi,
  validateLinuxReleaseBuildHost,
} from "./linux-runtime-abi";
import {
  RELEASE_ARTIFACTS,
  archiveName,
  binaryFileName,
  stagingDirectoryName,
  validateReleaseTag,
  type BuildProvenance,
  type ReleaseArtifact,
  type ReleaseManifest,
  type ReleaseSummary,
} from "./plan";
import {
  redistributionFulfillmentsForArtifact,
  thirdPartySourcesText,
  validateRedistributionPlan,
  verifyRedistributionSourceAssets,
} from "./redistribution";

const DEFAULT_REPO_ROOT = resolve(import.meta.dir, "../..");

export async function workspaceVersion(repoRoot = DEFAULT_REPO_ROOT): Promise<string> {
  const cargoToml = Bun.TOML.parse(await readFile(join(repoRoot, "Cargo.toml"), "utf8")) as {
    workspace?: { package?: { version?: unknown } };
  };
  const version = cargoToml.workspace?.package?.version;
  if (typeof version !== "string" || version.length === 0) {
    throw new Error("Cargo.toml [workspace.package].version is missing");
  }
  return version;
}

function run(invocation: BuildInvocation, cwd: string): void {
  const result = Bun.spawnSync({
    cmd: [...invocation.command],
    cwd,
    env: invocation.env ? { ...process.env, ...invocation.env } : process.env,
    stdout: "inherit",
    stderr: "inherit",
  });
  if (result.exitCode !== 0) {
    throw new Error(`${invocation.command.join(" ")} failed with exit code ${result.exitCode}`);
  }
}

async function sha256File(path: string): Promise<string> {
  const hash = createHash("sha256");
  const file = Bun.file(path);
  const reader = file.stream().getReader();
  for (;;) {
    const { done, value } = await reader.read();
    if (done) break;
    hash.update(value);
  }
  return hash.digest("hex");
}

async function assertFile(path: string, description: string): Promise<void> {
  let metadata;
  try {
    metadata = await stat(path);
  } catch {
    throw new Error(`${description} is missing: ${path}`);
  }
  if (!metadata.isFile()) throw new Error(`${description} is not a file: ${path}`);
}

export async function assertStaticPackageInputs(
  artifact: ReleaseArtifact,
  repoRoot = DEFAULT_REPO_ROOT,
): Promise<void> {
  validateRedistributionPlan();
  for (const file of artifact.staticFiles) {
    await assertFile(join(repoRoot, file.source), `package input ${file.source}`);
  }
}

async function copyPackageInputs(
  artifact: ReleaseArtifact,
  repoRoot: string,
  stageRoot: string,
): Promise<void> {
  await assertStaticPackageInputs(artifact, repoRoot);
  for (const file of artifact.staticFiles) {
    const destination = join(stageRoot, file.destination);
    await mkdir(dirname(destination), { recursive: true });
    await copyFile(join(repoRoot, file.source), destination);
  }
  if (artifact.thirdPartyPayloads.length > 0) {
    await writeFile(join(stageRoot, "THIRD-PARTY-SOURCES.txt"), thirdPartySourcesText(artifact));
  }
}

async function setStableMtime(path: string): Promise<void> {
  const fixed = new Date("1980-01-01T00:00:00.000Z");
  const metadata = await stat(path);
  if (metadata.isDirectory()) {
    for (const entry of await readdir(path)) await setStableMtime(join(path, entry));
  }
  await utimes(path, fixed, fixed);
}

export async function workspaceCommit(repoRoot = DEFAULT_REPO_ROOT): Promise<string> {
  const result = Bun.spawnSync({ cmd: ["git", "rev-parse", "HEAD"], cwd: repoRoot, stdout: "pipe", stderr: "pipe" });
  if (result.exitCode !== 0) throw new Error(`git rev-parse HEAD failed: ${result.stderr.toString()}`);
  const commit = result.stdout.toString().trim();
  if (!/^[0-9a-f]{40}$/.test(commit)) throw new Error(`invalid git commit identity: ${commit}`);
  return commit;
}

export async function packageBuiltReleaseArtifact(
  artifact: ReleaseArtifact,
  options: {
    repoRoot?: string;
    outDir: string;
    binary: string;
    version: string;
    commit: string;
    provenance: BuildProvenance;
  },
): Promise<ReleaseManifest> {
  const repoRoot = options.repoRoot ?? DEFAULT_REPO_ROOT;
  const outDir = resolve(repoRoot, options.outDir);
  await assertStaticPackageInputs(artifact, repoRoot);
  await assertFile(options.binary, `built binary for ${artifact.id}`);
  validateBuildProvenance(options.provenance, artifact);
  await mkdir(outDir, { recursive: true });

  const stageParent = join(outDir, `.stage-${artifact.id}`);
  const stageName = stagingDirectoryName(artifact, options.version);
  const stageRoot = join(stageParent, stageName);
  await rm(stageParent, { recursive: true, force: true });
  await mkdir(stageRoot, { recursive: true });

  try {
    await copyFile(options.binary, join(stageRoot, binaryFileName(artifact)));
    await copyPackageInputs(artifact, repoRoot, stageRoot);
    await setStableMtime(stageRoot);

    const outputName = archiveName(artifact, options.version);
    const outputPath = join(outDir, outputName);
    await createArchive(artifact, outDir, stageParent, stageName, outputName, repoRoot);
    verifyArchiveContents(artifact, outputPath, options.version, repoRoot);

    const manifest: ReleaseManifest = {
      schemaVersion: 3,
      artifactId: artifact.id,
      product: artifact.product,
      version: options.version,
      target: artifact.target,
      gitCommit: options.commit,
      archive: outputName,
      sha256: await sha256File(outputPath),
      redistributionFulfillments: redistributionFulfillmentsForArtifact(artifact),
      buildProvenance: options.provenance,
    };
    await writeFile(
      join(outDir, `${outputName}.manifest.json`),
      `${JSON.stringify(manifest, null, 2)}\n`,
    );
    return manifest;
  } finally {
    await rm(stageParent, { recursive: true, force: true });
  }
}

export async function buildReleaseArtifact(
  artifact: ReleaseArtifact,
  options: { repoRoot?: string; outDir: string; tag?: string },
): Promise<ReleaseManifest> {
  const repoRoot = options.repoRoot ?? DEFAULT_REPO_ROOT;
  const outDir = resolve(repoRoot, options.outDir);
  const version = await workspaceVersion(repoRoot);
  validateReleaseTag(version, options.tag);
  await assertStaticPackageInputs(artifact, repoRoot);

  if (artifact.platform === "windows") {
    run({ command: ["bun", "prepare-windows-deps.ts"] }, repoRoot);
  }
  const context = await resolveBuildContext(artifact, repoRoot);
  validateLinuxReleaseBuildHost(artifact, context.provenance);
  run(context.invocation, repoRoot);

  const binary = join(
    repoRoot,
    "target",
    artifact.target,
    "dist",
    binaryFileName(artifact),
  );
  if (artifact.platform === "linux") checkLinuxRuntimeAbi(artifact, binary);
  return packageBuiltReleaseArtifact(artifact, {
    repoRoot,
    outDir,
    binary,
    version,
    commit: await workspaceCommit(repoRoot),
    provenance: context.provenance,
  });
}

function validateManifestIdentity(
  manifest: ReleaseManifest,
  expected: ReleaseArtifact,
  version: string,
  commit: string,
): void {
  if (manifest.schemaVersion !== 3) throw new Error(`unsupported manifest schema for ${expected.id}`);
  if (manifest.artifactId !== expected.id) throw new Error(`manifest artifact mismatch for ${expected.id}`);
  if (manifest.product !== expected.product) throw new Error(`manifest product mismatch for ${expected.id}`);
  if (manifest.target !== expected.target) throw new Error(`manifest target mismatch for ${expected.id}`);
  if (manifest.version !== version) throw new Error(`manifest version mismatch for ${expected.id}`);
  if (manifest.gitCommit !== commit) throw new Error(`manifest commit mismatch for ${expected.id}`);
  if (manifest.archive !== archiveName(expected, version)) {
    throw new Error(`manifest archive name mismatch for ${expected.id}`);
  }
  if (
    JSON.stringify(manifest.redistributionFulfillments) !==
    JSON.stringify(redistributionFulfillmentsForArtifact(expected))
  ) {
    throw new Error(`manifest redistribution fulfillment mismatch for ${expected.id}`);
  }
  if (!/^[0-9a-f]{64}$/.test(manifest.sha256)) throw new Error(`invalid SHA-256 for ${expected.id}`);
  validateBuildProvenance(manifest.buildProvenance, expected);
}

export async function verifyReleaseArtifacts(
  directory: string,
  options: { repoRoot?: string; tag?: string; version?: string; commit?: string } = {},
): Promise<ReleaseSummary> {
  const repoRoot = options.repoRoot ?? DEFAULT_REPO_ROOT;
  const version = options.version ?? (await workspaceVersion(repoRoot));
  validateReleaseTag(version, options.tag);
  const commit = options.commit ?? (await workspaceCommit(repoRoot));
  const root = resolve(repoRoot, directory);

  const manifestFiles = (await readdir(root)).filter((name) => name.endsWith(".manifest.json"));
  if (manifestFiles.length !== RELEASE_ARTIFACTS.length) {
    throw new Error(
      `release manifest count mismatch: expected ${RELEASE_ARTIFACTS.length}, got ${manifestFiles.length}`,
    );
  }

  const byId = new Map<string, ReleaseManifest>();
  for (const file of manifestFiles) {
    const parsed = JSON.parse(await readFile(join(root, file), "utf8")) as ReleaseManifest;
    if (byId.has(parsed.artifactId)) throw new Error(`duplicate release manifest: ${parsed.artifactId}`);
    byId.set(parsed.artifactId, parsed);
  }

  const manifests: ReleaseManifest[] = [];
  for (const expected of RELEASE_ARTIFACTS) {
    const manifest = byId.get(expected.id);
    if (!manifest) throw new Error(`missing release manifest: ${expected.id}`);
    validateManifestIdentity(manifest, expected, version, commit);
    const archive = join(root, manifest.archive);
    await assertFile(archive, `release archive for ${expected.id}`);
    const digest = await sha256File(archive);
    if (digest !== manifest.sha256) {
      throw new Error(`archive digest mismatch for ${expected.id}: expected ${manifest.sha256}, got ${digest}`);
    }
    verifyArchiveContents(expected, archive, version, repoRoot);
    manifests.push(manifest);
  }

  for (const id of byId.keys()) {
    if (!RELEASE_ARTIFACTS.some((artifact) => artifact.id === id)) {
      throw new Error(`unexpected release manifest: ${id}`);
    }
  }

  const companionAssets = await verifyRedistributionSourceAssets(root, { gitCommit: commit, repoRoot });
  const summary: ReleaseSummary = {
    schemaVersion: 4,
    version,
    gitCommit: commit,
    artifacts: manifests,
    companionAssets,
  };
  await writeFile(join(root, "release-manifest.json"), `${JSON.stringify(summary, null, 2)}\n`);
  return summary;
}
