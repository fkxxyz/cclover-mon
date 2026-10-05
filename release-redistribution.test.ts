import { describe, expect, test } from "bun:test";
import { mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";

import {
  RELEASE_COMPANION_ASSETS,
  buildRedistributionSourceAsset,
  releaseArtifact,
  sourceManifestFileName,
  validateRedistributionPlan,
  verifyReleaseArtifacts,
  type ReleaseCompanionAssetPlan,
} from "./release";
import { COMMIT, VERSION, completeFixture, sha256 } from "./release-test-fixtures";

function run(command: readonly string[], cwd: string): string {
  const result = Bun.spawnSync({
    cmd: [...command],
    cwd,
    stdout: "pipe",
    stderr: "pipe",
  });
  if (result.exitCode !== 0) {
    throw new Error(`${command.join(" ")} failed: ${result.stderr.toString()}${result.stdout.toString()}`);
  }
  return result.stdout.toString().trim();
}

async function fixtureRepository(root: string): Promise<string> {
  const repository = join(root, "upstream");
  await mkdir(repository);
  run(["git", "init", "-q"], repository);
  run(["git", "config", "user.name", "fixture"], repository);
  run(["git", "config", "user.email", "fixture@example.invalid"], repository);
  await writeFile(join(repository, "README.md"), "fixture corresponding source\n");
  run(["git", "add", "README.md"], repository);
  run(["git", "commit", "-q", "-m", "fixture source"], repository);
  run(["git", "tag", "fixture-v1"], repository);
  return repository;
}

async function fixtureSubmoduleRepository(root: string): Promise<string> {
  const repository = join(root, "submodule-upstream");
  await mkdir(repository);
  run(["git", "init", "-q"], repository);
  run(["git", "config", "user.name", "fixture"], repository);
  run(["git", "config", "user.email", "fixture@example.invalid"], repository);
  await writeFile(join(repository, "SUBMODULE.md"), "fixture submodule source\n");
  run(["git", "add", "SUBMODULE.md"], repository);
  run(["git", "commit", "-q", "-m", "fixture submodule source"], repository);
  return repository;
}

describe("redistribution fulfillment", () => {
  test("builds deterministic source companion archives from a pinned source ref", async () => {
    const root = await mkdtemp(join(tmpdir(), "cclover-redistribution-source-test-"));
    try {
      const repository = await fixtureRepository(root);
      const expectedCommit = run(["git", "rev-parse", "HEAD"], repository);
      const plan: ReleaseCompanionAssetPlan = {
        id: "fixture-source",
        role: "redistribution-source",
        name: "fixture-1.0-source.tar.gz",
        sourceRoot: "fixture-1.0",
        fulfillsPayloads: ["pawnio-modules"],
        source: {
          dependency: "Fixture",
          version: "1.0",
          repository,
          ref: "fixture-v1",
          commit: expectedCommit,
        },
      };

      const first = await buildRedistributionSourceAsset(plan, {
        outDir: join(root, "out-one"),
        gitCommit: COMMIT,
        repoRoot: root,
      });
      const second = await buildRedistributionSourceAsset(plan, {
        outDir: join(root, "out-two"),
        gitCommit: COMMIT,
        repoRoot: root,
      });

      expect(first.resolvedSourceCommit).toBe(expectedCommit);
      expect(second.resolvedSourceCommit).toBe(expectedCommit);
      expect(first.producerTools.git.command).toBe("git");
      expect(first.producerTools.git.version).toStartWith("git version");
      expect(first.producerTools.archiveTools.map(({ role, command }) => `${role}:${command}`)).toEqual([
        "archiver:tar",
        "compressor:gzip",
      ]);
      expect(first.producerTools.archiveTools.every((tool) => Boolean(tool.version))).toBe(true);
      expect(first.sha256).toBe(second.sha256);
      expect(first.sha256).toBe(await sha256(join(root, "out-one", plan.name)));
      const listing = run(["tar", "-tzf", join(root, "out-one", plan.name)], root).split(/\r?\n/);
      expect(listing).toContain("fixture-1.0/README.md");
    } finally {
      await rm(root, { recursive: true, force: true });
    }
  });

  test("fails closed when a source ref moves away from its pinned commit", async () => {
    const root = await mkdtemp(join(tmpdir(), "cclover-redistribution-source-test-"));
    try {
      const repository = await fixtureRepository(root);
      const plan: ReleaseCompanionAssetPlan = {
        id: "fixture-source",
        role: "redistribution-source",
        name: "fixture-source.tar.gz",
        sourceRoot: "fixture",
        fulfillsPayloads: ["pawnio-modules"],
        source: {
          dependency: "Fixture",
          version: "1.0",
          repository,
          ref: "fixture-v1",
          commit: "b".repeat(40),
        },
      };

      await expect(
        buildRedistributionSourceAsset(plan, {
          outDir: join(root, "out"),
          gitCommit: COMMIT,
          repoRoot: root,
        }),
      ).rejects.toThrow("expected pinned commit");
    } finally {
      await rm(root, { recursive: true, force: true });
    }
  });

  test("materializes pinned gitlink source and records submodule provenance", async () => {
    const root = await mkdtemp(join(tmpdir(), "cclover-redistribution-source-test-"));
    try {
      const repository = await fixtureRepository(root);
      const submoduleRepository = await fixtureSubmoduleRepository(root);
      const submoduleCommit = run(["git", "rev-parse", "HEAD"], submoduleRepository);
      run(
        ["git", "-c", "protocol.file.allow=always", "submodule", "add", submoduleRepository, "PawnPP"],
        repository,
      );
      run(["git", "commit", "-q", "-am", "add fixture submodule"], repository);
      run(["git", "tag", "fixture-submodules"], repository);
      const expectedCommit = run(["git", "rev-parse", "HEAD"], repository);
      const plan: ReleaseCompanionAssetPlan = {
        id: "fixture-source",
        role: "redistribution-source",
        name: "fixture-source.tar.gz",
        sourceRoot: "fixture",
        fulfillsPayloads: ["pawnio-modules"],
        source: {
          dependency: "Fixture",
          version: "1.1",
          repository,
          ref: "fixture-submodules",
          commit: expectedCommit,
        },
      };

      const testHome = join(root, "git-home");
      await mkdir(testHome);
      await writeFile(join(testHome, ".gitconfig"), "[protocol \"file\"]\n\tallow = always\n");
      const previousHome = process.env.HOME;
      process.env.HOME = testHome;
      let manifest;
      try {
        manifest = await buildRedistributionSourceAsset(plan, {
          outDir: join(root, "out"),
          gitCommit: COMMIT,
          repoRoot: root,
        });
      } finally {
        if (previousHome === undefined) delete process.env.HOME;
        else process.env.HOME = previousHome;
      }

      expect(manifest.submodules).toEqual([
        { path: "PawnPP", repository: submoduleRepository, commit: submoduleCommit },
      ]);
      const listing = run(["tar", "-tzf", join(root, "out", plan.name)], root).split(/\r?\n/);
      expect(listing).toContain("fixture/PawnPP/SUBMODULE.md");
      expect(listing.some((entry) => entry.endsWith("/.git") || entry.includes("/.git/"))).toBe(false);
    } finally {
      await rm(root, { recursive: true, force: true });
    }
  });

  test("plan validation ties notices and one fulfillment to the payload actually embedded", () => {
    const artifact = releaseArtifact("cclover-mon-x86_64-pc-windows-msvc");
    const withoutDriverNotice = {
      ...artifact,
      staticFiles: artifact.staticFiles.filter(
        (file) => file.destination !== "LICENSES/PawnIO-NOTICE.txt",
      ),
    };
    expect(() => validateRedistributionPlan([withoutDriverNotice], RELEASE_COMPANION_ASSETS)).toThrow(
      `release artifact ${artifact.id} embeds pawnio-driver without required LICENSES/PawnIO-NOTICE.txt`,
    );
  });

  test("aggregate verification rejects missing source producer provenance", async () => {
    const root = await mkdtemp(join(tmpdir(), "cclover-redistribution-verify-test-"));
    try {
      await completeFixture(root);
      const source = RELEASE_COMPANION_ASSETS[0]!;
      const manifestPath = join(root, sourceManifestFileName(source));
      const manifest = JSON.parse(await readFile(manifestPath, "utf8"));
      delete manifest.producerTools;
      await writeFile(manifestPath, JSON.stringify(manifest));
      await expect(verifyReleaseArtifacts(root, { version: VERSION, commit: COMMIT })).rejects.toThrow(
        `invalid source git producer provenance for ${source.id}`,
      );
    } finally {
      await rm(root, { recursive: true, force: true });
    }
  });

  test("aggregate verification rejects missing or tampered companion source", async () => {
    const root = await mkdtemp(join(tmpdir(), "cclover-redistribution-verify-test-"));
    try {
      await completeFixture(root);
      const source = RELEASE_COMPANION_ASSETS[0]!;
      await writeFile(join(root, source.name), "tampered source bytes\n");
      await expect(verifyReleaseArtifacts(root, { version: VERSION, commit: COMMIT })).rejects.toThrow(
        `redistribution source digest mismatch for ${source.id}`,
      );
    } finally {
      await rm(root, { recursive: true, force: true });
    }
  });
});
