---
summary: "Defines Linux eBPF network attribution semantics and runtime correlation."
viewpoint: dynamic
concerns:
  - architecture-coherence
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

# Network Attribution

The cross-platform product semantic is **process × network interface × directional attributed network usage**. The product uses this metric to identify which processes consume an interface and to rank unexpected bandwidth users; it does not require every platform to count bytes at the same network-stack layer.

The Linux implementation measures **attributed socket payload bytes**, not wire bytes. This is a Linux-backend accounting semantic, not a cross-platform byte-equivalence requirement. TX counts successful `tcp_sendmsg` / `udp_sendmsg` return bytes while `ip_finish_output2` / `ip6_finish_output2` records the final output `ifindex`. TCP RX counts bytes copied to userspace at `skb_copy_datagram_iter`; UDP RX correlates the dequeued datagram interface from `__skb_recv_udp` with the matching `udp_recvmsg` return.

Process identity and final interface identity are not always available at the same network-stack layer, so bounded event-derived correlation state links socket/process context to final interface context. Missing correlation remains unattributed rather than guessed.

TGID is the process-level native locator. `ifindex` remains Linux-private and is canonicalized to the same core-owned `NetworkId` used by the corresponding interface snapshot before attribution crosses the platform boundary.
