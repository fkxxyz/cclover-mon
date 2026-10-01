import { existsSync, readFileSync, realpathSync } from "node:fs";
import { homedir } from "node:os";
import { isAbsolute, join, relative, resolve, sep } from "node:path";

import {
  type ValidationProfile,
  type ValidationPrivilege,
  validationExecution,
} from "../../validate";

export interface PathMapping {
  localPrefix: string;
  windowsPrefix: string;
}

export interface WindowsValidationConfig {
  powershell?: string;
  pathMappings?: readonly PathMapping[];
}

export interface HostEnvironment {
  platform: NodeJS.Platform;
  env: Readonly<Record<string, string | undefined>>;
  config: WindowsValidationConfig;
}

export interface DoctorResult {
  repository: string;
  windowsRepository?: string;
  powershell?: string;
  bridgeChecked: boolean;
  exitCode: number;
  message?: string;
}

export const DEFAULT_CONFIG_PATH = join(
  homedir(),
  ".config",
  "cclover-mon",
  "windows-validation.json",
);

export function configPath(env: Readonly<Record<string, string | undefined>> = process.env): string {
  const configured = env.CCLOVER_WINDOWS_VALIDATION_CONFIG;
  return configured ? resolve(configured) : DEFAULT_CONFIG_PATH;
}

export function loadConfig(
  env: Readonly<Record<string, string | undefined>> = process.env,
): WindowsValidationConfig {
  const path = configPath(env);
  if (!existsSync(path)) return {};
  const parsed = JSON.parse(readFileSync(path, "utf8")) as WindowsValidationConfig;
  if (parsed.pathMappings && !Array.isArray(parsed.pathMappings)) {
    throw new Error(`${path}: pathMappings must be an array`);
  }
  return parsed;
}

function normalizedLocalPath(path: string): string {
  return resolve(path);
}

function matchesPrefix(path: string, prefix: string): boolean {
  const suffix = relative(prefix, path);
  return suffix === "" || (!suffix.startsWith("..") && !isAbsolute(suffix));
}

function appendWindowsPath(prefix: string, suffix: string): string {
  if (!suffix) return prefix.replace(/[\\/]+$/, "");
  const cleanPrefix = prefix.replace(/[\\/]+$/, "");
  return `${cleanPrefix}\\${suffix.split(sep).join("\\")}`;
}

function configuredPathMapping(
  localPath: string,
  mappings: readonly PathMapping[],
): string | undefined {
  const candidates = mappings
    .map((mapping) => ({ ...mapping, localPrefix: normalizedLocalPath(mapping.localPrefix) }))
    .filter((mapping) => matchesPrefix(localPath, mapping.localPrefix))
    .sort((a, b) => b.localPrefix.length - a.localPrefix.length);
  const mapping = candidates[0];
  if (!mapping) return undefined;
  return appendWindowsPath(mapping.windowsPrefix, relative(mapping.localPrefix, localPath));
}

function wslDrivePath(localPath: string): string | undefined {
  const match = /^\/mnt\/([A-Za-z])(?:\/(.*))?$/.exec(localPath);
  if (!match) return undefined;
  const suffix = match[2]?.split("/").join("\\");
  return suffix ? `${match[1].toUpperCase()}:\\${suffix}` : `${match[1].toUpperCase()}:\\`;
}

export function mapRepositoryPath(
  repository: string,
  environment: Pick<HostEnvironment, "platform" | "env" | "config">,
): string {
  if (environment.platform === "win32") return repository;

  const localPath = normalizedLocalPath(repository);
  const configured = configuredPathMapping(localPath, environment.config.pathMappings ?? []);
  if (configured) return configured;

  const drivePath = wslDrivePath(localPath);
  if (drivePath) return drivePath;

  const distro = environment.env.WSL_DISTRO_NAME;
  if (distro) {
    return `\\\\wsl.localhost\\${distro}${localPath.split("/").join("\\")}`;
  }

  throw new Error(
    `no Windows path mapping for ${localPath}; configure ${configPath(environment.env)}`,
  );
}

export function discoverPowerShell(
  environment: Pick<HostEnvironment, "platform" | "config">,
): string | undefined {
  if (environment.platform === "win32") return "powershell.exe";
  if (environment.config.powershell) return environment.config.powershell;
  return (
    Bun.which("powershell.exe") ??
    (existsSync("/mnt/c/Windows/System32/WindowsPowerShell/v1.0/powershell.exe")
      ? "/mnt/c/Windows/System32/WindowsPowerShell/v1.0/powershell.exe"
      : undefined)
  );
}

export function windowsProfilePrivilege(profile: ValidationProfile): ValidationPrivilege {
  const execution = validationExecution(profile);
  if (execution.host !== "windows") {
    throw new Error(`${profile} is not a Windows-host validation profile`);
  }
  return execution.privilege;
}

