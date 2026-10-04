import { rm, writeFile } from "node:fs/promises";
import { join } from "node:path";

import {
  RELEASE_ARTIFACTS,
  packageBuiltReleaseArtifact,
  type BuildProvenance,
  type NativeToolRole,
  type ReleaseArtifact,
} from "./release";

export const VERSION = "0.1.0";
export const COMMIT = "0123456789abcdef0123456789abcdef01234567";

export function fixtureProvenance(artifact: ReleaseArtifact): BuildProvenance {
  const roles: NativeToolRole[] =
    artifact.platform === "windows"
      ? ["c-compiler", "archiver", "linker"]
      : artifact.product === "cclover-mon"
        ? ["c-compiler", "archiver", "bpf-compiler", "linker-driver", "linker"]
        : ["bpf-compiler", "linker-driver", "linker"];
  return {
    schemaVersion: 1,
    host: { os: "linux", arch: "x64", osRelease: "fixture Linux" },
    rustc: { command: "rustc", version: "rustc fixture" },
    cargo: { command: "cargo", version: "cargo fixture" },
    nativeTools: roles.map((role) =>
      artifact.platform === "windows" && role === "archiver"
        ? { role, command: "llvm-lib", executableSha256: "b".repeat(64) }
        : { role, command: `fixture-${role}`, version: `${role} fixture` },
    ),
    archiveTools:
      artifact.archiveFormat === "tar.gz"
        ? [
            { role: "archiver", command: "tar", version: "tar fixture" },
            { role: "compressor", command: "gzip", version: "gzip fixture" },
          ]
        : [{ role: "archiver", command: "zip", version: "zip fixture" }],
    ...(artifact.platform === "windows"
      ? {
          windows: {
            cargoXwin: { command: "cargo", version: "cargo-xwin fixture" },
            xwinArch: "x86,x86_64",
            sdkVersion: "10.0.26100",
            crtVersion: "14.44.17.14",
            sysrootManifestSha256: "a".repeat(64),
          },
        }
      : {}),
  };
}

export async function sha256(path: string): Promise<string> {
  const hasher = new Bun.CryptoHasher("sha256");
  hasher.update(await Bun.file(path).arrayBuffer());
  return hasher.digest("hex");
}

export async function completeFixture(root: string, commit = COMMIT): Promise<void> {
  const binary = join(root, "fake-release-binary");
  await writeFile(binary, "fake executable bytes");
  for (const artifact of RELEASE_ARTIFACTS) {
    await packageBuiltReleaseArtifact(artifact, {
      outDir: root,
      binary,
      version: VERSION,
      commit,
      provenance: fixtureProvenance(artifact),
    });
  }
  await rm(binary);
}
