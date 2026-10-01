---
summary: "Records duplicated Windows launch semantics between CLI parsing and early console preparation."
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
    - platform
---

# Windows Console Launch Authority

**Priority:** Medium

## Root cause

Windows console preparation must run early because the executable uses the GUI PE subsystem, but it currently infers terminal-facing behavior directly from raw command-line arguments instead of consuming the same parsed launch intent and resolved frontend plan as the application composition root. Launch semantics therefore have more than one authority.

## Evidence

The application CLI owns command and frontend intent, while Windows console preparation independently classifies raw arguments to decide whether to attach to or allocate a console. Automatic frontend selection increases the importance of this distinction: console preparation must not create a terminal merely to make TUI appear available, yet explicit TUI and diagnostic commands still need a usable console when launched without one.

## Governing constraint

Command/frontend intent and the need for terminal presentation must have one semantic authority. Windows-specific console code may own attach/allocate mechanics and their required early timing, but it must not independently reinterpret command names or frontend flags.

## Scope discovery

Trace process startup from the Windows entry point through parent-console attachment, CLI command/frontend parsing, automatic frontend resolution, explicit TUI selection, help and diagnostic commands, console allocation, and final frontend execution. Include tests that classify raw arguments or otherwise duplicate command/frontend knowledge outside the CLI and launch-policy authorities.

## Maintenance consequence

Adding or renaming a terminal-facing command, adding another terminal frontend, or changing frontend-selection semantics can require synchronized edits in both CLI logic and Windows console classification. Missing one edit creates Windows-only failures or unwanted console allocation that compile-time checks do not necessarily expose.

## Repair direction

Separate early console attachment from later console allocation, and expose enough parsed launch/command intent for Windows console allocation to consume that authority rather than reparsing raw arguments. Preserve the GUI-subsystem single-executable behavior and avoid introducing a generalized startup framework solely for this issue.

## Exit criteria

- Raw argument classification in Windows console preparation no longer duplicates command names or frontend flags owned by CLI parsing.
- Parent-console attachment can still occur early enough for GUI-subsystem launches without altering automatic frontend detection.
- Explicit terminal-facing commands and TUI launches allocate a console when required, while desktop/HTTP-only launches do not.
- Deterministic tests cover the relevant startup-state matrix without requiring maintainers to keep two argument classifiers synchronized.
