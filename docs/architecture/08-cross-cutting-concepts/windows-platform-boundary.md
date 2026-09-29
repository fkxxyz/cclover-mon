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
| network interfaces | IP Helper `GetIfTable2`; `InterfaceGuid` is the stable network identity source |
| physical disks | `CreateFile(\\.\\PhysicalDriveN)` plus `DeviceIoControl(IOCTL_DISK_PERFORMANCE)`; storage properties provide stable identity where available |
| hardware telemetry | coordinated CPU/Super-I/O/EC discovery and sampling through structured native interfaces or pinned signed PawnIO modules; typed temperature/fan projections share source topology and runtime ownership |
| GPUs | dynamically loaded NVIDIA NVML and AMD ADL from installed display drivers |
| per-process disk/network attribution | future ETW/WFP-class work; PawnIO is not the attribution mechanism |

Do not introduce PDH for these basic collectors where the direct structured source already owns the semantic.

## PawnIO runtime

Installed PawnIO driver state belongs to Windows and may be shared across processes. A cclover-mon process owns only its session, loaded Pawn modules, and hardware-telemetry source state. The hardware-telemetry runtime acquires long-lived sessions/modules and reuses them across temperature, fan, and future hardware-sensor projections; sampling never installs, starts, stops, removes, or unloads the machine-level driver.

The runtime talks directly to the documented buffered device IO-control interface rather than shipping `PawnIOLib.dll`. Only signed modules used by implemented collectors are embedded. PawnIO handles, module blobs, IOCTL identifiers, and NTSTATUS details remain platform-private. Privileged provisioning is outside sampling and is governed by [ADR 009](../09-architecture-decisions/009-windows-pawnio-provisioning.md).

Super-I/O and EC compatibility knowledge may be ported from the pinned reviewed LibreHardwareMonitor upstream according to [ADR 010](../09-architecture-decisions/010-windows-hardware-telemetry-upstream.md). LibreHardwareMonitor is not loaded or shipped at runtime. Chip-family register behavior stays separate from manufacturer/model-specific channel naming; improving a channel label must not change sensor identity.

## Vendor GPU runtime

NVIDIA loads driver-installed `nvml.dll`, enumerates devices once, and derives GPU identity from NVML UUID. AMD loads driver-installed ADL according to process bitness, enumerates present AMD adapters once, and derives identity from ADL UDID with PCI location as fallback. Both adapters resolve only the read-only ABI needed by the collector and reuse sessions across samples. Missing libraries or individual query failures degrade only the relevant source/device/field.

## Capability and identity semantics

Unimplemented Windows capabilities remain explicit typed unavailability. Per-process disk/network attribution and unimplemented hardware-sensor source families must not appear as successful empty observations or fabricated zeros. Zero fan RPM is valid data only when the source actually reports zero.

Process identity is PID plus process creation time. Network interface index and `PhysicalDriveN` are locators rather than durable identities. Disk observations that must fall back to `PhysicalDriveN` are degraded because that locator is session-local rather than reboot-stable.

Physical-network selection uses `MIB_IF_ROW2` hardware-interface capability rather than adapter-name patterns.

## Desktop integration

The Windows desktop host owns monitor-surface creation, drawing, tray integration, and resize realization. It uses Win32 window/message APIs plus GDI and consumes `NativeScene` through the native ABI. The surface is undecorated, non-resizable, excluded from taskbar and Alt+Tab, non-activating, top-right placed, below normal windows, and pointer-transparent. Dynamic resize reapplies placement and bottom Z-order; Explorer restart triggers shell/tray recovery.

Win32 handles, menu identifiers, font handles, device contexts, and drawing resources remain platform-private. User intent crosses upward only as platform-neutral lifecycle commands.
