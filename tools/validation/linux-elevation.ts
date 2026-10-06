import {
  type ValidationProfile,
  validationExecution,
} from "../../validate";

export interface LinuxElevationEnvironment {
  platform: NodeJS.Platform;
  uid: number | undefined;
  sudo: string | undefined;
}

function currentEnvironment(): LinuxElevationEnvironment {
  return {
    platform: process.platform,
    uid: typeof process.getuid === "function" ? process.getuid() : undefined,
    sudo: Bun.which("sudo") ?? undefined,
  };
}

function environmentArguments(environment: Readonly<Record<string, string>>): string[] {
  return Object.entries(environment).map(([key, value]) => `${key}=${value}`);
}

export function localElevatedCommand(
  profile: ValidationProfile,
  command: readonly string[],
  environment: Readonly<Record<string, string>> = {},
  host: LinuxElevationEnvironment = currentEnvironment(),
): string[] {
  const execution = validationExecution(profile);
  if (
    execution.host !== "local" ||
    execution.privilegeRequirement !== "elevation-required"
  ) {
    throw new Error(`${profile} does not require local elevation`);
  }
  if (host.platform !== "linux") {
    throw new Error(`${profile} local elevation requires Linux`);
  }
  if (host.uid === undefined) {
    throw new Error(`${profile} cannot determine the current Linux user ID`);
  }

  const explicitEnvironment = environmentArguments(environment);
  if (host.uid === 0) {
    return explicitEnvironment.length === 0
      ? [...command]
      : ["env", ...explicitEnvironment, ...command];
  }
  if (!host.sudo) {
    throw new Error(`${profile} requires local elevation, but sudo is unavailable`);
  }

  return explicitEnvironment.length === 0
    ? [host.sudo, "--", ...command]
    : [host.sudo, "--", "env", ...explicitEnvironment, ...command];
}
