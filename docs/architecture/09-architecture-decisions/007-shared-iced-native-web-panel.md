---
summary: "Records the decision to reuse one Iced panel implementation across native desktop and browser/WASM delivery while keeping HTTP as an external transport boundary."
viewpoint: decision
concerns:
  - architecture-coherence
  - maintainability
  - portability
  - security
activities:
  - orient
  - change
  - assess
facets:
  area:
    - whole-system
---

# ADR 007: Shared Iced Native and Web Panel

## Status

Superseded by ADR 008. This document remains as historical rationale for the HTTP/SSE transport and the former shared-Iced rendering decision. Where rendering ownership conflicts, ADR 008 governs.

## Decision

Use one Iced panel implementation for both native desktop rendering and the browser monitor page.

The native process remains the single sampling authority. When HTTP monitoring is enabled, each completed `MonitorState` is projected into an explicit Web transport schema containing only state required by the remotely rendered panel, then published to a bounded latest-state hub and serialized at the external HTTP boundary. `GET /api/v1/state` returns the current complete projection as JSON. Domain endpoints expose slices of that same projection for CPU, memory, disks, networks, temperatures, and processes, while `/api/v1/history/*` exposes the corresponding bounded histories. Browser clients receive the same current projection and subsequent complete-state updates over Server-Sent Events (SSE) at `/events`. Browser clients deserialize the Web projection in a `wasm32-unknown-unknown` Iced runtime, convert it back to panel-consumable state, and render through the same `ui::view` and `PanelLayout` used by the native desktop runtime.

Do not create an HTML/CSS reimplementation of the dashboard. Native and Web runtimes may differ in lifecycle, transport, window/canvas hosting, and target-specific dependencies, but panel structure, Iced widgets, colors, graph drawing, spacing, and layout authority stay shared.

HTTP monitoring is opt-in. `--http` enables it; `--http-bind <ip:port>` selects the listener and defaults to `127.0.0.1:9847`. Binding to a non-loopback or unspecified address is an explicit user action. The initial Web interface is read-only and assumes any deliberately exposed LAN is trusted; it provides no authentication or remote-control API.

## Rationale

A second browser-specific renderer would make every visual change a two-implementation maintenance task and would allow desktop/Web behavior to drift. Iced 0.14 can run on native targets and in the browser through WebAssembly, so the UI itself can remain one source of truth.

SSE matches the current one-way, one-Hertz state delivery requirement and is simpler than a bidirectional WebSocket protocol. Sending complete bounded Web-state projections keeps reconnect semantics trivial and avoids introducing a second incremental domain model.

Serialization belongs only at the process/network boundary. Native desktop rendering continues to consume typed state directly in-process and does not route through JSON or HTTP. The transport schema is an explicit exposure allowlist owned by the Web boundary; adding a core-only field does not alter the wire payload unless the projection is deliberately updated.

The complete snapshot API, domain/history API slices, and SSE stream share this one Web transport schema and latest-state authority. Domain endpoints are views over the Web projection, not independently owned models. Do not create a separate sampling path or duplicate schema authority for polling clients.

## Consequences

- `ui` means shared Iced panel code, not desktop-only code.
- Native desktop and Web/WASM targets compile the same `presentation` and `ui` modules.
- Platform collectors, eBPF, tray integration, layer-shell, and native CLI code are excluded from the WASM target.
- The native executable embeds the generated browser JavaScript/WASM assets and serves them itself; no Node.js, nginx, helper daemon, or second deployed executable is required.
- HTTP clients never start collectors and never influence sampling cadence.
- Per-client state buffering is bounded; slow clients may skip intermediate complete states instead of accumulating unbounded queues.
- The shared Iced frontend may pin a narrowly patched dependency revision when an upstream renderer defect breaks native/Web parity. Such a patch stays below the application UI boundary; do not duplicate or specialize panel widgets to work around renderer behavior. The current `iced_widget` pin isolates each `Canvas` in its own renderer layer and establishes that layer in Canvas-local coordinates before clipping, avoiding the Iced 0.14 WebGL multi-Canvas geometry/clipping defect tracked upstream as `iced-rs/iced#2400`.
- A future interactive Web surface requires a separate security and command-channel decision instead of silently extending the read-only SSE endpoint.
- Exact font parity cannot be guaranteed until fonts required by the shared UI are deliberately embedded with suitable redistribution terms. The WASM build includes an embedded fallback font so text remains renderable when the requested host font is unavailable.
