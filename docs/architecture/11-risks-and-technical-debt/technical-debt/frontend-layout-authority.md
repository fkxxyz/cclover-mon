---
summary: "Tracks duplicated widget geometry between rendered UI structure and manual panel-height calculation."
viewpoint: assurance
concerns:
  - maintainability
activities:
  - orient
  - change
  - assess
facets:
  area:
    - whole-system
---

# Frontend Layout Authority

## Status

Active — P2.

## Problem

`PanelLayout` is the authority for block presence/order and requested window height, but internal block geometry is still represented twice: once by the actual Iced widget tree and again by manual height formulas/constants.

## Evidence

Functions such as `metric_card_height`, `small_graph_card_height`, and `network_card_height` reproduce padding, row height, graph height, and spacing assumptions also encoded by the corresponding render helpers.

## Maintenance impact

Adding or changing a row, spacing value, or internal widget can make rendered height diverge from the requested desktop-surface height. The defect appears as clipping, extra space, or unstable resizing and may only be caught visually.

## Governing constraint

Renderer-specific geometry should have one effective authority. If exact precomputed sizing is required by the hosting protocol, geometry data used by rendering and size calculation must derive from the same structure/constants rather than parallel formulas.

## Resolution direction

Unify card geometry specifications or make height derivation consume the same structural descriptions used to render. Avoid pushing pixel geometry into renderer-neutral presentation.

## Exit criteria

Changing a card's structural geometry cannot require an independent manual update to a separate height model to keep the desktop surface correct.
