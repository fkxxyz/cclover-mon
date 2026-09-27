---
summary: "Defines the primary runtime building blocks, ownership boundaries, and dependency direction."
viewpoint: static
concerns:
  - architecture-coherence
  - performance
  - portability
  - maintainability
activities:
  - orient
  - change
  - assess
facets:
  area:
    - core
    - platform
    - ui
    - native-bridge
---

# System Decomposition

Runtime metric flow:

```text
OS APIs / native libraries
          ↓
platform: Linux | Windows
          ↓
core: sampling + derivation + history
          ↓
presentation: renderer-neutral dashboard semantics
          ↓
frontends: Iced desktop | future terminal
```

Static source dependencies use inversion at the collection boundary:

```text
app composition root ───────→ frontend
        │                        ↓
        ├────────→ presentation ─┘
        │              ↓
        ├────────────→ core
        │              ↑
        └────────→ platform
                       │
                       ↓
              OS APIs / native libraries
                       ↑
                optional C++ bridge
```

- **core** owns platform-neutral metric types, history, aggregation, and sampling contracts, including `Collector`.
- **platform** owns OS-specific collection and desktop integration, implements core-owned sampling contracts, and produces core-owned platform-neutral snapshots. Desktop integration owns native monitor-surface hosting and shell integration such as the system tray. Linux monitor hosting selects Wayland layer-shell or X11 without changing shared drawing, while both display sessions share one StatusNotifierItem tray backend. A platform backend is the batch composition point; metric-specific native IO, parsing, and mutable state belong to the corresponding metric responsibility rather than the backend itself.
- **presentation** converts `MonitorState` into renderer-neutral dashboard semantics: panel meaning, display values, shared formatting, and access to the corresponding history. It depends on core model types but not on Iced, terminal libraries, platform APIs, or application messages.
- **frontend** owns renderer-specific widgets, interaction, and layout. The Iced frontend is shared by Linux and Windows desktop builds. A future terminal frontend may consume the same presentation model while owning terminal-specific layout and interaction.
- **app** composes the selected platform collector, shared presentation, and frontend lifecycle. Frontends do not depend back on application message types when they only render data.
- **native bridge** adapts C++-only dependencies through a small C ABI.

The application composition root selects a platform backend and supplies it to the core sampler. `core` must not depend on `platform`; `platform` may depend on core-owned contracts and model types. Platform-specific types do not cross into core, presentation, or shared frontends.

Desktop shell actions cross the platform boundary only as small platform-neutral application commands such as `DesktopCommand::Quit`. Native tray protocols, menu identifiers, D-Bus objects, Win32 handles, and shell callbacks remain inside platform desktop integration. The application owns lifecycle semantics and performs normal shutdown; a platform tray callback does not terminate the process directly.

Renderer-specific layout has one authority inside each frontend. For the Iced desktop panel, one `PanelLayout` structure determines both which blocks render and the requested desktop-window height. Linux Wayland and X11 hosts consume that same size; protocol-specific placement remains outside the UI. Pixel dimensions do not belong to the renderer-neutral presentation model.

Within a platform backend, dependencies point from the backend to independent metric collectors:

```text
platform backend
  ├── CPU collector
  ├── memory collector
  ├── process collector
  ├── network collector
  ├── disk collector
  └── temperature collector
        ├── hwmon source
        └── NVIDIA NVML source
```

The backend may know every collector so it can assemble a batch snapshot. A metric collector does not depend on the backend or on sibling collectors. A metric collector may fan in multiple native sources when they represent the same platform-neutral metric. For Linux temperatures, generic hwmon sensors and NVIDIA NVML are peer sources owned by the temperature collector; neither becomes a separate core metric.

Any state that survives beyond one observation must use an explicit stable semantic identity owned by `core`. Native numeric IDs, enumeration order, device names, display labels, and other reusable or presentation-oriented values are locators or labels, not cross-sample identity, unless the platform contract explicitly guarantees their lifetime semantics. Cross-sample delta, history, joins, caches, and deduplication must key by the corresponding stable identity type rather than an incidental field such as PID or display text. `ProcessInstanceId` is the shared process-instance identity; PID remains an observable native locator inside that identity, not a standalone process identity.

Temperature snapshots carry a stable sensor identity distinct from their display label. Core history is keyed by that identity, while presentation/UI may derive human-readable labels independently. Native identities such as NVML UUIDs, PCI addresses, hwmon paths, handles, or library types remain platform-private; the platform collector translates them into the core-owned identity representation before crossing the boundary.

Linux collectors may own long-lived native instrumentation such as eBPF links and BPF maps when required by a metric. Those resources remain implementation details of the Linux platform layer. Disk and network attribution may share userspace lifecycle infrastructure, but their kernel-side observation logic remains independently owned because their attribution mechanisms differ.
