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
| swap/pagefile | native page-file accounting for actual configured capacity and occupancy; aggregate all active page files |
| processes | one `NtQuerySystemInformation(SystemProcessInformation)` snapshot for identity, CPU counters, and working set |
| network interfaces | IP Helper `GetIfTable2` for identity, state, and counters, correlated with Plug and Play device provenance for physical-interface selection; `InterfaceGuid` is the stable network identity source |
| physical disks | `CreateFile(\\.\\PhysicalDriveN)` plus `DeviceIoControl(IOCTL_DISK_PERFORMANCE)`; storage properties provide stable identity where available; `StorageDeviceTemperatureProperty` is the preferred storage-temperature source, with legacy ATA SMART used only as a compatibility fallback when the structured property is absent; logical drive letters are resolved through volume disk extents only as associated display metadata |
| structured temperatures | SetupAPI-discovered ACPI thermal-zone and battery interfaces use their documented temperature IOCTLs; topology is discovered outside the sampling hot path and sampled directly without WMI |
| hardware telemetry | coordinated CPU/Super-I/O/EC discovery and sampling through structured native interfaces or pinned signed PawnIO modules; typed temperature/fan projections share source topology and runtime ownership |
| GPUs | dynamically loaded NVIDIA NVML and AMD ADL from installed display drivers; D3DKMT adapter performance data supplies Intel GPU temperature when available |
| per-process network attribution | built-in NDU (`\\.\NduIoDevice`) accounting queried through a platform-private compatibility wrapper; NDU `IfLuid` is canonicalized to the same stable network identity used by the interface collector |
| per-process disk attribution | future Windows-native attribution work; PawnIO is not the attribution mechanism |

Do not introduce PDH for these basic collectors where the direct structured source already owns the semantic.

Windows projects shared `swap` from actual page-file capacity and occupancy, not from system commit accounting. Physical-memory and page-file observations therefore remain distinct even when a Windows memory-status API exposes fields whose names contain page-file terminology.

Storage temperature keeps the least-privilege structured query path independent from legacy ATA SMART. The normal storage-property query must not acquire read/write disk access merely because the SMART fallback requires it; fallback access failure affects only that optional source. The product projection exposes at most one representative structured temperature per physical disk: prefer sensor index 0 when present, otherwise the lowest valid sensor index. Diagnostic projection may retain every valid structured sensor. The representative sensor keeps its actual sensor index in stable identity even when a nonzero index is selected as fallback.

## PawnIO runtime

Installed PawnIO driver state belongs to Windows and may be shared across processes. A cclover-mon process owns only its session, loaded Pawn modules, and hardware-telemetry source state. The hardware-telemetry runtime acquires long-lived sessions/modules and reuses them across temperature, fan, and future hardware-sensor projections; sampling never installs, starts, stops, removes, or unloads the machine-level driver. Each independently failing hardware source owns its own initialization, bounded retry, stable-unavailability, and session-recovery state. Failure or recovery of one source does not reset unrelated source state.

The runtime talks directly to the documented buffered device IO-control interface rather than shipping `PawnIOLib.dll`. Only signed modules used by implemented collectors are embedded. PawnIO handles, module blobs, IOCTL identifiers, and NTSTATUS details remain platform-private. Privileged provisioning is outside sampling and is governed by [ADR 009](../09-architecture-decisions/009-windows-pawnio-provisioning.md).

Diagnostic probes activate only source paths that can produce the requested metric. A temperature probe may initialize Super-I/O or EC when those sources expose temperature channels, but it must not read fan-only channels merely because production sampling composes both projections into one hardware batch; the inverse applies to fan probes.

Selected unchanged Linux hwmon source is the primary compatibility upstream for eligible CPU and Super-I/O hardware according to [ADR 012](../09-architecture-decisions/012-linux-hwmon-windows-compatibility.md). Imported driver code executes only through a project-owned minimal Linux compatibility facade and Windows transport; Linux kernel device/sysfs/lifecycle concepts do not cross the platform boundary. LibreHardwareMonitor remains a secondary reviewed source for board mappings, EC knowledge, migration comparison, or unsupported hardware families and is not loaded or shipped at runtime. Chip-family register behavior stays separate from manufacturer/model-specific channel naming; improving a channel label must not change sensor identity.

The compatibility transport is authoritative for privileged hardware access. Imported Linux code does not gain arbitrary PCI, MSR, port-I/O, or register-write authority. Writes required only to perform a read protocol, such as bank/index/logical-device selection, may be exposed narrowly; persistent control, threshold, device-enable, PWM, or firmware-policy writes remain outside the telemetry capability unless separately decided.

Windows hardware sources separate native side effects from collector policy at the narrowest source-local seam: native APIs, driver calls, affinity changes, and vendor-runtime queries produce typed outcomes before applicability, fallback, identity, availability, degradation, backend selection, or product/diagnostic projection is decided. Those policy decisions must remain deterministically testable without requiring matching physical hardware. Prefer values and pure projection functions over dependency-injection frameworks; do not introduce a cross-source hardware abstraction merely to make tests injectable. Diagnostic text is an output of typed policy state, never an input or semantic test oracle.

