#define _GNU_SOURCE
#include "native_scene.h"
#include "wayland/wlr-layer-shell-unstable-v1-client-protocol.h"

#include <X11/Xatom.h>
#include <X11/Xlib.h>
#include <X11/Xutil.h>
#include <X11/extensions/Xrender.h>
#include <X11/extensions/shape.h>
#include <cairo/cairo-xlib.h>
#include <cairo/cairo.h>
#include <errno.h>
#include <fcntl.h>
#include <math.h>
#include <poll.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/stat.h>
#include <sys/types.h>
#include <time.h>
#include <unistd.h>
#include <wayland-client.h>

#define CCLOVER_MARGIN 16

typedef enum {
    CCLOVER_LINUX_HOST_OK = 0,
    CCLOVER_LINUX_HOST_X11_DISPLAY_CONNECT_FAILED = 101,
    CCLOVER_LINUX_HOST_WAYLAND_DISPLAY_CONNECT_FAILED = 201,
    CCLOVER_LINUX_HOST_WAYLAND_GLOBAL_DISCOVERY_FAILED = 202,
    CCLOVER_LINUX_HOST_WAYLAND_SURFACE_FAILED = 203,
    CCLOVER_LINUX_HOST_WAYLAND_DISPLAY_IO_FAILED = 204,
    CCLOVER_LINUX_HOST_WAYLAND_RENDER_FAILED = 205,
    CCLOVER_LINUX_HOST_WAYLAND_POLL_FAILED = 206,
} CcloverLinuxHostResult;

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

#include "linux/render.inc"
#include "linux/x11.inc"
#include "linux/wayland_protocol.inc"
#include "linux/wayland_buffers.inc"
#include "linux/wayland_run.inc"
