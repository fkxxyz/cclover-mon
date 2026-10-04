export type ReleasePlatform = "linux" | "windows";
export type ArchiveFormat = "tar.gz" | "zip";
export type NativeToolRole = "c-compiler" | "archiver" | "bpf-compiler" | "linker-driver" | "linker";
export type ArchiveToolRole = "archiver" | "compressor";

export interface ReleaseArtifact {
  readonly id: string;
  readonly product: "cclover-mon" | "cclover-mon-server";
  readonly cargoPackage: "cclover-mon" | "cclover-server";
  readonly binaryName: "cclover-mon" | "cclover-mon-server";
  readonly target: string;
  readonly platform: ReleasePlatform;
  readonly archiveFormat: ArchiveFormat;
  readonly staticFiles: readonly PackageFile[];
}

export interface PackageFile {
  readonly source: string;
  readonly destination: string;
}

export interface ToolIdentity {
  readonly command: string;
  readonly version?: string;
  readonly executableSha256?: string;
}

export interface NativeToolIdentity extends ToolIdentity {
  readonly role: NativeToolRole;
}

export interface ArchiveToolIdentity extends ToolIdentity {
  readonly role: ArchiveToolRole;
}

export interface BuildProvenance {
  readonly schemaVersion: 1;
  readonly host: {
    readonly os: string;
    readonly arch: string;
    readonly osRelease?: string;
    readonly runnerEnvironment?: string;
    readonly runnerImage?: string;
    readonly runnerImageVersion?: string;
  };
  readonly rustc: ToolIdentity;
  readonly cargo: ToolIdentity;
  readonly nativeTools: readonly NativeToolIdentity[];
  readonly archiveTools: readonly ArchiveToolIdentity[];
  readonly windows?: {
    readonly cargoXwin: ToolIdentity;
    readonly xwinArch: string;
    readonly sdkVersion: string;
    readonly crtVersion: string;
    readonly sysrootManifestSha256: string;
  };
}

export interface ReleaseManifest {
  readonly schemaVersion: 2;
  readonly artifactId: string;
  readonly product: ReleaseArtifact["product"];
  readonly version: string;
  readonly target: string;
  readonly gitCommit: string;
  readonly archive: string;
  readonly sha256: string;
  readonly buildProvenance: BuildProvenance;
}

export interface ReleaseSummary {
  readonly schemaVersion: 2;
  readonly version: string;
  readonly gitCommit: string;
  readonly artifacts: readonly ReleaseManifest[];
}

const PROJECT_LICENSE: PackageFile = { source: "LICENSE", destination: "LICENSE" };
const WINDOWS_LICENSE_FILES: readonly PackageFile[] = [
  {
    source: "LICENSES/PawnIO-NOTICE.txt",
    destination: "LICENSES/PawnIO-NOTICE.txt",
  },
  {
    source: "LICENSES/LGPL-2.1-or-later.txt",
    destination: "LICENSES/LGPL-2.1-or-later.txt",
  },
  {
    source: "LICENSES/MPL-2.0.txt",
    destination: "LICENSES/MPL-2.0.txt",
  },
];
const LINUX_SERVER_FILES: readonly PackageFile[] = [
  {
    source: "packaging/server/systemd/cclover-mon-server.service",
    destination: "systemd/cclover-mon-server.service",
  },
  {
    source: "packaging/server/systemd/ebpf-io.conf",
    destination: "systemd/ebpf-io.conf",
  },
];
const WINDOWS_SERVER_FILES: readonly PackageFile[] = [
  {
    source: "packaging/server/windows/install-service.ps1",
    destination: "install-service.ps1",
  },
];

function artifact(
  product: ReleaseArtifact["product"],
  cargoPackage: ReleaseArtifact["cargoPackage"],
  binaryName: ReleaseArtifact["binaryName"],
  target: string,
  platform: ReleasePlatform,
  archiveFormat: ArchiveFormat,
  staticFiles: readonly PackageFile[],
): ReleaseArtifact {
  return {
    id: `${product}-${target}`,
    product,
    cargoPackage,
    binaryName,
    target,
    platform,
    archiveFormat,
    staticFiles,
  };
}

const LINUX_TARGET = "x86_64-unknown-linux-gnu";
const WINDOWS_X64_TARGET = "x86_64-pc-windows-msvc";
const WINDOWS_X86_TARGET = "i686-pc-windows-msvc";

// This explicit list is the release product/target authority. Do not reconstruct
// a product × target matrix in workflow files or packaging code.
export const RELEASE_ARTIFACTS: readonly ReleaseArtifact[] = [
  artifact(
    "cclover-mon",
    "cclover-mon",
    "cclover-mon",
    LINUX_TARGET,
    "linux",
    "tar.gz",
    [PROJECT_LICENSE],
  ),
  artifact(
    "cclover-mon-server",
    "cclover-server",
    "cclover-mon-server",
    LINUX_TARGET,
    "linux",
    "tar.gz",
    [PROJECT_LICENSE, ...LINUX_SERVER_FILES],
  ),
  artifact(
    "cclover-mon",
    "cclover-mon",
    "cclover-mon",
    WINDOWS_X64_TARGET,
    "windows",
    "zip",
    [PROJECT_LICENSE, ...WINDOWS_LICENSE_FILES],
  ),
  artifact(
    "cclover-mon-server",
    "cclover-server",
    "cclover-mon-server",
    WINDOWS_X64_TARGET,
    "windows",
    "zip",
    [PROJECT_LICENSE, ...WINDOWS_LICENSE_FILES, ...WINDOWS_SERVER_FILES],
  ),
  artifact(
    "cclover-mon",
    "cclover-mon",
    "cclover-mon",
    WINDOWS_X86_TARGET,
    "windows",
    "zip",
    [PROJECT_LICENSE, ...WINDOWS_LICENSE_FILES],
  ),
  artifact(
    "cclover-mon-server",
    "cclover-server",
    "cclover-mon-server",
    WINDOWS_X86_TARGET,
    "windows",
    "zip",
    [PROJECT_LICENSE, ...WINDOWS_LICENSE_FILES, ...WINDOWS_SERVER_FILES],
  ),
];

export function releaseArtifact(id: string): ReleaseArtifact {
  const found = RELEASE_ARTIFACTS.find((candidate) => candidate.id === id);
  if (!found) throw new Error(`unknown release artifact: ${id}`);
  return found;
}

export function releaseMatrix(): {
  include: { id: string; platform: ReleasePlatform; target: string }[];
} {
  return {
    include: RELEASE_ARTIFACTS.map(({ id, platform, target }) => ({ id, platform, target })),
  };
}

export function archiveName(artifact: ReleaseArtifact, version: string): string {
  return `${artifact.product}-v${version}-${artifact.target}.${artifact.archiveFormat}`;
}

export function stagingDirectoryName(artifact: ReleaseArtifact, version: string): string {
  return `${artifact.product}-v${version}-${artifact.target}`;
}

export function binaryFileName(artifact: ReleaseArtifact): string {
  return artifact.platform === "windows" ? `${artifact.binaryName}.exe` : artifact.binaryName;
}

export function validateReleaseTag(version: string, tag?: string): void {
  if (tag !== undefined && tag !== `v${version}`) {
    throw new Error(`release tag ${tag} does not match workspace version v${version}`);
  }
}
