import { join, resolve } from "node:path";

import {
  binaryFileName,
  stagingDirectoryName,
  type ArchiveToolIdentity,
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

function archiveToolSpecs(artifact: ReleaseArtifact): readonly {
  role: ArchiveToolIdentity["role"];
  command: string;
}[] {
  return artifact.archiveFormat === "tar.gz"
    ? [
        { role: "archiver", command: "tar" },
        { role: "compressor", command: "gzip" },
      ]
    : [{ role: "archiver", command: "zip" }];
}

export function archiveToolProvenance(
  artifact: ReleaseArtifact,
  repoRoot = DEFAULT_REPO_ROOT,
): readonly ArchiveToolIdentity[] {
  return archiveToolSpecs(artifact).map(({ role, command }) => {
    const output = capture([command, "--version"], repoRoot);
    const lines = output
      .split(/\r?\n/)
      .map((line) => line.trim())
      .filter((line) => line.length > 0);
    const version =
      command === "zip"
        ? lines.find((line) => /^This is Zip /.test(line))
        : lines[0];
    if (!version) throw new Error(`${command} did not report a recognizable version`);
    return { role, command, version };
  });
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
  const tools = archiveToolSpecs(artifact);
  if (artifact.archiveFormat === "tar.gz") {
    const tar = tools.find((tool) => tool.role === "archiver")!;
    const compressor = tools.find((tool) => tool.role === "compressor")!;
    run(
      [
        tar.command,
        "--sort=name",
        "--mtime=1980-01-01T00:00:00Z",
        "--owner=0",
        "--group=0",
        "--numeric-owner",
        "--use-compress-program",
        compressor.command,
        "-cf",
        output,
        "-C",
        stageParent,
        stageName,
      ],
      repoRoot,
    );
    return;
  }
  const zip = tools.find((tool) => tool.role === "archiver")!;
  run([zip.command, "-X", "-q", "-r", output, stageName], stageParent);
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
