import { describe, expect, test } from "bun:test";
import { mkdtemp, rm, stat, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";

import {
  RELEASE_ARTIFACTS,
  GhGitHubPublicationGateway,
  archiveName,
  publishGitHubRelease,
  publishVerifiedReleaseArtifacts,
  verifyReleaseArtifacts,
  type GitHubPublicationGateway,
  type GitHubReleaseState,
} from "./release";
import { COMMIT, VERSION, completeFixture, sha256 } from "./release-test-fixtures";

class FakeGitHubPublicationGateway implements GitHubPublicationGateway {
  tagCommit = COMMIT;
  release: GitHubReleaseState | undefined;
  mutations: string[] = [];
  failUploadAfter: number | undefined;
  failPublishAfterMutation = false;
  private nextAssetId = 1;

  async resolveTagCommit(_tag: string): Promise<string> {
    return this.tagCommit;
  }

  async findRelease(_tag: string): Promise<GitHubReleaseState | undefined> {
    if (!this.release) return undefined;
    return { ...this.release, assets: this.release.assets.map((asset) => ({ ...asset })) };
  }

  async createDraft(tag: string): Promise<void> {
    this.mutations.push("create-draft");
    this.release = { id: 1, tag, draft: true, assets: [] };
  }

  async deleteAsset(_tag: string, name: string): Promise<void> {
    this.mutations.push(`delete:${name}`);
    if (!this.release) throw new Error("fake release is missing");
    this.release = {
      ...this.release,
      assets: this.release.assets.filter((asset) => asset.name !== name),
    };
  }

  async uploadAssets(_tag: string, paths: readonly string[]): Promise<void> {
    this.mutations.push("upload");
    if (!this.release) throw new Error("fake release is missing");
    for (let index = 0; index < paths.length; index += 1) {
      const path = paths[index]!;
      const name = path.split(/[\\/]/).at(-1)!;
      const metadata = await stat(path);
      const asset = {
        id: this.nextAssetId++,
        name,
        size: metadata.size,
        digest: `sha256:${await sha256(path)}`,
      };
      this.release = {
        ...this.release,
        assets: [...this.release.assets.filter((current) => current.name !== name), asset],
      };
      if (this.failUploadAfter === index + 1) {
        this.failUploadAfter = undefined;
        throw new Error("simulated upload failure");
      }
    }
  }

  async publishDraft(_tag: string): Promise<void> {
    this.mutations.push("publish");
    if (!this.release) throw new Error("fake release is missing");
    this.release = { ...this.release, draft: false };
    if (this.failPublishAfterMutation) {
      this.failPublishAfterMutation = false;
      throw new Error("simulated lost publish response");
    }
  }
}

function recordingGateway(stdout = ""): {
  gateway: GhGitHubPublicationGateway;
  calls: string[][];
} {
  const calls: string[][] = [];
  return {
    calls,
    gateway: new GhGitHubPublicationGateway((args) => {
      calls.push([...args]);
      return { stdout, stderr: "" };
    }),
  };
}

describe("release publication transaction", () => {
  test("creates, reconciles, verifies, and publishes an absent release", async () => {
    const root = await mkdtemp(join(tmpdir(), "cclover-release-publication-test-"));
    try {
      await completeFixture(root);
      const summary = await verifyReleaseArtifacts(root, { version: VERSION, commit: COMMIT });
      const gateway = new FakeGitHubPublicationGateway();

      await publishGitHubRelease(summary, root, `v${VERSION}`, gateway);

      expect(gateway.release?.draft).toBe(false);
      expect(gateway.mutations).toEqual(["create-draft", "upload", "publish"]);
      expect(gateway.release?.assets.map((asset) => asset.name).sort()).toEqual(
        [
          ...summary.artifacts.map((artifact) => artifact.archive),
          ...summary.companionAssets.map((asset) => asset.name),
          "release-manifest.json",
        ].sort(),
      );
    } finally {
      await rm(root, { recursive: true, force: true });
    }
  });

  test("repairs only a draft and removes unexpected assets before publication", async () => {
    const root = await mkdtemp(join(tmpdir(), "cclover-release-publication-test-"));
    try {
      await completeFixture(root);
      const summary = await verifyReleaseArtifacts(root, { version: VERSION, commit: COMMIT });
      const gateway = new FakeGitHubPublicationGateway();
      gateway.release = {
        id: 1,
        tag: `v${VERSION}`,
        draft: true,
        assets: [{ id: 9, name: "stale.zip", size: 1, digest: null }],
      };

      await publishGitHubRelease(summary, root, `v${VERSION}`, gateway);

      expect(gateway.mutations).toEqual(["delete:stale.zip", "upload", "publish"]);
      expect(gateway.release?.draft).toBe(false);
    } finally {
      await rm(root, { recursive: true, force: true });
    }
  });

  test("leaves a failed upload as a repairable draft and converges on retry", async () => {
    const root = await mkdtemp(join(tmpdir(), "cclover-release-publication-test-"));
    try {
      await completeFixture(root);
      const summary = await verifyReleaseArtifacts(root, { version: VERSION, commit: COMMIT });
      const gateway = new FakeGitHubPublicationGateway();
      gateway.failUploadAfter = 2;

      await expect(publishGitHubRelease(summary, root, `v${VERSION}`, gateway)).rejects.toThrow(
        "simulated upload failure",
      );
      expect(gateway.release?.draft).toBe(true);
      expect(gateway.mutations).not.toContain("publish");

      await publishGitHubRelease(summary, root, `v${VERSION}`, gateway);
      expect(gateway.release?.draft).toBe(false);
      expect(gateway.mutations.at(-1)).toBe("publish");
    } finally {
      await rm(root, { recursive: true, force: true });
    }
  });

  test("treats a lost publish response as success on the next retry", async () => {
    const root = await mkdtemp(join(tmpdir(), "cclover-release-publication-test-"));
    try {
      await completeFixture(root);
      const summary = await verifyReleaseArtifacts(root, { version: VERSION, commit: COMMIT });
      const gateway = new FakeGitHubPublicationGateway();
      gateway.failPublishAfterMutation = true;

      await expect(publishGitHubRelease(summary, root, `v${VERSION}`, gateway)).rejects.toThrow(
        "simulated lost publish response",
      );
      expect(gateway.release?.draft).toBe(false);
      const mutationCount = gateway.mutations.length;

      await publishGitHubRelease(summary, root, `v${VERSION}`, gateway);
      expect(gateway.mutations).toHaveLength(mutationCount);
    } finally {
      await rm(root, { recursive: true, force: true });
    }
  });

  test("never mutates an already-public release whose assets do not match", async () => {
    const root = await mkdtemp(join(tmpdir(), "cclover-release-publication-test-"));
    try {
      await completeFixture(root);
      const summary = await verifyReleaseArtifacts(root, { version: VERSION, commit: COMMIT });
      const gateway = new FakeGitHubPublicationGateway();
      gateway.release = {
        id: 1,
        tag: `v${VERSION}`,
        draft: false,
        assets: [{ id: 1, name: "wrong.zip", size: 1, digest: null }],
      };

      await expect(publishGitHubRelease(summary, root, `v${VERSION}`, gateway)).rejects.toThrow(
        "GitHub release asset set mismatch",
      );
      expect(gateway.mutations).toEqual([]);
    } finally {
      await rm(root, { recursive: true, force: true });
    }
  });

  test("rejects a moved tag before any release mutation", async () => {
    const root = await mkdtemp(join(tmpdir(), "cclover-release-publication-test-"));
    try {
      await completeFixture(root);
      const summary = await verifyReleaseArtifacts(root, { version: VERSION, commit: COMMIT });
      const gateway = new FakeGitHubPublicationGateway();
      gateway.tagCommit = "f".repeat(40);

      await expect(publishGitHubRelease(summary, root, `v${VERSION}`, gateway)).rejects.toThrow(
        `expected verified commit ${COMMIT}`,
      );
      expect(gateway.mutations).toEqual([]);
    } finally {
      await rm(root, { recursive: true, force: true });
    }
  });

  test("detects local asset mutation before contacting GitHub", async () => {
    const root = await mkdtemp(join(tmpdir(), "cclover-release-publication-test-"));
    try {
      await completeFixture(root);
      const summary = await verifyReleaseArtifacts(root, { version: VERSION, commit: COMMIT });
      await writeFile(join(root, summary.artifacts[0]!.archive), "changed after verification");
      const gateway = new FakeGitHubPublicationGateway();

      await expect(publishGitHubRelease(summary, root, `v${VERSION}`, gateway)).rejects.toThrow(
        "publication asset digest changed after verification",
      );
      expect(gateway.mutations).toEqual([]);
    } finally {
      await rm(root, { recursive: true, force: true });
    }
  });

  test("facade verification fails before any GitHub mutation", async () => {
    const root = await mkdtemp(join(tmpdir(), "cclover-release-publication-test-"));
    try {
      const git = Bun.spawnSync({ cmd: ["git", "rev-parse", "HEAD"], stdout: "pipe" });
      expect(git.exitCode).toBe(0);
      const currentCommit = git.stdout.toString().trim();
      await completeFixture(root, currentCommit);
      const omitted = RELEASE_ARTIFACTS[0]!;
      const archive = archiveName(omitted, VERSION);
      await rm(join(root, `${archive}.manifest.json`));
      const gateway = new FakeGitHubPublicationGateway();
      gateway.tagCommit = currentCommit;

      await expect(
        publishVerifiedReleaseArtifacts(root, `v${VERSION}`, gateway),
      ).rejects.toThrow("release manifest count mismatch");
      expect(gateway.mutations).toEqual([]);
    } finally {
      await rm(root, { recursive: true, force: true });
    }
  });
});

describe("GitHub publication adapter contract", () => {
  test("findRelease uses paginated REST listing and parses asset identity", async () => {
    const response = JSON.stringify([
      [
        {
          id: 4,
          tag_name: "v0.0.9",
          draft: false,
          assets: [],
        },
      ],
      [
        {
          id: 7,
          tag_name: `v${VERSION}`,
          draft: true,
          assets: [
            {
              id: 11,
              name: "artifact.zip",
              size: 123,
              digest: `sha256:${"a".repeat(64)}`,
            },
          ],
        },
      ],
    ]);
    const { gateway, calls } = recordingGateway(response);

    expect(await gateway.findRelease(`v${VERSION}`)).toEqual({
      id: 7,
      tag: `v${VERSION}`,
      draft: true,
      assets: [
        {
          id: 11,
          name: "artifact.zip",
          size: 123,
          digest: `sha256:${"a".repeat(64)}`,
        },
      ],
    });
    expect(calls).toEqual([
      ["api", "repos/{owner}/{repo}/releases?per_page=100", "--paginate", "--slurp"],
    ]);
  });

  test("findRelease rejects malformed API shape instead of guessing", async () => {
    const { gateway } = recordingGateway(JSON.stringify([[{ id: 7, tag_name: `v${VERSION}`, draft: "false", assets: [] }]]));
    await expect(gateway.findRelease(`v${VERSION}`)).rejects.toThrow("GitHub release.draft is invalid");
  });

  test("draft mutation methods emit exact gh commands", async () => {
    const { gateway, calls } = recordingGateway();
    const assets = ["/tmp/a.zip", "/tmp/release-manifest.json"];

    await gateway.createDraft(`v${VERSION}`);
    await gateway.deleteAsset(`v${VERSION}`, "stale.zip");
    await gateway.uploadAssets(`v${VERSION}`, assets);
    await gateway.publishDraft(`v${VERSION}`);

    expect(calls).toEqual([
      [
        "release",
        "create",
        `v${VERSION}`,
        "--draft",
        "--verify-tag",
        "--generate-notes",
        "--title",
        `v${VERSION}`,
      ],
      ["release", "delete-asset", `v${VERSION}`, "stale.zip", "--yes"],
      ["release", "upload", `v${VERSION}`, ...assets, "--clobber"],
      ["release", "edit", `v${VERSION}`, "--draft=false"],
    ]);
  });

  test("dereferences annotated Git tags to their commit identity", async () => {
    const calls: string[][] = [];
    const gateway = new GhGitHubPublicationGateway((args) => {
      calls.push([...args]);
      if (args[1]?.includes("/git/ref/tags/")) {
        return {
          stdout: JSON.stringify({ object: { type: "tag", sha: "a".repeat(40) } }),
          stderr: "",
        };
      }
      return {
        stdout: JSON.stringify({ object: { type: "commit", sha: COMMIT } }),
        stderr: "",
      };
    });

    expect(await gateway.resolveTagCommit(`v${VERSION}`)).toBe(COMMIT);
    expect(calls).toEqual([
      ["api", `repos/{owner}/{repo}/git/ref/tags/v${VERSION}`],
      ["api", `repos/{owner}/{repo}/git/tags/${"a".repeat(40)}`],
    ]);
  });
});
