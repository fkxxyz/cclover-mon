---
summary: "Records the lack of compiler-enforced boundaries between native C host responsibility fragments."
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
    - native-bridge
    - platform
---

# Native C Host Textual Module Boundaries

**Priority:** Low-medium

## Root cause

Linux and Windows native desktop hosts are split into responsibility-focused source fragments, but each platform still compiles those fragments by textual inclusion into one C translation unit. Physical ownership is clearer, while symbol visibility and dependency direction between responsibilities remain unenforced by the C compiler.

## Evidence

`linux_host.c` includes private Linux host fragments for rendering, X11, Wayland protocol handling, Wayland buffers, and the Wayland run loop. `windows_host.c` similarly includes private fragments for display policy, drawing, and shell/message-loop behavior. Static functions and state therefore remain directly reachable across fragments, and include order can become an implicit dependency even when directory layout suggests stronger separation.

## Governing constraint

Native host responsibility boundaries must remain local under expected changes: rendering, protocol/display integration, buffer management, and shell/event-loop policy should not acquire arbitrary cross-fragment dependencies or require include-order knowledge. When textual inclusion no longer preserves that locality, the boundary must be expressed with private C headers and translation units rather than convention alone.

## Scope discovery

Review `crates/cclover-desktop/native/linux_host.c`, `crates/cclover-desktop/native/linux/`, `crates/cclover-desktop/native/windows_host.c`, `crates/cclover-desktop/native/windows/`, and the desktop build script that selects native compilation inputs. Trace shared static functions, shared host state, declarations whose correctness depends on include order, and calls crossing responsibility fragments.

## Maintenance consequence

Future native-host changes can gradually couple otherwise separate responsibilities without any compiler-visible boundary violation. A fragment may appear locally understandable while silently depending on declarations or implementation details from another fragment, increasing context load and making later extraction into independently compiled modules more expensive.

## Repair direction

Do not split translation units only for structural symmetry. When cross-fragment coupling or include-order dependence becomes material, introduce the smallest private headers that define intentional interfaces and compile cohesive responsibilities as separate `.c` translation units. Keep the public Rust/C ABI unchanged unless a separate architectural need requires changing it.

## Exit criteria

- Scope discovery finds no responsibility boundary that relies on textual access to another fragment's private implementation.
- Cohesive native-host responsibilities that need independent boundaries compile as separate translation units behind private headers.
- Shared declarations have one private authority rather than duplicated prototypes or include-order assumptions.
- Linux and Windows native build validation passes with the same external desktop-host behavior and Rust/C ABI.
