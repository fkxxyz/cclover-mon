# Windows Native Validation

`validate.ts` is the command-plan authority. Windows runtime work should invoke a profile instead of copying its Cargo/Bun steps into a second script or document.

## Profiles

Run on a real Windows host:

```powershell
bun validate.ts windows-native
```

This runs deterministic Windows-host tests. It does not prove ETW runtime semantics.

For Windows FileIo ETW disk attribution:

```powershell
bun validate.ts windows-etw-runtime
```

This profile builds the production executable and runs `windows-etw-disk-smoke.ts`. The smoke opens a temporary file before cclover-mon starts, writes from a known Bun PID while the production `disk-attribution` probe runs, and requires a nonzero write-attribution row for that same PID. It therefore exercises real ETW session control, FileIo rundown seeding, correlation, physical-disk canonicalization, and production probe output.

The ETW profile may require administrator or Performance Log Users-equivalent authority to control the system-provider trace session. Permission failure is a validation-environment failure, not evidence that a fallback counter should be used.

## Linux/WSL development environments

Cross-build evidence remains available with:

```bash
bun validate.ts windows
```

That profile compiles target-specific tests and release binaries for both supported Windows architectures, but it does not execute Windows code.

If the current Linux/WSL environment already has a machine-local Windows host or administrator bridge, use it only to invoke the repository profile above from the Windows side. Do not copy the profile's individual commands into the bridge. A clean repository does not currently provision or discover that bridge automatically; this remaining infrastructure gap is tracked by `windows-native-validation-infrastructure.md`.

Do not report Windows runtime validation from cross-build success alone. Record which native profile ran, whether elevation was required, and which Windows environment supplied the evidence.
