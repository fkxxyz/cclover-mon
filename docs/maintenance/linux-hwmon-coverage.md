# Linux hwmon Windows Compatibility Status

Current implementation status for the Windows compatibility path defined by ADR 012.

This file records changeable coverage and validation state. Architecture rules remain in `docs/architecture/09-architecture-decisions/012-linux-hwmon-windows-compatibility.md`.

## Active upstream runtime paths

| Hardware | Upstream source | Windows transport | Status | Runtime validation |
| --- | --- | --- | --- | --- |
| Intel Core-family CPU temperature | `drivers/hwmon/coretemp.c` | PawnIO `IntelMSR.bin` | active | validated on Windows with Intel Core i7-11700F; 8 core channels plus package |
| AMD Family 0F temperature | `drivers/hwmon/k8temp.c` | PawnIO `AMDFamily0F.bin` | active | build-validated; no representative hardware runtime test yet |
| AMD Family 11h-16h temperature | `drivers/hwmon/k10temp.c` | PawnIO `AMDFamily10.bin` | active | build-validated; no representative hardware runtime test yet |
| AMD Family 17h-1Ah temperature | `drivers/hwmon/k10temp.c` | PawnIO `AMDFamily17.bin` | active | build-validated; no representative hardware runtime test yet |

Stable product-facing temperature IDs and names remain project-owned even when decode/probe logic comes from Linux upstream.

## Deliberate gaps

### AMD Family 10h

Remains on the project-owned legacy path.

Linux `k10temp` performs Erratum 319 handling that requires northbridge function-2 PCI configuration reads. The pinned official `AMDFamily10.bin` exposes the required function-3 temperature register reads but not function-2 configuration access. No other pinned official signed PawnIO module provides a suitable generic PCI-config capability.

Do not bypass the check or ship a locally rebuilt PawnIO module. Revisit when an official signed transport exposes the required read capability.

### Super-I/O drivers

No additional Super-I/O Linux driver is enabled directly yet.

- `nct6775-core.c` is vendored for compatibility work but its normal probe/init path performs state-changing hardware writes, so direct execution is currently ineligible under the read-only policy.
- `f71882fg.c` has usable read logic, but compiling/executing it cleanly would currently require a disproportionately large sysfs/PWM compatibility surface. Deferred on cost/benefit grounds.
- Linux v6.11 `it87.c` does not support the observed ITE chip ID `0x8637` on the Acer Nitro N50-620 test machine. Linux on that machine also does not bind an `it87` hwmon driver to the chip.
- Winbond `w83627ehf` has not been implemented because expected compatibility-facade cost is high relative to available runtime-validation value.

Existing project-owned Super-I/O readers remain the runtime fallback where applicable.

## Scope boundary

Do not force Linux source reuse where Windows already has a narrower structured native interface or where reuse would require emulating a large Linux subsystem.

Examples:

- NVMe temperature should use Windows storage/NVMe SMART/Health access rather than porting the Linux NVMe stack.
- ACPI thermal-zone temperature should use Windows ACPI/thermal facilities rather than porting the Linux thermal subsystem.

## Validation baseline

Validation contract for this compatibility path:

- `bun validate.ts fast` must pass, including the vendored-source digest and warning-isolation guards.
- `bun validate.ts windows` must pass for both x86_64 and i686 Windows targets.
- Vendored Linux hwmon files must remain digest-checked against the pinned Linux v6.11 commit.

Known warnings from unchanged, digest-pinned vendored Linux hwmon C are suppressed only around the vendor source include. Project-owned Windows bridge code before and after that include remains under the normal compiler warning policy, and newly introduced vendor warning classes remain visible until explicitly reviewed.

## When to extend coverage

Prioritize a new driver only when at least one of these is true:

- representative hardware is available for real Windows runtime validation;
- the Linux driver fits the existing compatibility surface with small incremental cost;
- an official signed PawnIO module adds a transport capability that removes a current blocker;
- upstream gains support for hardware the project actually needs.

Avoid expanding the compatibility facade solely to increase nominal driver count.

## Maintenance invariant

Any change to Windows runtime use of vendored Linux hwmon sources must update this page in the same change. `sync-linux-hwmon.test.ts` provides a minimal guard that active upstream driver names remain represented here; this page is still descriptive maintenance state rather than generated architecture authority.

Whenever the vendored Linux hwmon snapshot or its pinned digests change, rerun Windows cross-validation and review the warning-suppression allowlist against the new compiler output. Remove suppressions that are no longer required and add a new warning class only after confirming it originates in unchanged vendored source rather than project-owned bridge or compatibility code.
