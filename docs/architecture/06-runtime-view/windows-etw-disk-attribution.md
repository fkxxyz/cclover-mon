---
summary: "Defines Windows ETW runtime attribution for per-process logical disk I/O."
viewpoint: dynamic
concerns:
  - architecture-coherence
  - performance
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

# Windows ETW Disk Attribution

Windows process disk attribution is event-driven. A long-lived SystemTraceProvider real-time ETW session observes thread lifecycle plus FileIo lifecycle/read/write/completion events. The sampling cycle reads already-accumulated interval state; it does not start tracing or perform per-event storage queries.

The product semantic is **process × physical-storage identity × successful logical file read/write bytes**. It intentionally follows the logical-I/O semantic used by Linux disk attribution rather than physical-media traffic. Paging I/O is excluded. Physical `DiskIo` transfer bytes, process-wide `GetProcessIoCounters`, PDH process I/O counters, and physical-disk totals are not substitutes for this attribution metric because they account at different layers or scopes.

```text
SystemTraceProvider ETW
  ├── Thread start/end/rundown ──────────────── TID → PID
  ├── FileIo name/create/cleanup/close ─────── file key/object → volume route
  └── FileIo read/write + operation end ────── IRP → completed logical bytes
                                                  │
                                                  ▼
                                      PID × native disk number counters
                                                  │
                         sampling boundary ───────┤
                            current processes      │
                            current physical disks │
                                                  ▼
                                   ProcessInstanceId × DiskId
                                                  │
                                                  ▼
                                      ProcessDiskIoCounter
```

The ETW callback is a narrow event-ingestion boundary. It decodes only the required typed fields and updates bounded in-memory correlation state. It does not perform volume enumeration, `DeviceIoControl`, process enumeration, disk discovery, core identity construction, rate derivation, or ranking. Event payloads are decoded through TDH property metadata rather than fixed pointer-sized struct casts so x86 and x64 event layouts do not become business assumptions.

## Correlation and canonical identity

Thread events maintain the temporary `TID → PID` relation required by FileIo events. File name/create events map ETW file keys/objects to cached NT-volume routes. Read/write events create bounded pending entries keyed by IRP; successful completion closes the pending entry and contributes completed bytes. Correlation identifiers remain Windows-private.

Volume topology is refreshed outside the ETW callback. Windows volume GUIDs are resolved to NT device paths and `IOCTL_VOLUME_GET_VOLUME_DISK_EXTENTS` supplies native physical-disk numbers. A logical volume that maps to exactly one physical disk may be attributed to that disk. Multi-disk volumes are ambiguous at the FileIo layer: their I/O is skipped and the observation is degraded rather than duplicated, divided, or guessed.

The disk collector retains its current sample's native disk-number locator only inside the Windows platform boundary and canonicalizes it to the same `DiskId` emitted by the ordinary physical-disk observation. ETW paths, volume GUIDs, file objects, IRPs, thread IDs, and disk numbers never become core identities.

ETW interval state uses PID only as a temporary locator. An interval contributes to cumulative platform counters only when complete process snapshots at both interval boundaries map that PID to the same `ProcessInstanceId`. A process that starts/exits within the interval, a reused PID, or an interval bounded by an incomplete process snapshot is left unattributed rather than guessed. Core remains the authority for cumulative-counter delta/rate derivation and Top-N projection.

## Lifetime, bounded state, and failure

The production ETW session is long-lived for collector lifetime and uses only the required system-provider flags. Before it starts, the collector runs one short seed session whose consumer is confirmed attached before the controller stops it; SystemTraceProvider then emits FileIo rundown for files already open before cclover-mon started. The collector preserves those file-key mappings, clears seed-only thread/IRP/interval state, and starts the long-lived session. A process-wide named session has a system-wide named-object lease: another live cclover-mon instance is never stopped, while a session left behind by abnormal termination may be recovered only when its fixed session GUID matches cclover-mon.

Normal sampling drains interval aggregates while the event consumer continues independently. Volume topology is cached and refreshed outside the hot path. Open-file state retains volume identity rather than a precomputed disk target, so topology refreshes do not discard long-lived file correlations and later I/O resolves against the current route. A failed topology refresh keeps the previous known-good topology when one exists, marks attribution degraded, and rate-limits the next native refresh attempt; initial topology failure remains unavailable. Thread, file, pending-IRP, and interval-key maps have explicit bounds; reaching a bound preserves valid rows but degrades the observation.

ETW session-control permission failure makes disk attribution unavailable with `PermissionDenied`; it does not fall back to `GetProcessIoCounters`, PDH, or another counter with different semantics. Session/consumer failure is isolated to disk attribution and uses bounded retry. A successful empty interval is distinct from capability unavailability.

Event loss or event-decode loss invalidates cumulative attribution continuity. When loss increases, the collector stops the affected session; the controlled stop produces a fresh FileIo rundown, after which thread/IRP/interval state and cumulative platform counters are reset while the refreshed open-file mapping is retained. A new long-lived session then establishes a new baseline before later rates become trustworthy. It must not resume `Available` while retaining counters known to contain an unobservable hole. A consumer/session failure for which controlled rundown cannot be trusted discards file state and requires a fresh seed session after bounded retry.

Topology failures are partial when possible: resolvable single-disk volumes remain usable while the collection is degraded. Unknown local-volume paths, ambiguous multi-disk volumes, malformed completion data, unresolved current disk identities, and bounded-state overflow are diagnosable degradation rather than fabricated zeroes.

## Validation boundary

Deterministic tests cover correlation, paging-I/O exclusion, PID reuse, volume-path boundaries, ambiguous volumes, malformed completions, counter continuity, and loss reset behavior. The `windows-etw-runtime` native profile additionally proves that a known Windows process with a file handle opened before tracing starts receives nonzero write attribution through the production probe, covering real session control and FileIo rundown seeding. Broader Windows semantic validation must still establish exact completion-byte interpretation for successful buffered reads/writes, partial reads, cache-hit reads, and both x86 and x64 binaries. Architecture defines the successful-logical-byte semantic; an implementation must not promote an unvalidated Windows payload-field interpretation into that semantic merely because it compiles or passes the smoke.
