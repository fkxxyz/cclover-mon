---
summary: "Records that the documented fixed process Top-N region exists only for disk and network cards, so the CPU and memory cards still resize with the current row count."
viewpoint: assurance
concerns:
  - maintainability
activities:
  - orient
  - change
  - assess
facets:
  area:
    - ui
---

# Process Card Row Region Sizing

**Priority:** Low-medium

## Root cause

The graphical dashboard promises one layout rule for ranked process rows and implements it twice. Disk and network cards reserve a fixed I/O row region, so their height is independent of the current row count. CPU and memory cards place their process rows in a stack whose height is the number of rows currently present, so their height follows the sample.

The rule lives in prose — [Presentation and UI](../../05-building-block-view/presentation-and-ui.md) states that a Top-N subregion reserves its full fixed region whenever the collection is observable — while the implementation has no shared slot definition that both card families use. `IO_PROCESS_GEOMETRY` carries a slot count; the metric-card geometry carries only row height and spacing.

## Evidence

- [ADR 017](../../09-architecture-decisions/017-process-identity-and-ranking-ownership.md) moved the row count to `cclover-presentation` (`PROCESS_CARD_ROWS`, `IO_CARD_ROWS`), which made the row count explicit but left card height behavior unchanged.
- The disk and network cards build a stack with an explicit height covering three I/O rows; the CPU and memory cards build a stack without an explicit height.
- Consequence: at startup, or whenever fewer ranked candidates exist than the card's row budget, the CPU and memory cards are shorter than at steady state, and the whole panel height changes between samples. The documented invariant is therefore false for two of the four ranked regions.

## Governing constraint

A ranked subregion's height must be a function of the card's declared row budget and the observation capability, never of the current number of ranked rows. Presentation owns the row budget; `cclover-ui` owns the matching slot geometry; both card families must express the region the same way.

## Scope discovery

- All four ranked subregions: CPU, memory, per-disk I/O, per-network I/O.
- Any future variable-length card region, including graph-adjacent lists that do not exist yet.
- The shared geometry authority, so that a row budget and its rendered height cannot be defined independently.
- The prose invariant in the presentation View, which must either hold or be corrected.
- Exclude panel-level column and section geometry, which is not row-count dependent.

## Maintenance consequence

Panel height changes are visible product behavior; a reader of the presentation View currently cannot tell whether the rule holds. Anyone investigating panel jitter must first discover that the documented rule is only half implemented, and any fix has to touch two card families instead of one shared slot definition.

## Repair direction

Give the metric card the same explicit row budget the I/O card has — one shared slot definition consumed by both — so the region is reserved whenever the collection is observable and omitted when it is not. Until the product decision to change panel appearance is taken, keep the deviation stated in the presentation View with this record as the re-evaluation trigger.

## Exit criteria

- CPU and memory card height is identical for 0, 1, and row-budget-many ranked rows whenever the collection is observable.
- One slot definition, not two, expresses the reserved region for all four ranked subregions.
- The presentation View states the behavior that the code actually has.
- A test asserts height equality across those row counts, so the invariant cannot regress silently.
