export const XWIN_ARCH = "x86,x86_64";

export interface ValidationStep {
  name: string;
  command: readonly string[];
  env?: Readonly<Record<string, string>>;
}

export type ValidationHost = "local" | "windows";
export type ValidationPrivilege = "ordinary" | "elevated";

export interface ValidationExecution {
  host: ValidationHost;
  privilege: ValidationPrivilege;
}

interface ValidationProfileDefinition {
  steps: readonly ValidationStep[];
  execution: ValidationExecution;
}

const FAST_STEPS: readonly ValidationStep[] = [
  { name: "architecture dependency gate", command: ["bun", "archgate.ts"] },
  {
    name: "validation policy tests",
    command: [
      "bun",
      "test",
      "archgate.test.ts",
      "release.test.ts",
      "release-redistribution.test.ts",
      "release-publication.test.ts",
      "libbpf-compat.test.ts",
      "validate.test.ts",
      "windows-validate.test.ts",
      "sync-linux-hwmon.test.ts",
    ],
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
    name: "Linux libbpf ABI compatibility",
    command: ["bun", "libbpf-compat.ts", "target/release/cclover-mon"],
  },
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
  { name: "Windows dependency preparation", command: ["bun", "prepare-windows-deps.ts"] },
  ...["x86_64-pc-windows-msvc", "i686-pc-windows-msvc"].flatMap((target) => [
    {
      name: `Windows test compile (${target})`,
      command: [
        "cargo",
        "xwin",
        "test",
        "--locked",
        "-p",
        "cclover-mon",
        "-p",
        "cclover-desktop",
        "--no-default-features",
        "--no-run",
        "--target",
        target,
      ],
      env: { XWIN_ARCH },
    },
    {
      name: `Windows release build (${target})`,
      command: ["cargo", "xwin", "build", "--locked", "--release", "--target", target],
      env: { XWIN_ARCH },
    },
    {
      name: `Windows server release build (${target})`,
      command: [
        "cargo",
        "xwin",
        "build",
        "--locked",
        "--release",
        "-p",
        "cclover-server",
        "--target",
        target,
      ],
      env: { XWIN_ARCH },
    },
  ]),
];

const WINDOWS_NATIVE_STEPS: readonly ValidationStep[] = [
  { name: "Windows dependency preparation", command: ["bun", "prepare-windows-deps.ts"] },
  {
    name: "Windows deterministic tests",
    command: ["cargo", "test", "--locked", "-p", "cclover-mon", "--no-default-features"],
  },
  {
    name: "Windows desktop renderer tests",
    command: ["cargo", "test", "--locked", "-p", "cclover-desktop"],
  },
];

const WINDOWS_ETW_RUNTIME_STEPS: readonly ValidationStep[] = [
  { name: "Windows dependency preparation", command: ["bun", "prepare-windows-deps.ts"] },
  {
    name: "Windows release build for ETW runtime validation",
    command: ["cargo", "build", "--locked", "--release", "-p", "cclover-mon", "--no-default-features"],
  },
  { name: "Windows ETW disk-attribution smoke", command: ["bun", "windows-etw-disk-smoke.ts"] },
  {
    name: "Windows ETW completion-semantic validation",
    command: [
      "cargo",
      "run",
      "--locked",
      "-p",
      "cclover-platform",
      "--bin",
      "windows-etw-semantic",
      "--features",
      "windows-etw-validation",
    ],
  },
];

const LINUX_EBPF_RUNTIME_STEPS: readonly ValidationStep[] = [
  { name: "Linux eBPF release build", command: ["cargo", "build", "--locked", "--release"] },
  {
    name: "Linux libbpf ABI compatibility",
    command: ["bun", "libbpf-compat.ts", "target/release/cclover-mon"],
  },
  { name: "Linux eBPF scalar-fallback smoke", command: ["bun", "linux-ebpf-runtime-smoke.ts"] },
];

const WEB_BROWSER_STEPS: readonly ValidationStep[] = [
  { name: "web release build", command: ["cargo", "build", "--locked", "--release"] },
  { name: "Chromium web runtime smoke", command: ["bun", "web-browser-smoke.ts"] },
];

const SERVER_STEPS: readonly ValidationStep[] = [
  { name: "server tests", command: ["cargo", "test", "--locked", "-p", "cclover-server"] },
  {
    name: "server release build",
    command: ["cargo", "build", "--locked", "--release", "-p", "cclover-server"],
  },
  {
    name: "Linux server libbpf ABI compatibility",
    command: ["bun", "libbpf-compat.ts", "target/release/cclover-mon-server"],
  },
  { name: "server headless runtime smoke", command: ["bun", "server-smoke.ts"] },
];

export const VALIDATION_PROFILES = {
  fast: {
    steps: FAST_STEPS,
    execution: { host: "local", privilege: "ordinary" },
  },
  linux: {
    steps: LINUX_STEPS,
    execution: { host: "local", privilege: "ordinary" },
  },
  windows: {
    steps: WINDOWS_STEPS,
    execution: { host: "local", privilege: "ordinary" },
  },
  "linux-ebpf-runtime": {
    steps: LINUX_EBPF_RUNTIME_STEPS,
    execution: { host: "local", privilege: "elevated" },
  },
  "windows-native": {
    steps: WINDOWS_NATIVE_STEPS,
    execution: { host: "windows", privilege: "ordinary" },
  },
  "windows-etw-runtime": {
    steps: WINDOWS_ETW_RUNTIME_STEPS,
    execution: { host: "windows", privilege: "elevated" },
  },
  "web-browser": {
    steps: WEB_BROWSER_STEPS,
    execution: { host: "local", privilege: "ordinary" },
  },
  server: {
    steps: SERVER_STEPS,
    execution: { host: "local", privilege: "ordinary" },
  },
  portable: {
    steps: [...FAST_STEPS, ...LINUX_STEPS, ...WINDOWS_STEPS],
    execution: { host: "local", privilege: "ordinary" },
  },
} as const satisfies Record<string, ValidationProfileDefinition>;

export type ValidationProfile = keyof typeof VALIDATION_PROFILES;

export function validationSteps(profile: ValidationProfile): readonly ValidationStep[] {
  return VALIDATION_PROFILES[profile].steps;
}

export function validationExecution(profile: ValidationProfile): ValidationExecution {
  return VALIDATION_PROFILES[profile].execution;
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
  console.log(
    "usage: bun validate.ts <fast|linux|linux-ebpf-runtime|windows|windows-native|windows-etw-runtime|web-browser|server|portable>",
  );
}

export function isValidationProfile(value: string): value is ValidationProfile {
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
