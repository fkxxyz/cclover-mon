export * from "./tools/release/plan";
export * from "./tools/release/build-context";
export { archiveFiles, expectedArchiveFiles, verifyArchiveContents } from "./tools/release/archive";
export {
  buildRedistributionSourceAsset,
  buildRedistributionSourceAssets,
  redistributionFulfillmentsForArtifact,
  sourceManifestFileName,
  thirdPartySourcesText,
  validateRedistributionPlan,
  verifyRedistributionSourceAssets,
} from "./tools/release/redistribution";
export {
  assertStaticPackageInputs,
  buildReleaseArtifact,
  packageBuiltReleaseArtifact,
  verifyReleaseArtifacts,
  workspaceCommit,
  workspaceVersion,
} from "./tools/release/pipeline";
export {
  GhGitHubPublicationGateway,
  publishGitHubRelease,
  type GitHubPublicationGateway,
  type GitHubReleaseAsset,
  type GitHubReleaseState,
} from "./tools/release/publication";

import {
  RELEASE_ARTIFACTS,
  releaseArtifact,
  releaseMatrix,
  validateReleaseTag,
  type ReleaseSummary,
} from "./tools/release/plan";
import {
  buildReleaseArtifact,
  verifyReleaseArtifacts,
  workspaceCommit,
  workspaceVersion,
} from "./tools/release/pipeline";
import {
  buildRedistributionSourceAssets,
  validateRedistributionPlan,
} from "./tools/release/redistribution";
import {
  publishGitHubRelease,
  type GitHubPublicationGateway,
} from "./tools/release/publication";

export async function publishVerifiedReleaseArtifacts(
  directory: string,
  tag: string,
  gateway?: GitHubPublicationGateway,
): Promise<ReleaseSummary> {
  const summary = await verifyReleaseArtifacts(directory, { tag });
  await publishGitHubRelease(summary, directory, tag, gateway);
  return summary;
}

function usage(): void {
  console.log(`usage:
  bun release.ts plan [--json] [--tag vVERSION]
  bun release.ts build <artifact-id> [--out-dir DIR] [--tag vVERSION]
  bun release.ts sources [--out-dir DIR] [--tag vVERSION]
  bun release.ts verify <artifact-directory> [--tag vVERSION]
  bun release.ts publish <artifact-directory> --tag vVERSION`);
}

function option(args: readonly string[], name: string): string | undefined {
  const index = args.indexOf(name);
  if (index === -1) return undefined;
  const value = args[index + 1];
  if (!value || value.startsWith("--")) throw new Error(`${name} requires a value`);
  return value;
}

function positional(args: readonly string[]): string[] {
  const result: string[] = [];
  for (let index = 0; index < args.length; index += 1) {
    if (args[index]?.startsWith("--")) {
      if (args[index] !== "--json") index += 1;
      continue;
    }
    result.push(args[index]!);
  }
  return result;
}

async function main(args: readonly string[]): Promise<number> {
  const [command, ...rest] = args;
  if (command === "--help" || command === "-h" || !command) {
    usage();
    return command ? 0 : 2;
  }
  const tag = option(rest, "--tag");
  if (command === "plan") {
    const version = await workspaceVersion();
    validateReleaseTag(version, tag);
    validateRedistributionPlan();
    if (rest.includes("--json")) {
      console.log(JSON.stringify(releaseMatrix()));
    } else {
      console.log(`release v${version}`);
      for (const item of RELEASE_ARTIFACTS) console.log(`${item.id}\t${item.platform}\t${item.target}`);
    }
    return 0;
  }
  if (command === "build") {
    const [id] = positional(rest);
    if (!id) throw new Error("build requires an artifact id");
    const outDir = option(rest, "--out-dir") ?? "release-out";
    const manifest = await buildReleaseArtifact(releaseArtifact(id), { outDir, tag });
    console.log(`${manifest.archive}\t${manifest.sha256}`);
    return 0;
  }
  if (command === "sources") {
    const version = await workspaceVersion();
    validateReleaseTag(version, tag);
    const outDir = option(rest, "--out-dir") ?? "release-out";
    const manifests = await buildRedistributionSourceAssets({
      outDir,
      gitCommit: await workspaceCommit(),
    });
    for (const manifest of manifests) console.log(`${manifest.name}\t${manifest.sha256}`);
    return 0;
  }
  if (command === "verify") {
    const [directory] = positional(rest);
    if (!directory) throw new Error("verify requires an artifact directory");
    const summary = await verifyReleaseArtifacts(directory, { tag });
    console.log(
      `verified ${summary.artifacts.length} product artifacts and ${summary.companionAssets.length} companion assets for v${summary.version} at ${summary.gitCommit}`,
    );
    return 0;
  }
  if (command === "publish") {
    const [directory] = positional(rest);
    if (!directory) throw new Error("publish requires an artifact directory");
    if (!tag) throw new Error("publish requires --tag");
    const summary = await publishVerifiedReleaseArtifacts(directory, tag);
    console.log(`published verified GitHub release ${tag} at ${summary.gitCommit}`);
    return 0;
  }
  usage();
  return 2;
}

if (import.meta.main) {
  try {
    process.exitCode = await main(process.argv.slice(2));
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    process.exitCode = 1;
  }
}
