---
summary: "Records the earlier LibreHardwareMonitor-first Windows hardware-compatibility strategy superseded by ADR 012."
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
    - whole-system
---

# ADR 010: Windows Hardware Telemetry Upstream

## Status

Superseded for hardware-compatibility upstream and synchronization strategy by [ADR 012](012-linux-hwmon-windows-compatibility.md). The coordinated Windows hardware-telemetry ownership, typed metric projections, identity semantics, source-local failure isolation, and read-only product boundary introduced here remain part of the current architecture and are defined by the active platform Views.

## Historical Decision

Windows motherboard and low-level CPU hardware telemetry is one platform subsystem that owns hardware discovery, long-lived PawnIO capabilities, bus synchronization, source retry state, and topology. Temperature, fan speed, and future voltage/power metrics are strongly typed projections of one coordinated hardware observation; they do not independently rediscover or reopen the same physical source.

```text
Windows hardware telemetry
  ├── board identity
  ├── CPU-native sources
  ├── Super-I/O
  │   ├── Nuvoton
  │   ├── ITE
  │   ├── Fintek
  │   └── Winbond
  └── embedded-controller sources
        ↓
  one coordinated hardware observation
        ├── temperatures
        ├── fans
        └── future typed metrics
```

PawnIO is the privileged hardware-access mechanism where no suitable structured Windows API exists. The hardware-telemetry runtime owns process-local PawnIO module sessions and reuses them across samples. Each loaded Pawn module has its own PawnIO session handle; shared ownership means one subsystem manages their lifecycle, not that unrelated modules share one handle. It embeds only pinned official signed modules actually used by implemented sources. Provisioning remains governed by [ADR 009](009-windows-pawnio-provisioning.md).

LibreHardwareMonitor (LHM) is a reviewed **upstream compatibility-knowledge source**, not a runtime dependency. cclover-mon may port and attribute relevant chip identification, register layouts, decoding algorithms, hardware quirks, and motherboard channel mappings while retaining project-owned Rust runtime, typed metric contracts, lifecycle, and failure semantics. The application does not host CLR or ship `LibreHardwareMonitorLib.dll`.

LHM-derived implementation is divided by stability:

- Super-I/O family drivers own chip detection, register access sequencing, tachometer decoding, and family quirks.
- Board mapping owns manufacturer/model-specific channel labels separately from chip drivers.
- Machine-readable hardware facts may be normalized into repository-owned declarative tables; algorithmic register behavior remains reviewed source code.

The reviewed LHM revision and provenance are pinned in `deps/librehardwaremonitor.ts`. `lhm-sync.ts status <checkout>` compares a candidate checkout against that reviewed revision and reports changes to the declared relevant hardware sources. Updating upstream compatibility knowledge is explicit: tooling may identify relevant upstream changes and regenerate declarative data, but production register-access algorithms are never silently replaced from upstream. Algorithm or I/O-sequence changes require review and deterministic tests before adoption.

The operational update procedure is maintained separately in [`docs/maintenance/librehardwaremonitor-sync.md`](../../../maintenance/librehardwaremonitor-sync.md). This ADR remains the authority for stable architectural constraints; the runbook owns repository-maintenance steps.

## Metric and Identity Semantics

Core retains metric-specific contracts such as temperature and fan snapshots rather than a generic name/type/value/unit sensor bag. Sharing happens below the metric boundary in hardware discovery and access.

CPU hardware adapters may collect finer-grained thermal channels than the product exposes by default. The normal product temperature projection includes primary package/die-style CPU temperatures and excludes per-core, per-CCD, or similarly fine-grained CPU channels. Diagnostic probes may expose those detailed channels without changing the product projection. Channel visibility is an explicit adapter semantic, not inferred from display labels or opaque sensor identifiers.

Hardware sensor identity is independent from display naming. Native slot numbers, enumeration order, and board labels are locators or presentation metadata. A source canonicalizes physical source identity plus stable chip/channel identity before producing a core-owned sensor identity. Improving a board mapping from `Fan #1` to `CPU Fan` must not reset history or create a new semantic sensor.

Zero RPM is a valid fan observation. Unsupported, inaccessible, or failed fan sources remain typed unavailability/degradation rather than fabricated zero.

## Runtime Lifecycle

Stable topology is discovered outside the one-second sampling hot path and reused while valid. Sampling reads known channels once per coordinated source observation. Reinitialization occurs only through bounded retry after source failure or when the source explicitly requires topology refresh.

LPC/ISA and EC transactions use the interoperable Windows named synchronization primitives expected by the hardware-monitoring ecosystem, with locks held only around the physical transaction. A failure in one source family degrades only projections that depend on that source. Within one source, optional topology or channel capabilities have the same failure-boundary rule: failure of a narrower capability must not invalidate independently readable observations from that source. For example, physical-core topology failure may degrade per-core CPU temperatures while an independently readable package temperature remains available.

Hardware telemetry is read-only product capability. Register writes required by a documented read protocol, such as bank or logical-device selection, are protocol mechanics; changing fan PWM, control mode, firmware policy, or other machine behavior requires a separate architecture decision.

## Scope Boundary

Structured Windows metrics with stable native APIs remain independent collectors: CPU utilization, memory, processes, network interfaces, disk I/O, storage temperature, ACPI thermal zones, and battery temperature continue to use their direct Windows sources. GPU telemetry remains outside the PawnIO hardware subsystem: NVML/ADL remain authoritative for NVIDIA/AMD device telemetry, while D3DKMT may supply Windows-native fields such as Intel GPU temperature when the graphics stack exposes them.

Linux does not mirror the Windows implementation. Linux uses kernel-exposed hwmon/sysfs sources and converges only at the same core-owned typed metric contracts.

## Historical Rationale

Windows exposes motherboard sensors through heterogeneous Super-I/O and embedded-controller hardware rather than one stable system API. Reimplementing the compatibility database independently would duplicate years of hardware research, while embedding LHM itself would add CLR/runtime/deployment cost and couple cclover-mon to an unrelated object model. Treating LHM as reviewed upstream knowledge preserves its compatibility value without creating a runtime dependency.

One hardware runtime also prevents each future metric from opening PawnIO, rediscovering the same chip, and independently serializing access to the same physical bus.

## Historical Consequences

- Existing Intel package-temperature code moves under hardware telemetry rather than remaining a temperature-owned PawnIO runtime.
- Fan speed becomes a separate typed shared metric; GPU fan fields remain part of `GpuSnapshot` because they are device telemetry from NVML/ADL.
- Adding voltage, motherboard power, or EC metrics extends hardware observations and typed projections without changing Super-I/O discovery ownership.
- New LHM support is not automatically inherited. A pinned-upstream review/sync workflow is an intentional maintenance task.
- Hardware-family support may be implemented without local real-hardware validation, but documentation and release claims distinguish encoded support from runtime-validated support.
