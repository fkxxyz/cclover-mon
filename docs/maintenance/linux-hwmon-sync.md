# Linux hwmon Sync Runbook

Use this runbook when updating the vendored Linux hwmon snapshot used by the Windows compatibility path.

Architecture authority: `docs/architecture/09-architecture-decisions/012-linux-hwmon-windows-compatibility.md`.
Dependency authority: `deps/linux-hwmon.ts`.
Current runtime coverage and blockers: `docs/maintenance/linux-hwmon-coverage.md`.

## Workflow

1. Update the pinned ref, exact commit, selected paths, and expected SHA-256 values in `deps/linux-hwmon.ts`.
2. Run:

   ```bash
   bun sync-linux-hwmon.ts
   ```

   The tool uses `CCLOVER_MON_DEPS_CACHE` when set, otherwise `~/.cache/cclover-mon/deps`, and keeps the Linux checkout outside the repository.
3. Review the resulting vendored diff. Any change inside `vendor/linux/` must be explainable solely by the upstream revision/path selection; do not edit vendored files manually.
4. Adapt project-owned compatibility headers, wrappers, or Windows transports as required by upstream API changes.
5. Run:

   ```bash
   bun validate.ts fast
   bun validate.ts windows
   ```

   Run Linux validation too when shared or Linux-facing code changes.

## Completion Criteria

A sync is complete when the manifest identifies one exact upstream commit, vendored files match their declared digests, compatibility code builds for supported Windows targets, and the relevant validation profiles pass.
