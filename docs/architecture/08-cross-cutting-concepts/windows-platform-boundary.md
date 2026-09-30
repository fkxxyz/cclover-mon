---
summary: "Defines Windows-specific collector, runtime-integration, identity, and desktop-host boundaries."
viewpoint: static
concerns:
  - architecture-coherence
  - performance
  - portability
  - maintainability
  - security
activities:
  - orient
  - change
  - assess
facets:
  area:
    - platform
---

# Windows Platform Boundary

Prefer direct structured native APIs that expose the required counters without subprocesses or WMI polling.

| Metric responsibility | Native source |
| --- | --- |
| aggregate CPU | `GetSystemTimes`; `GetActiveProcessorCount(ALL_PROCESSOR_GROUPS)` provides logical-CPU scale for process CPU derivation |
| physical memory | `GlobalMemoryStatusEx` |
| processes | one `NtQuerySystemInformation(SystemProcessInformation)` snapshot for identity, CPU counters, and working set |
| network interfaces | IP Helper `GetIfTable2` for identity, state, and counters, correlated with Plug and Play device provenance for physical-interface selection; `InterfaceGuid` is the stable network identity source |
| physical disks | `CreateFile(\\.\\PhysicalDriveN)` plus `DeviceIoControl(IOCTL_DISK_PERFORMANCE)`; storage properties provide stable identity where available; logical drive letters are resolved through volume disk extents only as associated display metadata |
| hardware telemetry | coordinated CPU/Super-I/O/EC discovery and sampling through structured native interfaces or pinned signed PawnIO modules; typed temperature/fan projections share source topology and runtime ownership |
| GPUs | dynamically loaded NVIDIA NVML and AMD ADL from installed display drivers |
| per-process disk/network attribution | future ETW/WFP-class work; PawnIO is not the attribution mechanism |

Do not introduce PDH for these basic collectors where the direct structured source already owns the semantic.

## PawnIO runtime

Installed PawnIO driver state belongs to Windows and may be shared across processes. A cclover-mon process owns only its session, loaded Pawn modules, and hardware-telemetry source state. The hardware-telemetry runtime acquires long-lived sessions/modules and reuses them across temperature, fan, and future hardware-sensor projections; sampling never installs, starts, stops, removes, or unloads the machine-level driver. Each independently failing hardware source owns its own initialization, bounded retry, stable-unavailability, and session-recovery state. Failure or recovery of one source does not reset unrelated source state.

The runtime talks directly to the documented buffered device IO-control interface rather than shipping `PawnIOLib.dll`. Only signed modules used by implemented collectors are embedded. PawnIO handles, module blobs, IOCTL identifiers, and NTSTATUS details remain platform-private. Privileged provisioning is outside sampling and is governed by [ADR 009](../09-architecture-decisions/009-windows-pawnio-provisioning.md).

Diagnostic probes activate only source paths that can produce the requested metric. A temperature probe may initialize Super-I/O or EC when those sources expose temperature channels, but it must not read fan-only channels merely because production sampling composes both projections into one hardware batch; the inverse applies to fan probes.

Super-I/O and EC compatibility knowledge may be ported from the pinned reviewed LibreHardwareMonitor upstream according to [ADR 010](../09-architecture-decisions/010-windows-hardware-telemetry-upstream.md). LibreHardwareMonitor is not loaded or shipped at runtime. Chip-family register behavior stays separate from manufacturer/model-specific channel naming; improving a channel label must not change sensor identity.

## Vendor GPU runtime

NVIDIA loads driver-installed `nvml.dll`, enumerates devices once, and derives GPU identity from NVML UUID. AMD loads driver-installed ADL according to process bitness, enumerates present AMD adapters once, and derives identity from ADL UDID with PCI location as fallback. Both adapters resolve only the read-only ABI needed by the collector and reuse sessions across samples. Missing libraries or individual query failures degrade only the relevant source/device/field.

## Capability and identity semantics

Unimplemented Windows capabilities remain explicit typed unavailability. Per-process disk/network attribution and unimplemented hardware-sensor source families must not appear as successful empty observations or fabricated zeros. Zero fan RPM is valid data only when the source actually reports zero.

Process identity is PID plus process creation time. Network interface index and `PhysicalDriveN` are locators rather than durable identities. Disk observations that must fall back to `PhysicalDriveN` are degraded because that locator is session-local rather than reboot-stable.

The PID 0 idle record returned by `SystemProcessInformation` represents aggregate processor idle time rather than a monitorable process. The Windows process collector filters it before data crosses into the shared process domain. The `System` process remains a normal process observation.

Disk I/O accounting remains physical-device based. Drive letters are mapped to physical disk numbers with `IOCTL_VOLUME_GET_VOLUME_DISK_EXTENTS` and cross the platform boundary only as structured associated labels. They never become `DiskId` values and never redefine the counter scope. Failure to resolve this auxiliary topology does not invalidate a successful physical-disk observation; the dashboard falls back to the physical disk's system label. [ADR 011](../09-architecture-decisions/011-physical-disk-display-aliases.md) owns this semantic.

Windows network presentation represents active hardware-backed network adapters, not every active IP Helper interface. `GetIfTable2` remains authoritative for interface identity, operational state, and traffic counters, but `MIB_IF_ROW2.HardwareInterface` is only a candidate signal and is insufficient to establish physical-device provenance. Candidate interfaces are correlated with Windows Plug and Play device metadata; software/root-enumerated interfaces are excluded while hardware bus-backed adapters remain eligible. Classification is capability/provenance-based and must not depend on adapter aliases, vendor names, descriptions, or maintained product-name blacklists.

Failure to resolve device provenance is distinct from a confirmed software interface. Classification uncertainty remains diagnosable and must not silently become a negative physical-device result; a hardware candidate with unknown provenance may remain visible only as degraded data.

## Desktop integration

The Windows desktop host owns monitor-surface creation, drawing, tray integration, and resize realization. It uses Win32 window/message APIs plus GDI and consumes `NativeScene` through the native ABI. The surface is undecorated, non-resizable, excluded from taskbar and Alt+Tab, non-activating, top-right placed, below normal windows, and pointer-transparent. Dynamic resize reapplies placement and bottom Z-order; Explorer restart triggers shell/tray recovery.

The host establishes the best available process DPI awareness before creating DPI-sensitive drawing resources. `NativeScene` geometry stays in 96-DPI logical units; the host owns per-monitor DPI, monitor work area, logical-to-physical conversion, DPI-dependent font resources, and `WM_DPICHANGED` transitions. A DPI transition invalidates DPI-dependent resources before rebuilding the scene, resizing, placing, and repainting the surface. Newer DPI APIs are capability-detected so older supported Windows versions degrade to the best available awareness level rather than becoming load-time dependencies.

Win32 handles, menu identifiers, font handles, device contexts, DPI state, monitor handles, and drawing resources remain platform-private. User intent crosses upward only as platform-neutral lifecycle commands.
