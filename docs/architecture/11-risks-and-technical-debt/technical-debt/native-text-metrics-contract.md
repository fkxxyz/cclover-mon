---
summary: "Tracks native text layout geometry that still assumes preferred-font metrics before platform renderers realize or fall back the font."
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

# Native Text Metrics Contract

## Status

Active — P2.

## Root cause

`cclover-ui::NativeScene` resolves absolute text-cell geometry before the platform renderer realizes the requested font, while native renderers are allowed to fall back when the preferred face is unavailable. The shared lowering therefore needs an assumed glyph advance to place natural-width text even though the actual Cairo or GDI face and metrics are known only below that boundary.

## Evidence

The current graphical dashboard prefers Inconsolata. Native scene lowering uses Inconsolata's half-em monospace advance for natural-width cells, while Linux realizes text through Cairo/fontconfig and Windows through GDI. During the native-renderer migration, a generic `monospace` fallback and then per-glyph integer rounding caused the MEMORY header value group to move left, overlap the `MEMORY` label, and reserve visibly incorrect spacing even though the shared font sizes and row geometry were unchanged. Correcting the preferred family and retaining fractional half-em advances restores current parity, but the geometry contract still assumes the renderer successfully realizes compatible metrics.

## Governing constraint

Shared dashboard structure and layout policy remain owned by `cclover-ui`, but natural-width text geometry must not silently depend on platform font substitution producing the same metrics as the preferred face. A renderer fallback must either preserve the metric contract or participate through a narrow measurement/realization contract without becoming a second owner of dashboard layout.

## Scope discovery

Trace every `NativeScene` text row that uses natural-width cells, every shared font role and size, Linux Cairo font selection and hinting, Windows GDI font creation and fallback behavior, DPI/output scaling, and Web rendering where visual parity is expected. Include missing Inconsolata, different fontconfig/GDI substitutions, non-ASCII/localized labels, future proportional fonts, and font-role changes as expected evolution cases.

## Maintenance consequence

A font, localization, DPI, or fallback change can produce overlap or spacing drift while all shared layout tests still pass. The failure is renderer- and machine-dependent, so future UI changes can require manual visual debugging on multiple platforms. Adding ad-hoc width constants would create multiple layout authorities and make the problem grow with each renderer.

## Repair direction

Keep the solution narrow. Either make the native font realization deterministic enough that the shared metric contract is guaranteed, or add a renderer-neutral measurement/realization step that supplies actual text extents before final native geometry is emitted. Do not introduce a general widget toolkit or move card/layout policy into platform renderers merely to solve text measurement.

## Exit criteria

The debt closes when native natural-width text placement is derived from a metric contract that is guaranteed by font realization or measured explicitly, representative preferred-font and fallback cases are deterministic, and Linux/Windows can change font realization without hand-tuning shared width constants or duplicating dashboard layout policy.
