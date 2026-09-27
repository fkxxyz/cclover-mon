---
summary: "Defines Linux eBPF attribution for per-process disk-device and network-interface I/O."
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

Linux process I/O attribution is event-driven. Small eBPF programs observe selected kernel paths, aggregate counters in bounded BPF maps, and expose those counters to the Linux platform collector. The shared core remains unaware of eBPF, kernel structs, device numbers, interface indexes, and attach mechanisms.

```text
disk kernel paths                    network kernel paths
       │                                    │
       ▼                                    ▼
 disk eBPF programs                  network eBPF programs
       │                                    │
       ├── (TGID, device, R/W)              ├── (TGID, ifindex, RX/TX)
       │          → byte counters           │          → byte counters
       └──────────────────┬─────────────────┘
                          ▼
                    bounded BPF maps
                          │
                          ▼
                 Linux platform collectors
                          │
                native-ID resolution/cache
                          │
                          ▼
                core-owned typed snapshots
```

## Disk Attribution

The required semantic is **process × storage device × read/write bytes**. Disk attribution may use more than one kernel observation point when needed to preserve process identity across asynchronous block-I/O execution. A request observed later in the block layer may execute in writeback or kernel-worker context, so the design must not assume that `current` at final device submission is the originating process.

The Linux implementation therefore owns any correlation state needed to carry originating TGID information from a process-associated point to the device-associated point. Device identity remains native while inside the Linux collector and is resolved through sysfs into the product's device identity before crossing the platform boundary.

The implemented disk metric is **logical file I/O bytes**. It attaches to `vfs_read` / `vfs_write` return paths, attributes successful returned bytes to the current TGID, reads the backing filesystem `s_dev` through CO-RE, then resolves that native device to the parent product block-device identity in userspace. It therefore does not claim to be physical media traffic and must not be compared directly with block-layer sector counters as if the byte definitions were identical. This choice intentionally avoids writeback-worker misattribution because process identity and the logical I/O result are observed in the same call context.

## Network Attribution

The required semantic is **process × network interface × RX/TX bytes**. Process identity and final interface identity are not necessarily available at the same network-stack layer, especially for receive traffic. The network eBPF implementation may therefore maintain bounded correlation state between process/socket context and later packet or routing context.

Network attribution must preserve these rules:

- use TGID for process-level attribution rather than thread ID;
- retain interface identity as `ifindex` inside the Linux implementation and resolve names in userspace;
- the implemented network metric is **attributed socket payload bytes**, not wire bytes: TX counts successful `tcp_sendmsg` / `udp_sendmsg` return bytes while `ip_finish_output2` / `ip6_finish_output2` records the final output `ifindex` for that socket; TCP RX counts bytes copied to userspace at `skb_copy_datagram_iter`, while UDP RX carries the dequeued datagram's interface from `__skb_recv_udp` to the matching `udp_recvmsg` return;
- interface correlation is bounded and event-derived; when no final TX interface or RX interface can be established, the event remains unattributed rather than being assigned to a guessed interface;
- treat loopback, tunnels, bridges, VPN interfaces, network namespaces, TCP, and UDP as explicit validation cases rather than assuming physical-Ethernet semantics.

## Shared Collection Contract

Disk and network eBPF programs share the same architectural pattern but not the same hook abstraction. Their kernel-side programs remain independent because attribution mechanics differ. Userspace is partitioned the same way: one narrow shared runtime owns libbpf FFI, object/link lifecycle, and generic map access; independent disk and network collectors own their BPF object selection, map schema, native-ID resolution, row shaping, and merge semantics. The top-level eBPF I/O collector only composes those metric collectors. Shared infrastructure must not become an attribution-semantic authority.

Normal sampling reads already-aggregated BPF map state; it does not enumerate all processes to discover activity and does not launch helper processes. Rates are derived from monotonic byte counters using the actual elapsed sampling interval. Disk and network counter maps are independent bounded LRU hashes keyed by `TGID × native identity × direction`. Each value stores the process-group leader start time; when a PID is reused, the first event from the new process instance replaces that key's cumulative counter instead of inheriting the old instance's bytes. Userspace normalizes that kernel start time into the same Linux birth-marker domain used by `/proc/<pid>/stat` before constructing the core-owned `ProcessInstanceId`; core rate derivation keys by that identity rather than TGID alone. Map iteration remains bounded.

Map key/value structs consumed directly by Rust are explicit C↔Rust ABI. Build-time validation derives the C-side size, alignment, and field offsets from clang's BPF-target record layout and checks the corresponding Rust `#[repr(C)]` types with compile-time assertions. A schema drift that changes either side's binary layout must therefore fail the normal Rust build before userspace can decode map bytes with an incompatible type.

## Lifecycle and Failure

The selected Linux backend loads and attaches the required BPF programs during collector initialization and keeps their links alive for the collector lifetime. Normal shutdown releases those links and associated userspace resources.

Failure to load, verify, or attach an eBPF program is a capability failure, not a zero measurement. The affected metric is reported as unavailable with a diagnostic reason. Failure of disk attribution must not disable unrelated collectors, and failure of network attribution must not disable disk attribution.

The implementation must distinguish at least:

- unsupported or insufficient kernel/BTF facilities;
- insufficient privileges/capabilities;
- attach-point incompatibility;
- BPF map allocation or access failure;
- transient native-ID resolution failure.

## Privilege Boundary

Loading and attaching eBPF is Linux-platform privileged work. Request only the capabilities required by the selected kernel and attach mechanisms. Do not broaden the entire application to unrestricted root authority merely to simplify loading. Any compatibility fallback that requires stronger authority must be explicit in deployment documentation and diagnosable at runtime.

No eBPF object, kernel pointer, `dev_t`, `ifindex`, BPF map handle, or libbpf type crosses into `core`, `presentation`, or frontend code.

## Coverage and Validation

The initial disk hooks cover scalar `vfs_read` / `vfs_write` calls. Vector I/O, splice-like paths, memory-mapped I/O, and other kernel paths are outside this first metric definition unless later hooks add them without changing the declared logical-byte semantic.

The initial network hooks cover TCP/UDP send paths through `tcp_sendmsg` / `udp_sendmsg`, observe final IPv4/IPv6 output-interface selection at `ip_finish_output2` / `ip6_finish_output2`, cover stream receive copies through `skb_copy_datagram_iter`, and cover UDP receive by correlating `__skb_recv_udp` with `udp_recvmsg` on the receiving thread. Socket-to-interface and short-lived RX correlation maps are bounded LRU maps. Protocol paths bypassing these hooks remain unattributed. Network namespaces are resolved in the collector process's namespace, so an `ifindex` that cannot be named there is skipped and diagnosed.

Controlled runtime validation must include both TCP and UDP loopback so process identity, interface identity, direction, and payload-byte semantics can be checked against exact transferred byte counts. Bridges, tunnels, VPNs, cross-namespace traffic, and route changes remain separate representative validation cases rather than being inferred from loopback success.

Validation uses controlled workloads that generate known disk and network traffic and compares attributed totals/rates with independent system evidence. Tests must include concurrent processes so attribution errors cannot hide behind whole-system totals. Performance validation measures both event-path overhead and userspace sampling cost under representative high-I/O workloads.
