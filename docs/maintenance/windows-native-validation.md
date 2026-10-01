# Windows Native Validation

`validate.ts` is the validation-plan authority. It owns each profile's commands, required host, and required privilege. Host transport must invoke a profile; it must not copy the profile's Cargo/Bun commands.

## Repository entry point

From Linux/WSL, use:

```bash
bun windows-validate.ts doctor
bun windows-validate.ts windows-native
bun windows-validate.ts windows-etw-runtime
```

`windows-validate.ts` resolves the current repository/worktree, maps it to a Windows-visible path, and routes the profile according to `validate.ts` metadata. `windows-native` uses ordinary Windows authority. `windows-etw-runtime` uses the provisioned elevated bridge automatically; callers do not select elevation themselves.

On Windows itself, the same command executes the selected profile directly in the current repository.

Cross-build evidence remains separate:

```bash
bun validate.ts windows
```

That profile compiles target-specific tests and release binaries for both supported Windows architectures, but it does not execute Windows code.

## One-time elevated bridge provisioning

Provision the repository-owned scheduled-task bridge once:

```bash
bun windows-validate.ts install
```

The installer requests UAC once, copies the narrow elevated worker to `%LOCALAPPDATA%\cclover-mon-validation`, and registers the `cclover-mon-validation-elevated` scheduled task for the current user. Later elevated validation requests use that task without another UAC prompt where local Windows policy permits it.

The task is not a general administrator shell. It reads only a repository path plus validation profile, then invokes that worktree's `tools/windows-validation/runner.ps1`, whose only operation is `bun validate.ts <profile>`.

To remove the machine-level bridge, run `tools/windows-validation/uninstall.ps1` from Windows PowerShell with administrator approval.

## Host and path discovery

Standard WSL sessions require no repository-local configuration when Windows interop is available. Repository paths under `/mnt/<drive>` map to that Windows drive; other WSL paths map through `\\wsl.localhost\<distro>`.

For a Linux environment that reaches a Windows host through a machine-specific PowerShell shim or exposes the repository under a different host path, create:

```text
~/.config/cclover-mon/windows-validation.json
```

Example:

```json
{
  "powershell": "/path/to/powershell.exe",
  "pathMappings": [
    {
      "localPrefix": "/run/media/user/wsl",
      "windowsPrefix": "\\\\wsl.localhost\\Arch"
    }
  ]
}
```

`powershell` and path prefixes are machine facts only. Validation commands and privilege policy remain repository-owned. Multiple mappings are allowed; the most specific matching local prefix wins. This preserves arbitrary Git worktree suffixes rather than hard-coding the main checkout path.

Set `CCLOVER_WINDOWS_VALIDATION_CONFIG` only when a non-default config-file location is required.

## Diagnostics

Run:

```bash
bun windows-validate.ts doctor
```

It reports the resolved repository, Windows path mapping, PowerShell transport, Windows Bun/Cargo availability, repository runner visibility, and elevated scheduled-task provisioning. A fresh session should pass `doctor` before native Windows validation is treated as available.

## Profiles

`windows-native` runs deterministic Windows-host tests. It does not prove ETW runtime semantics.

`windows-etw-runtime` builds the production executable and runs `windows-etw-disk-smoke.ts`. The smoke opens a temporary file before cclover-mon starts, writes from a known Bun PID while the production `disk-attribution` probe runs, and requires nonzero write attribution for that same PID. It exercises real ETW session control, FileIo rundown seeding, correlation, physical-disk canonicalization, and production probe output.

Do not report Windows runtime validation from cross-build success alone. Record which native profile ran and which Windows environment supplied the evidence.
