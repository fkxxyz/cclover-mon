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
#define CCLOVER_TEXT_MEASURE_CACHE_SIZE 1024


#include "linux/render.inc"
#include "linux/x11.inc"
#include "linux/wayland_protocol.inc"
#include "linux/wayland_buffers.inc"
#include "linux/wayland_run.inc"
