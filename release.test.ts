import { describe, expect, test } from "bun:test";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";

import {
  RELEASE_ARTIFACTS,
  archiveFiles,
  archiveName,
  assertStaticPackageInputs,
  binaryFileName,
  buildInvocation,
  expectedArchiveFiles,
  packageBuiltReleaseArtifact,
  releaseArtifact,
  releaseMatrix,
  validateReleaseTag,
  verifyReleaseArtifacts,
  type ReleaseManifest,
} from "./release";

const VERSION = "0.1.0";
const COMMIT = "0123456789abcdef0123456789abcdef01234567";

async function sha256(path: string): Promise<string> {
  const hasher = new Bun.CryptoHasher("sha256");
  hasher.update(await Bun.file(path).arrayBuffer());
  return hasher.digest("hex");
}

async function completeFixture(root: string): Promise<void> {
  const binary = join(root, "fake-release-binary");
  await writeFile(binary, "fake executable bytes");
  for (const artifact of RELEASE_ARTIFACTS) {
    await packageBuiltReleaseArtifact(artifact, {
      outDir: root,
      binary,
      version: VERSION,
      commit: COMMIT,
    });
  }
  await rm(binary);
}

describe("release authority", () => {
  test("declares each supported product/target artifact exactly once", () => {
    expect(RELEASE_ARTIFACTS).toHaveLength(6);
    expect(new Set(RELEASE_ARTIFACTS.map((artifact) => artifact.id)).size).toBe(6);
    expect(releaseMatrix()).toEqual({
      include: RELEASE_ARTIFACTS.map(({ id, platform, target }) => ({ id, platform, target })),
    });
  });

  test("server packages own platform service assets while interactive packages do not", () => {
    const linuxServer = releaseArtifact("cclover-mon-server-x86_64-unknown-linux-gnu");
    expect(linuxServer.staticFiles.map((file) => file.destination)).toContain(
      "systemd/cclover-mon-server.service",
    );
    expect(linuxServer.staticFiles.map((file) => file.destination)).toContain("systemd/ebpf-io.conf");

    const windowsServer = releaseArtifact("cclover-mon-server-x86_64-pc-windows-msvc");
    expect(windowsServer.staticFiles.map((file) => file.destination)).toContain("install-service.ps1");

    for (const artifact of RELEASE_ARTIFACTS.filter((item) => item.product === "cclover-mon")) {
      expect(artifact.staticFiles.some((file) => file.destination.includes("systemd"))).toBe(false);
      expect(artifact.staticFiles.some((file) => file.destination === "install-service.ps1")).toBe(false);
    }
  });

  test("Windows artifacts carry redistribution notices and source provenance inputs", () => {
    for (const artifact of RELEASE_ARTIFACTS.filter((item) => item.platform === "windows")) {
      const destinations = artifact.staticFiles.map((file) => file.destination);
      expect(destinations).toContain("LICENSES/PawnIO-NOTICE.txt");
      expect(destinations).toContain("LICENSES/LGPL-2.1-or-later.txt");
      expect(destinations).toContain("LICENSES/MPL-2.0.txt");
    }
  });

  test("all declared static package inputs exist", async () => {
    for (const artifact of RELEASE_ARTIFACTS) await assertStaticPackageInputs(artifact);
  });

  test("Windows release builds pin xwin architecture and every release build uses dist", () => {
    for (const artifact of RELEASE_ARTIFACTS) {
      const invocation = buildInvocation(artifact);
      expect(invocation.command).toContain("--profile");
      expect(invocation.command).toContain("dist");
      expect(invocation.command).toContain("--target");
      expect(invocation.command).toContain(artifact.target);
      expect(invocation.command).toContain(artifact.cargoPackage);
      if (artifact.platform === "windows") {
        expect(invocation.command.slice(0, 3)).toEqual(["cargo", "xwin", "build"]);
        expect(invocation.env).toEqual({ XWIN_ARCH: "x86,x86_64" });
        expect(binaryFileName(artifact)).toEndWith(".exe");
      } else {
        expect(invocation.command.slice(0, 2)).toEqual(["cargo", "build"]);
        expect(invocation.env).toBeUndefined();
      }
    }
  });

  test("artifact names encode product, version, and exact Rust target", () => {
    expect(archiveName(releaseArtifact("cclover-mon-x86_64-unknown-linux-gnu"), VERSION)).toBe(
      "cclover-mon-v0.1.0-x86_64-unknown-linux-gnu.tar.gz",
    );
    expect(archiveName(releaseArtifact("cclover-mon-server-i686-pc-windows-msvc"), VERSION)).toBe(
      "cclover-mon-server-v0.1.0-i686-pc-windows-msvc.zip",
    );
  });

  test("tag must match the workspace version", () => {
    expect(() => validateReleaseTag(VERSION, "v0.1.0")).not.toThrow();
    expect(() => validateReleaseTag(VERSION, "v0.2.0")).toThrow(
      "release tag v0.2.0 does not match workspace version v0.1.0",
    );
  });

  test("packaging seam emits exactly the declared files for both archive formats", async () => {
    const root = await mkdtemp(join(tmpdir(), "cclover-release-package-test-"));
    try {
      await completeFixture(root);
      for (const id of [
        "cclover-mon-server-x86_64-unknown-linux-gnu",
        "cclover-mon-server-x86_64-pc-windows-msvc",
      ]) {
        const artifact = releaseArtifact(id);
        expect(archiveFiles(artifact, join(root, archiveName(artifact, VERSION)))).toEqual(
          expectedArchiveFiles(artifact, VERSION),
        );
      }
    } finally {
      await rm(root, { recursive: true, force: true });
    }
  });
});

