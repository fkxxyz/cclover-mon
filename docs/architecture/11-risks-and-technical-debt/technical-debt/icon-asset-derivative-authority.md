---
summary: "Records the manual synchronization cost between canonical application artwork and platform icon derivatives."
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
    - whole-system
---

# Icon Asset Derivative Authority

**Priority:** Low

## Root cause

The application has one canonical PNG artwork, but the Windows multi-size ICO and Linux hicolor PNGs are maintained as separate checked-in outputs without a repository-owned generation or consistency-check command.

## Evidence

Introducing and correcting the icon required repeating the PNG-to-ICO and PNG-resize conversions manually. The release plan includes each Linux derivative as an independent package input, while Windows embeds the separate ICO resource. A future canonical-image change can therefore leave one or more platform outputs stale without making the mismatch obvious during ordinary compilation.

The observed edge cleanup was corrected in the canonical PNG and all derivatives were regenerated. This record covers only the remaining synchronization mechanism, not that resolved visual defect.

## Governing constraint

Every shipped platform icon must derive from the canonical artwork through one reproducible, repository-owned conversion contract. Generated variants must not become independent visual authorities.

## Scope discovery

Review `assets/icons/cclover-mon.png`, `assets/windows/cclover-mon.ico`, the 128px and 256px Linux hicolor PNGs, Windows resource embedding, Linux tray inclusion, and the release-plan package inputs. Include every new platform size or format added later.

## Maintenance consequence

An artwork correction requires manual regeneration and synchronization across formats. A missed derivative can make the tray, executable icon, launcher icon, and packaged Linux icon disagree; reconstructing conversion details from prior command history adds avoidable maintenance work.

The current product has one icon and no demonstrated frequent artwork churn, so the expected cost is limited and this debt remains low priority.

## Repair direction

Add one small repository-owned command that generates the ICO and hicolor PNGs from the canonical PNG using declared sizes and conversion settings. Provide a check mode for release inputs. Keep generation out of runtime and ordinary application builds; choose validation placement only after accounting for the image-tool dependency and its version-dependent output behavior.

## Exit criteria

- One maintained command defines all supported icon sizes and output paths.
- Regeneration consumes only the canonical artwork and produces every checked-in derivative.
- Check mode detects stale or missing variants without rewriting the worktree.
- Validation runs the check with an explicitly provisioned or otherwise guaranteed image tool, and confirms generated outputs match the declared conversion contract.
- Release-plan icon inputs remain limited to the interactive product.
