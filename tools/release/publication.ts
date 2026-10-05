import { createHash } from "node:crypto";
import { stat } from "node:fs/promises";
import { join, resolve } from "node:path";

import type { ReleaseSummary } from "./plan";

interface ExpectedAsset {
  readonly name: string;
  readonly path: string;
  readonly size: number;
  readonly sha256: string;
}

interface ExpectedPublication {
  readonly tag: string;
  readonly gitCommit: string;
  readonly assets: readonly ExpectedAsset[];
}

export interface GitHubReleaseAsset {
  readonly id: number;
  readonly name: string;
  readonly size: number;
  readonly digest?: string | null;
}

export interface GitHubReleaseState {
  readonly id: number;
  readonly tag: string;
  readonly draft: boolean;
  readonly assets: readonly GitHubReleaseAsset[];
}

export interface GitHubPublicationGateway {
  resolveTagCommit(tag: string): Promise<string>;
  findRelease(tag: string): Promise<GitHubReleaseState | undefined>;
  createDraft(tag: string): Promise<void>;
  deleteAsset(tag: string, name: string): Promise<void>;
  uploadAssets(tag: string, paths: readonly string[]): Promise<void>;
  publishDraft(tag: string): Promise<void>;
}

interface GhResult {
  readonly stdout: string;
  readonly stderr: string;
}

type GhExecutor = (args: readonly string[]) => GhResult;

function executeGh(args: readonly string[]): GhResult {
  const result = Bun.spawnSync({
    cmd: ["gh", ...args],
    stdout: "pipe",
    stderr: "pipe",
  });
  const stdout = result.stdout.toString();
  const stderr = result.stderr.toString();
  if (result.exitCode !== 0) {
    const detail = stderr.trim() || stdout.trim() || `exit code ${result.exitCode}`;
    throw new Error(`gh ${args.join(" ")} failed: ${detail}`);
  }
  return { stdout, stderr };
}

function parseObject(value: unknown, label: string): Record<string, unknown> {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    throw new Error(`${label} is not an object`);
  }
  return value as Record<string, unknown>;
}

function requiredString(object: Record<string, unknown>, field: string, label: string): string {
  const value = object[field];
  if (typeof value !== "string" || value.length === 0) throw new Error(`${label}.${field} is invalid`);
  return value;
}

function requiredNumber(object: Record<string, unknown>, field: string, label: string): number {
  const value = object[field];
  if (typeof value !== "number" || !Number.isSafeInteger(value) || value < 0) {
    throw new Error(`${label}.${field} is invalid`);
  }
  return value;
}

function parseReleaseAsset(value: unknown): GitHubReleaseAsset {
  const object = parseObject(value, "GitHub release asset");
  const digest = object.digest;
  if (digest !== undefined && digest !== null && typeof digest !== "string") {
    throw new Error("GitHub release asset.digest is invalid");
  }
  return {
    id: requiredNumber(object, "id", "GitHub release asset"),
    name: requiredString(object, "name", "GitHub release asset"),
    size: requiredNumber(object, "size", "GitHub release asset"),
    digest,
  };
}

function parseRelease(value: unknown): GitHubReleaseState {
  const object = parseObject(value, "GitHub release");
  const draft = object.draft;
  const assets = object.assets;
  if (typeof draft !== "boolean") throw new Error("GitHub release.draft is invalid");
  if (!Array.isArray(assets)) throw new Error("GitHub release.assets is invalid");
  return {
    id: requiredNumber(object, "id", "GitHub release"),
    tag: requiredString(object, "tag_name", "GitHub release"),
    draft,
    assets: assets.map(parseReleaseAsset),
  };
}

function parseReleasePages(text: string): GitHubReleaseState[] {
  let parsed: unknown;
  try {
    parsed = JSON.parse(text);
  } catch (error) {
    throw new Error(`invalid GitHub release response: ${error instanceof Error ? error.message : String(error)}`);
  }
  if (!Array.isArray(parsed)) throw new Error("GitHub release response is not a page array");
  const releases: GitHubReleaseState[] = [];
  for (const page of parsed) {
    if (!Array.isArray(page)) throw new Error("GitHub release response contains a non-array page");
    releases.push(...page.map(parseRelease));
  }
  return releases;
}

