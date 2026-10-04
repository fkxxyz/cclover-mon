import { createHash } from "node:crypto";
import { readFile, stat } from "node:fs/promises";
import { join, resolve } from "node:path";

import { XWIN_ARCH } from "../../validate";
import { archiveToolProvenance } from "./archive";
import type {
  BuildProvenance,
  NativeToolIdentity,
  NativeToolRole,
  ReleaseArtifact,
  ToolIdentity,
} from "./plan";

const DEFAULT_REPO_ROOT = resolve(import.meta.dir, "../..");

export interface BuildInvocation {
  readonly command: readonly string[];
  readonly env?: Readonly<Record<string, string>>;
}

export interface ResolvedBuildContext {
  readonly invocation: BuildInvocation;
  readonly provenance: BuildProvenance;
}

function capture(
  command: string,
  args: readonly string[],
  cwd: string,
  env: Readonly<Record<string, string | undefined>> = process.env,
): string {
  const result = Bun.spawnSync({
    cmd: [command, ...args],
    cwd,
    env,
    stdout: "pipe",
    stderr: "pipe",
  });
  if (result.exitCode !== 0) {
    const detail = result.stderr.toString().trim() || result.stdout.toString().trim();
    throw new Error(
      `${[command, ...args].join(" ")} failed with exit code ${result.exitCode}${detail ? `: ${detail}` : ""}`,
    );
  }
  return (result.stdout.toString().trim() || result.stderr.toString().trim()).trim();
}

function firstLine(value: string): string {
  const line = value.split(/\r?\n/).find((candidate) => candidate.trim().length > 0)?.trim();
  if (!line) throw new Error("tool version output is empty");
  return line;
}

function executable(value: string | undefined, fallback: string, label: string): string {
  const command = value?.trim() || fallback;
  if (/\s/.test(command)) {
    throw new Error(`${label} must name one executable so release provenance can identify it: ${command}`);
  }
  return command;
}

function toolIdentity(
  command: string,
  args: readonly string[],
  cwd: string,
  env: Readonly<Record<string, string | undefined>> = process.env,
): ToolIdentity {
  return { command, version: firstLine(capture(command, args, cwd, env)) };
}

function nativeTool(
  role: NativeToolRole,
  command: string,
  args: readonly string[],
  cwd: string,
  env: Readonly<Record<string, string | undefined>> = process.env,
): NativeToolIdentity {
  return { role, ...toolIdentity(command, args, cwd, env) };
}

async function executableSha256(
  command: string,
  env: Readonly<Record<string, string | undefined>>,
): Promise<string> {
  const candidates = command.includes("/")
    ? [command]
    : (env.PATH ?? "")
        .split(":")
        .filter((entry) => entry.length > 0)
        .map((entry) => join(entry, command));
  for (const candidate of candidates) {
    try {
      const metadata = await stat(candidate);
      if (!metadata.isFile()) continue;
      const hash = createHash("sha256");
      hash.update(new Uint8Array(await Bun.file(candidate).arrayBuffer()));
      return hash.digest("hex");
    } catch {
      // Continue through PATH candidates.
    }
  }
  throw new Error(`cannot resolve executable for provenance: ${command}`);
}

function buildCommand(artifact: ReleaseArtifact): readonly string[] {
  return [
    "build",
    "--locked",
    "--profile",
    "dist",
    "-p",
    artifact.cargoPackage,
    "--target",
    artifact.target,
  ];
}

export function buildInvocation(artifact: ReleaseArtifact): BuildInvocation {
  const command = buildCommand(artifact);
  if (artifact.platform === "windows") {
    return { command: ["cargo", "xwin", ...command], env: { XWIN_ARCH } };
  }
  return { command: ["cargo", ...command] };
}

export function validateHostedRunnerIdentity(
  env: Readonly<Record<string, string | undefined>>,
): void {
  const hostedGitHubRunner =
    env.GITHUB_ACTIONS === "true" && env.RUNNER_ENVIRONMENT !== "self-hosted";
  if (hostedGitHubRunner && (!env.ImageOS || !env.ImageVersion)) {
    throw new Error("GitHub-hosted release build is missing ImageOS/ImageVersion provenance");
  }
}

