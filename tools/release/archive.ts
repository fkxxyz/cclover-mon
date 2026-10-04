import { join, resolve } from "node:path";

import {
  binaryFileName,
  stagingDirectoryName,
  type ReleaseArtifact,
} from "./plan";

const DEFAULT_REPO_ROOT = resolve(import.meta.dir, "../..");

function run(command: readonly string[], cwd: string): void {
  const result = Bun.spawnSync({
    cmd: [...command],
    cwd,
    stdout: "inherit",
    stderr: "inherit",
  });
  if (result.exitCode !== 0) {
    throw new Error(`${command.join(" ")} failed with exit code ${result.exitCode}`);
  }
}

function capture(command: readonly string[], cwd: string): string {
  const result = Bun.spawnSync({
    cmd: [...command],
    cwd,
    stdout: "pipe",
    stderr: "pipe",
  });
  if (result.exitCode !== 0) {
    throw new Error(
      `${command.join(" ")} failed with exit code ${result.exitCode}: ${result.stderr.toString().trim()}`,
    );
  }
  return result.stdout.toString();
}

export async function createArchive(
  artifact: ReleaseArtifact,
  outDir: string,
  stageParent: string,
  stageName: string,
  outputName: string,
  repoRoot = DEFAULT_REPO_ROOT,
): Promise<void> {
  const output = join(outDir, outputName);
  if (artifact.archiveFormat === "tar.gz") {
    run(
      [
        "tar",
        "--sort=name",
        "--mtime=1980-01-01T00:00:00Z",
        "--owner=0",
        "--group=0",
        "--numeric-owner",
        "-czf",
        output,
        "-C",
        stageParent,
        stageName,
      ],
      repoRoot,
    );
    return;
  }
  run(["zip", "-X", "-q", "-r", output, stageName], stageParent);
}

export function expectedArchiveFiles(
  artifact: ReleaseArtifact,
  version: string,
): readonly string[] {
  const root = stagingDirectoryName(artifact, version);
  const files = [
    binaryFileName(artifact),
    ...artifact.staticFiles.map((file) => file.destination),
    ...(artifact.platform === "windows" ? ["THIRD-PARTY-SOURCES.txt"] : []),
  ];
  return files.map((file) => `${root}/${file}`).sort();
}

export function archiveFiles(
  artifact: ReleaseArtifact,
  archivePath: string,
  repoRoot = DEFAULT_REPO_ROOT,
): readonly string[] {
  const listing =
    artifact.archiveFormat === "tar.gz"
      ? capture(["tar", "-tzf", archivePath], repoRoot)
      : capture(["unzip", "-Z1", archivePath], repoRoot);
  return listing
    .split(/\r?\n/)
    .map((entry) => entry.trim())
    .filter((entry) => entry.length > 0 && !entry.endsWith("/"))
    .sort();
}

export function verifyArchiveContents(
  artifact: ReleaseArtifact,
  archivePath: string,
  version: string,
  repoRoot = DEFAULT_REPO_ROOT,
): void {
  const expected = expectedArchiveFiles(artifact, version);
  const actual = archiveFiles(artifact, archivePath, repoRoot);
  if (JSON.stringify(actual) !== JSON.stringify(expected)) {
    throw new Error(
      `archive contents mismatch for ${artifact.id}: expected ${JSON.stringify(expected)}, got ${JSON.stringify(actual)}`,
    );
  }
}
