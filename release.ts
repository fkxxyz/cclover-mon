export * from "./tools/release/plan";
export { archiveFiles, expectedArchiveFiles, verifyArchiveContents } from "./tools/release/archive";
export {
  assertStaticPackageInputs,
  buildReleaseArtifact,
  packageBuiltReleaseArtifact,
  verifyReleaseArtifacts,
  workspaceVersion,
} from "./tools/release/pipeline";

import {
  RELEASE_ARTIFACTS,
  releaseArtifact,
  releaseMatrix,
  validateReleaseTag,
} from "./tools/release/plan";
import {
  buildReleaseArtifact,
  verifyReleaseArtifacts,
  workspaceVersion,
} from "./tools/release/pipeline";

function usage(): void {
  console.log(`usage:
  bun release.ts plan [--json] [--tag vVERSION]
  bun release.ts build <artifact-id> [--out-dir DIR] [--tag vVERSION]
  bun release.ts verify <artifact-directory> [--tag vVERSION]`);
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
  if (command === "verify") {
    const [directory] = positional(rest);
    if (!directory) throw new Error("verify requires an artifact directory");
    const summary = await verifyReleaseArtifacts(directory, { tag });
    console.log(`verified ${summary.artifacts.length} release artifacts for v${summary.version} at ${summary.gitCommit}`);
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
