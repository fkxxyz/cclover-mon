---
summary: "Chooses one renderer-neutral dashboard tree and one final graphical Scene as authority for native and Web rendering."
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

# ADR 008: Shared UI Tree and Graphical Scene with Platform Renderers

## Decision

Keep one shared renderer-neutral dashboard definition and one shared final graphical scene while keeping platform rendering mechanics independent of a cross-platform GUI framework.

`cclover-ui` owns dashboard structure, element ordering, visibility, visual tokens, graph semantics, layout policy, and final logical geometry. It first derives the small application-specific dashboard tree and then lowers that tree exactly once into `Scene`, whose primitives contain authoritative rectangles, points, colors, clipping, alignment, and requested surface dimensions.

```text
MonitorState
    ↓
cclover-presentation
    ├── terminal frontend
    └── cclover-ui
          ↓ DashboardUi
          ↓ shared layout
          ↓ Scene
          ├── Windows GDI renderer
          ├── Linux Cairo renderer
          └── Web SVG serializer
```

Every graphical renderer consumes the same `Scene`. Renderers may realize fonts differently and own device scaling, rasterization, clipping mechanics, native surface lifecycle, or SVG serialization, but they must not recalculate dashboard layout. Dynamic text receives a shared layout slot before rendering; glyph measurement must not move sibling elements or change card, row, graph, or panel geometry.

Web rendering stays in native Rust and serializes `Scene` to SVG. SSE carries rendered SVG markup; browser JavaScript installs updates into the DOM and may measure marked must-fit SVG text only to assert renderer conformance. Web CSS may establish page-level presentation such as margins, overflow, or a system-font fallback, but neither CSS nor JavaScript may use Flexbox, Grid, intrinsic text measurement, or other browser layout mechanisms to reconstruct dashboard geometry.

Do not rebuild a general-purpose widget framework. The dashboard tree and layout vocabulary exist only for this fixed monitor UI. Add layout capabilities only for demonstrated dashboard needs.

## Rationale

The prior Iced/winit/wgpu stack imposed graphics-adapter, surface, redraw, and compatibility complexity disproportionate to this small read-only monitor. Platform-native GDI/Cairo rendering removes that dependency while keeping the final rendering boundary small.

Sharing only the higher-level dashboard tree was insufficient for visual parity: native rendering lowered it to absolute geometry while Web independently interpreted rows and stacks with CSS layout and browser text metrics. Both paths could obey the same semantic tree yet produce materially different geometry. Moving the shared authority one level lower eliminates that second layout authority without restoring a cross-platform GUI framework.

Text remains the one intentionally renderer-dependent visual detail. Shipping a bundled font solely for pixel identity would increase distribution size. Instead, `cclover-ui` allocates deterministic `Fixed` and `Fill` text slots; renderers draw platform fonts inside those authoritative rectangles and clip where requested. Font choice can therefore affect glyph shape and small baseline details, but not surrounding geometry.

## Ownership Rules

`cclover-presentation` owns metric meaning and formatted display semantics.

`cclover-ui` owns:

- card/section structure and ordering;
- semantic values shown in each card;
- text roles and visual tokens;
- fixed and fill text-slot budgets;
- spacing, dimensions, and absolute graphical geometry;
- graph ranges, scaling policy, series assignment, and final graph points;
- progress geometry and clipping rectangles;
- the final `Scene` primitive stream.

Graphical renderer adapters own only realization details:

- system-font selection and glyph rasterization inside the supplied text rectangle;
- native drawing APIs or SVG serialization;
- clipping implementation;
- DPI/device scaling of the complete logical scene;
- surface/buffer, window/event-loop, or browser DOM lifecycle.

A renderer must not:

- reallocate row or text widths from intrinsic content size;
- add renderer-specific card padding, gaps, or placement;
- independently calculate graph or progress geometry;
- infer metric availability, formatting, ranking, or visibility.

Platform desktop hosts additionally own tray integration, placement, pointer passthrough, taskbar/Alt+Tab behavior, and native lifecycle policy.

## Text Geometry Contract

Dynamic text never controls sibling geometry. Dashboard rows allocate text through the deliberately small `CellWidth` vocabulary:

- `Fixed(width)` reserves an explicit logical-pixel slot;
- `Fill` receives remaining row width after fixed slots and gaps.

Text alignment and clipping are part of the shared tree and become final `Scene::Text` rectangles. Formatting and slot budgets must be designed together. Bounded metric values must already satisfy their presentation-declared column budget before entering graphical layout and lower with an explicit must-fit contract. Clipping is reserved for explicitly unbounded text such as identity labels.

The concrete renderer may measure the actual realized must-fit string after font selection, fallback, weight, logical size, and device scaling are known, but that measurement is only a conformance assertion against the authoritative rectangle. It may not become a layout input. Cairo and GDI compare in final device coordinates and Web compares the browser's realized SVG text width, allowing only one physical pixel for coordinate-rounding differences. If the realized string exceeds its slot, the renderer rejects the new frame and preserves the previous valid frame rather than truncating, clipping, shrinking, horizontally scaling, or re-laying out text. Repeated violations are reported without per-frame log spam.

## Web Boundary

`cclover-web-ui` accepts `Scene`, not `DashboardUi`, `Card`, `Row`, `GraphSpec`, or raw `MonitorState`. It mechanically maps scene primitives to SVG elements and marks must-fit text with its authoritative width. After installing a candidate SVG, the browser adapter may read the realized SVG text length solely to validate that contract; a failed candidate is rolled back to the previous valid markup. The adapter never changes font size, slot width, or element placement from that measurement, so the browser remains a drawing target rather than a second layout engine.

Changing an existing dashboard layout changes `cclover-ui` once. Renderer changes are required only when the shared scene primitive vocabulary itself changes or a renderer must improve realization of an existing primitive.

## Migration Outcome

Windows and Linux consume the shared `Scene` through the existing narrow native ABI and render with GDI and Cairo respectively. Web consumes the same `Scene`, serializes it to SVG in native Rust, and streams the SVG through the existing EventSource/DOM path. Iced, winit, wgpu, `wasm-bindgen`, and `web-sys` remain outside the renderer dependency graph.

The native C ABI carries only the finalized Scene command stream and state lifecycle callbacks, including the must-fit text flag. Renderer font measurements and measured widths remain private to the renderer and never cross back into shared layout.

## Consequences

- Structure and geometry are identical by construction across graphical renderers.
- Platform fonts may differ without moving sibling elements or changing dashboard geometry.
- Web no longer maintains a parallel card/row/graph layout implementation.
- Native compatibility remains isolated to small platform renderers/hosts.
- Geometry can be tested once at the Scene boundary; renderer tests focus on faithful primitive realization.
- The shared tree/layout must remain application-specific and small; turning it into a generic widget toolkit would recreate the abstraction cost this decision avoids.

## Supersedes

This decision continues to supersede ADR 007's requirement that native desktop and Web share one Iced panel implementation. It also strengthens ADR 008's earlier form, which allowed Web to independently lower the dashboard tree through browser-native layout.
