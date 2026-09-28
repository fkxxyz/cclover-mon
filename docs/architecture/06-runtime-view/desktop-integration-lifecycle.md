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

Desktop integration has two independent native responsibilities: hosting the monitor surface and exposing platform shell integration such as a system tray item. Both remain behind the platform boundary while application lifecycle semantics and desired surface geometry remain platform-neutral. Every native desktop target enters the same platform-owned host contract; only the host implementation differs by OS/display protocol.

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

Native tray callbacks translate user intent into platform-neutral `DesktopCommand` values. They do not terminate the process or mutate Iced state directly:

```text
user chooses Quit
        ↓
native tray menu callback
        ↓
DesktopCommand::Quit
        ↓
application update loop
        ↓
normal application/runtime shutdown
```

When application state changes the desired monitor-surface size, the application only updates that platform-neutral geometry. On Linux, the desktop host distinguishes desired geometry from compositor-realized geometry and treats a resize request as incomplete until window events report that the realized size has converged to the desired size. If an initial or later configure reports a different size, the host re-applies the current desired geometry. Linux uses a layer-shell size action on Wayland or an Iced/X11 window resize on X11, while Windows uses its native Iced window host. Protocol-specific resize messages never enter the application message enum.

Future Windows notification-area integration must emit the same command rather than exposing Win32 menu identifiers or handles to the application lifecycle.

Tray availability is optional desktop integration. Failure to connect to the session bus, register a StatusNotifierItem, or find a compatible tray host emits a diagnostic and leaves the monitor surface and metric sampling operational. Tray failure must not be represented as metric unavailability and must not terminate the application.

The tray service lives in the same process as the application. Its lifetime is bounded by the application runtime; no helper daemon or second executable owns tray state.
