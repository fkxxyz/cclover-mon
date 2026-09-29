---
summary: "Records external and operational architecture risks that require evidence or runtime validation."
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

# Architecture Risks

These items are risks or intentional capability limits that can remain even when the code structure is sound. Structural maintenance debt is tracked separately in [Technical Debt](technical-debt.md).

- Linux native desktop rendering depends on system Cairo, X11/Xext, and Wayland client libraries plus checked-in generated layer-shell protocol code; packaging and protocol-library upgrades require native build and runtime validation.
- Native metric parity will vary by operating system; the shared model must preserve common semantics without flattening meaningful platform-specific data.
- Third-party C++ SDKs may impose runtime or packaging costs that must be measured before adoption.
- X11 window-manager behavior varies across EWMH implementations; top-right placement, skip-taskbar/pager, focus avoidance, transparency, and desktop-like stacking require runtime validation on representative window managers.
- Linux eBPF I/O attribution depends on kernel BTF and attach-point compatibility. CO-RE reduces struct-layout coupling but does not guarantee every supported kernel exposes equivalent hook semantics; compatibility must be demonstrated on representative kernels.
- BPF loading and attachment require elevated authority. Packaging must avoid turning the whole application into unrestricted root solely for I/O attribution; exact capability requirements must be measured against the chosen attach mechanisms and supported kernels.
- Disk attribution deliberately measures logical scalar `vfs_read` / `vfs_write` bytes rather than physical block traffic. That preserves originating TGID across buffered writeback but does not cover vector I/O, splice-like paths, mmap I/O, or other logical-I/O paths until equivalent hooks are added.
- Network attribution measures successful socket payload bytes and correlates process-context send/receive completion with event-observed interface identity. TCP and UDP loopback are runtime-validated, but bridges, tunnels, VPNs, network namespaces, less common protocols, and route changes can expose hook or correlation gaps and still require representative validation.
- BPF maps introduce bounded kernel memory and lifecycle state. Values carry process-group leader start time so PID reuse resets counters, but map eviction under high cardinality remains an intentional bounded-state tradeoff and must be included in stress validation.
- Browser rendering depends on WebAssembly plus browser DOM/CSS/SVG behavior; representative Chromium/Firefox runtime validation is required because native or WASM compilation does not prove browser layout and rendering.
- Font-role semantics are shared, but font realization is renderer-owned. Linux native rendering resolves Cairo's `monospace` family, Windows uses its native monospace fallback, and Web uses the browser font stack. Exact glyph metrics and appearance can therefore differ across renderers even though dashboard geometry and semantic roles are shared.
- Read-only HTTP monitoring has no authentication by design in v1. Non-loopback binding therefore assumes a trusted LAN and must remain an explicit user configuration rather than a default.

## Representative Desktop Runtime Smoke Matrix

Deterministic source/build checks protect contracts they can actually prove. Desktop integration still requires representative runtime evidence because compositor and shell semantics cannot be established by source inspection alone.

| Environment | Runtime evidence |
| --- | --- |
| Linux Wayland | Top-right placement, bottom layer, zero exclusive zone, pointer passthrough, output-scale changes, dynamic resize/repaint, and tray registration/re-registration when the watcher appears or restarts. |
| Linux X11/XWayland | Top-right placement, below/taskbar/pager policy, Shape input passthrough, dynamic resize/repaint, and tray registration/re-registration. |
| Windows | Top-right placement, taskbar/Alt+Tab exclusion, non-activation, pointer passthrough, bottom Z-order, repaint after occlusion/desktop refresh, and Explorer restart restoring shell ownership, surface policy, and tray icon. |
| Web | Representative Chromium and Firefox sessions render the same shared dashboard structure and graph/layout semantics without browser-specific corruption. |

These checks remain risk evidence rather than CI substitutes. A source-level contract test may assert that required native policy primitives still exist, but passing that test does not prove a real compositor or shell honors them correctly.
