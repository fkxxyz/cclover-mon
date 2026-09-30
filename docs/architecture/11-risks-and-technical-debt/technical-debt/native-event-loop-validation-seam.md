---
summary: "Records the missing deterministic behavioral test seam for native desktop event-loop wakeup and redraw scheduling."
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

# Native Event-Loop Validation Seam

**Priority:** Medium

## Root cause

Native desktop event-loop scheduling is implemented directly against real X11, Wayland, and Win32 host loops without a deterministic behavioral seam that can exercise the shared lifecycle independently of those operating-system environments.

## Evidence

Current validation can prove structural properties such as the absence of periodic 100 ms polling and can compile both Linux and Windows host implementations. Full behavioral validation of the sequence from state publication through native wakeup, state consumption, and redraw still depends on a usable real desktop environment. Linux runtime validation can additionally be blocked before the event loop by compositor or layer-shell capability, while Windows cross-compilation cannot execute the Win32 message loop.

## Governing constraint

Native desktop scheduling changes should be verifiable deterministically at the behavioral boundary: a published state must produce a wake, the latest state must be consumed exactly through the native bridge semantics, and the host must request a frame without relying on periodic polling. Platform adapters should remain responsible only for translating that wake into the corresponding OS event primitive.

## Scope discovery

Review the shared desktop state bridge, Linux X11 and Wayland wait/dispatch paths, Windows message-loop wake routing, native host shutdown behavior, and tests that currently assert only source structure or require a live desktop. Include any future native desktop backend that consumes the same state-to-frame scheduling contract.

## Maintenance consequence

Changes to wake primitives, shutdown ordering, display dispatch, or message routing can compile successfully while still introducing lost wakes, duplicate frame requests, or shutdown races. Without a deterministic behavioral seam, maintainers must rely on platform-specific runtime checks for confidence, increasing verification cost and making regressions easier to miss when the required environment is unavailable.

## Repair direction

Extract only the smallest renderer-neutral scheduling contract needed to drive deterministic tests of state-ready, wake, consume-latest, and frame-request behavior. Keep X11, Wayland, and Win32 primitives in their platform adapters; do not simulate complete window systems or introduce a generalized event-loop framework merely for testing.

## Exit criteria

- Deterministic tests exercise state publication through wake handling to frame-request semantics without requiring X11, Wayland, or Win32 runtime availability.
- Tests cover coalescing to the latest pending state and shutdown ordering sufficiently to detect lost-wake and post-shutdown wake regressions.
- Linux and Windows native hosts remain thin adapters from OS events into the same tested scheduling contract.
- Existing platform runtime checks remain supplementary evidence rather than the only way to validate event-loop behavior.
