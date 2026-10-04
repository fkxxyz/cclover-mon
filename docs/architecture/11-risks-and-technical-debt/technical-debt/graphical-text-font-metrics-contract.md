---
summary: "Records the missing renderer-level guarantee that actual graphical font metrics satisfy shared text-slot width budgets."
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
    - ui
---

# Graphical Text Font Metrics Contract

**Priority:** Medium

## Root cause

Shared graphical geometry now gives every bounded metric value an explicit maximum column count and derives each fixed text-slot width from a conservative monospace advance budget. The final renderers, however, do not mechanically guarantee that the font actually selected at runtime satisfies that same advance bound. Font selection and fallback therefore remain an implicit assumption below otherwise-shared geometry authority.

## Optimization dimension

Primary: user experience efficiency. Secondary: architecture and change efficiency, portability.

## Current cost

A machine whose selected or fallback monospace font has a wider advance than the shared budget can still render a structurally valid `Scene` with text extending beyond its intended slot. Because bounded metric values intentionally no longer use silent clipping, such a mismatch can produce overlap rather than hidden truncation. Future font-family, platform, DPI, or fallback changes also require manual reasoning about whether all graphical renderers still satisfy the geometry assumption.

## Evidence

The graphical UI owns renderer-neutral fixed text-slot geometry and currently derives slot width from a shared conservative monospace advance ratio. Web rendering requests a CSS monospace font stack, while Linux Cairo and Windows GDI request Inconsolata and may receive platform font fallback. The shared UI can prove formatter bounds and slot capacity, but it cannot prove which concrete font face each renderer ultimately uses or its actual glyph advance.

## Reachable better state

Define the smallest renderer typography conformance contract needed to guarantee that fonts used for bounded metric values fit the shared advance budget. Preserve shared `Scene` geometry: renderers should validate or satisfy the typography contract rather than independently re-layout dashboard content. The solution may use constrained font selection, renderer-level conformance checks, or another small mechanism that establishes the same invariant without bundling fonts into the product.

## Governing constraint

For every graphical renderer, the concrete font used for bounded metric text must satisfy the shared text-slot advance budget at the requested logical size. A renderer must not silently substitute a font whose metrics invalidate shared geometry.

## Scope discovery

Review every graphical text backend that consumes `Scene` text primitives: Web SVG/CSS, Linux Cairo, and Windows GDI. Include font selection, fallback behavior, DPI/logical-size conversion, bold variants used by bounded values, and any future graphical renderer. Re-run scope discovery against all fixed bounded text slots when closing the debt.

## Repair direction

Add one explicit renderer typography contract and the cheapest sufficient conformance proof for each backend. Keep formatter bounds in `cclover-presentation`, slot geometry in `cclover-ui`, and renderer-specific font realization in renderer adapters. Do not introduce renderer-specific dashboard geometry, runtime layout negotiation, or bundled fonts solely to close this debt unless evidence shows a smaller conformance mechanism is insufficient.

## Exit criteria

- Web, Linux Cairo, and Windows GDI each have a mechanical or deterministic validation path proving their bounded-value font realization satisfies the shared advance budget.
- Font fallback cannot silently select a face that violates the bounded-value width contract.
- Bold and regular bounded-value variants are covered at every logical size used by fixed slots.
- Shared `Scene` geometry remains the only dashboard layout authority; no renderer-specific slot widths or text re-layout are introduced.
- A regression in renderer font selection or metrics fails deterministic validation or a required platform acceptance check before release.