export function repositoryRoot(cwd = process.cwd()): string {
  const result = Bun.spawnSync({
    cmd: ["git", "rev-parse", "--show-toplevel"],
    cwd,
    stdout: "pipe",
    stderr: "pipe",
  });
  if (result.exitCode !== 0) {
    throw new Error(String(result.stderr).trim() || "unable to resolve repository root");
  }
  return realpathSync(String(result.stdout).trim());
}

function windowsScriptPath(windowsRepository: string, script: string): string {
  return `${windowsRepository.replace(/[\\/]+$/, "")}\\tools\\windows-validation\\${script}`;
}

function powershellBase(powershell: string): string[] {
  return [powershell, "-NoLogo", "-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass"];
}

export function runWindowsValidation(
  profile: ValidationProfile,
  cwd = process.cwd(),
  environment: HostEnvironment = {
    platform: process.platform,
    env: process.env,
    config: loadConfig(),
  },
): number {
  const privilege = windowsProfilePrivilege(profile);
  const repository = repositoryRoot(cwd);

  if (environment.platform === "win32") {
    return Bun.spawnSync({
      cmd: ["bun", "validate.ts", profile],
      cwd: repository,
      stdout: "inherit",
      stderr: "inherit",
    }).exitCode;
  }

  const powershell = discoverPowerShell(environment);
  if (!powershell) {
    throw new Error(
      `Windows PowerShell transport not found; configure ${configPath(environment.env)} or run from WSL with Windows interop enabled`,
    );
  }
  const windowsRepository = mapRepositoryPath(repository, environment);
  const invoke = windowsScriptPath(windowsRepository, "invoke.ps1");
  return Bun.spawnSync({
    cmd: [
      ...powershellBase(powershell),
      "-File",
      invoke,
      "-Repository",
      windowsRepository,
      "-Profile",
      profile,
      "-Privilege",
      privilege,
    ],
    stdout: "inherit",
    stderr: "inherit",
  }).exitCode;
}

export function installWindowsValidationBridge(
  cwd = process.cwd(),
  environment: HostEnvironment = {
    platform: process.platform,
    env: process.env,
    config: loadConfig(),
  },
): number {
  const repository = repositoryRoot(cwd);
  const powershell = discoverPowerShell(environment);
  if (!powershell) {
    throw new Error(`Windows PowerShell transport not found; configure ${configPath(environment.env)}`);
  }
  const windowsRepository = mapRepositoryPath(repository, environment);
  const install = windowsScriptPath(windowsRepository, "install.ps1");
  return Bun.spawnSync({
    cmd: [...powershellBase(powershell), "-File", install],
    stdout: "inherit",
    stderr: "inherit",
  }).exitCode;
}

export function doctorWindowsValidation(
  cwd = process.cwd(),
  environment: HostEnvironment = {
    platform: process.platform,
    env: process.env,
    config: loadConfig(),
  },
): DoctorResult {
  const repository = repositoryRoot(cwd);
  if (environment.platform === "win32") {
    const powershell = discoverPowerShell(environment);
    if (!powershell) {
      return { repository, bridgeChecked: false, exitCode: 1, message: "PowerShell not found" };
    }
    const script = windowsScriptPath(repository, "invoke.ps1");
    const result = Bun.spawnSync({
      cmd: [...powershellBase(powershell), "-File", script, "-Repository", repository, "-Doctor"],
      stdout: "inherit",
      stderr: "inherit",
    });
    return {
      repository,
      windowsRepository: repository,
      powershell,
      bridgeChecked: true,
      exitCode: result.exitCode,
    };
  }

  const powershell = discoverPowerShell(environment);
  let windowsRepository: string;
  try {
    windowsRepository = mapRepositoryPath(repository, environment);
  } catch (error) {
    return {
      repository,
      powershell,
      bridgeChecked: false,
      exitCode: 1,
      message: error instanceof Error ? error.message : String(error),
    };
  }
  if (!powershell) {
    return {
      repository,
      windowsRepository,
      bridgeChecked: false,
      exitCode: 1,
      message: `Windows PowerShell transport not found; configure ${configPath(environment.env)}`,
    };
  }

  const invoke = windowsScriptPath(windowsRepository, "invoke.ps1");
  const result = Bun.spawnSync({
    cmd: [
      ...powershellBase(powershell),
      "-File",
      invoke,
      "-Repository",
      windowsRepository,
      "-Doctor",
    ],
    stdout: "inherit",
    stderr: "inherit",
  });
  return {
    repository,
    windowsRepository,
    powershell,
    bridgeChecked: true,
    exitCode: result.exitCode,
  };
}
