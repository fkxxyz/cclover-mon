---
summary: "Records the remaining real-Windows semantic validation gap for FileIo ETW disk attribution."
viewpoint: assurance
concerns:
  - maintainability
  - portability
activities:
  - orient
  - change
  - assess
facets:
  area:
    - platform
    - whole-system
---

# Windows ETW Disk Semantic Validation

**Priority:** High

## Root cause

Windows process disk attribution depends on runtime semantics of SystemTraceProvider FileIo initiation, completion, and rundown events. The implementation and pure state machine are deterministically testable, and a real-Windows smoke can prove that the production collector attributes nonzero writes for a known process with a file handle opened before tracing starts, but the exact mapping from supported FileIo completion payloads to the product contract of successful logical read/write bytes is not yet covered by deterministic native tests.

## Evidence

The `windows-etw-runtime` validation profile builds the production binary, keeps a temporary file open before tracing starts, generates sustained writes from a known workload PID, runs the production `disk-attribution` probe, and requires a nonzero attributed write row for that PID. This establishes real ETW session control, FileIo rundown seeding, PID/file/disk correlation, and production probe integration on a suitable Windows host.

The remaining semantic cases are narrower but important: exact completion-byte interpretation for buffered writes, reads, partial reads/writes, EOF behavior, cache-hit reads, failed operations, and representative x86 runtime behavior are not asserted against known expected byte counts. Cross-compilation cannot establish those runtime contracts.

## Governing constraint

Windows attribution must report the same product semantic as Linux: successful logical file read/write bytes attributed to a stable process and one unambiguous physical-storage identity. Requested byte counts, physical `DiskIo` transfer bytes, or process-wide I/O counters must not be substituted when completion semantics differ.

## Scope discovery

Review the Windows ETW FileIo decoder, read/write initiation fields, operation-completion fields, paging-I/O filtering, FileIo rundown seed path, event-loss epoch recovery, PID continuity projection, and the `windows-etw-runtime` native harness. Include every supported FileIo schema/version encountered on supported Windows builds and both supported executable architectures where runtime behavior may differ. Exclude core rate derivation and ranking unless a semantic fixture shows the platform counter contract itself changed.

## Maintenance consequence

Changes to ETW property decoding, completion accounting, Windows SDK bindings, supported Windows versions, or session lifecycle can compile and pass pure tests while silently over-counting, under-counting, or dropping valid logical I/O. Without deterministic semantic cases, maintainers must recreate ad hoc Windows workloads and manually interpret probe output before they can trust such changes.

## Repair direction

Extend the existing Windows-native ETW runtime harness with small controlled workloads whose expected logical byte results are known. Cover read, write, partial completion, EOF/cache behavior, and failure cases using the production collector rather than a duplicate diagnostic implementation. Add representative x86 execution where the supported Windows environment permits it. Keep the harness narrow; do not build a general ETW simulator.

## Exit criteria

- Real-Windows automated tests exercise successful buffered read and write operations with known expected logical byte counts through the production attribution path.
- Partial/EOF and failed-operation cases prove that completion accounting, not requested size, governs reported successful bytes.
- A cache-hit read case demonstrates that the metric remains logical FileIo rather than physical `DiskIo` traffic.
- The pre-opened-file rundown path remains covered by a native test.
- Runtime evidence covers both supported executable architectures, or an explicit supported-platform constraint explains why one architecture cannot be executed natively.
- Scope discovery confirms every supported FileIo payload interpretation is covered by the same semantic contract and no alternate byte-count authority exists.
