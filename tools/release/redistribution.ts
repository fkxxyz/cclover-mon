import { mkdir, mkdtemp, readdir, readFile, rename, rm, stat, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

import { PAWNIO, type PawnioPayloadId } from "../../deps/pawnio";
import {
  RELEASE_ARTIFACTS,
  RELEASE_COMPANION_ASSETS,
  type ArchiveToolIdentity,
  type ReleaseArtifact,
  type ReleaseCompanionAssetManifest,
  type ReleaseCompanionAssetPlan,
  type RedistributionFulfillmentEvidence,
  type ToolIdentity,
} from "./plan";
import {
  materializeRetainedSource,
  type SourceAcquisitionPolicy,
} from "./source-retention";

const DEFAULT_REPO_ROOT = resolve(import.meta.dir, "../..");

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

function firstLine(output: string, command: string): string {
  const line = output
    .split(/\r?\n/)
    .map((candidate) => candidate.trim())
    .find((candidate) => candidate.length > 0);
  if (!line) throw new Error(`${command} did not report a version`);
  return line;
}

function sourceProducerTools(cwd: string): ReleaseCompanionAssetManifest["producerTools"] {
  const git: ToolIdentity = {
    command: "git",
    version: firstLine(capture(["git", "--version"], cwd), "git"),
  };
  const archiveTools: ArchiveToolIdentity[] = [
    {
      role: "archiver",
      command: "tar",
      version: firstLine(capture(["tar", "--version"], cwd), "tar"),
    },
    {
      role: "compressor",
      command: "gzip",
      version: firstLine(capture(["gzip", "--version"], cwd), "gzip"),
    },
  ];
  return { git, archiveTools };
}

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

async function assertFile(path: string, description: string): Promise<void> {
  let metadata;
  try {
    metadata = await stat(path);
  } catch {
    throw new Error(`${description} is missing: ${path}`);
  }
  if (!metadata.isFile()) throw new Error(`${description} is not a file: ${path}`);
}

function payloadMetadata(id: PawnioPayloadId): {
  dependency: string;
  version: string;
  sourceUrl: string;
  binaryInput: string;
  binarySha256: string;
  license: string;
  noticeFiles: readonly string[];
  fulfillment: "companion-source";
} {
  if (id === PAWNIO.driver.id) {
    return {
      dependency: "PawnIO driver",
      version: PAWNIO.driver.version,
      sourceUrl: PAWNIO.driver.correspondingSource.browseUrl,
      binaryInput: PAWNIO.driver.installerUrl,
      binarySha256: PAWNIO.driver.installerSha256,
      license: PAWNIO.driver.redistribution.license,
      noticeFiles: PAWNIO.driver.redistribution.noticeFiles,
      fulfillment: PAWNIO.driver.redistribution.fulfillment,
    };
  }
  if (id === PAWNIO.modules.id) {
    return {
      dependency: "PawnIO.Modules",
      version: PAWNIO.modules.version,
      sourceUrl: PAWNIO.modules.correspondingSource.browseUrl,
      binaryInput: PAWNIO.modules.archiveUrl,
      binarySha256: PAWNIO.modules.archiveSha256,
      license: PAWNIO.modules.redistribution.license,
      noticeFiles: PAWNIO.modules.redistribution.noticeFiles,
      fulfillment: PAWNIO.modules.redistribution.fulfillment,
    };
  }
  const unreachable: never = id;
  throw new Error(`unknown redistributed payload: ${String(unreachable)}`);
}

function companionForPayload(
  payload: PawnioPayloadId,
  companions: readonly ReleaseCompanionAssetPlan[] = RELEASE_COMPANION_ASSETS,
): ReleaseCompanionAssetPlan {
  const matches = companions.filter((companion) => companion.fulfillsPayloads.includes(payload));
  if (matches.length !== 1) {
    throw new Error(
      `redistributed payload ${payload} must have exactly one source fulfillment, got ${matches.length}`,
    );
  }
  return matches[0]!;
}

export function redistributionFulfillmentsForArtifact(
  artifact: ReleaseArtifact,
  companions: readonly ReleaseCompanionAssetPlan[] = RELEASE_COMPANION_ASSETS,
): readonly RedistributionFulfillmentEvidence[] {
  return artifact.thirdPartyPayloads.map((payload) => {
    const companion = companionForPayload(payload, companions);
    return {
      payload,
      companionAssetId: companion.id,
      companionAsset: companion.name,
    };
  });
}

export function validateRedistributionPlan(
  artifacts: readonly ReleaseArtifact[] = RELEASE_ARTIFACTS,
  companions: readonly ReleaseCompanionAssetPlan[] = RELEASE_COMPANION_ASSETS,
): void {
  const knownPayloads = new Set<PawnioPayloadId>([PAWNIO.driver.id, PAWNIO.modules.id]);
  const companionIds = companions.map((companion) => companion.id);
  const companionNames = companions.map((companion) => companion.name);
  if (new Set(companionIds).size !== companionIds.length) {
    throw new Error("release companion asset ids must be unique");
  }
  if (new Set(companionNames).size !== companionNames.length) {
    throw new Error("release companion asset names must be unique");
  }

  const embeddedPayloads = new Set(artifacts.flatMap((artifact) => artifact.thirdPartyPayloads));
  for (const companion of companions) {
    if (companion.role !== "redistribution-source") {
      throw new Error(`unsupported release companion role for ${companion.id}: ${companion.role}`);
    }
    if (companion.fulfillsPayloads.length === 0) {
      throw new Error(`release companion ${companion.id} fulfills no redistributed payload`);
    }
    if (new Set(companion.fulfillsPayloads).size !== companion.fulfillsPayloads.length) {
      throw new Error(`release companion ${companion.id} declares duplicate fulfilled payloads`);
    }
    if (!/^[0-9a-f]{40}$/.test(companion.source.commit)) {
      throw new Error(`release companion ${companion.id} has invalid pinned source commit`);
    }
    for (const payload of companion.fulfillsPayloads) {
      if (!knownPayloads.has(payload)) {
        throw new Error(`release companion ${companion.id} fulfills unknown payload ${payload}`);
      }
      if (!embeddedPayloads.has(payload)) {
        throw new Error(`release companion ${companion.id} fulfills payload ${payload} that no product embeds`);
      }
    }
  }

  for (const artifact of artifacts) {
    if (new Set(artifact.thirdPartyPayloads).size !== artifact.thirdPartyPayloads.length) {
      throw new Error(`release artifact ${artifact.id} declares duplicate third-party payloads`);
    }
    const destinations = new Set(artifact.staticFiles.map((file) => file.destination));
    for (const payload of artifact.thirdPartyPayloads) {
      if (!knownPayloads.has(payload)) {
        throw new Error(`release artifact ${artifact.id} embeds unknown payload ${payload}`);
      }
      const metadata = payloadMetadata(payload);
      if (metadata.fulfillment !== "companion-source") {
        throw new Error(`unsupported redistribution fulfillment for ${payload}: ${metadata.fulfillment}`);
      }
      for (const notice of metadata.noticeFiles) {
        if (!destinations.has(notice)) {
          throw new Error(`release artifact ${artifact.id} embeds ${payload} without required ${notice}`);
        }
      }
      companionForPayload(payload, companions);
    }
  }
}

export function thirdPartySourcesText(artifact: ReleaseArtifact): string {
  validateRedistributionPlan();
  const sections = artifact.thirdPartyPayloads.map((payload) => {
    const metadata = payloadMetadata(payload);
    const companion = companionForPayload(payload);
    return [
      `${metadata.dependency} ${metadata.version}`,
      `  Source: ${metadata.sourceUrl}`,
      `  Pinned source commit: ${companion.source.commit}`,
      `  Binary input: ${metadata.binaryInput}`,
      `  Binary SHA-256: ${metadata.binarySha256}`,
      `  License: ${metadata.license}`,
      ...metadata.noticeFiles.map((notice) => `  Notice/license: ${notice}`),
      `  Corresponding source asset: ${companion.name}`,
      "  Source asset SHA-256 and resolved upstream commit: release-manifest.json",
    ].join("\n");
  });
  return [
    "Third-party redistribution mapping for embedded Windows payloads",
    "",
    ...sections.flatMap((section) => [section, ""]),
    "The companion source assets are published with the same cclover-mon release and are verified before publication.",
    "",
  ].join("\n");
}

export function sourceManifestFileName(plan: ReleaseCompanionAssetPlan): string {
  return `${plan.name}.source.json`;
}

export async function buildRedistributionSourceAsset(
  plan: ReleaseCompanionAssetPlan,
  options: {
    outDir: string;
    gitCommit: string;
    repoRoot?: string;
    sourcePolicy?: SourceAcquisitionPolicy;
    sourceCacheRoot?: string;
  },
): Promise<ReleaseCompanionAssetManifest> {
  const repoRoot = options.repoRoot ?? DEFAULT_REPO_ROOT;
  if (!/^[0-9a-f]{40}$/.test(options.gitCommit)) {
    throw new Error(`invalid release git commit for ${plan.id}: ${options.gitCommit}`);
  }
  const outDir = resolve(repoRoot, options.outDir);
  await mkdir(outDir, { recursive: true });
  const temporary = await mkdtemp(join(tmpdir(), `cclover-${plan.id}-`));
  const checkout = join(temporary, plan.sourceRoot);
  const tarPath = join(temporary, `${plan.id}.tar`);
  const gzPath = `${tarPath}.gz`;
  const outputPath = join(outDir, plan.name);

  try {
    const retained = await materializeRetainedSource(plan, checkout, {
      repoRoot,
      policy: options.sourcePolicy,
      cacheRoot: options.sourceCacheRoot,
    });
    const resolvedSourceCommit = retained.resolvedSourceCommit;
    const submodules = retained.submodules;
    const producerTools = sourceProducerTools(repoRoot);

    run(
      [
        "tar",
        "--sort=name",
        "--mtime=1980-01-01T00:00:00Z",
        "--owner=0",
        "--group=0",
        "--numeric-owner",
        "--exclude=.git",
        "--exclude=*/.git",
        "-cf",
        tarPath,
        "-C",
        temporary,
        plan.sourceRoot,
      ],
      repoRoot,
    );
    run(["gzip", "-n", "-9", tarPath], repoRoot);
    await rm(outputPath, { force: true });
    await rename(gzPath, outputPath);

    const manifest: ReleaseCompanionAssetManifest = {
      schemaVersion: 1,
      ...plan,
      gitCommit: options.gitCommit,
      sha256: await sha256File(outputPath),
      resolvedSourceCommit,
      producerTools,
      submodules,
    };
    await writeFile(
      join(outDir, sourceManifestFileName(plan)),
      `${JSON.stringify(manifest, null, 2)}\n`,
    );
    return manifest;
  } finally {
    await rm(temporary, { recursive: true, force: true });
  }
}

export async function buildRedistributionSourceAssets(
  options: {
    outDir: string;
    gitCommit: string;
    repoRoot?: string;
    sourcePolicy?: SourceAcquisitionPolicy;
    sourceCacheRoot?: string;
  },
): Promise<readonly ReleaseCompanionAssetManifest[]> {
  validateRedistributionPlan();
  const manifests: ReleaseCompanionAssetManifest[] = [];
  for (const plan of RELEASE_COMPANION_ASSETS) {
    manifests.push(await buildRedistributionSourceAsset(plan, options));
  }
  return manifests;
}

function sameJson(left: unknown, right: unknown): boolean {
  return JSON.stringify(left) === JSON.stringify(right);
}

function validateSourceManifest(
  manifest: ReleaseCompanionAssetManifest,
  plan: ReleaseCompanionAssetPlan,
  gitCommit: string,
): void {
  if (manifest.schemaVersion !== 1) throw new Error(`unsupported source manifest schema for ${plan.id}`);
  if (manifest.id !== plan.id) throw new Error(`source manifest id mismatch for ${plan.id}`);
  if (manifest.role !== plan.role) throw new Error(`source manifest role mismatch for ${plan.id}`);
  if (manifest.name !== plan.name) throw new Error(`source manifest asset mismatch for ${plan.id}`);
  if (manifest.sourceRoot !== plan.sourceRoot) throw new Error(`source manifest root mismatch for ${plan.id}`);
  if (!sameJson(manifest.fulfillsPayloads, plan.fulfillsPayloads)) {
    throw new Error(`source manifest payload mapping mismatch for ${plan.id}`);
  }
  if (!sameJson(manifest.source, plan.source)) {
    throw new Error(`source manifest provenance mismatch for ${plan.id}`);
  }
  if (manifest.gitCommit !== gitCommit) throw new Error(`source manifest commit mismatch for ${plan.id}`);
  if (!/^[0-9a-f]{40}$/.test(manifest.resolvedSourceCommit ?? "")) {
    throw new Error(`invalid resolved source commit for ${plan.id}`);
  }
  if (manifest.resolvedSourceCommit !== plan.source.commit) {
    throw new Error(`resolved source commit mismatch for ${plan.id}`);
  }
  if (
    !manifest.producerTools ||
    manifest.producerTools.git?.command !== "git" ||
    typeof manifest.producerTools.git.version !== "string" ||
    manifest.producerTools.git.version.length === 0
  ) {
    throw new Error(`invalid source git producer provenance for ${plan.id}`);
  }
  const archiveTools = manifest.producerTools.archiveTools;
  if (!Array.isArray(archiveTools) || archiveTools.length !== 2) {
    throw new Error(`invalid source archive producer provenance for ${plan.id}`);
  }
  const expectedArchiveTools = [
    { role: "archiver", command: "tar" },
    { role: "compressor", command: "gzip" },
  ] as const satisfies readonly Pick<ArchiveToolIdentity, "role" | "command">[];
  for (const [index, expected] of expectedArchiveTools.entries()) {
    const actual = archiveTools[index];
    if (
      actual?.role !== expected.role ||
      actual.command !== expected.command ||
      typeof actual.version !== "string" ||
      actual.version.length === 0
    ) {
      throw new Error(`invalid source archive producer provenance for ${plan.id}`);
    }
  }
  if (!Array.isArray(manifest.submodules)) {
    throw new Error(`missing source submodule provenance for ${plan.id}`);
  }
  const submodulePaths = new Set<string>();
  for (const submodule of manifest.submodules) {
    if (
      !submodule ||
      typeof submodule.path !== "string" ||
      submodule.path.length === 0 ||
      typeof submodule.repository !== "string" ||
      submodule.repository.length === 0 ||
      !/^[0-9a-f]{40}$/.test(submodule.commit ?? "")
    ) {
      throw new Error(`invalid source submodule provenance for ${plan.id}`);
    }
    if (submodulePaths.has(submodule.path)) {
      throw new Error(`duplicate source submodule path for ${plan.id}: ${submodule.path}`);
    }
    submodulePaths.add(submodule.path);
  }
  if (!/^[0-9a-f]{64}$/.test(manifest.sha256 ?? "")) {
    throw new Error(`invalid source asset SHA-256 for ${plan.id}`);
  }
}

export async function verifyRedistributionSourceAssets(
  directory: string,
  options: { gitCommit: string; repoRoot?: string },
): Promise<readonly ReleaseCompanionAssetManifest[]> {
  validateRedistributionPlan();
  const repoRoot = options.repoRoot ?? DEFAULT_REPO_ROOT;
  const root = resolve(repoRoot, directory);
  const sourceManifestFiles = (await readdir(root)).filter((name) => name.endsWith(".source.json"));
  if (sourceManifestFiles.length !== RELEASE_COMPANION_ASSETS.length) {
    throw new Error(
      `redistribution source manifest count mismatch: expected ${RELEASE_COMPANION_ASSETS.length}, got ${sourceManifestFiles.length}`,
    );
  }

  const expectedNames = new Set(RELEASE_COMPANION_ASSETS.map(sourceManifestFileName));
  for (const name of sourceManifestFiles) {
    if (!expectedNames.has(name)) throw new Error(`unexpected redistribution source manifest: ${name}`);
  }

  const manifests: ReleaseCompanionAssetManifest[] = [];
  for (const plan of RELEASE_COMPANION_ASSETS) {
    const manifestPath = join(root, sourceManifestFileName(plan));
    await assertFile(manifestPath, `redistribution source manifest for ${plan.id}`);
    const manifest = JSON.parse(await readFile(manifestPath, "utf8")) as ReleaseCompanionAssetManifest;
    validateSourceManifest(manifest, plan, options.gitCommit);
    const asset = join(root, plan.name);
    await assertFile(asset, `redistribution source asset for ${plan.id}`);
    const digest = await sha256File(asset);
    if (digest !== manifest.sha256) {
      throw new Error(
        `redistribution source digest mismatch for ${plan.id}: expected ${manifest.sha256}, got ${digest}`,
      );
    }
    manifests.push(manifest);
  }
  return manifests;
}
