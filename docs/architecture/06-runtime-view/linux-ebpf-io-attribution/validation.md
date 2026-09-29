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

Bridges, tunnels, VPNs, cross-namespace traffic, route changes, and less common protocol paths remain representative validation cases rather than being inferred from loopback success. Performance validation is defined in the performance View rather than duplicated here.
