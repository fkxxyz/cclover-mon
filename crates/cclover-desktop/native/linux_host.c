#include "linux/host_result.h"

const char *cclover_linux_host_error_message(int code) {
    switch (code) {
    case CCLOVER_LINUX_HOST_X11_DISPLAY_CONNECT_FAILED:
        return "X11 display connection failed";
    case CCLOVER_LINUX_HOST_WAYLAND_DISPLAY_CONNECT_FAILED:
        return "Wayland display connection failed";
    case CCLOVER_LINUX_HOST_WAYLAND_GLOBAL_DISCOVERY_FAILED:
        return "Wayland required globals could not be discovered";
    case CCLOVER_LINUX_HOST_WAYLAND_SURFACE_FAILED:
        return "Wayland layer surface creation or recovery failed";
    case CCLOVER_LINUX_HOST_WAYLAND_DISPLAY_IO_FAILED:
        return "Wayland display connection I/O failed";
    case CCLOVER_LINUX_HOST_WAYLAND_RENDER_FAILED:
        return "Wayland rendering failed";
    case CCLOVER_LINUX_HOST_WAYLAND_POLL_FAILED:
        return "Wayland event polling failed";
    default:
        return "unknown Linux native desktop host failure";
    }
}
