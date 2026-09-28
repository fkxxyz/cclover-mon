---
summary: "Defines how cclover-mon performance is measured, diagnosed, optimized, and regression-checked."
viewpoint: performance
concerns:
  - performance
activities:
  - orient
  - change
  - assess
facets:
  area:
    - whole-system
    - core
    - platform
    - ui
    - native-bridge
---

# Performance Analysis

Performance work follows an evidence chain from whole-process cost to the dominant source-level cause:

```text
baseline
  ↓
subsystem attribution
  ↓
function-level profiling
  ↓
source-level cause
  ↓
minimal optimization
  ↓
same-workload remeasurement
```

## Measurement Layers

- **Process baseline** — use process-level tools such as `ps`, `top`, and `perf stat` to establish CPU time, CPU percentage, memory footprint, wakeups, or other metrics relevant to the reported problem.
- **Subsystem attribution** — use production-path diagnostics to remove layers while preserving the work being measured. `perf headless` runs the full production `Collector → Sampler → MonitorState` cycle without creating a UI; `perf collector <name>` runs one production collector without probe formatting. Use `probe <collector> [--raw]` for correctness inspection, not collector benchmarking, because probe reporting intentionally formats diagnostic output.
- **Function attribution** — on Linux, use sampling profilers such as `perf record -g` and `perf report` to identify dominant functions and call paths across the complete process.
- **Source attribution** — connect measured hot functions to the responsible algorithm, allocation, IO, data movement, layout, text, or rendering path before selecting an optimization.

## Performance Diagnostic CLI

The performance CLI is a workload selector for external profilers, not an internal benchmark framework. It keeps production semantics and suppresses presentation or diagnostic output that would contaminate the workload.

```text
cclover-mon perf headless [--duration <seconds> | --samples <count>]
cclover-mon perf collector <name> [--duration <seconds> | --samples <count>]
```

`perf headless` keeps the normal one-second sampling cadence and executes native collection, core derivation, Top-N aggregation, and bounded history updates. It does not initialize Iced, create a window, build presentation objects, or render.

`perf collector <name>` executes the selected production collector at the normal one-second cadence and discards its result without formatting it. Collector names are the same controlled set accepted by `probe`: `cpu`, `memory`, `processes`, `network`, `network-attribution`, `disk`, `disk-attribution`, and `temperatures`. The attribution-specific names isolate userspace BPF-map sampling from the existing whole-interface and whole-device collectors.

The default run is unbounded so tools such as `perf record -p <pid>` can attach. `--duration` and `--samples` provide bounded runs for repeatable `perf stat`, `time`, or scripted A/B measurements. These controls change only test termination, not the production sampling cadence.

Further isolation points such as core-only derivation, presentation construction, UI layout, or rendering should be added only when they can reuse the corresponding production implementation without introducing a parallel implementation or renderer-specific test path that measures different work.

## Optimization Rules

- Establish a baseline before optimizing.
- Optimize the dominant measured cost rather than code that merely appears inefficient.
- Prefer the smallest change that removes the measured source of cost without changing product semantics unnecessarily.
- Collector timing is local evidence; whole-process profiling is required when cost may also come from core, UI, allocator, renderer, driver, or native-library work.
- Preserve the production one-second sampling cadence in performance diagnostics unless the performance question explicitly concerns cadence itself.
- Repeated native polling should reuse discovery and long-lived handles when the native interface permits it. In particular, temperature sampling should not rescan stable hwmon/NVML topology or recreate an NVML session on every one-second sample; topology refresh is a separate lifecycle concern. Within one sampling cycle, repeated attribution rows for the same native locator must share one topology/identity resolution rather than repeating sysfs or equivalent native discovery per row. Event-driven attribution state must also retire entries for dead process instances so per-sample map traversal scales with current/recent activity rather than process-lifetime history.
- Do not include diagnostic formatting, terminal output, or synthetic replacement work in a benchmark path unless that work is the subject being measured.
- Select metrics that match the problem. CPU time, wakeups, allocation, RSS, and data movement are independent dimensions and do not all need measurement for every investigation.

## Validation

Remeasure after the change under the same workload, sampling interval, build profile, and comparable runtime conditions used for the baseline. A performance optimization is accepted only when measurement demonstrates the intended improvement without violating existing correctness or architecture constraints.

For event-driven Linux eBPF collectors, measure two costs separately:

- **event-path overhead** — compare a controlled high-I/O workload with attribution disabled and enabled, using whole-process/system CPU and workload throughput/latency as evidence; `CCLOVER_MON_DISABLE_EBPF_IO=1` is the diagnostic A/B switch for this measurement and is not a separate production collector implementation;
- **sampling overhead** — measure `perf collector disk-attribution` and `perf collector network-attribution` to isolate BPF-map iteration, native-ID resolution, and output shaping at the configured sampling cadence.

Do not infer low overhead only from the absence of userspace polling. Hook frequency, map contention, per-event work, and map cardinality can dominate. Validate both ordinary desktop traffic and a deliberately high event-rate workload, and keep map sizes/cardinality bounded.
