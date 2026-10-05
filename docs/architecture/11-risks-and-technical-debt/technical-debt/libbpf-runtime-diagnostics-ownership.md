---
summary: "Records that libbpf can still emit raw stderr diagnostics outside cclover-mon's typed diagnostic and logging policy."
viewpoint: assurance
concerns:
  - maintainability
  - architecture-coherence
activities:
  - orient
  - change
  - assess
facets:
  area:
    - platform
---

# libbpf Runtime Diagnostics Ownership

**Priority:** Low-medium

## Root cause

The Linux eBPF boundary does not own libbpf's runtime print policy. libbpf can therefore write diagnostics directly to process stderr independently of cclover-mon's typed collection failures, probe notes, and development logging policy.

## Primary cost dimension

User/operator diagnosis and maintenance.

## Current cost

Expected eBPF unavailability can produce raw third-party error text even while the rest of the product continues normally. Users and operators can mistake those messages for process-wide startup failures, systemd journals can accumulate noisy duplicate diagnostics, and maintainers must reason about two reporting paths for the same underlying failure: cclover-mon's structured failure semantics and libbpf's default stderr output.

## Evidence

During ordinary unprivileged server runtime smoke, libbpf printed messages such as `Failed to bump RLIMIT_MEMLOCK` and `Couldn't load trivial BPF program` while the HTTP server continued operating and the dependent attribution metrics followed the product's existing unavailability semantics. The messages bypassed the probe/devlog path and appeared directly on stderr.

## Cost mechanism

Third-party logging policy is allowed to escape the Linux platform boundary. Because libbpf owns when and how those messages are printed, cclover-mon cannot consistently decide which expected failures should stay quiet during normal operation, which details belong in probe diagnostics, and which unexpected failures deserve development logging.

## Reachable better state

Own libbpf diagnostic routing at the Linux platform boundary. Use libbpf's print callback or equivalent supported mechanism to translate or filter its messages according to cclover-mon's existing diagnostic policy: normal operation remains quiet for expected metric-local unavailability, while probe/development diagnostics retain enough cause information for investigation. Do not introduce a second general logging framework solely for libbpf.

## Governing constraint

A native dependency must not bypass the product's diagnostic policy for expected platform-local failures when the product already has typed failure and diagnostic channels for that boundary.

## Scope discovery

Inspect all libbpf initialization, object loading, attach, map-access, and teardown paths that can emit library diagnostics. Compare normal sampling, `probe`, development logging, server startup, and interactive startup behavior. Distinguish messages needed to diagnose unexpected native failures from expected capability/privilege unavailability that should remain metric-local.

## Repair direction

Install the narrowest supported libbpf print-control mechanism near the existing eBPF runtime adapter. Route actionable messages into the existing probe/devlog policy where practical, suppress only noise whose semantic cause is already represented by typed failure state, and preserve failure isolation. Avoid parsing stderr text after emission or adding broad logging configuration surface.

## Exit criteria

- Ordinary unprivileged server/interactive startup does not emit raw libbpf stderr noise for expected attribution unavailability.
- `probe` and/or development diagnostics still expose sufficient cause information for the same failure.
- Typed attribution failure semantics remain the authority for product behavior; libbpf text does not become a second decision channel.
- Unexpected libbpf failures remain diagnosable rather than being globally silenced.
- Disk and network attribution share the same diagnostic-routing policy rather than maintaining independent suppression rules.
