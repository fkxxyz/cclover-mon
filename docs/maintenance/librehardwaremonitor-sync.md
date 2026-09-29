# LibreHardwareMonitor Sync Runbook

Use this runbook when updating Windows hardware-telemetry compatibility knowledge derived from LibreHardwareMonitor (LHM).

Architecture authority: `docs/architecture/09-architecture-decisions/010-windows-hardware-telemetry-upstream.md`.
Dependency authority: `deps/librehardwaremonitor.ts`.

## Workflow

1. Prepare a clean LHM checkout at the candidate upstream revision.
2. Run:

   ```bash
   bun lhm-sync.ts status <LibreHardwareMonitor checkout>
   ```

3. Review only relevant upstream changes reported by the tool and classify them:
   - hardware data: chip IDs, aliases, board/model mappings, channel labels, constants;
   - family behavior: register layouts, bank/page selection, tachometer decoding, unlock/exit sequences, quirks;
   - unrelated LHM runtime/UI/.NET changes.
4. Port only changes needed by cclover-mon. Do not import LHM runtime architecture, CLR dependencies, UI, threading, settings, or generic sensor objects.
5. For hardware-data-only changes, update repository-owned tables/source directly after review.
6. For register algorithms or I/O sequences, review the upstream diff manually and port intentionally. Never auto-replace production hardware-access code from upstream.
7. Preserve MPL-2.0 provenance on files containing LHM-derived code. Add provenance to new derived files before commit.
8. Update `deps/librehardwaremonitor.ts` only after the candidate revision has been fully reviewed and adopted. The recorded reviewed revision must describe the code actually incorporated.
9. Run the validation profiles appropriate to the changed scope. At minimum:

   ```bash
   bun validate.ts fast
   bun validate.ts windows
   ```

   Run Linux validation too when shared model, presentation, API, or cross-platform code changes.
10. If architecture constraints changed, update the relevant architecture View and run:

    ```bash
    bun archdoc.ts check
    ```

## Completion Criteria

A sync is complete only when:

- every reported relevant upstream change was classified;
- adopted algorithmic changes were manually reviewed;
- derived-source provenance remains accurate;
- the reviewed LHM revision matches the incorporated code;
- required validation passes;
- unsupported or unreviewed hardware changes remain explicitly unsupported rather than guessed.