describe("release completeness", () => {
  test("accepts exactly one verified manifest per declared artifact", async () => {
    const root = await mkdtemp(join(tmpdir(), "cclover-release-test-"));
    try {
      await completeFixture(root);
      const summary = await verifyReleaseArtifacts(root, { version: VERSION, commit: COMMIT });
      expect(summary.artifacts.map((manifest) => manifest.artifactId)).toEqual(
        RELEASE_ARTIFACTS.map((artifact) => artifact.id),
      );
      const written = JSON.parse(await readFile(join(root, "release-manifest.json"), "utf8"));
      expect(written.version).toBe(VERSION);
      expect(written.gitCommit).toBe(COMMIT);
      expect(written.artifacts).toHaveLength(6);
    } finally {
      await rm(root, { recursive: true, force: true });
    }
  });

  test("fails when any product/target artifact is omitted", async () => {
    const root = await mkdtemp(join(tmpdir(), "cclover-release-test-"));
    try {
      await completeFixture(root);
      const omitted = RELEASE_ARTIFACTS[0]!;
      const archive = archiveName(omitted, VERSION);
      await rm(join(root, `${archive}.manifest.json`));
      await expect(verifyReleaseArtifacts(root, { version: VERSION, commit: COMMIT })).rejects.toThrow(
        "release manifest count mismatch: expected 6, got 5",
      );
    } finally {
      await rm(root, { recursive: true, force: true });
    }
  });

  test("fails when artifacts come from a different revision", async () => {
    const root = await mkdtemp(join(tmpdir(), "cclover-release-test-"));
    try {
      await completeFixture(root);
      const artifact = RELEASE_ARTIFACTS[0]!;
      const archive = archiveName(artifact, VERSION);
      const path = join(root, `${archive}.manifest.json`);
      const manifest = JSON.parse(await readFile(path, "utf8")) as ReleaseManifest;
      await writeFile(path, JSON.stringify({ ...manifest, gitCommit: "f".repeat(40) }));
      await expect(verifyReleaseArtifacts(root, { version: VERSION, commit: COMMIT })).rejects.toThrow(
        `manifest commit mismatch for ${artifact.id}`,
      );
    } finally {
      await rm(root, { recursive: true, force: true });
    }
  });

  test("fails when archive bytes do not match the manifest digest", async () => {
    const root = await mkdtemp(join(tmpdir(), "cclover-release-test-"));
    try {
      await completeFixture(root);
      const artifact = RELEASE_ARTIFACTS[0]!;
      await writeFile(join(root, archiveName(artifact, VERSION)), "tampered");
      await expect(verifyReleaseArtifacts(root, { version: VERSION, commit: COMMIT })).rejects.toThrow(
        `archive digest mismatch for ${artifact.id}`,
      );
    } finally {
      await rm(root, { recursive: true, force: true });
    }
  });

  test("fails when a validly hashed archive contains undeclared files", async () => {
    const root = await mkdtemp(join(tmpdir(), "cclover-release-test-"));
    try {
      await completeFixture(root);
      const artifact = releaseArtifact("cclover-mon-server-x86_64-pc-windows-msvc");
      const archive = archiveName(artifact, VERSION);
      const archivePath = join(root, archive);
      const extra = join(root, "unexpected.txt");
      await writeFile(extra, "unexpected");
      const result = Bun.spawnSync({
        cmd: ["zip", "-q", archivePath, "unexpected.txt"],
        cwd: root,
        stdout: "pipe",
        stderr: "pipe",
      });
      expect(result.exitCode).toBe(0);

      const manifestPath = join(root, `${archive}.manifest.json`);
      const manifest = JSON.parse(await readFile(manifestPath, "utf8")) as ReleaseManifest;
      await writeFile(
        manifestPath,
        `${JSON.stringify({ ...manifest, sha256: await sha256(archivePath) })}\n`,
      );

      await expect(verifyReleaseArtifacts(root, { version: VERSION, commit: COMMIT })).rejects.toThrow(
        `archive contents mismatch for ${artifact.id}`,
      );
    } finally {
      await rm(root, { recursive: true, force: true });
    }
  });
});
