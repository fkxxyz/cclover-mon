---
summary: "Chooses Windows SystemTraceProvider FileIo ETW for per-process logical disk attribution."
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

# ADR 014: Windows ETW Disk Attribution

## Decision

Windows per-process disk attribution will use a long-lived SystemTraceProvider real-time ETW session and the kernel FileIo event family. It will attribute successful logical file read/write bytes to a process and, only when volume topology resolves unambiguously, to one physical-disk identity.

The implementation uses the existing `windows-sys` dependency and direct ETW/TDH APIs behind a narrow Windows-private adapter. It does not add a general tracing framework or a .NET/C++ ETW dependency. Required event fields are decoded by name through TDH so pointer width and event-version details remain below the attribution state machine.

The attribution layer observes thread lifecycle/rundown, FileIo file-name/object lifecycle, FileIo read/write initiation, and operation completion. Paging I/O is excluded. File/IRP/thread identifiers are correlation locators only. Native disk numbers are canonicalized at the sampling boundary to the same `DiskId` produced by the ordinary physical-disk collector. PID is likewise canonicalized through complete `SystemProcessInformation` snapshots to the existing `ProcessInstanceId`.

The ETW event consumer continuously accumulates bounded interval state independently from the one-second product sampling cadence. No storage IOCTL or process/disk enumeration occurs in the ETW callback. Because FileIo rundown is emitted at trace termination rather than session startup, the collector performs one short start/stop seed trace before the production session and retains the resulting mappings for files that were already open. Volume-to-physical-disk topology is cached outside the hot path. A volume with more than one physical extent cannot be divided accurately from FileIo events and is therefore omitted with degraded status rather than guessed.

ETW event loss or decode loss invalidates cumulative continuity and resets the attribution epoch. Permission failure or ETW/session failure makes disk attribution unavailable; there is no fallback to a differently scoped process-I/O counter.

## Rationale

The shared product disk-attribution semantic is process × storage device × logical successful read/write bytes. Linux already measures logical successful file I/O rather than physical block traffic. Windows physical `DiskIo` events provide excellent physical-device activity data but can execute in cache/writeback/system context and therefore do not preserve the same process-responsibility semantic.

FileIo events preserve the issuing thread/process context and file-object identity needed for logical attribution. Completion correlation allows the Windows backend to account completed operations instead of treating requested size as successful bytes. Volume topology then provides a Windows-native route to the physical-disk identity used by the existing disk cards.

ETW is event-driven and avoids process-by-process polling or helper programs. Keeping ETW session control, TDH decoding, and correlation state inside the Windows platform backend preserves the existing core contract: platform code returns cumulative typed counters, while core owns rate derivation, joining, ranking, and history.

## Consequences

- Starting a system-provider ETW session may require elevated or Performance Log Users-equivalent authority; failure is explicit capability unavailability.
- FileIo and thread events add event-processing overhead even when the UI samples once per second; the hot path must remain bounded and free of topology I/O.
- TDH property decoding favors compatibility and maintainability over hand-coded payload offsets. If profiling proves it material, the implementation may cache narrow validated decode plans without creating a generic ETW framework.
- Multi-disk logical volumes cannot be truthfully projected to one physical disk from FileIo alone and therefore degrade attribution completeness.
- Event loss requires a new counter baseline, because continuing cumulative counters across a known gap would silently undercount later rates.
- Representative Windows runtime validation remains part of the validation plan for the supported FileIo schema. The validation-only ETW semantic harness exercises the production collector with exact successful-completion byte assertions, including buffered reads/writes, partial EOF, failure/zero-byte cases, and a pre-warmed cache-hit read.

## Rejected Alternatives

### Physical `DiskIo` ETW as the product counter

Rejected because it measures physical/lower-storage activity. Cache hits may generate no physical event, delayed writeback and read-ahead may be charged to `System`, and one logical request can become different physical operations. That semantic does not match the existing logical per-process disk metric.

### `GetProcessIoCounters` or PDH process I/O counters

Rejected because those process counters combine file, network, and device I/O and do not identify a physical disk. They cannot provide trustworthy process × disk attribution.

### Joining physical-disk totals with process-wide I/O

Rejected because independent totals contain no causal relation that permits accurate process-to-disk assignment.

### Custom storage/filter driver

Rejected because the required logical attribution is available through ETW. A custom kernel driver would add signing, installation, ABI, security, and system-stability costs without a proportional product benefit.

### Generic ETW abstraction or external TraceEvent/TraceProcessor runtime

Rejected because the product needs a small fixed set of Windows kernel events. A general event framework would add API surface and maintenance authority unrelated to the metric, while .NET tooling would conflict with the native single-executable deployment model.

## Reassessment

Revisit this decision if supported Windows versions cannot expose stable FileIo completion semantics, required ETW privilege is unacceptable, measured ETW overhead is material for normal monitoring, or a documented lower-cost API provides equivalent process × physical-disk logical attribution.