async function hostProvenance(): Promise<BuildProvenance["host"]> {
  validateHostedRunnerIdentity(process.env);
  let osRelease: string | undefined;
  try {
    const text = await readFile("/etc/os-release", "utf8");
    const match = text.match(/^PRETTY_NAME=(?:"([^"]+)"|([^\n]+))$/m);
    osRelease = (match?.[1] ?? match?.[2])?.trim();
  } catch {
    // Non-Linux or minimal build hosts may not expose /etc/os-release.
  }
  return {
    os: process.platform,
    arch: process.arch,
    ...(osRelease ? { osRelease } : {}),
    ...(process.env.RUNNER_ENVIRONMENT
      ? { runnerEnvironment: process.env.RUNNER_ENVIRONMENT }
      : {}),
    ...(process.env.ImageOS ? { runnerImage: process.env.ImageOS } : {}),
    ...(process.env.ImageVersion ? { runnerImageVersion: process.env.ImageVersion } : {}),
  };
}

function targetEnvSuffix(target: string): string {
  return target.replaceAll("-", "_");
}

function linuxCc(artifact: ReleaseArtifact): string {
  const suffix = targetEnvSuffix(artifact.target);
  return executable(
    process.env[`CC_${artifact.target}`] ??
      process.env[`CC_${suffix}`] ??
      process.env.TARGET_CC ??
      process.env.CC,
    "cc",
    "Linux C compiler",
  );
}

function linuxAr(artifact: ReleaseArtifact): string {
  const suffix = targetEnvSuffix(artifact.target);
  return executable(
    process.env[`AR_${artifact.target}`] ??
      process.env[`AR_${suffix}`] ??
      process.env.TARGET_AR ??
      process.env.AR,
    "ar",
    "Linux archiver",
  );
}

function linuxLinker(artifact: ReleaseArtifact): string {
  const key = `CARGO_TARGET_${targetEnvSuffix(artifact.target).toUpperCase()}_LINKER`;
  return executable(process.env[key], "cc", "Linux linker driver");
}

function linuxBuildData(
  artifact: ReleaseArtifact,
  repoRoot: string,
): { invocation: BuildInvocation; nativeTools: NativeToolIdentity[] } {
  const tools: NativeToolIdentity[] = [];
  const env: Record<string, string> = {};
  if (artifact.product === "cclover-mon") {
    const cc = linuxCc(artifact);
    tools.push(nativeTool("c-compiler", cc, ["--version"], repoRoot));
    env.CC = cc;
    const ar = linuxAr(artifact);
    tools.push(nativeTool("archiver", ar, ["--version"], repoRoot));
    env.AR = ar;
  }
  const bpfClang = executable(process.env.BPF_CLANG, "clang", "BPF_CLANG");
  tools.push(nativeTool("bpf-compiler", bpfClang, ["--version"], repoRoot));
  env.BPF_CLANG = bpfClang;
  const linkerDriver = linuxLinker(artifact);
  tools.push(nativeTool("linker-driver", linkerDriver, ["--version"], repoRoot));
  const linker = firstLine(capture(linkerDriver, ["-print-prog-name=ld"], repoRoot));
  tools.push(nativeTool("linker", linker, ["--version"], repoRoot));
  env[`CARGO_TARGET_${targetEnvSuffix(artifact.target).toUpperCase()}_LINKER`] = linkerDriver;
  return { invocation: { command: buildInvocation(artifact).command, env }, nativeTools: tools };
}

function parseShellExports(output: string): Record<string, string> {
  const result: Record<string, string> = {};
  for (const line of output.split(/\r?\n/)) {
    const match = line.match(/^export ([A-Za-z0-9_]+)="(.*)";$/);
    if (match) result[match[1]!] = match[2]!;
  }
  return result;
}

function oneMatch(values: readonly string[], label: string): string {
  const unique = [...new Set(values)];
  if (unique.length !== 1) {
    throw new Error(`expected one ${label} in cargo-xwin sysroot manifest, got ${JSON.stringify(unique)}`);
  }
  return unique[0]!;
}