export class GhGitHubPublicationGateway implements GitHubPublicationGateway {
  constructor(private readonly execute: GhExecutor = executeGh) {}

  async resolveTagCommit(tag: string): Promise<string> {
    const encodedTag = encodeURIComponent(tag);
    let object = this.gitObject(`repos/{owner}/{repo}/git/ref/tags/${encodedTag}`, `tag ${tag}`);
    const visited = new Set<string>();
    for (let depth = 0; depth < 16; depth += 1) {
      if (object.type === "commit") return object.sha;
      if (object.type !== "tag") {
        throw new Error(`tag ${tag} resolves to unsupported Git object type ${object.type}`);
      }
      if (visited.has(object.sha)) throw new Error(`tag ${tag} contains a cyclic tag-object chain`);
      visited.add(object.sha);
      object = this.gitObject(`repos/{owner}/{repo}/git/tags/${object.sha}`, `tag object ${object.sha}`);
    }
    throw new Error(`tag ${tag} exceeds the supported annotated-tag indirection depth`);
  }

  async findRelease(tag: string): Promise<GitHubReleaseState | undefined> {
    const response = this.execute([
      "api",
      "repos/{owner}/{repo}/releases?per_page=100",
      "--paginate",
      "--slurp",
    ]);
    const matches = parseReleasePages(response.stdout).filter((release) => release.tag === tag);
    if (matches.length > 1) throw new Error(`multiple GitHub releases found for tag ${tag}`);
    return matches[0];
  }

  async createDraft(tag: string): Promise<void> {
    this.execute([
      "release",
      "create",
      tag,
      "--draft",
      "--verify-tag",
      "--generate-notes",
      "--title",
      tag,
    ]);
  }

  async deleteAsset(tag: string, name: string): Promise<void> {
    this.execute(["release", "delete-asset", tag, name, "--yes"]);
  }

  async uploadAssets(tag: string, paths: readonly string[]): Promise<void> {
    if (paths.length === 0) throw new Error("cannot publish a release without assets");
    this.execute(["release", "upload", tag, ...paths, "--clobber"]);
  }

  async publishDraft(tag: string): Promise<void> {
    this.execute(["release", "edit", tag, "--draft=false"]);
  }

  private gitObject(endpoint: string, label: string): { type: string; sha: string } {
    const response = this.execute(["api", endpoint]);
    let parsed: unknown;
    try {
      parsed = JSON.parse(response.stdout);
    } catch (error) {
      throw new Error(`invalid ${label} response: ${error instanceof Error ? error.message : String(error)}`);
    }
    const root = parseObject(parsed, label);
    const object = parseObject(root.object, `${label}.object`);
    return {
      type: requiredString(object, "type", `${label}.object`),
      sha: requiredString(object, "sha", `${label}.object`),
    };
  }
}

async function sha256File(path: string): Promise<string> {
  const hash = createHash("sha256");
  const reader = Bun.file(path).stream().getReader();
  for (;;) {
    const { done, value } = await reader.read();
    if (done) break;
    hash.update(value);
  }
  return hash.digest("hex");
}

async function expectedAsset(path: string, expectedSha256?: string): Promise<ExpectedAsset> {
  let metadata;
  try {
    metadata = await stat(path);
  } catch {
    throw new Error(`publication asset is missing: ${path}`);
  }
  if (!metadata.isFile()) throw new Error(`publication asset is not a file: ${path}`);
  const sha256 = await sha256File(path);
  if (expectedSha256 !== undefined && sha256 !== expectedSha256) {
    throw new Error(`publication asset digest changed after verification: ${path}`);
  }
  return {
    name: path.split(/[\\/]/).at(-1)!,
    path,
    size: metadata.size,
    sha256,
  };
}

async function expectedPublication(
  summary: ReleaseSummary,
  directory: string,
  tag: string,
): Promise<ExpectedPublication> {
  if (tag !== `v${summary.version}`) {
    throw new Error(`release tag ${tag} does not match verified release version v${summary.version}`);
  }
  if (!/^[0-9a-f]{40}$/.test(summary.gitCommit)) {
    throw new Error(`verified release has invalid git commit identity: ${summary.gitCommit}`);
  }
  const root = resolve(directory);
  const assets = await Promise.all([
    ...summary.artifacts.map((artifact) =>
      expectedAsset(join(root, artifact.archive), artifact.sha256),
    ),
    ...summary.companionAssets.map((asset) =>
      expectedAsset(join(root, asset.name), asset.sha256),
    ),
    expectedAsset(join(root, "release-manifest.json")),
  ]);
  const names = assets.map((asset) => asset.name);
  if (new Set(names).size !== names.length) throw new Error("verified release contains duplicate publication asset names");
  return { tag, gitCommit: summary.gitCommit, assets };
}

