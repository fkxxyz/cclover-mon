---
summary: "Tracks remaining dynamic histories that use mutable device names as persistent identity."
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

# Dynamic-Series Identity

## Status

Active — P2.

## Problem

Network and disk dynamic history still use device/interface names as cross-sample identity. Names are useful locators and labels but are not a general stable identity contract.

## Evidence

`MonitorHistory.networks` and `MonitorHistory.disks` are keyed by `String`, and sampler/history matching uses `NetworkCounter.name` and `DiskCounter.name`. Temperature history has already been migrated to stable sensor identity, and process sampling now uses `ProcessInstanceId`.

## Maintenance impact

Rename, reuse, hotplug, namespace, or future platform differences can reset or misassociate history and deltas. Additional features may accidentally treat presentation-oriented names as durable entity identity.

## Governing constraint

Any state surviving beyond one observation must key by a core-owned stable semantic identity. Display names and native locators may change without changing entity identity.

## Resolution direction

Introduce stable core identities for network and disk series and translate platform-native identity into those types before crossing the platform boundary. Keep display labels separate.

## Exit criteria

Network and disk cross-sample deltas/history no longer key by mutable names, and presentation labels can change without altering history identity.
