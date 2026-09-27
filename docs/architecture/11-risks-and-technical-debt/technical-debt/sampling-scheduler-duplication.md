---
summary: "Tracks duplicated sampling cadence and overrun behavior across GUI and CLI execution paths."
viewpoint: assurance
concerns:
  - maintainability
  - performance
activities:
  - orient
  - change
  - assess
facets:
  area:
    - whole-system
---

# Sampling Scheduler Duplication

## Status

Active — P2.

## Problem

GUI monitoring, CLI dump sampling, and performance sampling independently implement elapsed-time measurement and sleep/timer scheduling around the same sampler.

## Evidence

`app.rs::monitor_stream` and CLI dump/perf loops each calculate remaining `SAMPLE_INTERVAL` time and independently choose async timers or blocking sleeps.

## Maintenance impact

Changes to cadence, warmup, overrun handling, missed-tick policy, cancellation, or backpressure can drift between normal UI, diagnostics, and benchmarks even though they are expected to represent the same sampling semantics.

## Governing constraint

Sampling cadence policy should have one semantic authority while allowing different execution mechanisms, such as async timers for GUI and blocking waits for CLI.

## Resolution direction

Extract the minimum shared scheduling policy rather than forcing all callers through one async/runtime abstraction. Preserve benchmark control over deadlines and measured workload.

## Exit criteria

Cadence and overrun rules are defined once, and GUI/CLI adapters only provide the waiting mechanism or mode-specific termination behavior.
