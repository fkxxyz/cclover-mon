import { isValidationProfile, validationExecution } from "./validate";
import {
  doctorWindowsValidation,
  installWindowsValidationBridge,
  runWindowsValidation,
} from "./tools/windows-validation/host";

function usage(): void {
  console.log("usage: bun windows-validate.ts <doctor|install|windows-native|windows-etw-runtime>");
}

function run(argv: readonly string[]): number {
  const command = argv[0];
  if (!command || command === "--help" || command === "-h") {
    usage();
    return command ? 0 : 2;
  }

  if (command === "doctor") {
    const result = doctorWindowsValidation();
    console.log(`repository: ${result.repository}`);
    if (result.windowsRepository) console.log(`windows repository: ${result.windowsRepository}`);
    if (result.powershell) console.log(`powershell: ${result.powershell}`);
    if (result.message) console.error(result.message);
    return result.exitCode;
  }

  if (command === "install") {
    return installWindowsValidationBridge();
  }

  if (!isValidationProfile(command) || validationExecution(command).host !== "windows") {
    console.error(`${command} is not a Windows-host validation profile`);
    usage();
    return 2;
  }

  return runWindowsValidation(command);
}

if (import.meta.main) {
  try {
    process.exitCode = run(process.argv.slice(2));
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    process.exitCode = 1;
  }
}
