---
summary: "Tracks remaining cross-sample metric state that still keys dynamic entities by mutable device or interface names."
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

Stable core identities now cover ordinary network and disk series, but process I/O attribution still uses device and interface names as cross-sample identity. The same domain entities therefore have different identity contracts depending on which metric path observes them.

## Evidence

`NetworkCounter` and `DiskCounter` carry `NetworkId` and `DiskId`, and their delta/history paths key by those types. `ProcessDiskIoCounter` and `ProcessNetworkIoCounter` instead carry `device: String` and `interface: String`; process I/O rate derivation keys previous samples by `(ProcessInstanceId, &str)`. Linux eBPF attribution resolves native device numbers and ifindexes directly to names before constructing those core counters instead of translating them to the same stable core identity authority used by ordinary disk and network collection.

## Maintenance impact

Rename, name reuse, hotplug, namespace differences, or future platform backends can reset or misassociate process I/O deltas. Joining process attribution with ordinary device/interface metrics also requires matching display-oriented strings instead of shared semantic identity.

## Governing constraint

Any state that survives one observation must key dynamic entities by an explicit core-owned stable semantic identity. Display labels and native locators are not persistent identity. When multiple collectors represent the same domain entity, platform translation must canonicalize them to the same core identity.

## Scope discovery

Inspect every delta, history, cross-sample join, cache, merge, deduplication, and attribution path. Search for names, labels, native IDs, or generic strings used as keys across observations, then trace native-to-core identity translation at the platform boundary.

## Resolution direction

Carry stable disk/network identity through process attribution counters and derived snapshots, keeping display names separate. Reuse one Linux native-to-core identity authority for ordinary collection and eBPF attribution rather than maintaining parallel name-based resolution rules.

## Exit criteria

No discovered cross-sample disk or network state keys by mutable display names. Process I/O deltas use stable core identity, ordinary and attribution paths canonicalize the same represented entity consistently, and changing a display label does not change series identity or require string-based joins.
