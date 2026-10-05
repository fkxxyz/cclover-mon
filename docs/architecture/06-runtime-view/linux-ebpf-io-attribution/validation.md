---
summary: "Defines Linux eBPF attribution coverage boundaries and runtime correctness validation."
viewpoint: dynamic
concerns:
  - architecture-coherence
  - maintainability
activities:
  - orient
  - change
  - assess
facets:
  area:
    - platform
---

# eBPF Attribution Validation

Disk coverage currently includes scalar `vfs_read` / `vfs_write`. Vector I/O, splice-like paths, memory-mapped I/O, and other paths remain outside the metric until added without changing the declared logical-byte semantic.

Network coverage includes TCP/UDP send through `tcp_sendmsg` / `udp_sendmsg`, final IPv4/IPv6 output-interface observation at `ip_finish_output2` / `ip6_finish_output2`, TCP receive copies through `skb_copy_datagram_iter`, and UDP receive correlation from `__skb_recv_udp` to `udp_recvmsg`. Paths bypassing these hooks remain unattributed. Network namespaces are resolved from the collector process's namespace; an `ifindex` that cannot be named there is skipped and diagnosed.

Controlled correctness validation includes TCP and UDP loopback with exact transferred byte counts so process identity, interface identity, direction, and payload-byte semantics are independently checkable. Concurrent processes are required so attribution errors cannot hide behind whole-system totals.

Userspace libbpf compatibility has two repository-owned proofs. Ordinary Linux/server validation inspects the built ELF and rejects a libbpf SONAME other than `libbpf.so.1` or any direct import newer than the deployment-owned `LIBBPF_1.0.0` ceiling. The separate elevated `linux-ebpf-runtime` profile runs the production attribution probe with a validation-only preload shim that forces `bpf_map_lookup_batch` to return `EOPNOTSUPP`; disk and network attribution must remain available through scalar traversal and emit a probe diagnostic identifying the fallback. This shim replaces only the batch syscall wrapper and does not duplicate collector or BPF-program behavior.

The same elevated profile also proves production-context stale-state retirement for the isolated performance workloads. A validation-only preload observer transparently forwards real libbpf map reads and deletes while identifying only the production `disk_bytes` and `network_bytes` maps by kernel-reported map name. The smoke starts the real `perf collector disk-attribution` and `perf collector network-attribution` workloads, creates a short-lived process that produces attribution state, confirms the target PID is observed in the production map, then requires a later production sample to delete that PID's map key after the complete process snapshot no longer contains the process. The observer does not alter map contents, collector inputs, return values, or production lifecycle semantics; it exists only to make the native side effect externally assertable.

Bridges, tunnels, VPNs, cross-namespace traffic, route changes, and less common protocol paths remain representative validation cases rather than being inferred from loopback success. Performance validation is defined in the performance View rather than duplicated here.
