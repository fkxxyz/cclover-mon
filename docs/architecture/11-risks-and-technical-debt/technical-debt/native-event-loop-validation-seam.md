---
summary: "Records the missing deterministic behavioral test seam for native desktop event-loop scheduling and recoverable host lifecycle transitions."
viewpoint: assurance
concerns:
  - maintainability
  - performance
  - portability
activities:
  - orient
  - change
  - assess
facets:
  area:
    - native-bridge
    - platform
    - whole-system
---

# Native Event-Loop and Lifecycle Validation Seam

**Priority:** Medium

## Root cause

Native desktop event-loop scheduling and recoverable protocol lifecycle transitions are implemented directly against real X11, Wayland, and Win32 host loops without a deterministic behavioral seam that can exercise those state transitions independently of the operating-system environments.

## Evidence

Current validation can prove structural properties such as the absence of periodic GUI polling, explicit interruptible shutdown of the shared desktop state bridge, separation of Linux state-ready and lifecycle wake sources, and the intended Wayland surface-loss recovery structure; it can also compile both Linux and Windows host implementations. Full behavioral validation still depends on usable real desktop environments. The Wayland TTY-switch regression demonstrated this gap directly: a compositor `zwlr_layer_surface_v1.closed` event caused the desktop host to return successfully and terminate the application until real Wayfire/VT reproduction exposed the lifecycle mismatch. The repaired host now survives repeated physical-output/NOOP output churn and recreates the layer surface, but deterministic tests still assert source structure rather than driving the lifecycle transitions themselves. Linux runtime validation can additionally be blocked by compositor or layer-shell capability, while Windows cross-compilation cannot execute the Win32 message loop.

## Governing constraint

Native desktop scheduling and recoverable lifecycle changes should be verifiable deterministically at the behavioral boundary. A published state must produce a wake, the latest state must be consumed exactly through the native bridge semantics, and the host must request a frame without relying on periodic polling. Recoverable protocol events must transition only the affected host resource lifecycle rather than application lifetime; repeated dynamic-resource add/remove sequences must not accumulate stale state. Platform adapters should remain responsible for translating these tested decisions into the corresponding OS event primitives.

## Scope discovery

Review the shared desktop state bridge, Linux X11 and Wayland wait/dispatch paths, Wayland layer-surface and `wl_output` lifecycle transitions, Windows message-loop wake routing, native host shutdown behavior, and tests that currently assert only source structure or require a live desktop. Include any future native desktop backend that consumes the same state-to-frame scheduling contract or owns recoverable protocol resources.

## Maintenance consequence

Changes to wake primitives, display dispatch, message routing, or protocol-resource lifecycle can compile successfully while still introducing lost wakes, duplicate frame requests, stale native resources, or accidental process termination. The Wayland TTY-switch failure was a concrete example: source compilation and ordinary desktop operation did not reveal that surface closure had been compressed into application closure. Explicit shutdown channels and the repaired Wayland surface/output lifecycles remove known defects, but without a complete deterministic host behavioral seam, maintainers still rely on platform-specific runtime checks for end-to-end confidence.

## Repair direction

Extract only the smallest pure behavioral seams needed to drive deterministic tests of state-ready, wake, consume-latest, frame-request, shutdown, and recoverable resource-lifecycle transitions. Wayland-specific state such as layer-surface recreation and output-slot bookkeeping may stay Wayland-specific; the goal is deterministic state transition coverage, not a false cross-platform lifecycle abstraction. Keep X11, Wayland, and Win32 primitives in their platform adapters; do not simulate complete window systems or introduce a generalized event-loop framework merely for testing.

## Exit criteria

- Deterministic tests exercise state publication through wake handling to frame-request semantics without requiring X11, Wayland, or Win32 runtime availability.
- Tests cover coalescing to the latest pending state and shutdown ordering sufficiently to detect lost-wake and post-shutdown wake regressions.
- Wayland tests drive layer-surface `closed`/recreate/configure transitions and repeated `wl_output` add/remove churn without a live compositor, proving that recoverable resource loss cannot terminate the application or exhaust stale output slots.
- Linux and Windows native hosts remain thin adapters around the tested behavioral seams; platform-specific lifecycle rules remain local rather than forced into one generalized abstraction.
- Existing platform runtime checks remain supplementary integration evidence rather than the only way to validate event-loop and lifecycle behavior.
