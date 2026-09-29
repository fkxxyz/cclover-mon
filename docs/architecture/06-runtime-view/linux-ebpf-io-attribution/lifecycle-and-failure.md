---
summary: "Defines eBPF attribution lifecycle, bounded state, process identity, privilege, and failure behavior."
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

# eBPF Attribution Lifecycle and Failure

The selected Linux backend loads and attaches metric-specific BPF programs during collector initialization and keeps links alive for collector lifetime. Normal shutdown releases links and userspace resources.

Normal sampling reads already-aggregated BPF map state rather than enumerating all processes or launching helpers. Disk and network counter maps are independent bounded LRU hashes keyed by `TGID × native identity × direction`. Values carry process-group leader start time so PID reuse replaces the old cumulative counter instead of inheriting it.

Userspace normalizes the kernel start time into the same Linux birth-marker domain as `/proc/<pid>/stat` before constructing `ProcessInstanceId`. Native disk/interface identifiers are likewise canonicalized to the same `DiskId` / `NetworkId` used by device snapshots. Production sampling may retire attribution keys only when the same cycle's complete process snapshot proves the full process identity is no longer active; degraded or unavailable process snapshots do not authorize deletion.

Failure to load, verify, attach, allocate, access, or resolve native identity is capability failure, not a zero measurement. Disk and network attribution fail independently. At minimum diagnostics distinguish unsupported kernel/BTF facilities, insufficient authority, attach incompatibility, map allocation/access failure, and transient identity-resolution failure.

Loading and attaching eBPF is privileged Linux work. Request only authority required by the selected mechanism; a stronger compatibility fallback must be explicit in deployment documentation and diagnosable. eBPF objects, kernel pointers, BPF handles, `dev_t`, `ifindex`, and libbpf types stay below the platform boundary.
