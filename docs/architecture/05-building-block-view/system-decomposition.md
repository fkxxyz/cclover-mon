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
cclover-ui: shared dashboard tree + visual/layout authority
          ↓
renderers: native desktop | Web | terminal TUI
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
                 native C adapter
                       ↑
          optional C++ compatibility shim
```

- **core** owns platform-neutral metric types, history, aggregation, and sampling contracts, including `Collector` and the generic `Collection<T>` observation outcome used by every raw metric.
- **platform** owns OS-specific collection and implements core-owned sampling contracts, producing core-owned platform-neutral snapshots. The platform responsibility for desktop integration is physically isolated in the `cclover-desktop` package, which owns the complete native monitor-surface host: display-protocol selection, runtime creation, protocol-specific placement/input/window policy, host-side resize realization, and shell integration such as the system tray. Linux monitor hosting selects Wayland layer-shell or X11 without changing shared drawing, while both display sessions share one StatusNotifierItem tray backend. A platform backend is the batch composition point; metric-specific native IO, parsing, and mutable state belong to the corresponding metric responsibility rather than the backend itself.
- **presentation** converts `MonitorState` into renderer-neutral dashboard semantics: panel meaning, display values, shared formatting, and access to the corresponding history. It depends on core model types but not on desktop, Web, terminal, or platform APIs.
- **cclover-ui** is the cross-renderer dashboard authority. It consumes presentation semantics and defines card/section structure, ordering, shared visual tokens, graph policy, and geometry that must remain consistent across graphical renderers. Its element vocabulary is application-specific and deliberately small; it must not grow into a general widget toolkit.
- **renderers** realize that shared dashboard definition. Native desktop renderers consume `cclover-ui::NativeScene`, the shared lowering of dashboard structure into absolute drawing primitives; platform code owns primitive execution, font realization/measurement, and surface integration. Native scene lowering may query the renderer for actual text extents, but `cclover-ui` remains the sole authority that converts those measurements into row geometry. Web owns browser realization and transport lifecycle and consumes the higher-level dashboard tree through the Web/WASM adapter instead of `NativeScene`. The terminal frontend continues to consume presentation semantics directly because terminal layout is materially different.
- **desktop host** inside `cclover-desktop` bridges the native latest-state receiver to `NativeScene`, exposes only the shared scene/poll ABI to platform-native code, and owns desktop lifecycle signals. The root application composes the selected platform collector/runtime with this host; shared UI/presentation code does not select or manipulate native display protocols.
- **native boundary** uses C for thin OS/API/protocol adaptation when that removes disproportionate Rust framework or dependency cost without moving application semantics out of Rust. Rust remains responsible for typed state, parsing where memory safety is materially useful, cross-sample semantics, synchronization, and non-trivial resource lifetime. C++ is used only as a narrow compatibility shim for C++-only dependencies and remains behind a C ABI.

The application composition root selects a platform backend and supplies it to the core sampler. `core` must not depend on `platform`; `platform` may depend on core-owned contracts and model types. Platform-specific types do not cross into core, presentation, or shared frontends. Shared contracts must name and model platform-neutral semantics: native units, counters, handles, protocol terms, or source-specific vocabulary are translated at the platform boundary rather than exposed through core-owned field or type names.

Language choice follows responsibility rather than directory or layer. Keep stateful safety-sensitive logic in Rust when it owns lifetime, synchronization, parsing, identity, derivation, availability, or other application semantics. Prefer C for thin native hosts and protocol/API glue whose value is direct access to an existing C ABI, especially when a Rust implementation would introduce a large framework or runtime for little product logic. Do not migrate safe Rust collectors to C merely because they call native facilities; a migration must remove meaningful dependency, binary-size, build, or interoperability cost. C++ remains reserved for C++-only dependencies. ADR 001 is the authority for this decision process.

An independently failing metric capability keeps its observation status as program data for as long as downstream behavior depends on that distinction. `Available(empty)` means the capability was observed successfully and produced no entries; `Unavailable(reason)` means it could not be observed. Core derivation, Web transport, and presentation must not collapse unavailable observations into zero, an empty collection, or an unqualified `None`. Presentation maps the preserved status to renderer-neutral dashboard semantics; frontends render those semantics and do not infer collector health from missing values, platform identity, or source-specific errors. Failure of an optional child capability, such as per-process attribution, does not make an otherwise available parent metric unavailable.

These top-level source dependency directions are mechanically checked by the repository's millisecond-scale `bun archgate.ts` gate. The gate protects only static dependency direction; it does not replace compilation, Clippy, runtime validation, or architecture-document validation.

Desktop shell actions cross the platform boundary only as small platform-neutral lifecycle signals such as the shared desktop quit flag. Native tray protocols, menu identifiers, D-Bus objects, Win32 handles, display-server selection, layer-shell actions, X11/EWMH operations, and host-side resize mechanics remain inside platform desktop integration. `NativeScene` owns requested surface geometry; the desktop host translates that geometry into the active native protocol.

Graphical dashboard structure and shared geometry have one authority in `cclover-ui`. Any dimension that affects both rendered geometry and desktop-surface sizing must be defined once there and consumed by renderer adapters; parallel platform-specific copies are not allowed. `NativeScene` is the shared native lowering into absolute drawing primitives. Web may map the higher-level dashboard tree to browser-native layout/drawing instead of consuming desktop pixel primitives. Pixel geometry does not belong to the metric-semantic `cclover-presentation` layer.

Natural-width native text is measured through the actual platform font realization before `NativeScene` finalizes row geometry. Linux Cairo and Windows GDI own font selection, fallback, and measurement; they return only text extents through the native host contract. `cclover-ui` owns how those extents affect cell widths, gaps, alignment, and clipping. Shared layout must not infer native text width from font-family assumptions such as fixed glyph advances, and native hosts must not independently rebuild row layout.

Frontend helper APIs group related semantic inputs into small purpose-specific parameter structures once positional arguments span multiple independent concerns. Do not preserve argument explosion with lint suppression, and do not generalize this rule into a renderer-agnostic widget framework when a local semantic structure is sufficient.

The native runtime owns the only sampler. Each completed typed `MonitorState` is published once to a bounded native latest-state hub consumed independently by enabled desktop and terminal frontends. When HTTP is enabled, that same completed state is projected in parallel into two explicit schemas: the internal browser transport used by SSE `/events`, and the public versioned API contract used by `/api/v1/*`. The browser transport may evolve with the bundled Web client and is deserialized back into panel-consumable state; the public API schema is independently owned and must not change merely because core or browser-transport types change. Split `/api/v1` endpoints are views over the single API-v1 projection, not independent serialization authorities. The current browser dashboard transport intentionally carries only its compact card-oriented process projections rather than the complete process domain; a future process-oriented Web view must extend that transport by projecting `ProcessDomainSnapshot`, not by reconstructing processes from Top-N card lists. Enabling multiple frontends must not create additional samplers, collectors, or sampling cadences. Core model types themselves are not wire schemas.

Within a platform backend, dependencies point from the backend to independent metric collectors:

```text
platform backend
  ├── CPU collector
  ├── memory collector
  ├── process collector
  ├── network collector
  ├── disk collector
  ├── temperature collector
  │     └── generic hwmon source
  ├── GPU collector
  │     └── AMD amdgpu DRM/sysfs + device hwmon non-temperature telemetry
  └── shared NVIDIA telemetry adapter
        └── one cached NVML session/device set for GPU telemetry
```

The backend may know every collector so it can assemble a batch snapshot. A metric collector does not depend on the backend or on sibling collectors. A metric collector may fan in multiple native sources when they represent the same platform-neutral metric. Linux GPU collection merges AMD `amdgpu` native telemetry and NVIDIA NVML into one `GpuSnapshot` sequence. Generic hwmon discovery reports temperature observations without vendor exclusion rules. At batch composition, the Linux backend reconciles temperature observations against actually discovered GPUs by canonical physical-device identity: a matching temperature is consumed into the GPU snapshot, while unmatched sensors remain in the generic temperature collection. An existing vendor GPU temperature wins over a matching hwmon fallback. NVIDIA NVML session/device lifetime is owned by a shared Linux telemetry adapter; metric policy remains outside the raw adapter.

Windows follows the same collector partition rather than one monolithic Win32 backend. CPU uses system timing counters, memory uses the system memory-status API, process collection uses one NT system-process snapshot, network uses IP Helper interface counters, and disk uses storage/device IO controls. The implemented temperature source keeps one PawnIO session with the pinned signed Intel MSR module for Intel package temperature; additional CPU/Super-I/O and SMART/NVMe sources remain peers. The Windows GPU collector owns independently optional dynamically loaded NVIDIA NVML and AMD ADL sessions, translates NVML UUID or ADL UDID/PCI identity plus vendor telemetry into `GpuSnapshot`, and keeps each vendor failure isolated. Native APIs and handles do not cross into core. The Windows desktop host separately owns native monitor-surface policy—top-right placement, bottom window level, taskbar exclusion, undecorated sizing, pointer passthrough, and native drawing realization—while shared dashboard semantics and structure remain platform-neutral.

Core sampling orchestration follows the same responsibility split. A top-level sampling/derivation function coordinates snapshot-wide flow, while metric-specific delta/rate derivation belongs in focused helpers with independently testable inputs and outputs. New metric logic must not accumulate as another inline branch inside an already cross-metric orchestration body when it can be separated without changing semantics.

Any state that survives beyond one observation must use an explicit stable semantic identity owned by `core`. Native numeric IDs, enumeration order, device names, display labels, and other reusable or presentation-oriented values are locators or labels, not cross-sample identity, unless the platform contract explicitly guarantees their lifetime semantics. Cross-sample delta, history, joins, caches, and deduplication must key by the corresponding stable identity type rather than an incidental field such as PID or display text. `ProcessInstanceId` is the shared process-instance identity; PID remains an observable native locator inside that identity, not a standalone process identity.

Core owns the authoritative process-centric projection as `ProcessDomainSnapshot`, keyed by `ProcessInstanceId`. Each sampling cycle joins currently observable process metadata, per-process CPU and memory, per-disk I/O rates, and per-interface network I/O rates into that domain before any dashboard-oriented Top-N truncation. The domain uses the union of identities reported by process and attribution sources, so a process observed only by I/O attribution remains representable with missing metadata instead of being discarded. Capability status for metadata, CPU, memory, disk I/O, and network I/O remains explicit at domain level; an unavailable capability is not represented as a zero-valued process metric. The complete keyed map is shared behind `Arc` so publishing/cloning `MonitorState` does not deep-copy the full process table. Existing CPU, memory, disk, and network card lists are derived core projections from this domain, not parallel authorities.

Temperature, GPU, network, and disk snapshots carry stable identities distinct from their display labels. Core delta/history association is keyed by those identities where history or cross-sample association exists, while presentation/UI may derive or display human-readable labels independently. `GpuId`, `NetworkId`, and `DiskId` are core-owned opaque identity types; platform-native locators remain uninterpreted outside the platform translation boundary. Native identities such as NVML UUIDs, PCI addresses, canonical sysfs device paths, hwmon paths, handles, device numbers, interface indices, or library types remain platform-private; the platform collector translates them into the core-owned identity representation before crossing the boundary.

Linux collectors may own long-lived native instrumentation such as eBPF links and BPF maps when required by a metric. Those resources remain implementation details of the Linux platform layer. Disk and network attribution may share userspace lifecycle infrastructure, but their kernel-side observation logic remains independently owned because their attribution mechanisms differ.
