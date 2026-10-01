---
summary: "Records the missing real-Windows SCM lifecycle acceptance coverage for cclover-mon-server."
viewpoint: assurance
concerns:
  - maintainability
  - portability
activities:
  - orient
  - change
  - assess
facets:
  area:
    - whole-system
---

# Windows Server SCM Lifecycle Validation

**Priority:** Medium-high

## Root cause

`cclover-mon-server` implements a Windows Service Control Manager adapter, but current automated evidence stops at cross-compilation. No real-Windows acceptance path exercises service registration, SCM state transitions, HTTP readiness, STOP/SHUTDOWN control delivery, cooperative teardown, and service removal as one production lifecycle.

## Optimization dimension

Primary: test and verification efficiency. Secondary: reliability and recovery efficiency, operations and lifecycle efficiency.

## Current cost

Changes to Windows service startup, control handling, status reporting, privilege assumptions, or shutdown ordering can compile successfully while failing only when installed under SCM. Maintainers must therefore perform ad hoc service installation and manual state inspection to establish the integration contract, increasing verification effort and regression risk for every service-lifecycle change.

## Evidence

The server is continuously cross-built for both supported MSVC targets, while the Linux server smoke proves HTTP readiness and signal-driven teardown only on Linux. The Windows service adapter has no repository-owned real-host test that installs `cclover-mon-server`, starts it through SCM, waits for `SERVICE_RUNNING` and `/readyz`, requests stop, observes `SERVICE_STOPPED`, and cleans up the temporary service.

## Reachable better state

Add a bounded Windows-native server-service acceptance profile that installs a temporary service using the production executable, starts it through SCM, verifies readiness, stops it through SCM, verifies the stopped state and process exit, and removes the service in cleanup. Reuse the existing Windows native/admin execution path rather than building a parallel service harness.

## Governing constraint

A supported Windows service lifecycle must be proven by real SCM execution; cross-compilation may prove API/type compatibility but must not be treated as evidence that SCM state and shutdown behavior work end to end.

## Scope discovery

Cover service installation arguments, `StartServiceCtrlDispatcherW`, service-handler registration, START_PENDING/RUNNING/STOP_PENDING/STOPPED transitions, STOP and SHUTDOWN controls, HTTP listener/readiness, the shared `Shutdown` path, sampler and HTTP worker teardown, process exit status, and test-service cleanup. Include both supported process architectures when the Windows validation environment can execute them.

## Repair direction

Add the smallest Windows-only smoke around the production server executable and existing installation mechanism. Keep deterministic CLI/state-transition logic at lower layers, and reserve the real-host test for facts only SCM can prove. Ensure cleanup runs even after failed assertions so the test does not leave a registered service behind.

## Exit criteria

- A repository-owned Windows-native validation profile installs a temporary `cclover-mon-server` service through SCM.
- The test observes successful startup and `SERVICE_RUNNING`.
- `/readyz` becomes successful through the service-started process.
- SCM STOP reaches the production control handler and the process reaches `SERVICE_STOPPED` without forced termination.
- The temporary service is removed reliably after success or failure.
- The profile runs in the project's supported Windows validation environment without manual interpretation.