Representative Windows hardware remains required to validate ABI, driver, firmware, signed PawnIO capability, and vendor-runtime compatibility that cannot be simulated meaningfully. Such runtime checks complement rather than replace deterministic collector-policy tests.

## GPU runtime

NVIDIA loads driver-installed `nvml.dll`, enumerates devices once, and derives GPU identity from NVML UUID. AMD loads driver-installed ADL according to process bitness, enumerates present AMD adapters once, and derives identity from ADL UDID with PCI location as fallback. Intel GPU temperature uses the Windows D3DKMT adapter-performance query rather than introducing another vendor runtime. D3DKMT establishes GPU-vendor topology before vendor runtimes are required. Vendor presence is tri-state: present, absent, or unknown. Only confirmed absence suppresses a vendor backend; incomplete topology discovery remains unknown and must not silently suppress a potentially applicable backend. GPU adapters retain only read-only state needed by sampling and reuse sessions across samples. Missing required vendor libraries, unsupported telemetry on a present or potentially present device, or individual query failures degrade only the relevant source/device/field.

## Capability and identity semantics

Unimplemented Windows capabilities remain explicit typed unavailability. Per-process disk attribution and unimplemented hardware-sensor source families must not appear as successful empty observations or fabricated zeros. Zero fan RPM is valid data only when the source actually reports zero.

Windows per-process network attribution uses NDU network-usage accounting semantics. NDU and Linux eBPF are not required to count bytes at the same network-stack layer: the shared product contract is trustworthy process × interface × RX/TX attribution suitable for bandwidth ranking and anomaly discovery. Within one backend the accounting semantics must remain stable and directional, but the UI and public model must not claim byte-for-byte cross-platform equivalence.

NDU is a built-in Windows component but its `NduIoDevice` IOCTL ABI is undocumented. All device names, IOCTL values, envelope parsing, self-relative wire offsets, and schema assumptions therefore stay inside one Windows-private wrapper. Unknown or malformed layouts fail closed as unavailable/degraded data; collector code must never guess offsets, reinterpret a new schema, or silently emit zeroes. The wrapper owns `START_STATS_COLLECTION` / interval `QUERY_STATS` / `STOP_STATS_COLLECTION` lifetime. A successful query consumes the current accounting interval, so callers treat returned bytes as interval usage rather than cumulative OS counters. Permission failure affects only this capability.

Process identity is PID plus process creation time. Network interface index and `PhysicalDriveN` are locators rather than durable identities. Disk observations that must fall back to `PhysicalDriveN` are degraded because that locator is session-local rather than reboot-stable.

The PID 0 idle record returned by `SystemProcessInformation` represents aggregate processor idle time rather than a monitorable process. The Windows process collector filters it before data crosses into the shared process domain. The `System` process remains a normal process observation.

Disk I/O accounting remains physical-device based. Drive letters are mapped to physical disk numbers with `IOCTL_VOLUME_GET_VOLUME_DISK_EXTENTS` and cross the platform boundary only as structured associated labels. They never become `DiskId` values and never redefine the counter scope. Failure to resolve this auxiliary topology does not invalidate a successful physical-disk observation; the dashboard falls back to the physical disk's system label. [ADR 011](../09-architecture-decisions/011-physical-disk-display-aliases.md) owns this semantic.

Windows network presentation represents active hardware-backed network adapters, not every active IP Helper interface. `GetIfTable2` remains authoritative for interface identity, operational state, and traffic counters, but `MIB_IF_ROW2.HardwareInterface` is only a candidate signal and is insufficient to establish physical-device provenance. Candidate interfaces are correlated with Windows Plug and Play device metadata; software/root-enumerated interfaces are excluded while hardware bus-backed adapters remain eligible. Classification is capability/provenance-based and must not depend on adapter aliases, vendor names, descriptions, or maintained product-name blacklists.

Failure to resolve device provenance is distinct from a confirmed software interface. Classification uncertainty remains diagnosable and must not silently become a negative physical-device result; a hardware candidate with unknown provenance may remain visible only as degraded data.

## Desktop integration

The Windows desktop host owns monitor-surface creation, drawing, tray integration, and resize realization. It uses Win32 window/message APIs plus GDI and consumes `NativeScene` through the native ABI. The surface is undecorated, non-resizable, excluded from taskbar and Alt+Tab, non-activating, top-right placed, below normal windows, and pointer-transparent. Dynamic resize reapplies placement and bottom Z-order; Explorer restart triggers shell/tray recovery.

The host establishes the best available process DPI awareness before creating DPI-sensitive drawing resources. `NativeScene` geometry stays in 96-DPI logical units; the host owns per-monitor DPI, monitor work area, logical-to-physical conversion, DPI-dependent font resources, and `WM_DPICHANGED` transitions. A DPI transition invalidates DPI-dependent resources before rebuilding the scene, resizing, placing, and repainting the surface. Newer DPI APIs are capability-detected so older supported Windows versions degrade to the best available awareness level rather than becoming load-time dependencies.

Win32 handles, menu identifiers, font handles, device contexts, DPI state, monitor handles, and drawing resources remain platform-private. User intent crosses upward only as platform-neutral lifecycle commands.
