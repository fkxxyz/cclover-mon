export const XWIN_ARCH = "x86,x86_64";

export interface ValidationStep {
  name: string;
  command: readonly string[];
  env?: Readonly<Record<string, string>>;
}

const FAST_STEPS: readonly ValidationStep[] = [
  { name: "architecture dependency gate", command: ["bun", "archgate.ts"] },
  {
    name: "validation policy tests",
    command: ["bun", "test", "archgate.test.ts", "validate.test.ts"],
  },
  { name: "Rust formatting", command: ["cargo", "fmt", "--all", "--check"] },
  { name: "architecture documentation", command: ["bun", "archdoc.ts", "check"] },
];

const LINUX_STEPS: readonly ValidationStep[] = [
  { name: "workspace tests", command: ["cargo", "test", "--locked", "--workspace"] },
  {
    name: "workspace Clippy",
    command: [
      "cargo",
      "clippy",
      "--locked",
      "--workspace",
      "--all-targets",
      "--all-features",
      "--",
      "-D",
      "warnings",
    ],
  },
  { name: "release build", command: ["cargo", "build", "--locked", "--release"] },
  {
    name: "minimal-feature tests",
    command: ["cargo", "test", "--locked", "-p", "cclover-mon", "--no-default-features"],
  },
  {
    name: "minimal-feature Clippy",
    command: [
      "cargo",
      "clippy",
      "--locked",
      "-p",
      "cclover-mon",
      "--no-default-features",
      "--all-targets",
      "--",
      "-D",
      "warnings",
    ],
  },
];

const WINDOWS_STEPS: readonly ValidationStep[] = [
  "x86_64-pc-windows-msvc",
  "i686-pc-windows-msvc",
].map((target) => ({
  name: `Windows release build (${target})`,
  command: ["cargo", "xwin", "build", "--locked", "--release", "--target", target],
  env: { XWIN_ARCH },
}));

export const VALIDATION_PROFILES = {
  fast: FAST_STEPS,
  linux: LINUX_STEPS,
  windows: WINDOWS_STEPS,
  all: [...FAST_STEPS, ...LINUX_STEPS, ...WINDOWS_STEPS],
} as const;

export type ValidationProfile = keyof typeof VALIDATION_PROFILES;

export function validationSteps(profile: ValidationProfile): readonly ValidationStep[] {
  return VALIDATION_PROFILES[profile];
}

function quote(value: string): string {
  return /^[A-Za-z0-9_./,:=@+\-]+$/.test(value) ? value : JSON.stringify(value);
}

export function formatValidationStep(step: ValidationStep): string {
  const environment = Object.entries(step.env ?? {})
    .map(([key, value]) => `${key}=${quote(value)}`)
    .join(" ");
  const command = step.command.map(quote).join(" ");
  return environment ? `${environment} ${command}` : command;
}

function usage(): void {
  console.log("usage: bun validate.ts <fast|linux|windows|all>");
}

function isValidationProfile(value: string): value is ValidationProfile {
  return value in VALIDATION_PROFILES;
}

function run(profile: ValidationProfile): number {
  for (const step of validationSteps(profile)) {
    console.log(`\n==> ${step.name}`);
    console.log(`    ${formatValidationStep(step)}`);
    const result = Bun.spawnSync({
      cmd: [...step.command],
      env: step.env ? { ...process.env, ...step.env } : process.env,
      stdout: "inherit",
      stderr: "inherit",
    });
    if (result.exitCode !== 0) return result.exitCode;
  }
  return 0;
}

if (import.meta.main) {
  const profile = process.argv[2];
  if (profile === "--help" || profile === "-h") {
    usage();
  } else if (!profile || !isValidationProfile(profile)) {
    usage();
    process.exitCode = 2;
  } else {
    process.exitCode = run(profile);
  }
}
