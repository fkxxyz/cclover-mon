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
| Temperature source fan-in | Linux hwmon and optional NVIDIA NVML temperature sources produce one platform-neutral temperature sequence; adding/removing one source does not create a source-specific core or UI contract. |
| Stable semantic identity | Any delta, history, cross-sample join, cache, or deduplication that survives one observation is keyed by an explicit stable core identity rather than PID, display text, enumeration order, or another incidental locator. |
| Process identity | Reusing a PID for a new process instance must not inherit CPU or per-process I/O counters from the prior process; collectors that observe the same process through different native sources must canonicalize to the same `ProcessInstanceId`. |
| Temperature identity | Multiple GPUs and other same-name sensors retain independent history through stable sensor identity; display labels are not used as metric identity. |
| Optional NVIDIA telemetry | Missing NVML, zero NVIDIA GPUs, or one unsupported NVIDIA temperature sensor removes only the corresponding entries; startup, hwmon temperatures, and unrelated metrics remain operational with no helper subprocess. |
| Frontend reuse | Desktop and future terminal frontends consume the same renderer-neutral dashboard presentation without duplicating rate derivation, Top-N aggregation, unavailable-value semantics, or common value formatting. |
| Layout authority | A frontend structural change has one layout authority; the Iced block structure used to render the desktop panel is also the structure used to derive its requested panel height. |
| Desktop integration portability | Linux and Windows may use different native tray mechanisms while application-visible desktop commands and shutdown semantics remain platform-neutral. X11 and Wayland do not require separate Linux tray implementations. |
| Desktop integration resilience | Failure to register a native system tray emits a diagnostic but leaves metric sampling and the monitor surface operational. |
| Diagnosability | A developer can distinguish native collection, derivation, runtime timing, and presentation failures using `probe <collector> [--raw]`, `dump`, development logs, and screenshots respectively. |
| Collector isolation | A collector can be exercised independently through the same production collector implementation, with elapsed time and failure/skip reasons visible without starting the GUI. |
| Sampling health | A sampling cycle that exceeds its configured interval produces an overrun diagnostic containing actual duration and target interval. |
| Diagnostic overhead | Development observability does not require a background metrics service, persistent logging pipeline, subprocess polling, or a second metric transport. |
| Event-driven I/O attribution | Per-process disk-device and network-interface attribution is collected without periodic process-wide subprocess polling; kernel-side state and userspace map iteration remain bounded. |
| Attribution correctness | Concurrent controlled workloads can distinguish which TGID generated disk and network activity and which device/interface receives the attribution; whole-system totals alone are insufficient validation. |
| Metric semantics | Disk bytes explicitly identify logical-vs-physical semantics and network bytes identify payload/L3/wire-like semantics; values from different layers are not presented as interchangeable. |
| Privilege minimization | Enabling Linux eBPF I/O attribution does not require unrestricted root authority when the supported kernel exposes sufficient narrower capabilities. |
| Safe capability failure | Missing BTF, capabilities, verifier acceptance, or attach support makes only the affected metric unavailable and exposes a diagnostic reason; it never fabricates a zero measurement. |
| UI iteration | Pure visual changes can use reloadable resources when supported by the selected UI toolkit. |
