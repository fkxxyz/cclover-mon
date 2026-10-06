import { describe, expect, test } from "bun:test";

import {
  mapRepositoryPath,
  windowsProfileExecutionPrivilege,
  type WindowsValidationConfig,
} from "./tools/windows-validation/host";

function environment(
  config: WindowsValidationConfig = {},
  env: Record<string, string | undefined> = {},
) {
  return { platform: "linux" as const, config, env };
}

describe("Windows validation host execution", () => {
  test("profile privilege requirement maps to Windows execution privilege", () => {
    expect(windowsProfileExecutionPrivilege("windows-native")).toBe("ordinary");
    expect(windowsProfileExecutionPrivilege("windows-etw-runtime")).toBe("elevated");
    expect(() => windowsProfileExecutionPrivilege("windows")).toThrow(
      "windows is not a Windows-host validation profile",
    );
  });

  test("configured prefix mapping preserves arbitrary worktree suffixes", () => {
    const config = {
      pathMappings: [
        {
          localPrefix: "/run/media/fkxxyz/wsl",
          windowsPrefix: "\\\\wsl.localhost\\Arch",
        },
      ],
    };
    expect(
      mapRepositoryPath(
        "/run/media/fkxxyz/wsl/home/fkxxyz/pro/fkxxyz/cclover-mon-wt-validation",
        environment(config),
      ),
    ).toBe("\\\\wsl.localhost\\Arch\\home\\fkxxyz\\pro\\fkxxyz\\cclover-mon-wt-validation");
  });

  test("most specific configured path mapping wins", () => {
    const config = {
      pathMappings: [
        { localPrefix: "/srv", windowsPrefix: "Z:\\srv" },
        { localPrefix: "/srv/worktrees", windowsPrefix: "Y:\\worktrees" },
      ],
    };
    expect(mapRepositoryPath("/srv/worktrees/repo", environment(config))).toBe(
      "Y:\\worktrees\\repo",
    );
  });

  test("WSL distro provides repository mapping without machine config", () => {
    expect(
      mapRepositoryPath(
        "/home/fkxxyz/pro/fkxxyz/cclover-mon",
        environment({}, { WSL_DISTRO_NAME: "Arch" }),
      ),
    ).toBe("\\\\wsl.localhost\\Arch\\home\\fkxxyz\\pro\\fkxxyz\\cclover-mon");
  });

  test("WSL mounted Windows drives map directly", () => {
    expect(mapRepositoryPath("/mnt/c/src/cclover-mon", environment())).toBe(
      "C:\\src\\cclover-mon",
    );
  });

  test("non-WSL Linux requires an explicit path mapping", () => {
    expect(() => mapRepositoryPath("/home/fkxxyz/cclover-mon", environment())).toThrow(
      "no Windows path mapping",
    );
  });
});
