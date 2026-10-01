---
summary: "Chooses Windows NDU interval accounting for per-process per-interface network usage behind an isolated compatibility wrapper."
viewpoint: decision
concerns:
  - architecture-coherence
  - maintainability
  - portability
  - performance
  - security
activities:
  - orient
  - change
  - assess
facets:
  area:
    - platform
---

# ADR 013: Windows NDU Network Attribution

## Decision

Windows per-process per-interface network usage will use the built-in Network Data Usage (NDU) accounting facility exposed by `Ndu.sys` through `\\.\NduIoDevice`.

The Windows backend keeps the undocumented ABI behind one Rust compatibility wrapper. Collector and core code consume typed process/interface/directional usage only; they do not know IOCTL numbers, self-relative offsets, raw NDU attribution kinds, or device-handle lifecycle.

NDU accounting is interval based for this integration: start collection once, query the current interval at the sampling boundary, then continue accumulating the next interval until the next query. Session shutdown stops collection and closes the device handle. `IfLuid` remains Windows-private and is canonicalized to the same `InterfaceGuid`-derived `NetworkId` used by the Windows interface collector before data crosses the platform boundary.

The Windows attribution collector adapts those consumed interval bytes into monotonic cumulative `ProcessNetworkIoCounter` values before they cross the platform boundary, so the core keeps its existing cumulative-counter delta/rate semantics. `IfLuid` canonicalization uses the documented LUID-to-GUID conversion path and the same GUID key construction as the normal interface collector; attribution does not depend on interface names, route inference, or the success of the current interface observation.

NDU exposes a PID but not the process creation timestamp required by `ProcessInstanceId`. An interval is therefore attributed to a process only when complete process snapshots at both interval boundaries map that PID to the same `ProcessInstanceId`. A process that starts or exits within the interval, or a PID that is reused across the interval, is intentionally left unattributed rather than guessed. An unavailable or incomplete process snapshot still consumes the NDU interval but breaks identity continuity; stale cumulative attribution state may be retired only from a later complete process snapshot.

The product requires trustworthy process × interface × RX/TX attribution suitable for identifying and ranking bandwidth consumers. It does **not** require NDU accounting bytes to equal Linux socket-payload bytes exactly. Accounting-layer semantics are platform-owned and must remain stable and diagnosable within a backend.

## Compatibility Boundary

The observed NDU ABI is not a documented Microsoft application contract. The wrapper therefore treats every raw layout assumption as version-sensitive compatibility knowledge:

- `NduIoDevice`, IOCTL identifiers, envelope status/size fields, record strides, and self-relative pointer rules are private to the wrapper;
- every offset and count is bounds checked before use;
- unknown or malformed layouts fail closed instead of guessing;
- no NDU structure or native identifier becomes a core-owned type;
- runtime permission or ABI failure makes only network attribution unavailable;
- comments beside private constants record the observation source and semantics needed for future revalidation.

Representative Windows runtime validation remains required after Windows updates that materially change `Ndu.sys` / `nduprov.dll` behavior. Public symbols, `nduprov.dll` call sites, NDU ETW diagnostics, and controlled traffic tests are compatibility evidence; copied Microsoft implementation code is not part of the repository.

## Rationale

NDU already performs the difficult Windows-native process/interface accounting and returns per-attribution per-interface directional byte usage. It avoids route/interface guessing and avoids maintaining a custom WFP callout driver, packet-capture correlation engine, or high-volume ETW event stream in the normal sampling path.

Controlled Windows testing established that enabling the NDU accounting session activates the diagnostic per-flow ETW stream, while direct `QUERY_STATS` returns already-aggregated process/interface records. Direct interval queries are therefore a lower-overhead and more failure-contained production boundary than reconstructing the same accounting from ETW events.

## Consequences

- Windows network attribution may require elevated authority to open/use the NDU device.
- The project owns compatibility testing for an undocumented ABI.
- NDU failure remains isolated from ordinary interface totals collected through IP Helper.
- Windows and Linux may report slightly different byte totals for the same application payload because their accounting layers differ; this is acceptable for the product goal.
- A future documented Windows API that provides equivalent real-time process × interface attribution should replace this private ABI when its cost and fidelity are comparable.

## Rejected Alternatives

### Kernel network ETW as the production source

ETW exposes process/flow information and NDU exposes diagnostic per-flow interface byte events, but a production event join would require high-volume event consumption, lifecycle correlation, and loss handling that NDU already performs internally.

### Route inference from process TCP/IP events

Rejected because source-address or routing-table inference can be wrong for VPNs, tunnels, multihoming, route changes, and layered interfaces. Interface identity must come from the accounting source rather than a guess.

### Custom WFP callout driver

Technically capable, but substantially more expensive to build, sign, install, validate, and maintain. It remains a fallback only if NDU compatibility becomes unacceptable and no documented equivalent exists.

## Reassessment

Revisit this decision if supported Windows builds materially diverge in NDU ABI, required privilege becomes unacceptable, interval query overhead is too high, attribution correctness fails on representative VPN/tunnel workloads, or a documented native API reaches equivalent fidelity.
