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

Windows notification-area integration emits the same platform-neutral lifecycle intent rather than exposing Win32 menu identifiers or handles to shared application state.

Tray availability is optional desktop integration. Failure to connect to the session bus, register a StatusNotifierItem, or find a compatible tray host emits a diagnostic and leaves the monitor surface and metric sampling operational. Tray failure must not be represented as metric unavailability and must not terminate the application.

The tray host and application have independent startup and restart ordering. On Linux, absence of `org.kde.StatusNotifierWatcher` during application startup is transient rather than a permanent tray failure: the tray service remains alive, observes the watcher appearing later, and registers then. If the watcher disappears and returns while the application remains running, tray registration must recover without restarting the application. On Windows, Explorer restart is treated the same way: the host registers the shell's `TaskbarCreated` message and, when it is broadcast, rebinds the monitor surface to the current shell owner, reapplies placement/Z-order, re-registers the notification-area icon, and invalidates the surface for repaint without restarting the sampler or process.

The tray service lives in the same process as the application. Its lifetime is bounded by the application runtime; no helper daemon or second executable owns tray state.
