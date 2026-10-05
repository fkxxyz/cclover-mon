---
summary: "Chooses single-executable, idempotent PawnIO provisioning with machine-level driver ownership and process-level runtime sessions."
viewpoint: decision
concerns:
  - architecture-coherence
  - maintainability
  - portability
  - security
activities:
  - orient
  - change
  - assess
facets:
  area:
    - whole-system
---

# ADR 009: Windows PawnIO Provisioning and Runtime Ownership

## Decision

Preserve the Windows single-file distribution model while treating PawnIO as a machine-level capability rather than a process-owned driver.

The published Windows artifact remains one `cclover-mon.exe`. For each supported Windows target, build preparation derives the smallest verified production PawnIO payload required by that target and embeds that payload plus only the signed Pawn modules actually used by implemented collectors. Upstream installers, utilities, debug artifacts, unrestricted/test-signed drivers, unrelated architectures, and other packaging-only inputs are not runtime dependencies.

Driver provisioning and metric sampling are separate lifecycles:

```text
build preparation
  pinned upstream PawnIO artifacts
    ↓ verify declared provenance/integrity
  derive target production payload
    ↓ verify selected components
  embed target provisioning resources + required signed modules

machine lifecycle, only when required
  check target/OS compatibility
    ├── unsupported → PawnIO-backed metrics unsupported
    └── supported
          ↓
  detect compatible PawnIO
    ├── present  ────────────────┐
    └── absent/incompatible      │
          ↓                     │
      short-lived elevated      │
      provisioning mode         │
          ↓                     │
      supported Windows driver  │
      installation APIs         │
          └─────────────────────┘
                    ↓
process lifecycle
  acquire PawnIO session/module handles once
                    ↓
  repeated hardware-telemetry sampling
                    ↓
  release process-owned handles
```

Provisioning is idempotent and version-aware. If a compatible PawnIO installation is already usable, provisioning is a no-op. Concurrent application instances converge on installed system state rather than each assuming ownership of the driver. Upgrade logic must not silently downgrade a newer compatible installation.

## Target and OS Compatibility

Target and OS compatibility are checked before extracting embedded provisioning resources, requesting elevation, or attempting a machine change. Unsupported combinations report PawnIO-backed metrics as `Unsupported`; they do not trigger UAC or attempt installation.

The repository-owned dependency declaration maps supported application targets to compatible PawnIO driver architectures and minimum Windows requirements. That mapping is explicit and must not be inferred only from process pointer width. If a build can use an already-installed compatible PawnIO runtime but cannot safely provision it on that host architecture, runtime use and provisioning support remain separate capabilities.

## Provisioning Resource Contract

Architecture documentation defines the runtime resource contract, not the upstream installer layout. Build preparation may parse, extract, transform, or repackage pinned upstream release artifacts as needed, but those details belong to build tooling and may change when upstream packaging changes. Runtime code must not depend on upstream installer resource identifiers, archive formats, compression layers, temporary filenames, or other packaging internals.

The embedded provisioning resource contains only the production driver package needed by the target. Driver package bytes that participate in upstream signing/catalog validation remain byte-for-byte identical to the verified upstream package. Any outer container or compression used solely for embedding is a project-owned implementation detail and may change without an architecture decision, provided runtime extraction remains deterministic and does not alter signed driver contents.

When provisioning is required, the application stages the embedded target package in a private temporary location, installs it through supported Windows driver-installation APIs, preserves restart-required status, and removes staging files best-effort afterward. Driver Store state intentionally persists. Provisioning does not use legacy service installation as a substitute for the supported PnP/driver-package path.

A failed provisioning attempt may clean up only state it created during that attempt. It must not remove a pre-existing PawnIO installation or devnode merely because installation, upgrade, or access failed. Normal shutdown releases only process-owned resources and never uninstalls shared PawnIO machine state.

## Privilege Boundary

Provisioning requires administrator authority. The ordinary application process must not remain elevated merely to perform installation work: when provisioning is required, the same executable may relaunch into a narrowly scoped elevated provisioning mode, perform the idempotent machine change, then terminate that elevated mode. No secondary helper executable or daemon is deployed.

Provisioning privilege and runtime device-access privilege are distinct. Successful one-time installation does not imply that a later non-elevated process can open the PawnIO device. Normal application execution supports both elevated and non-elevated sessions: an elevated session may use PawnIO-backed telemetry when the installed device policy permits it, while a non-elevated session remains a fully supported application mode even when PawnIO-backed metrics are unavailable. cclover-mon follows the installed driver's access policy and does not modify the signed INF, broaden the device ACL, proxy PawnIO through an elevated helper, or automatically elevate the sampling path merely to bypass that policy. Runtime access denial is a metric-local permission failure.

Declining elevation, installation failure, required reboot, signature rejection, incompatible driver state, or runtime access denial fails closed for PawnIO-backed metrics. None of these conditions justifies disabling Windows driver-signature enforcement or installing the unrestricted/test-signed PawnIO package.

## Runtime Ownership and Interface

