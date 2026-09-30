---
summary: "Chooses selected upstream Linux hwmon source as the primary Windows PC hardware-compatibility upstream behind a minimal compatibility facade and read-only Windows transports."
viewpoint: decision
concerns:
  - architecture-coherence
  - maintainability
  - portability
  - performance
  - security
activities:
  - orient
  - change
  - assess
facets:
  area:
    - platform
    - native-bridge
    - whole-system
---

# ADR 012: Linux hwmon Source for Windows Hardware Compatibility

## Decision

Selected Linux kernel `drivers/hwmon` sources are the primary upstream for low-level x86 PC hardware compatibility that can be realized through cclover-mon's supported Windows hardware transports. This supersedes ADR 010's LibreHardwareMonitor-first compatibility-source strategy. LibreHardwareMonitor remains a secondary reviewed source for motherboard channel mappings, embedded-controller knowledge, migration comparison, or hardware families not represented by the selected Linux drivers; it is not a runtime dependency.

The repository vendors an exact, manifest-selected snapshot of required Linux source files at a pinned upstream commit. Vendored Linux files remain byte-for-byte upstream and retain their original SPDX identifiers, copyright notices, and license terms. Project code does not patch or rewrite those files. Windows-specific adaptation belongs in project-owned wrappers, the minimal Linux compatibility facade, or the Windows hardware transport. If an upstream driver cannot be used without source modification, it is not imported until the incompatibility can be expressed outside the vendored file.

```text
selected upstream Linux hwmon source
        ↓ unchanged C source
project-owned minimal Linux compatibility facade
        ↓
project-owned Windows hardware transport
        ↓
structured Windows API or signed PawnIO capability
        ↓
Windows hardware-telemetry runtime
        ↓
typed temperature / fan / future hardware observations
```

The compatibility facade implements only Linux in-kernel APIs actually required by the selected driver set. It is not a general Linux-kernel emulator and must not expand merely to make unrelated `drivers/hwmon` files compile. Driver inclusion is capability-based: an upstream driver is eligible when its hardware access can be mapped to an explicitly supported Windows transport and its runtime behavior satisfies the Windows telemetry safety boundary.

Imported Linux C is compiled with a compiler mode capable of the GNU C extensions used by the kernel while targeting the Windows ABI. Cargo remains final build and link authority. Wrapper translation units and compatibility headers may define module-registration, device, hwmon, regmap, synchronization, allocation, timing, CPU-topology, PCI, MSR, SMN, or similar facade behavior without modifying the vendored driver source.

## Runtime and Safety Boundary

The Windows hardware-telemetry runtime continues to own discovery, long-lived PawnIO sessions/modules, bus synchronization, retry state, and stable topology. Linux driver lifecycle constructs are compatibility inputs, not new application ownership. Linux sysfs, kernel device objects, module lifetime, or other kernel-only concepts must terminate inside the compatibility boundary and must not cross into core-owned metric contracts.

Hardware telemetry remains a read-only product capability. The transport may perform writes that are strictly required by a documented read protocol, such as bank, index, or logical-device selection, but imported driver behavior does not grant authority to change fan PWM, control mode, thresholds, firmware policy, device enablement, or other persistent machine state. A driver path that requires such state-changing writes is ineligible for direct execution unless a separate architecture decision explicitly authorizes that behavior.

The compatibility layer does not expose arbitrary privileged hardware access merely because a Linux driver can request it. Windows transport capabilities remain narrow and explicit. PawnIO use continues to rely on pinned official signed modules and their capability checks under ADR 009. Existing cross-process hardware-bus synchronization remains in force where the underlying physical access requires it.

## Upstream Synchronization

Ordinary cclover-mon development and builds consume the checked-in vendor snapshot and do not require a Linux kernel checkout. Linux source retrieval occurs only during explicit upstream synchronization.

Synchronization tooling uses a user-scoped cache outside the repository/worktree, fetches only the required upstream revision and paths through Git partial/sparse retrieval, resolves the requested ref to an exact commit, and copies only manifest-selected source into the vendor snapshot. The pinned commit and source provenance are repository data. Updating an existing selected driver should normally be a revision bump plus compile/test validation; changes in Linux internal APIs are absorbed in the shared compatibility facade rather than patched into individual vendored drivers.

New upstream hwmon files are not automatically enabled. They are first classified against the supported Windows transport set, the read-only policy, and compatibility-facade scope. Adding a new chip to an already-supported upstream driver and transport should require no project-specific hardware algorithm port.

## Scope

Initial compatibility work is limited to PC hardware transports with direct product value, such as x86 CPU MSR/CPUID paths, required PCI configuration access, AMD SMN, and LPC/Super-I/O. Additional transport classes such as SMBus or EC may be added deliberately when useful. Support for unrelated I2C, SPI, Device Tree, regulator, BMC, embedded, or SoC-only hwmon drivers does not justify recreating their Linux subsystems on Windows.

The project may keep a project-owned Windows implementation for a hardware family when reproducing the Linux driver's kernel lifecycle would cost more than the compatibility value it provides. Zero modification of selected vendored upstream files is the invariant; universal compilation of all Linux hwmon drivers is not a goal.

## Licensing

Project-owned source is licensed `GPL-2.0-or-later` so it can form one combined work with imported Linux files licensed `GPL-2.0-only`. Vendored third-party files retain their own license identities and notices. A distributed target containing GPL-2.0-only Linux code must satisfy GPL version 2 for the combined work; this does not relicense project-owned files away from `GPL-2.0-or-later` or replace separate obligations attached to MPL, PawnIO, or other third-party material.

Dependency selection for a target that combines GPL-2.0-only Linux code must remain compatible with GPL version 2. A dependency's alternative license may be used when the dependency offers a GPLv2-compatible choice, such as `MIT OR Apache-2.0` used under MIT terms.

## Rationale

Linux hwmon already contains the hardware IDs, register layouts, decoding rules, generation tables, and quirks that cclover-mon otherwise has to port manually from another compatibility project. Keeping selected upstream driver files unchanged moves maintenance effort from repeated hardware-algorithm translation to a shared compatibility surface. When Linux adds support inside an already-supported driver and transport, cclover-mon can inherit that work by synchronizing source instead of recreating it.

The compatibility boundary also prevents that reuse strategy from turning into a Linux-kernel port. Project-owned transport policy preserves Windows privilege and safety requirements even when upstream Linux code assumes kernel authority that the product intentionally does not grant.

## Consequences

- Linux hwmon becomes the primary source-code upstream for eligible Windows PC hardware compatibility; LHM becomes secondary compatibility knowledge.
- Selected Linux source is checked in and buildable without cloning the Linux repository during ordinary development.
- Upstream source files have zero local patches; compatibility fixes belong to shared project-owned layers.
- Linux internal API churn can require compatibility-facade maintenance even when hardware logic is unchanged.
- New hardware on an existing supported transport can often be inherited by an upstream revision update, while a genuinely new transport still requires Windows capability work.
- The read-only Windows product boundary can make some Linux driver paths intentionally ineligible despite successful compilation.
- The project license changes from `GPL-3.0-or-later` to `GPL-2.0-or-later`; imported source and other third-party material retain their own licenses.
