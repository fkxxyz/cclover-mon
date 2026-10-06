---
summary: "Defines production-path diagnostic workloads used by external performance profilers."
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
---

# Performance Diagnostic Workloads

Repository-owned performance diagnostics select production-faithful workloads for external profilers; they are not a parallel benchmark framework. Collector and headless isolation stays in the native performance CLI, while active Web delivery uses a narrow external launcher so subscriber-side CPU is not charged to the measured cclover-mon process.

```text
cclover-mon perf headless [--duration <seconds> | --samples <count>]
cclover-mon perf collector <name> [--duration <seconds> | --samples <count>]
bun perf-web.ts --executable <cclover-mon> --updates <count>
```

`perf headless` runs the normal `Collector → Sampler → MonitorState` cycle at production cadence without creating UI/presentation work. `perf collector <name>` runs the selected production collector at the same cadence and discards its result without probe formatting. Isolated collector workloads also execute the minimum production context required to preserve lifecycle, identity, bounded-state, and projection semantics. Context-dependent workloads consume the same platform-private product-composition helpers as normal sampling rather than maintaining an independent prerequisite or projection recipe. That prerequisite work is part of whole-process workload cost; use a sampling profiler when function-level attribution is required.

Collector names are the same controlled set accepted by `probe`: `cpu`, `memory`, `processes`, `network`, `network-attribution`, `disk`, `disk-attribution`, `temperatures`, `fans`, and `gpu`. Probe and performance diagnostics may use different orchestration when probe-specific detail projections would not match production sampling semantics; performance dispatch must not reuse probe orchestration merely because both select the same collector implementation.

The native CLI default run is unbounded for profiler attachment. `--duration` and `--samples` only terminate the run; they do not alter cadence. Diagnostic formatting, terminal output, or synthetic replacement work is excluded unless that work is the subject being measured.

`perf-web.ts` launches the supplied executable through its ordinary production HTTP composition (`--http --http-bind 127.0.0.1:0`) and establishes a real `/events` SSE subscriber from the launcher process. The first complete SSE event carrying a `data` field is an activation handshake and is not counted as fixed work; `--updates N` completes only after N subsequent complete data-bearing SSE events. Multiple `data:` lines within one SSE event still represent one dashboard update, and comment/keepalive events do not count. Each counted event therefore traverses the production sampler publication, HTTP state hub, shared Dashboard/Scene construction, SVG rendering, JSON serialization, SSE framing, and socket write path. Browser DOM parsing, layout, painting, and compositor work are intentionally outside this server-side workload.

The SSE consumer must remain outside the measured cclover-mon process. Subscriber socket reads, framing scans, and payload draining are control-side work that a real browser performs in another process; including them in cclover-mon's child CPU would make Web payload changes alter both the measured producer cost and benchmark-controller cost. The launcher discovers the ephemeral loopback address from the production server's existing listen diagnostic, keeps the subscriber active, and terminates the production process after fixed work. Startup/address discovery and dashboard progress are bounded: failure to report the listen address, establish SSE response, activate rendering, or continue delivering dashboard events fails closed and tears down the child instead of waiting indefinitely; keepalives do not reset dashboard progress. When run directly it reports the measured child PID after activation, so sampling profilers can attach to that production process without changing workload semantics; choose a sufficiently large update count for an interactive profiling window. The launcher does not inject synthetic state, call renderer internals, change bounded latest-state semantics, or add runtime performance hooks.

Additional isolation points are justified only when they reuse the corresponding production implementation rather than creating a parallel implementation that measures different work. Isolation may remove unrelated production work, but it must not remove context that materially changes the selected collector's steady-state behavior.

Comparative experiments do not add another workload implementation. `perf-compare.ts` invokes the native `perf` CLI authority for headless/collector work and the shared `perf-web.ts` launcher authority for active Web work. The comparison layer owns only balanced execution order, executable identity, measured child CPU-time collection, and noise-aware summarization; it does not reconstruct HTTP readiness or SSE lifecycle policy.