The Windows hardware-telemetry runtime owns long-lived PawnIO sessions/modules for as long as the backend needs PawnIO-backed sensors. Initialization discovers and opens each required capability once; repeated temperature, fan, and future hardware-sensor samples reuse stable handles and topology. Sampling must not install, start, stop, unload, rediscover, or re-provision the driver on every cycle.

cclover-mon talks to PawnIO through a small in-process Windows platform adapter over the documented device IO-control contract rather than deploying `PawnIOLib.dll`. This keeps the single-file runtime free of an extract-and-load DLL lifecycle and permits one client implementation to support compatible process bitnesses. Platform-neutral code does not see PawnIO handles, IOCTL values, or NTSTATUS details.

Required Pawn modules are embedded as pinned official signed `.bin` blobs from a pinned PawnIO.Modules release. Only modules actually consumed by implemented collectors are embedded; the complete module release ZIP is a build input, not a runtime payload. Module handles follow the same process ownership rule as the device session and are reused across samples.

The process owns its session. Windows owns installed driver state. A previous process crash must not require cleanup before the next process can acquire PawnIO. If the driver disappears, becomes unusable, or an open session fails, the collector reports typed unavailability and may retry acquisition according to bounded platform retry policy; it does not repeatedly invoke privileged provisioning from the sampling loop.

## Distribution and Supply Chain

cclover-mon does not rebuild, modify, self-sign, or substitute an unrestricted PawnIO kernel driver as part of normal releases. One repository-owned dependency declaration is the authority for pinned PawnIO and PawnIO.Modules versions, official artifact URLs, artifact digests, target/OS compatibility, selected driver-component digests, and required module digests. Those values must not be independently duplicated across build scripts, CI, and runtime code.

Dependency preparation, not Rust `build.rs`, owns network access. Build orchestration checks a user-scoped cache outside the repository/worktree, downloads missing pinned upstream artifacts, verifies their declared digests, derives the target provisioning resources/modules, and caches the verified result. Valid cache entries are reusable across worktrees and survive `cargo clean`. Corrupt or mismatched inputs are rejected; an artifact whose digest does not match is a hard build failure.

Rust compilation consumes only already-prepared, verified target resources and embeds them into the Windows executable. A raw `cargo` build must not silently reach the network; when required prepared resources are absent, it fails with deterministic guidance to run the repository dependency-preparation/build entry point. Release and CI builds use the same declaration and verification path.

Windows remains responsible for validating the signed driver/catalog during installation. Build preparation verifies that selected production components match the pinned dependency declaration and does not patch INF contents, driver binaries, catalogs, or signatures.

PawnIO and PawnIO.Modules redistribution is a release gate. The project license does not replace their upstream licenses. The dependency declaration records, for each redistributed payload, its applicable license/notice material and the corresponding upstream source repository, human-readable ref, and exact immutable commit. `tools/release/plan.ts` records which product artifacts actually embed each payload and declares one release-level companion source asset for each corresponding-source obligation. `tools/release/redistribution.ts` projects those declarations into `THIRD-PARTY-SOURCES.txt`, requires each ref to resolve to its pinned commit, recursively materializes Git submodules at the gitlink commits recorded by the source tree, records submodule provenance plus the resolved upstream commit and archive SHA-256, and creates a deterministic source archive without Git metadata. Any ref drift or submodule commit mismatch fails closed rather than silently publishing different or incomplete source.

The project fulfills the current PawnIO driver and PawnIO.Modules source-delivery obligations by publishing those companion corresponding-source archives with the same GitHub release, rather than by issuing a written source offer. Aggregate release verification requires every embedded payload to map to exactly one fulfillment asset, requires the applicable notice/license files in the product archive, ties every source manifest to the same cclover-mon Git commit as the product artifacts, and verifies each companion asset digest. Publication treats the verified product archives, companion source assets, and aggregate release manifest as one exact remote asset set; any missing or mismatched fulfillment evidence blocks publication. If a future dependency version changes its licensing terms, source topology, or required fulfillment mechanism, the dependency declaration and release plan must be updated before that payload can ship.

## Rationale

Requiring users to install PawnIO manually breaks the desired portable single-file product experience. Shipping the entire upstream installer would also carry packaging logic and unrelated artifacts that are not runtime requirements. Deriving only the verified target production payload at build time keeps the release artifact small while leaving signed driver-package contents unchanged. Direct in-process device access likewise removes a DLL deployment boundary that provides no required isolation.

## Consequences

- Each supported Windows target embeds only the PawnIO provisioning resources and signed modules that target needs; unsupported targets embed none.
- First provisioning may require UAC and may require a reboot.
- The Windows machine retains the installed PawnIO driver after cclover-mon exits.
- Multiple application instances can coexist without unloading shared driver state.
- PawnIO failure degrades only dependent sensors.
- Normal sampling remains cheap because provisioning is outside the sampling cadence and process-owned handles are reused.
- Runtime access remains subject to the installed PawnIO device policy; both elevated and non-elevated application sessions are supported, and changing that policy requires a separate security decision rather than an implicit packaging workaround.
