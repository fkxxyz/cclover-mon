---
summary: "Chooses a layered per-pixel-alpha Windows panel realized by GDI two-render alpha recovery so translucent Scene fills match Linux."
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
    - platform
---

# ADR 015: Windows Per-Pixel-Alpha Panel

## Decision

The Windows panel is a layered window published with `UpdateLayeredWindow` (`ULW_ALPHA`), so translucent `Scene` fills show the desktop behind the panel as they do on Linux. The shared `Scene` remains the only color authority; the Windows host does not blend against a private background.

GDI keeps no destination alpha channel, so each frame is realized as follows:

1. Translucent fills (`FillRect`, `Polygon`) are blended with `AlphaBlend` using the `Scene` alpha.
2. The same `Scene` is rendered into two 32-bit DIB sections, one pre-cleared to opaque black and one to opaque white.
3. A pure compiled policy (`alpha_policy`) recovers premultiplied `0xAARRGGBB` per pixel. Over black a pixel equals its premultiplied color; over white it equals that color plus `(1 - alpha) * 255`, so the channel difference yields alpha.
4. The recovered surface is published. Window shape comes from that alpha, so rounded corners are transparent pixels rather than a window region.

Translucency is limited to fills. `Scene` strokes, polylines, and text stay opaque, and a `cclover-ui` test enforces that boundary. Windows text uses grayscale antialiasing because ClearType sub-pixel fringes are invalid on a per-pixel-alpha surface.

A frame rejected by typography conformance leaves the previously published frame on screen. A publication failure is reported once until the next success, and a failure to publish the first frame fails startup with the Win32 error.

## Rationale

The panel was opaque on Windows because the GDI renderer pre-blended every translucent color against a hard-coded `#0b1020` and the window used uniform opacity 255. That duplicated the background authority in `cclover-ui` and silently diverged from Linux and Web, which both honor `Scene` alpha. Two-render recovery keeps the GDI renderer, the single executable, and the shared `Scene` contract, and needs only `msimg32`, which ships with Windows.

## Consequences

- Each frame is rendered twice and every pixel is recovered, and the host repaints the full surface without using `Scene` damage metadata. Two full-panel DIBs stay allocated. The per-frame cost is unmeasured.
- Alpha is a fill-only capability. Adding alpha to another primitive requires a different Windows realization first.
- GDI's gamma-adjusted text blending is not linear between black and white, so glyph-edge alpha may carry a small error. This is inferred, not measured.
- Alpha recovery, rounded-corner transparency, and overlapping-fill composition are asserted by a Windows-host pixel test through the real GDI path. Visual acceptance on real Windows was reported by the product owner.

## Rejected Alternatives

Only pre-blending and window-level alpha were rejected on observed behavior. The others were judged by reasoning and not prototyped.

### Pre-blend against a fixed background

This was the previous behavior. It produces an opaque panel and duplicates the background authority.

### Whole-window `SetLayeredWindowAttributes` opacity

Uniform opacity makes text and graphs translucent too and cannot express per-primitive alpha.

### Color-key transparency

It is binary, has no partial alpha, and leaves edge fringes.

### Single render with alpha fix-up

GDI leaves the alpha byte undefined for most operations, so coverage cannot be recovered reliably from one render.

### Cairo, GDI+, or Direct2D on Windows

Each would add a graphics stack or a second Windows text realization and reopen [ADR 008](008-shared-ui-tree-platform-renderers.md)'s platform-native choice. That is larger than the defect warranted.

## Reassessment

Reassess when measured per-frame Windows CPU is material to the low-overhead goal, when glyph-edge error is visible on real desktops, or when `Scene` needs alpha on a non-fill primitive.
