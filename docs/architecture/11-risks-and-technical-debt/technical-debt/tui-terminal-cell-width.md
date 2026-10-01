---
summary: "Records the TUI's lack of one terminal display-cell width authority for Unicode text."
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

# TUI Terminal Cell Width

**Priority:** Low-medium

## Root cause

The terminal frontend measures visible text using Unicode scalar-value counts rather than terminal display-cell width. Truncation, padding, column alignment, and ANSI-aware width handling therefore share an approximation that is correct for ASCII-heavy output but not for all text a terminal can display.

## Evidence

The TUI receives externally sourced names such as process names, device labels, interface names, and sensor labels. Its current layout helpers treat each non-ANSI Unicode scalar value as one visible column. Full-width characters, combining sequences, and other Unicode text can occupy a different number of terminal cells, so otherwise-correct table and responsive-layout calculations can become misaligned or truncate at the wrong boundary.

## Governing constraint

Every TUI operation that reasons about horizontal space must use one terminal display-cell width authority. Truncation, padding, column composition, header placement, and ANSI-decorated text must agree on the same width semantics rather than each maintaining its own character-count approximation.

## Scope discovery

Review all TUI helpers and native-terminal boundaries that measure, truncate, pad, align, or compose visible text. Include externally sourced labels and names, ANSI-styled strings, future localization, and both Unix and Windows terminal paths. Re-run scope discovery when closing this debt so no separate width calculation remains outside the shared authority.

## Maintenance consequence

Future support for non-ASCII process names, localized labels, or user-visible device names can produce visually corrupted columns despite passing ASCII-focused layout tests. Fixing individual tables independently would create multiple width authorities and make responsive behavior harder to reason about across terminal implementations.

## Repair direction

Introduce the smallest shared TUI cell-width abstraction needed for display width, cell-safe truncation, and padding, then route all horizontal layout through it. Prefer a well-defined Unicode width implementation over table-specific patches, while keeping terminal layout policy in Rust and the native boundary limited to terminal mechanics.

## Exit criteria

- One TUI authority computes visible terminal-cell width for plain and ANSI-decorated text.
- Truncation and padding operate on terminal cells without splitting or miscounting representative wide and combining Unicode text.
- All TUI column and responsive-layout calculations consume that authority rather than raw scalar-value counts.
- Deterministic tests cover ASCII, full-width text, combining text, and ANSI-decorated variants.
- Linux and Windows terminal output use the same Rust-side width semantics.
