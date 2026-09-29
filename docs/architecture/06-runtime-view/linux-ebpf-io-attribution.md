---
summary: "Indexes Linux eBPF runtime attribution for per-process disk-device and network-interface I/O."
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

# Linux eBPF I/O Attribution

Linux process I/O attribution is event-driven. Metric-specific eBPF programs observe selected kernel paths, aggregate counters in bounded maps, and expose them to Linux platform collectors. Core remains unaware of eBPF, kernel structs, attach mechanisms, and native device/interface identifiers.

```text
disk kernel paths                    network kernel paths
       │                                    │
       ▼                                    ▼
 disk eBPF programs                  network eBPF programs
       │                                    │
       └──────────── bounded BPF maps ──────┘
                          │
                          ▼
                 Linux platform collectors
                          │
                native-ID canonicalization
                          │
                          ▼
                core-owned typed snapshots
```

Disk and network share lifecycle/map-access infrastructure where useful but not one generic hook abstraction; their attribution mechanics and semantic byte definitions differ.

Detailed runtime Views:

- [Disk Attribution](linux-ebpf-io-attribution/disk.md) — logical process × storage-device read/write bytes.
- [Network Attribution](linux-ebpf-io-attribution/network.md) — attributed process × interface RX/TX payload bytes.
- [Lifecycle and Failure](linux-ebpf-io-attribution/lifecycle-and-failure.md) — bounded maps, process identity, attach lifetime, privilege, and failure semantics.
- [Validation](linux-ebpf-io-attribution/validation.md) — coverage boundaries and correctness evidence.

Performance measurement is defined separately in [eBPF Performance Validation](../13-performance-view/ebpf-performance-validation.md).