export function parseXwinSysrootManifest(done: string): {
  sdkVersion: string;
  crtVersion: string;
  sha256: string;
} {
  const lines = done.split(/\r?\n/).filter((line) => line.length > 0);
  return {
    sdkVersion: oneMatch(
      lines.flatMap((line) => line.match(/Win(?:10|11)SDK_([0-9.]+)/)?.[1] ?? []),
      "Windows SDK version",
    ),
    crtVersion: oneMatch(
      lines.flatMap((line) => line.match(/Microsoft\.VC\.([0-9.]+)\.CRT\./)?.[1] ?? []),
      "MSVC CRT version",
    ),
    sha256: createHash("sha256").update(done).digest("hex"),
  };
}

async function windowsBuildData(
  artifact: ReleaseArtifact,
  repoRoot: string,
): Promise<{
  nativeTools: NativeToolIdentity[];
  windows: NonNullable<BuildProvenance["windows"]>;
}> {
  const xwinProcessEnv = { ...process.env, XWIN_ARCH };
  const xwinExports = parseShellExports(
    capture("cargo", ["xwin", "env", "--target", artifact.target], repoRoot, xwinProcessEnv),
  );
  const suffix = targetEnvSuffix(artifact.target);
  const compiler = xwinExports[`CC_${suffix}`] ?? xwinExports.TARGET_CC;
  const linker = xwinExports[`CARGO_TARGET_${suffix.toUpperCase()}_LINKER`];
  const archiver = xwinExports[`AR_${suffix}`] ?? xwinExports.TARGET_AR;
  if (!compiler || !linker || !archiver) {
    throw new Error(`cargo xwin env did not resolve compiler/linker/archiver for ${artifact.target}`);
  }

  const resolvedEnv = { ...process.env, ...xwinExports, XWIN_ARCH };
  const lib = xwinExports.LIB;
  if (!lib) throw new Error(`cargo xwin env did not expose LIB for ${artifact.target}`);
  const crtEntry = lib.split(";").find((entry) => entry.includes("/crt/lib/"));
  if (!crtEntry) throw new Error(`cargo xwin env did not expose a CRT sysroot for ${artifact.target}`);
  const marker = "/crt/lib/";
  const markerIndex = crtEntry.indexOf(marker);
  const sysroot = crtEntry.slice(0, markerIndex);
  const done = await readFile(join(sysroot, "DONE"), "utf8");
  const sysrootIdentity = parseXwinSysrootManifest(done);

  return {
    nativeTools: [
      nativeTool("c-compiler", executable(compiler, compiler, "Windows C compiler"), ["--version"], repoRoot, resolvedEnv),
      {
        role: "archiver",
        command: executable(archiver, archiver, "Windows archiver"),
        executableSha256: await executableSha256(archiver, resolvedEnv),
      },
      nativeTool("linker", executable(linker, linker, "Windows linker"), ["--version"], repoRoot, resolvedEnv),
    ],
    windows: {
      cargoXwin: {
        command: "cargo xwin",
        version: firstLine(capture("cargo", ["xwin", "--version"], repoRoot, xwinProcessEnv)),
      },
      xwinArch: XWIN_ARCH,
      sdkVersion: sysrootIdentity.sdkVersion,
      crtVersion: sysrootIdentity.crtVersion,
      sysrootManifestSha256: sysrootIdentity.sha256,
    },
  };
}

export async function resolveBuildContext(
  artifact: ReleaseArtifact,
  repoRoot = DEFAULT_REPO_ROOT,
): Promise<ResolvedBuildContext> {
  const rustc = executable(process.env.RUSTC, "rustc", "RUSTC");
  const base = {
    schemaVersion: 1 as const,
    host: await hostProvenance(),
    rustc: toolIdentity(rustc, ["-vV"], repoRoot),
    cargo: toolIdentity("cargo", ["-Vv"], repoRoot),
    archiveTools: archiveToolProvenance(artifact, repoRoot),
  };
  if (artifact.platform === "windows") {
    const windows = await windowsBuildData(artifact, repoRoot);
    return {
      invocation: buildInvocation(artifact),
      provenance: { ...base, nativeTools: windows.nativeTools, windows: windows.windows },
    };
  }
  const linux = linuxBuildData(artifact, repoRoot);
  return {
    invocation: linux.invocation,
    provenance: { ...base, nativeTools: linux.nativeTools },
  };
}

function requireString(value: unknown, label: string): string {
  if (typeof value !== "string" || value.length === 0) throw new Error(`missing ${label}`);
  return value;
}

