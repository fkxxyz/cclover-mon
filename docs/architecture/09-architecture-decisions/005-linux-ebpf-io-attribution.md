---
summary: "Chooses libbpf CO-RE eBPF for Linux per-process disk-device and network-interface I/O attribution."
viewpoint: decision
concerns:
  - architecture-coherence
  - performance
  - maintainability
  - portability
  - security
activities:
  - orient
  - change
  - assess
facets:
  area:
    - platform
---

# ADR 005: Linux eBPF I/O Attribution

## Decision

Linux per-process disk-device and network-interface I/O attribution will use precompiled eBPF programs loaded by the Linux platform backend through libbpf using CO-RE where kernel/BTF support permits it.

The implementation will use separate disk and network eBPF programs because their attribution problems differ, while sharing userspace lifecycle and aggregation infrastructure where useful.

The initial Linux implementation defines these layer semantics explicitly:

- disk attribution is logical successful `vfs_read` / `vfs_write` bytes, resolved from filesystem `s_dev` to a parent block-device identity;
- network attribution is attributed socket payload bytes: TX correlates `tcp_sendmsg` / `udp_sendmsg` with the final IPv4/IPv6 output interface, TCP RX uses process-context copy bytes, and UDP RX correlates the dequeued datagram interface with `udp_recvmsg`;
- neither metric is physical media traffic or wire-equivalent traffic.


The required product semantics are:

```text
disk    = TGID × storage-device identity × read/write bytes
network = TGID × interface identity      × RX/TX bytes
```

The kernel side aggregates counters before userspace reads them. The normal application remains a single application process; no BCC runtime, helper daemon, or periodically spawned monitoring command is part of the production architecture.

## Rationale

Periodic `/proc` scanning can provide useful per-process aggregate I/O but does not directly provide the required process × device or process × interface attribution. Launching `iotop`, `pidstat`, `nethogs`, BCC tools, or similar commands would add subprocess lifecycle, parsing, scheduling noise, and an external runtime dependency to every sample.

A loadable kernel module could expose arbitrary data but would introduce a substantially larger privilege and stability surface, kernel-module ABI/build concerns, and more deployment coupling than required for observability.

BCC is valuable as reference implementation and diagnostic tooling, but its traditional runtime-compilation model requires Python/BCC plus clang/LLVM on the monitored machine. cclover-mon instead compiles BPF objects at build time and loads them directly at runtime.

libbpf CO-RE keeps Linux-specific tracing code behind the platform boundary while reducing dependence on exact kernel struct layouts across BTF-enabled kernels. It also preserves the existing single-process native architecture.

## Consequences

- Linux builds gain a BPF compilation step and libbpf-facing integration.
- Deployment must account for kernel/BTF support and least-privilege BPF loading/attachment.
- eBPF unavailability becomes an explicit unavailable-metric state, not a fabricated zero.
- Disk and network collectors may maintain kernel-side correlation state whose size and lifetime must be bounded.
- Network attribution semantics require explicit validation across TCP, UDP, loopback, tunnels, bridges, VPNs, and namespaces.
- Windows must implement the same shared semantic model with Windows-native facilities; eBPF itself never becomes a shared-core dependency.

## Rejected Alternatives

### Periodic helper-process polling

Rejected for production collection because it adds process-spawn overhead, text parsing, external-tool dependency, and weaker ownership of metric semantics.

### Full `/proc` process scanning

Retained as a possible diagnostic/reference source where useful, but rejected as the primary mechanism for the required cross-product attribution because the needed per-device and per-interface association is not exposed directly in the required form.

### BCC as a runtime dependency

Rejected for production packaging because runtime compilation and the associated toolchain are unnecessary. BCC tools remain valuable design references and independent validation aids.

### Custom kernel module

Rejected because the required tracing and aggregation fit eBPF's constrained execution model, avoiding a larger trusted kernel extension and kernel-module distribution lifecycle.

### One generic kernel hook abstraction for disk and network

Rejected because the commonality is in aggregation and userspace consumption, not in attribution mechanics. Disk and network hooks should evolve independently behind one platform-facing collection contract.

## Reassessment

Revisit this decision if representative supported Linux systems cannot provide sufficiently stable BTF/attach support, if required privileges are unacceptable for the product deployment model, or if measured eBPF overhead is materially worse than a simpler native source that provides the same semantics.
