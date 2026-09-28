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
frontends: shared Iced native/Web panel | terminal TUI
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

- **core** owns platform-neutral metric types, history, aggregation, and sampling contracts, including `Collector` and the generic `Collection<T>` observation outcome used by every raw metric.
- **platform** owns OS-specific collection and implements core-owned sampling contracts, producing core-owned platform-neutral snapshots. The platform responsibility for desktop integration is physically isolated in the `cclover-desktop` package, which owns the complete native monitor-surface host: display-protocol selection, runtime creation, protocol-specific placement/input/window policy, host-side resize realization, and shell integration such as the system tray. Linux monitor hosting selects Wayland layer-shell or X11 without changing shared drawing, while both display sessions share one StatusNotifierItem tray backend. A platform backend is the batch composition point; metric-specific native IO, parsing, and mutable state belong to the corresponding metric responsibility rather than the backend itself.
- **presentation** converts `MonitorState` into renderer-neutral dashboard semantics: panel meaning, display values, shared formatting, and access to the corresponding history. It depends on core model types but not on Iced, terminal libraries, platform APIs, or application messages.
- **frontend** owns renderer-specific widgets, interaction, and layout. One Iced panel implementation is shared by Linux/Windows native desktop and browser/WASM builds; native and Web runtimes differ only in hosting and state transport. The terminal frontend consumes the same presentation model while owning terminal-specific layout and interaction. It does not own sampling or depend on platform collection; the execution adapter composes those concerns.
- **desktop app** inside `cclover-desktop` supplies platform-neutral application lifecycle behavior to the selected desktop host; the root application composes the selected platform collector/runtime with the chosen frontend. It may expose desired surface geometry, but it does not select or manipulate native display protocols. Frontends do not depend back on application message types when they only render data.
- **native bridge** adapts C++-only dependencies through a small C ABI.

The application composition root selects a platform backend and supplies it to the core sampler. `core` must not depend on `platform`; `platform` may depend on core-owned contracts and model types. Platform-specific types do not cross into core, presentation, or shared frontends. Shared contracts must name and model platform-neutral semantics: native units, counters, handles, protocol terms, or source-specific vocabulary are translated at the platform boundary rather than exposed through core-owned field or type names.

An independently failing metric capability keeps its observation status as program data for as long as downstream behavior depends on that distinction. `Available(empty)` means the capability was observed successfully and produced no entries; `Unavailable(reason)` means it could not be observed. Core derivation, Web transport, and presentation must not collapse unavailable observations into zero, an empty collection, or an unqualified `None`. Presentation maps the preserved status to renderer-neutral dashboard semantics; frontends render those semantics and do not infer collector health from missing values, platform identity, or source-specific errors. Failure of an optional child capability, such as per-process attribution, does not make an otherwise available parent metric unavailable.

These top-level source dependency directions are mechanically checked by the repository's millisecond-scale `bun archgate.ts` gate. The gate protects only static dependency direction; it does not replace compilation, Clippy, runtime validation, or architecture-document validation.

Desktop shell actions cross the platform boundary only as small platform-neutral application commands such as `DesktopCommand::Quit`. Native tray protocols, menu identifiers, D-Bus objects, Win32 handles, display-server selection, layer-shell actions, X11/EWMH operations, and host-side resize mechanics remain inside platform desktop integration. The application owns lifecycle semantics and desired surface geometry; the desktop host translates that geometry into the active native protocol. A platform tray callback does not terminate the process directly.

Renderer-specific layout has one authority inside each frontend. For the shared Iced panel, `PanelLayout` determines which blocks render and the requested panel height, while renderer geometry specifications own the pixel dimensions used by both widget construction and height derivation. Any dimension that affects both rendered geometry and desktop-surface sizing must be defined once and consumed by both paths; parallel hard-coded geometry is not allowed. Linux Wayland/X11 hosts and the Web/WASM runtime consume the same layout; protocol-specific placement and browser canvas hosting remain outside the UI. Pixel dimensions do not belong to the renderer-neutral presentation model.

Frontend helper APIs group related semantic inputs into small purpose-specific parameter structures once positional arguments span multiple independent concerns. Do not preserve argument explosion with lint suppression, and do not generalize this rule into a renderer-agnostic widget framework when a local semantic structure is sufficient.

The native runtime owns the only sampler. Each completed typed `MonitorState` is published once to a bounded native latest-state hub consumed independently by enabled desktop and terminal frontends, while optional HTTP delivery projects that same completed state into the explicit Web transport schema and serialized latest-state hub. Enabling multiple frontends must not create additional samplers, collectors, or sampling cadences. Browser/WASM code contains no platform backend or collector; it deserializes the Web projection back into panel-consumable state and feeds it into the same Iced presentation/view path. Core model types themselves are not the wire schema.

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

Windows follows the same collector partition rather than one monolithic Win32 backend. CPU uses system timing counters, memory uses the system memory-status API, process collection uses one NT system-process snapshot, network uses IP Helper interface counters, and disk uses storage/device IO controls. Windows temperature support is intentionally a fan-in responsibility: PawnIO-backed CPU/Super-I/O access, GPU vendor APIs, and SMART/NVMe storage sensors may coexist as peer sources without exposing their native APIs to core. The Windows desktop host separately owns native monitor-surface policy—top-right placement, bottom window level, taskbar exclusion, undecorated sizing, and pointer passthrough—while shared Iced drawing remains platform-neutral.

Core sampling orchestration follows the same responsibility split. A top-level sampling/derivation function coordinates snapshot-wide flow, while metric-specific delta/rate derivation belongs in focused helpers with independently testable inputs and outputs. New metric logic must not accumulate as another inline branch inside an already cross-metric orchestration body when it can be separated without changing semantics.

Any state that survives beyond one observation must use an explicit stable semantic identity owned by `core`. Native numeric IDs, enumeration order, device names, display labels, and other reusable or presentation-oriented values are locators or labels, not cross-sample identity, unless the platform contract explicitly guarantees their lifetime semantics. Cross-sample delta, history, joins, caches, and deduplication must key by the corresponding stable identity type rather than an incidental field such as PID or display text. `ProcessInstanceId` is the shared process-instance identity; PID remains an observable native locator inside that identity, not a standalone process identity.

Temperature, network, and disk snapshots carry stable identities distinct from their display labels. Core delta/history association is keyed by those identities, while presentation/UI may derive or display human-readable labels independently. `NetworkId` and `DiskId` are core-owned opaque identity types; platform-native locators remain uninterpreted outside the platform translation boundary. Native identities such as NVML UUIDs, PCI addresses, hwmon paths, handles, device numbers, interface indices, or library types remain platform-private; the platform collector translates them into the core-owned identity representation before crossing the boundary.

Linux collectors may own long-lived native instrumentation such as eBPF links and BPF maps when required by a metric. Those resources remain implementation details of the Linux platform layer. Disk and network attribution may share userspace lifecycle infrastructure, but their kernel-side observation logic remains independently owned because their attribution mechanisms differ.