function validateTool(value: unknown, label: string): void {
  if (!value || typeof value !== "object") throw new Error(`missing ${label}`);
  const tool = value as Partial<ToolIdentity>;
  requireString(tool.command, `${label} command`);
  const hasVersion = typeof tool.version === "string" && tool.version.length > 0;
  const hasExecutableDigest = /^[0-9a-f]{64}$/.test(tool.executableSha256 ?? "");
  if (!hasVersion && !hasExecutableDigest) {
    throw new Error(`missing ${label} version or executable SHA-256`);
  }
}

export function validateBuildProvenance(provenance: unknown, artifact: ReleaseArtifact): void {
  if (!provenance || typeof provenance !== "object") {
    throw new Error(`missing build provenance for ${artifact.id}`);
  }
  const value = provenance as Partial<BuildProvenance>;
  if (value.schemaVersion !== 1) throw new Error(`unsupported build provenance schema for ${artifact.id}`);
  if (!value.host || typeof value.host !== "object") throw new Error(`missing build host for ${artifact.id}`);
  requireString(value.host.os, `build host OS for ${artifact.id}`);
  requireString(value.host.arch, `build host architecture for ${artifact.id}`);
  validateTool(value.rustc, `rustc identity for ${artifact.id}`);
  validateTool(value.cargo, `cargo identity for ${artifact.id}`);
  if (!Array.isArray(value.archiveTools)) {
    throw new Error(`missing archive tool identities for ${artifact.id}`);
  }
  const archiveRoles = value.archiveTools.map((tool) => tool.role);
  const expectedArchiveRoles = artifact.archiveFormat === "tar.gz" ? ["archiver", "compressor"] : ["archiver"];
  if (JSON.stringify(archiveRoles) !== JSON.stringify(expectedArchiveRoles)) {
    throw new Error(`archive tool roles mismatch for ${artifact.id}`);
  }
  for (const tool of value.archiveTools) {
    validateTool(tool, `${tool.role} archive tool identity for ${artifact.id}`);
  }
  if (!Array.isArray(value.nativeTools)) throw new Error(`missing native tool identities for ${artifact.id}`);
  const tools = value.nativeTools as readonly NativeToolIdentity[];
  const roles = tools.map((tool) => tool.role);
  if (new Set(roles).size !== roles.length) throw new Error(`duplicate native tool role for ${artifact.id}`);
  for (const tool of tools) {
    if (!["c-compiler", "archiver", "bpf-compiler", "linker-driver", "linker"].includes(tool.role)) {
      throw new Error(`unknown native tool role for ${artifact.id}: ${String(tool.role)}`);
    }
    validateTool(tool, `${tool.role} identity for ${artifact.id}`);
  }
  const requiredRoles: NativeToolRole[] =
    artifact.platform === "windows"
      ? ["c-compiler", "archiver", "linker"]
      : artifact.product === "cclover-mon"
        ? ["c-compiler", "archiver", "bpf-compiler", "linker-driver", "linker"]
        : ["bpf-compiler", "linker-driver", "linker"];
  for (const role of requiredRoles) {
    if (!roles.includes(role)) throw new Error(`missing ${role} provenance for ${artifact.id}`);
  }

  if (artifact.platform === "windows") {
    if (!value.windows || typeof value.windows !== "object") {
      throw new Error(`missing Windows build provenance for ${artifact.id}`);
    }
    validateTool(value.windows.cargoXwin, `cargo-xwin identity for ${artifact.id}`);
    if (value.windows.xwinArch !== XWIN_ARCH) {
      throw new Error(`Windows provenance XWIN_ARCH mismatch for ${artifact.id}`);
    }
    requireString(value.windows.sdkVersion, `Windows SDK version for ${artifact.id}`);
    requireString(value.windows.crtVersion, `MSVC CRT version for ${artifact.id}`);
    if (!/^[0-9a-f]{64}$/.test(value.windows.sysrootManifestSha256 ?? "")) {
      throw new Error(`invalid cargo-xwin sysroot manifest SHA-256 for ${artifact.id}`);
    }
  } else if (value.windows !== undefined) {
    throw new Error(`unexpected Windows build provenance for ${artifact.id}`);
  }
}