function sha256Digest(digest: string | null | undefined): string | undefined {
  if (!digest) return undefined;
  const match = /^sha256:([0-9a-f]{64})$/.exec(digest);
  return match?.[1];
}

function assertAssetSet(expected: ExpectedPublication, release: GitHubReleaseState): void {
  const remoteByName = new Map<string, GitHubReleaseAsset>();
  for (const asset of release.assets) {
    if (remoteByName.has(asset.name)) throw new Error(`GitHub release contains duplicate asset ${asset.name}`);
    remoteByName.set(asset.name, asset);
  }
  const expectedNames = new Set(expected.assets.map((asset) => asset.name));
  const remoteNames = new Set(remoteByName.keys());
  const missing = [...expectedNames].filter((name) => !remoteNames.has(name)).sort();
  const unexpected = [...remoteNames].filter((name) => !expectedNames.has(name)).sort();
  if (missing.length > 0 || unexpected.length > 0) {
    throw new Error(
      `GitHub release asset set mismatch for ${expected.tag}: missing=[${missing.join(", ")}], unexpected=[${unexpected.join(", ")}]`,
    );
  }
  for (const asset of expected.assets) {
    const remote = remoteByName.get(asset.name)!;
    if (remote.size !== asset.size) {
      throw new Error(
        `GitHub release asset size mismatch for ${asset.name}: expected ${asset.size}, got ${remote.size}`,
      );
    }
    const remoteSha256 = sha256Digest(remote.digest);
    if (remoteSha256 !== undefined && remoteSha256 !== asset.sha256) {
      throw new Error(
        `GitHub release asset digest mismatch for ${asset.name}: expected ${asset.sha256}, got ${remoteSha256}`,
      );
    }
  }
}

async function assertTagCommit(
  expected: ExpectedPublication,
  gateway: GitHubPublicationGateway,
): Promise<void> {
  const remoteCommit = await gateway.resolveTagCommit(expected.tag);
  if (remoteCommit !== expected.gitCommit) {
    throw new Error(
      `release tag ${expected.tag} points to ${remoteCommit}, expected verified commit ${expected.gitCommit}`,
    );
  }
}

export async function publishGitHubRelease(
  summary: ReleaseSummary,
  directory: string,
  tag: string,
  gateway: GitHubPublicationGateway = new GhGitHubPublicationGateway(),
): Promise<void> {
  const expected = await expectedPublication(summary, directory, tag);
  await assertTagCommit(expected, gateway);

  let release = await gateway.findRelease(tag);
  if (release && !release.draft) {
    assertAssetSet(expected, release);
    return;
  }

  if (!release) {
    await gateway.createDraft(tag);
    release = await gateway.findRelease(tag);
    if (!release) throw new Error(`GitHub draft release ${tag} was not observable after creation`);
    if (!release.draft) throw new Error(`new GitHub release ${tag} became public before asset reconciliation`);
  }

  const expectedNames = new Set(expected.assets.map((asset) => asset.name));
  for (const asset of release.assets) {
    if (!expectedNames.has(asset.name)) await gateway.deleteAsset(tag, asset.name);
  }

  await gateway.uploadAssets(tag, expected.assets.map((asset) => asset.path));

  release = await gateway.findRelease(tag);
  if (!release) throw new Error(`GitHub draft release ${tag} disappeared during publication`);
  if (!release.draft) throw new Error(`GitHub release ${tag} became public before verification completed`);
  assertAssetSet(expected, release);

  // A mutable tag can move independently of the draft. Re-check immediately
  // before the only operation that makes the release public.
  await assertTagCommit(expected, gateway);
  await gateway.publishDraft(tag);

  release = await gateway.findRelease(tag);
  if (!release) throw new Error(`GitHub release ${tag} disappeared after publication`);
  if (release.draft) throw new Error(`GitHub release ${tag} remained a draft after publication`);
  assertAssetSet(expected, release);
}
