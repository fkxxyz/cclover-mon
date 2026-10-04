---
summary: "Defines desktop surface and system-tray lifecycle, command delivery, graceful shutdown, and degraded operation."
viewpoint: dynamic
concerns:
  - architecture-coherence
  - maintainability
activities:
  - orient
  - change
  - assess
facets:
  area:
    - platform
---

# Desktop Integration Lifecycle

Desktop integration has two independent native responsibilities: hosting the monitor surface and exposing platform shell integration such as a system tray item. Both remain behind the native desktop boundary in `cclover-desktop` while application lifecycle semantics and desired surface geometry remain platform-neutral. Every native desktop target enters the same desktop-host contract; only the host implementation differs by OS/display protocol.

Normal startup is:

```text
application composition root
        ↓
platform desktop host
  ├── select native monitor-surface protocol
  ├── create the protocol-specific runtime/window
  ├── adapt application lifecycle + desired surface geometry
  └── start native tray integration
          ↓
      register icon and native menu
```

On Linux, X11 versus Wayland selection applies to the monitor surface only. The application does not branch on that choice. Both session types use the same StatusNotifierItem tray implementation over the desktop session D-Bus.

Native tray callbacks translate user intent into a platform-neutral quit signal owned by the desktop boundary. They do not expose native menu identifiers or mutate renderer state directly:

```text
user chooses Quit
        ↓
native tray menu callback
        ↓
desktop quit signal
        ↓
native host loop observes signal
        ↓
normal application/runtime shutdown
```

When application state changes the desired monitor-surface size, the shared dashboard tree and `Scene` remain the geometry authority. The active desktop host realizes that size through its native protocol: Wayland layer-shell on Linux Wayland, X11 window geometry on Linux X11, and Win32 on Windows. Protocol-specific resize messages never enter shared dashboard semantics.

On Wayland, the layer surface has a shorter lifetime than the desktop host. A compositor may close a layer surface when an output disappears or is reconfigured; this is a recoverable surface-lifecycle event, not application quit intent. The Wayland host destroys the closed surface, recreates it against the compositor's current output topology, and forces the replacement surface to receive a complete first frame while preserving application/runtime state. Dynamic `wl_output` globals are tracked symmetrically across registry add/remove events so repeated output churn does not accumulate stale output state. Loss of the Wayland display connection remains a host failure rather than a surface-recovery event.

The Wayland adapter translates protocol callbacks into a small OS-independent lifecycle policy compiled into the production host. That policy owns `absent → waiting-configure → active → recreate-pending` transitions, dirty/draw eligibility, output-slot lookup/reset, and effective-scale derivation. Deterministic native tests drive repeated close/recreate/configure cycles and output add/remove churn against that same policy; the adapter alone performs `wl_*` resource operations. X11 and Win32 retain their own smaller platform-local policies rather than sharing a false cross-platform window lifecycle abstraction.

Windows notification-area integration emits the same platform-neutral lifecycle intent rather than exposing Win32 menu identifiers or handles to shared application state.

Tray availability is optional desktop integration. Failure to connect to the session bus, register a StatusNotifierItem, or find a compatible tray host emits a diagnostic and leaves the monitor surface and metric sampling operational. Tray failure must not be represented as metric unavailability and must not terminate the application.

The tray host and application have independent startup and restart ordering. On Linux, absence of `org.kde.StatusNotifierWatcher` during application startup is transient rather than a permanent tray failure: the tray service remains alive, observes the watcher appearing later, and registers then. If the watcher disappears and returns while the application remains running, tray registration must recover without restarting the application. On Windows, Explorer restart is treated the same way: the host registers the shell's `TaskbarCreated` message and, when it is broadcast, rebinds the monitor surface to the current shell owner, reapplies placement/Z-order, re-registers the notification-area icon, and invalidates the surface for repaint without restarting the sampler or process.

The tray service lives in the same process as the application. Its lifetime is bounded by the application runtime; no helper daemon or second executable owns tray state.
