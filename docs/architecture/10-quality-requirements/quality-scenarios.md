---
summary: "Defines the observable quality requirements that drive cclover-mon architecture."
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

# Quality Requirements

| Quality | Scenario |
|---|---|
| Performance | Adding a metric does not require subprocess polling, per-widget collection, or serialization between collection and UI. |
| Efficiency | Sampling reuses bounded storage and avoids work proportional to UI object count. |
| Portability | Linux and Windows produce the same shared semantic model without leaking native API types into shared code. |
| Extensibility | A new native metric source is added behind the platform or bridge boundary without changing unrelated collectors or UI contracts. |
| Architecture boundary enforcement | A forbidden top-level source dependency among the declared `core`, `platform`, `presentation`, `ui`, and `tui` domains is rejected by a millisecond-scale repository gate without invoking Cargo or compiling the application. |
| Repository validation enforcement | Deterministic architecture, formatting, build, test, lint, and supported Windows cross-build checks are declared once in `validate.ts`; local workflows and CI invoke the same profiles rather than maintaining parallel command lists. |
| Supported-target compile coverage | Deterministic validation compiles both production code and `#[cfg(test)]` code for every supported Rust target, so target-specific test code cannot remain outside the validation matrix. |
| Windows hardware-policy testability | CPU telemetry, storage temperature, ACPI/battery temperature, and GPU topology/backend policy are exercised on Windows CI from typed scripted outcomes without requiring a particular CPU, disk, battery, or GPU; cross-validation still compiles the same target-specific tests for both supported Windows architectures. |
| Native safety boundary | Rust code denies unsafe operations by default; required native unsafety is confined to explicit adapter modules with documented local safety invariants, while metric collectors consume safe APIs. |
| GPU source fan-in | Platform-native vendor sources fan into one `GpuSnapshot` model; adding or removing one source does not create a vendor-specific core or UI contract. |
| Stable semantic identity | Any delta, history, cross-sample join, cache, or deduplication that survives one observation is keyed by an explicit stable core identity rather than PID, display text, enumeration order, or another incidental locator. |
| Process identity | Reusing a PID for a new process instance must not inherit CPU or per-process I/O counters from the prior process; collectors that observe the same process through different native sources must canonicalize to the same `ProcessInstanceId`. |
| GPU identity | Utilization, VRAM, and temperature history for each GPU share one stable `GpuId`; display labels and enumeration positions are not used as metric identity. |
| Optional GPU telemetry | A missing vendor runtime/source or unsupported device field leaves only that source/field unavailable; startup and unrelated metrics remain operational with no helper subprocess or fabricated zero. |
| Frontend semantic reuse | Native desktop, Web, and terminal frontends consume the same presentation semantics without duplicating derivation, ranking, availability, or common formatting. |
| Graphical UI authority | Native desktop and Web consume one `cclover-ui` dashboard/layout authority and the same final `Scene`; structural, visual-token, graph-policy, text-slot, or geometry changes are defined once. |
| Desktop integration portability | Linux and Windows may use different native tray mechanisms while application-visible desktop commands and shutdown semantics remain platform-neutral. X11 and Wayland do not require separate Linux tray implementations. |
| Desktop integration resilience | Failure to register a native system tray emits a diagnostic but leaves metric sampling and the monitor surface operational. |
| Diagnosability | A developer can distinguish native collection, derivation, runtime timing, and presentation failures using `probe <collector> [--raw]`, `dump`, development logs, and screenshots respectively. |
| Collector isolation | A collector can be exercised independently through the same production collector implementation, with elapsed time and failure/skip reasons visible without starting the GUI. |
| Collector outcome semantics | A successful empty observation, usable partial observation, and unavailable observation remain distinct typed states through the raw sampling boundary; probe/derivation decisions do not parse diagnostic text, and recovery after unavailability starts a fresh delta baseline. |
| Sampling health | A sampling cycle that exceeds its configured interval produces an overrun diagnostic containing actual duration and target interval. |
| Startup snapshot | The first successful collection publishes all currently observable entities without waiting one sampling interval; delta-derived rates with no comparable baseline are zero for that cycle, while genuinely unavailable sources remain unavailable. |
| Diagnostic overhead | Development observability does not require a background metrics service, persistent logging pipeline, subprocess polling, or a second metric transport. |
| Web monitor opt-in | Starting without `--http` opens no HTTP listener. Enabling HTTP with no explicit bind listens only on `127.0.0.1:9847`; LAN exposure requires an explicit bind address. |
| Web sampling authority | Any number of browser clients reuse completed states from the native sampler; they do not add collectors or change sampling cadence. Slow clients cannot create unbounded state queues. |
| Web transport exposure | Adding an internal core state field does not make it remotely visible or alter the HTTP/SSE schema unless the explicit Web transport projection is deliberately updated. |
| Event-driven I/O attribution | Per-process disk-device and network-interface attribution is collected without periodic process-wide subprocess polling; kernel-side state and userspace map iteration remain bounded. |
| Attribution correctness | Concurrent controlled workloads can distinguish which TGID generated disk and network activity and which device/interface receives the attribution; whole-system totals alone are insufficient validation. |
| Metric semantics | Disk bytes explicitly identify logical-vs-physical semantics and network bytes identify payload/L3/wire-like semantics; values from different layers are not presented as interchangeable. |
| Privilege minimization | Enabling Linux eBPF I/O attribution does not require unrestricted root authority when the supported kernel exposes sufficient narrower capabilities. |
| Safe capability failure | Missing BTF, capabilities, verifier acceptance, or attach support makes only the affected metric unavailable and exposes a diagnostic reason; it never fabricates a zero measurement. |
