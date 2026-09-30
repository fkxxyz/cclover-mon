---
summary: "Chooses one renderer-neutral dashboard tree as UI authority, with separate native desktop and Web renderers."
viewpoint: decision
concerns:
  - architecture-coherence
  - maintainability
  - portability
  - performance
activities:
  - orient
  - change
  - assess
facets:
  area:
    - whole-system
---

# ADR 008: Shared UI Tree with Platform Renderers

## Decision

Keep one shared renderer-neutral dashboard definition and stop making a cross-platform GUI framework the owner of desktop rendering.

The shared `cclover-ui` layer owns dashboard structure, element ordering, visibility, visual tokens, graph semantics, and geometry that must stay identical across renderers. It consumes `cclover-presentation` and emits a deliberately small dashboard tree composed of rows, stacks, text cells, progress bars, graphs, sections, and cards. Renderer-specific code consumes that tree; it does not independently reconstruct CPU, memory, GPU, disk, network, process, or availability presentation.

```text
MonitorState
    ↓
cclover-presentation
    ├── terminal frontend
    └── cclover-ui: shared dashboard tree + visual/layout authority
          ├── native desktop renderer
          │     ├── Windows native host/drawing
          │     └── Linux native host/drawing
          └── Web renderer
```

Native desktop rendering is platform-owned. Windows uses the Win32 window/message/tray stack and GDI drawing appropriate to the supported Windows range. Linux owns Wayland/X11 hosting and may share one software/native drawing implementation across those hosts. Native renderers execute shared UI elements and drawing primitives; they do not own dashboard semantics.

For native desktop rendering, `cclover-ui::NativeScene` is the final shared lowering step. It contains renderer-ready primitives and absolute geometry derived from the dashboard tree. Platform-native hosts execute that scene; they do not repeat dashboard layout calculations. Web is not required to consume `NativeScene` and may lower the higher-level dashboard tree through browser-native layout/drawing instead. `NativeScene` is therefore a native rendering contract, not a generic cross-platform GUI toolkit.

Web remains a separate renderer, but rendering stays in native Rust rather than a browser WASM runtime. `cclover-web-ui` lowers the shared dashboard structure and visual tokens to HTML/SVG/CSS in the native process. SSE carries rendered dashboard markup; a small static JavaScript adapter only installs updates into the DOM. The browser therefore does not reconstruct dashboard semantics.

Do not rebuild a general-purpose widget framework. The shared tree exists only to describe this fixed monitor dashboard. Add abstractions only when multiple renderers need the same dashboard rule.

## Rationale

The monitor UI is small and read-only: text, cards, separators, progress bars, and bounded history graphs. The prior Iced/winit/wgpu stack made this small surface depend on graphics-adapter selection, surface presentation, redraw behavior, and compatibility layers far below application semantics.

Windows XP + One-Core-API experiments demonstrated the mismatch. Monitoring state, runtime, subscriptions, and application updates could remain functional while wgpu adapter creation or later surface presentation failed. GL and software paths moved the failure into different framework layers without making application behavior easier to control. Continuing to patch those layers would spend complexity on framework compatibility rather than monitoring functionality.

Maintaining fully independent platform UIs would create a different problem: every dashboard change would require synchronized edits in several renderers. The shared dashboard tree solves that at the governing boundary. A renderer receives an already-decided UI and only realizes it using its platform's mechanisms.

## Ownership Rules

`cclover-presentation` owns metric meaning and formatted display semantics.

`cclover-ui` owns cross-renderer dashboard decisions:

- card/section structure and ordering;
- which semantic values appear in each card;
- text roles and visual tokens;
- shared spacing and geometry where parity requires exact dimensions;
- graph ranges, auto-scaling policy, series assignment, and visual role;
- renderer-neutral element composition.

Renderer adapters own only realization details:

- font API and text measurement;
- native drawing or Web HTML/SVG generation;
- surface/buffer or DOM lifecycle;
- clipping implementation;
- DPI/device scaling;
- window/event-loop or browser update integration.

Platform desktop hosts additionally own tray integration, placement, pointer passthrough, taskbar/Alt+Tab behavior, and native lifecycle policy.

No renderer may infer metric availability, ranking, formatting, graph range, or card visibility from platform identity or raw monitor state when the shared layers already own that decision.

## Web Boundary

Web is not required to consume a desktop `Text(x, y)`/`Rect(x, y)` scene. Browser layout mechanisms differ materially from native desktop surfaces. Shared UI authority therefore stops at the renderer-neutral dashboard tree and style/layout tokens. A native renderer lowers that tree into absolute drawing primitives; `cclover-web-ui` lowers it server-side into HTML/SVG/CSS. Browser JavaScript remains generic transport/DOM glue.

Changing a dashboard card should change the shared tree once. Renderer changes should be required only when introducing a genuinely new renderer-neutral element kind.

## Migration Outcome

The migration is complete. Windows consumes `NativeScene` through a narrow C ABI and renders with Win32/GDI; Linux consumes the same scene through native Wayland/X11 hosting with Cairo; Web lowers the higher-level dashboard tree to HTML/SVG/CSS in native Rust and streams rendered updates to a thin browser client. Iced, winit, wgpu, `wasm-bindgen`, and `web-sys` are not renderer dependencies.

## Consequences

- UI consistency is enforced above renderer implementations instead of by sharing one graphics framework.
- Native desktop compatibility failures are isolated to small platform renderers/hosts.
- Web uses native Rust HTML/SVG generation plus browser DOM installation without duplicating dashboard semantics or requiring WebAssembly.
- New platform renderers implement a small stable element vocabulary instead of every monitor card independently.
- Exact glyph metrics may differ by native font stack; structure, values, geometry policy, colors, graph semantics, and ordering remain shared.
- The shared UI tree must stay application-specific and small; turning it into a generic toolkit would recreate the abstraction cost this decision removes.

## Supersedes

This decision supersedes ADR 007's requirement that native desktop and Web share one Iced panel implementation.
