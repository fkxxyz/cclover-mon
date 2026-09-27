---
summary: "Records current architecture risks that require evidence during implementation."
viewpoint: assurance
concerns:
  - performance
  - portability
  - maintainability
  - security
activities:
  - orient
  - change
  - assess
facets:
  area:
    - whole-system
---

# Risks and Technical Debt

- Linux layer-shell integration currently depends on `iced_layershell` 0.19.1 and exact `winit-core` / `winit-common` 0.31.0-beta.2 compatibility pins; upgrades require runtime and build validation.
- Native metric parity will vary by operating system; the shared model must preserve common semantics without flattening meaningful platform-specific data.
- Third-party C++ SDKs may impose runtime or packaging costs that must be measured before adoption.
- Probe availability is partly inferred from diagnostic text in the Linux backend. Diagnostic wording must not carry program semantics; collector status should become typed before probe reporting grows further.
- X11 window-manager behavior varies across EWMH implementations; top-right placement, skip-taskbar/pager, focus avoidance, transparency, and desktop-like stacking require runtime validation on representative window managers.
- Windows desktop integration still remains in `main.rs`; move it behind the platform desktop-integration boundary when the Windows backend becomes real.
- Linux eBPF I/O attribution depends on kernel BTF and attach-point compatibility. CO-RE reduces struct-layout coupling but does not guarantee every supported kernel exposes equivalent hook semantics; compatibility must be demonstrated on representative kernels.
- BPF loading and attachment require elevated authority. Packaging must avoid turning the whole application into unrestricted root solely for I/O attribution; exact capability requirements must be measured against the chosen attach mechanisms and supported kernels.
- Disk attribution deliberately measures logical scalar `vfs_read` / `vfs_write` bytes rather than physical block traffic. That preserves originating TGID across buffered writeback but does not cover vector I/O, splice-like paths, mmap I/O, or other logical-I/O paths until equivalent hooks are added.
- Network attribution measures successful socket payload bytes and correlates process-context send/receive completion with event-observed interface identity. TCP and UDP loopback are runtime-validated, but bridges, tunnels, VPNs, network namespaces, less common protocols, and route changes can expose hook or correlation gaps and still require representative validation.
- BPF maps introduce bounded kernel memory and lifecycle state. Values carry process-group leader start time so PID reuse resets counters, but map eviction under high cardinality remains an intentional bounded-state tradeoff and must be included in stress validation.
- Core process sampling currently treats PID as process identity when deriving cross-sample CPU deltas. PID reuse can therefore associate a new process instance with counters from an earlier process that held the same PID. Introduce a stable process-instance identity, such as PID plus process start time, at the shared model boundary before additional per-process joins or history depend on PID alone.
- Dynamic metric history currently uses display names as identity for several device/sensor series, most notably temperatures. Friendly-name changes, duplicate-name ordinals, or enumeration changes can therefore reset or misassociate history. Separate stable metric identity from display labels in the shared model and key history by identity rather than presentation text.
- The collector catalog has multiple maintenance authorities: `ProbeKind`, string parsing/naming, Linux probe dispatch, performance dispatch, normal backend composition, and CLI help text. Adding or renaming a collector therefore requires shotgun edits and can leave diagnostics inconsistent. Consolidate collector metadata and user-visible naming behind one controlled authority while keeping metric-specific collection algorithms independently owned.
- Linux eBPF userspace support currently combines generic libbpf FFI/object/map lifecycle with disk-attribution and network-attribution semantics in one `ebpf_io` module and collector. As hook logic, native-ID resolution, caching, and diagnostics grow, unrelated attribution mechanisms can become coupled again. Split genuinely shared eBPF runtime/map-access infrastructure from independently owned disk and network attribution modules before that module becomes a second backend-level responsibility aggregate.
- `cclover-mon dump` currently maintains byte, percentage, and unavailable-value formatting separately from the renderer-neutral presentation layer. The rules already differ, and a future terminal frontend could create a third formatting authority. Reuse shared presentation/formatting semantics for human-readable frontends and diagnostics where appropriate; keep `perf` paths formatting-free so benchmark workload semantics remain unchanged.
