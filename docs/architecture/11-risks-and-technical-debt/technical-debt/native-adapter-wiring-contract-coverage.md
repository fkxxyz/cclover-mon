---
summary: "Records native adapter wiring facts that still lack stable deterministic contract coverage below full OS integration."
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
    - native-bridge
    - platform
---

# Native Adapter Wiring Contract Coverage

**Priority:** Low-medium

## Root cause

Some native desktop adapter correctness depends on glue that maps an external OS or protocol event into an already-tested production policy or action, but that mapping has no stable deterministic contract seam of its own. The underlying policy can be correct while the adapter calls the wrong action, omits the call, or stops converging related protocol paths.

## Optimization dimension

Primary: test and verification efficiency. Secondary: architecture and change efficiency, portability.

## Current cost

Semantics-preserving native-host refactors can break selected adapter wiring without failing the lower-level policy tests. Maintainers then need a real compositor, D-Bus session, Win32 runtime, or manual source reasoning to establish whether the production callback/dispatch path still reaches the intended policy or cleanup action. This makes narrow native-host changes more expensive to verify than the governed behavior itself.

## Evidence

Wayland buffer policy deterministically proves that the absence of a previous frame baseline forces a full redraw, and Wayland lifecycle policy deterministically proves surface close/recreate behavior. The production surface-destroy path is still the layer that must connect those facts by invalidating the buffer baseline before a replacement surface draws; there is no deterministic adapter contract test for that wiring.

The Linux tray declares D-BusMenu grouped methods such as `EventGroup` and `AboutToShowGroup`, while the production method dispatcher must implement the declared methods and keep grouped activation semantically aligned with the single-event path. Stable protocol tokens can be checked statically, but dispatcher convergence is behavioral glue and is not currently proven by a low-cost compiled contract seam.

## Reachable better state

Add the smallest deterministic seam only where repeated maintenance pressure justifies it. Prefer extracting tiny production-used adapter decisions or dispatch tables that can be compiled and tested without a compositor, D-Bus daemon, X server, or Win32 session. Where the integration fact cannot be separated cheaply from the OS runtime, cover it with a targeted native acceptance check instead of reconstructing the runtime in mocks.

## Governing constraint

For every material native adapter mapping between an external event/protocol operation and an internal tested policy or action, correctness must be established at the cheapest stable layer that proves the actual production wiring. Private helper-name or expression matching must not be used as a behavioral proxy.

## Scope discovery

Review native desktop adapters at the boundary where OS callbacks, protocol methods, messages, or surface lifecycle events invoke production policy/actions. Include Wayland surface destruction/recreation and buffer-baseline invalidation, Linux StatusNotifierItem/D-BusMenu method dispatch, X11 event-to-lifecycle mapping, and Win32 message-to-policy mapping. Exclude mappings already covered by deterministic compiled seams or required real-host acceptance. Re-run this review when a native adapter is materially refactored.

## Repair direction

Migrate one uncovered wiring fact at a time when its adapter next changes. Reuse existing native-policy test infrastructure. Do not introduce a general native mock framework, fake compositor, fake Win32 subsystem, or D-Bus simulator solely to close this debt. Prefer a small shared dispatch/decision seam when the mapping is naturally pure; otherwise add a focused runtime acceptance check.

## Exit criteria

- Wayland surface replacement has a stable proof that the old incremental-frame baseline cannot survive into drawing on the replacement surface.
- D-BusMenu grouped method declarations and production dispatch remain mechanically aligned, including convergence of grouped and single activation semantics.
- Scope discovery confirms that other material native adapter mappings are either deterministically covered at a production-used seam or intentionally covered by targeted real-runtime acceptance.
- No behavioral adapter contract depends on private helper names, field names, local expression spelling, or function-body ordering.
- The added proof layers remain materially cheaper and more stable than the integration paths they replace; no broad native-runtime simulation framework is introduced.
