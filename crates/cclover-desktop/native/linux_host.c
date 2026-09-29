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

static double cclover_profile_cpu_ms(void) {
    struct timespec ts;
    clock_gettime(CLOCK_THREAD_CPUTIME_ID, &ts);
    return (double)ts.tv_sec * 1000.0 + (double)ts.tv_nsec / 1000000.0;
}

typedef struct {
    uint64_t hash;
    uint8_t *text;
    size_t text_len;
    uint32_t size;
    uint32_t flags;
    float width;
} CcloverTextMeasureCacheEntry;

typedef struct {
    uint32_t size;
    uint32_t bold;
    cairo_font_extents_t extents;
    int valid;
} CcloverFontMetricsCacheEntry;

typedef struct {
    void *context;
    const CcloverCallbacks *callbacks;
    cairo_surface_t *measure_surface;
    cairo_t *measure_cr;
    CcloverTextMeasureCacheEntry measure_cache[CCLOVER_TEXT_MEASURE_CACHE_SIZE];
    CcloverFontMetricsCacheEntry font_metrics[32];
} CcloverHost;

static void cclover_source_argb(cairo_t *cr, uint32_t argb) {
    double a = ((argb >> 24) & 0xff) / 255.0;
    double r = ((argb >> 16) & 0xff) / 255.0;
    double g = ((argb >> 8) & 0xff) / 255.0;
    double b = (argb & 0xff) / 255.0;
    cairo_set_source_rgba(cr, r, g, b, a);
}

static void cclover_round_rect(cairo_t *cr, const CcloverCommand *cmd) {
    double x = cmd->x, y = cmd->y, w = cmd->width, h = cmd->height;
    double r = fmin(cmd->radius, fmin(w, h) / 2.0);
    if (r <= 0.0) {
        cairo_rectangle(cr, x, y, w, h);
        return;
    }
    cairo_new_sub_path(cr);
    cairo_arc(cr, x + w - r, y + r, r, -M_PI / 2.0, 0.0);
    cairo_arc(cr, x + w - r, y + h - r, r, 0.0, M_PI / 2.0);
    cairo_arc(cr, x + r, y + h - r, r, M_PI / 2.0, M_PI);
    cairo_arc(cr, x + r, y + r, r, M_PI, M_PI * 3.0 / 2.0);
    cairo_close_path(cr);
}

static char *cclover_text_copy(const uint8_t *bytes, size_t len, char stack[512]) {
    char *text = stack;
    if (len + 1 > 512) {
        text = malloc(len + 1);
        if (!text) return NULL;
    }
    memcpy(text, bytes, len);
    text[len] = '\0';
    return text;
}

static void cclover_configure_context(cairo_t *cr) {
    cairo_font_options_t *font_options = cairo_font_options_create();
    cairo_font_options_set_hint_metrics(font_options, CAIRO_HINT_METRICS_OFF);
    cairo_set_font_options(cr, font_options);
    cairo_font_options_destroy(font_options);
}

static void cclover_select_font(cairo_t *cr, uint32_t size, int bold) {
    cairo_select_font_face(cr, "Inconsolata", CAIRO_FONT_SLANT_NORMAL,
                           bold ? CAIRO_FONT_WEIGHT_BOLD : CAIRO_FONT_WEIGHT_NORMAL);
    cairo_set_font_size(cr, size);
}

static int cclover_measure_init(CcloverHost *host) {
    host->measure_surface = cairo_image_surface_create(CAIRO_FORMAT_ARGB32, 1, 1);
    if (cairo_surface_status(host->measure_surface) != CAIRO_STATUS_SUCCESS) return -1;
    host->measure_cr = cairo_create(host->measure_surface);
    if (cairo_status(host->measure_cr) != CAIRO_STATUS_SUCCESS) return -1;
    cclover_configure_context(host->measure_cr);
    return 0;
}

static void cclover_measure_destroy(CcloverHost *host) {
    size_t i;
    for (i = 0; i < CCLOVER_TEXT_MEASURE_CACHE_SIZE; ++i) {
        free(host->measure_cache[i].text);
        host->measure_cache[i].text = NULL;
    }
    if (host->measure_cr) cairo_destroy(host->measure_cr);
    if (host->measure_surface) cairo_surface_destroy(host->measure_surface);
    host->measure_cr = NULL;
    host->measure_surface = NULL;
}

static uint64_t cclover_measure_hash(const uint8_t *bytes, size_t len,
                                     uint32_t size, uint32_t flags) {
    uint64_t hash = UINT64_C(1469598103934665603);
    size_t i;
    for (i = 0; i < len; ++i) {
        hash ^= bytes[i];
        hash *= UINT64_C(1099511628211);
    }
    hash ^= size;
    hash *= UINT64_C(1099511628211);
    hash ^= flags;
    hash *= UINT64_C(1099511628211);
    return hash;
}

static float cclover_measure_text(void *context, const uint8_t *bytes, size_t len,
                                  uint32_t size, uint32_t flags) {
    CcloverHost *host = context;
    uint64_t hash = cclover_measure_hash(bytes, len, size, flags);
    CcloverTextMeasureCacheEntry *cached =
        &host->measure_cache[hash % CCLOVER_TEXT_MEASURE_CACHE_SIZE];
    char stack[512];
    char *text;
    cairo_text_extents_t ext;
    if (cached->text && cached->hash == hash && cached->text_len == len &&
        cached->size == size && cached->flags == flags &&
        memcmp(cached->text, bytes, len) == 0)
        return cached->width;
    text = cclover_text_copy(bytes, len, stack);
    if (!text) return 0.0f;
    cclover_select_font(host->measure_cr, size, (flags & CCLOVER_TEXT_BOLD) != 0);
    cairo_text_extents(host->measure_cr, text, &ext);
    if (len > 0) {
        uint8_t *copy = malloc(len);
        if (copy) {
            memcpy(copy, bytes, len);
            free(cached->text);
            cached->hash = hash;
            cached->text = copy;
            cached->text_len = len;
            cached->size = size;
            cached->flags = flags;
            cached->width = (float)ext.x_advance;
        }
    }
    if (text != stack) free(text);
    return (float)ext.x_advance;
}


static void cclover_font_extents(CcloverHost *host, cairo_t *cr, uint32_t size,
                                 uint32_t bold, cairo_font_extents_t *extents) {
    CcloverFontMetricsCacheEntry *cached =
        &host->font_metrics[(size * 2u + bold) % 32u];
    if (cached->valid && cached->size == size && cached->bold == bold) {
        *extents = cached->extents;
        return;
    }
    cairo_font_extents(cr, extents);
    cached->size = size;
    cached->bold = bold;
    cached->extents = *extents;
    cached->valid = 1;
}

static void cclover_draw_text(CcloverHost *host, cairo_t *cr, const CcloverCommand *cmd,
                              uint32_t *font_size, uint32_t *font_bold, int *font_valid) {
    char stack[512];
    char *text = cclover_text_copy(cmd->text, cmd->text_len, stack);
    cairo_font_extents_t font;
    uint32_t bold;
    int clipped;
    double x, y;
    if (!text) return;

    bold = (cmd->flags & CCLOVER_TEXT_BOLD) != 0;
    if (!*font_valid || *font_size != cmd->text_size || *font_bold != bold) {
        cclover_select_font(cr, cmd->text_size, bold);
        *font_size = cmd->text_size;
        *font_bold = bold;
        *font_valid = 1;
    }
    clipped = (cmd->flags & CCLOVER_TEXT_CLIP) != 0;
    if (clipped) {
        cairo_save(cr);
        cairo_rectangle(cr, cmd->x, cmd->y, cmd->width, cmd->height);
        cairo_clip(cr);
    }
    cclover_font_extents(host, cr, cmd->text_size, bold, &font);
    /* NativeScene gives end-aligned cells their measured natural width, so the
       rect's left edge is already the final text origin. */
    x = cmd->x;
    y = cmd->y + (cmd->height - (font.ascent + font.descent)) / 2.0 + font.ascent;
    cclover_source_argb(cr, cmd->color);
    cairo_move_to(cr, x, y);
    cairo_show_text(cr, text);
    if (clipped) cairo_restore(cr);
    if (text != stack) free(text);
}

#define CCLOVER_DRAW_ALL 0
#define CCLOVER_DRAW_STATIC 1
#define CCLOVER_DRAW_DYNAMIC 2

static int cclover_draw_command(int mode, const CcloverCommand *cmd) {
    int is_static = (cmd->flags & CCLOVER_STATIC_CONTENT) != 0;
    return mode == CCLOVER_DRAW_ALL ||
           (mode == CCLOVER_DRAW_STATIC && is_static) ||
           (mode == CCLOVER_DRAW_DYNAMIC && !is_static);
}

static void cclover_draw_scene(CcloverHost *host, cairo_t *cr, const CcloverScene *scene,
                               int clear, int mode) {
    size_t i, j;
    uint32_t font_size = 0;
    uint32_t font_bold = 0;
    int font_valid = 0;
    cairo_save(cr);
    if (clear) {
        cairo_set_operator(cr, CAIRO_OPERATOR_SOURCE);
        cairo_set_source_rgba(cr, 0, 0, 0, 0);
        cairo_paint(cr);
    }
    cairo_set_operator(cr, CAIRO_OPERATOR_OVER);

    for (i = 0; i < scene->command_count; ++i) {
        const CcloverCommand *cmd = &scene->commands[i];
        if (!cclover_draw_command(mode, cmd)) continue;
        if (cmd->kind == CCLOVER_CMD_FILL_RECT) {
            cclover_round_rect(cr, cmd);
            cclover_source_argb(cr, cmd->color);
            cairo_fill(cr);
        } else if (cmd->kind == CCLOVER_CMD_STROKE_RECT) {
            cclover_round_rect(cr, cmd);
            cclover_source_argb(cr, cmd->color);
            cairo_set_line_width(cr, cmd->stroke_width);
            cairo_stroke(cr);
        } else if (cmd->kind == CCLOVER_CMD_TEXT) {
            cclover_draw_text(host, cr, cmd, &font_size, &font_bold, &font_valid);
        } else if ((cmd->kind == CCLOVER_CMD_POLYLINE ||
                    cmd->kind == CCLOVER_CMD_POLYGON) &&
                   cmd->point_count > 0 &&
                   cmd->point_offset + cmd->point_count <= scene->point_count) {
            const CcloverPoint *first = &scene->points[cmd->point_offset];
            cairo_move_to(cr, first->x, first->y);
            for (j = 1; j < cmd->point_count; ++j) {
                const CcloverPoint *point = &scene->points[cmd->point_offset + j];
                cairo_line_to(cr, point->x, point->y);
            }
            cclover_source_argb(cr, cmd->color);
            if (cmd->kind == CCLOVER_CMD_POLYGON) {
                cairo_close_path(cr);
                cairo_fill(cr);
            } else {
                cairo_set_line_width(cr, cmd->stroke_width);
                cairo_stroke(cr);
            }
        }
    }
    cairo_restore(cr);
}

static void cclover_scene(CcloverHost *host, CcloverScene *scene) {
    memset(scene, 0, sizeof(*scene));
    host->callbacks->scene(host->context, host, cclover_measure_text, scene);
}

/* ---------------- X11 host ---------------- */

static Atom x11_atom(Display *display, const char *name) {
    return XInternAtom(display, name, False);
}

static void x11_state(Display *display, Window window, Atom state) {
    XEvent event;
    memset(&event, 0, sizeof(event));
    event.xclient.type = ClientMessage;
    event.xclient.window = window;
    event.xclient.message_type = x11_atom(display, "_NET_WM_STATE");
    event.xclient.format = 32;
    event.xclient.data.l[0] = 1;
    event.xclient.data.l[1] = (long)state;
    XSendEvent(display, DefaultRootWindow(display), False,
               SubstructureRedirectMask | SubstructureNotifyMask, &event);
}

static void x11_place(Display *display, Window window, uint32_t width, uint32_t height) {
    int screen = DefaultScreen(display);
    int x = DisplayWidth(display, screen) - (int)width - CCLOVER_MARGIN;
    Atom moveresize = x11_atom(display, "_NET_MOVERESIZE_WINDOW");
    XEvent event;
    if (x < 0) x = 0;
    XMoveResizeWindow(display, window, x, CCLOVER_MARGIN, width, height);
    XLowerWindow(display, window);

    memset(&event, 0, sizeof(event));
    event.xclient.type = ClientMessage;
    event.xclient.window = window;
    event.xclient.message_type = moveresize;
    event.xclient.format = 32;
    event.xclient.data.l[0] = 10 | (1 << 8) | (1 << 9) | (1 << 12);
    event.xclient.data.l[1] = x;
    event.xclient.data.l[2] = CCLOVER_MARGIN;
    XSendEvent(display, DefaultRootWindow(display), False,
               SubstructureRedirectMask | SubstructureNotifyMask, &event);
}

static void x11_policy(Display *display, Window window) {
    typedef struct {
        unsigned long flags;
        unsigned long functions;
        unsigned long decorations;
        long input_mode;
        unsigned long status;
    } MotifHints;
    Atom states[3];
    Atom wm_state = x11_atom(display, "_NET_WM_STATE");
    Atom wm_desktop = x11_atom(display, "_NET_WM_DESKTOP");
    Atom motif_hints = x11_atom(display, "_MOTIF_WM_HINTS");
    unsigned long all_desktops = 0xffffffffUL;
    MotifHints motif = { 2, 0, 0, 0, 0 };
    XWMHints *hints = XAllocWMHints();

    states[0] = x11_atom(display, "_NET_WM_STATE_SKIP_TASKBAR");
    states[1] = x11_atom(display, "_NET_WM_STATE_SKIP_PAGER");
    states[2] = x11_atom(display, "_NET_WM_STATE_BELOW");
    XChangeProperty(display, window, wm_state, XA_ATOM, 32, PropModeReplace,
                    (unsigned char *)states, 3);
    XChangeProperty(display, window, wm_desktop, XA_CARDINAL, 32, PropModeReplace,
                    (unsigned char *)&all_desktops, 1);
    XChangeProperty(display, window, motif_hints, motif_hints, 32, PropModeReplace,
                    (unsigned char *)&motif, 5);
    if (hints) {
        hints->flags = InputHint;
        hints->input = False;
        XSetWMHints(display, window, hints);
        XFree(hints);
    }
    XShapeCombineRectangles(display, window, ShapeInput, 0, 0, NULL, 0,
                            ShapeSet, Unsorted);
}

int cclover_linux_x11_run(void *context, const CcloverCallbacks *callbacks) {
    CcloverHost host = { .context = context, .callbacks = callbacks };
    CcloverScene scene;
    Display *display = XOpenDisplay(NULL);
    Window window;
    cairo_surface_t *surface;
    cairo_t *cr;
    int screen, fd, running = 1, dirty = 1;
    uint32_t width, height;
    XVisualInfo visual_info;
    Visual *visual;
    int depth;
    Colormap colormap;
    XSetWindowAttributes attrs;
    if (!display) return 101;
    if (cclover_measure_init(&host) < 0) {
        cclover_measure_destroy(&host);
        XCloseDisplay(display);
        return 102;
    }

    cclover_scene(&host, &scene);
    width = scene.width;
    height = scene.height;
    screen = DefaultScreen(display);
    visual = DefaultVisual(display, screen);
    depth = DefaultDepth(display, screen);
    if (XMatchVisualInfo(display, screen, 32, TrueColor, &visual_info)) {
        XRenderPictFormat *format = XRenderFindVisualFormat(display, visual_info.visual);
        if (format && format->type == PictTypeDirect && format->direct.alphaMask) {
            visual = visual_info.visual;
            depth = visual_info.depth;
        }
    }
    colormap = XCreateColormap(display, RootWindow(display, screen), visual, AllocNone);
    memset(&attrs, 0, sizeof(attrs));
    attrs.colormap = colormap;
    attrs.border_pixel = 0;
    attrs.background_pixel = 0;
    attrs.event_mask = ExposureMask | StructureNotifyMask;
    window = XCreateWindow(display, RootWindow(display, screen), 0, 0,
                           width, height, 0, depth, InputOutput, visual,
                           CWColormap | CWBorderPixel | CWBackPixel | CWEventMask,
                           &attrs);
    XStoreName(display, window, "cclover-mon");
    x11_policy(display, window);
    XMapWindow(display, window);
    x11_place(display, window, width, height);
    x11_state(display, window, x11_atom(display, "_NET_WM_STATE_SKIP_TASKBAR"));
    x11_state(display, window, x11_atom(display, "_NET_WM_STATE_SKIP_PAGER"));
    x11_state(display, window, x11_atom(display, "_NET_WM_STATE_BELOW"));
    XFlush(display);

    surface = cairo_xlib_surface_create(display, window, visual, width, height);
    cr = cairo_create(surface);
    cclover_configure_context(cr);
    fd = ConnectionNumber(display);

    while (running) {
        struct pollfd pfd = { fd, POLLIN, 0 };
        uint32_t status = callbacks->poll(context);
        if (status & CCLOVER_POLL_QUIT) break;
        if (status & CCLOVER_POLL_FRAME) {
            cclover_scene(&host, &scene);
            if (scene.width != width || scene.height != height) {
                width = scene.width;
                height = scene.height;
                x11_place(display, window, width, height);
                cairo_xlib_surface_set_size(surface, width, height);
            }
            dirty = 1;
        }
        while (XPending(display)) {
            XEvent event;
            XNextEvent(display, &event);
            if (event.type == Expose) dirty = 1;
            if (event.type == DestroyNotify) running = 0;
        }
        if (!running) break;
        if (dirty) {
            cclover_draw_scene(&host, cr, &scene, 1, CCLOVER_DRAW_ALL);
            cairo_surface_flush(surface);
            XFlush(display);
            dirty = 0;
        }
        poll(&pfd, 1, 100);
    }

    cairo_destroy(cr);
    cairo_surface_destroy(surface);
    XDestroyWindow(display, window);
    XFreeColormap(display, colormap);
    XCloseDisplay(display);
    cclover_measure_destroy(&host);
    return 0;
}

/* ---------------- Wayland host ---------------- */

#define CCLOVER_MAX_OUTPUTS 16
#define CCLOVER_WAYLAND_BUFFER_COUNT 2

typedef struct WaylandHost WaylandHost;

typedef struct {
    struct wl_output *output;
    WaylandHost *host;
    int32_t scale;
    int entered;
} WaylandOutput;

typedef struct {
    struct wl_display *display;
    struct wl_compositor *compositor;
    struct wl_shm *shm;
    struct zwlr_layer_shell_v1 *layer_shell;
    WaylandOutput outputs[CCLOVER_MAX_OUTPUTS];
    size_t output_count;
    WaylandHost *host;
} WaylandGlobals;

typedef struct WaylandBuffer {
    struct wl_buffer *buffer;
    void *data;
    size_t size;
    cairo_surface_t *image;
    cairo_t *cr;
    uint32_t width;
    uint32_t height;
    int32_t scale;
    int busy;
} WaylandBuffer;

typedef struct {
    void *data;
    size_t size;
    cairo_surface_t *image;
    cairo_t *cr;
    uint32_t width;
    uint32_t height;
    int32_t scale;
    uint64_t revision;
} WaylandStaticLayer;

struct WaylandHost {
    CcloverHost host;
    WaylandGlobals globals;
    struct wl_surface *surface;
    struct zwlr_layer_surface_v1 *layer_surface;
    int configured;
    int closed;
    int dirty;
    int32_t scale;
    uint32_t width;
    uint32_t height;
    WaylandBuffer buffers[CCLOVER_WAYLAND_BUFFER_COUNT];
    WaylandBuffer *previous_buffer;
    WaylandStaticLayer static_layer;
};

static void wayland_update_scale(WaylandHost *host) {
    size_t i;
    int32_t scale = 1;
    for (i = 0; i < host->globals.output_count; ++i) {
        WaylandOutput *output = &host->globals.outputs[i];
        if (output->entered && output->scale > scale)
            scale = output->scale;
    }
    if (scale != host->scale) {
        host->scale = scale;
        host->dirty = 1;
    }
}

static void wayland_set_initial_scale(WaylandHost *host) {
    size_t i;
    int32_t scale = 1;
    for (i = 0; i < host->globals.output_count; ++i) {
        if (host->globals.outputs[i].scale > scale)
            scale = host->globals.outputs[i].scale;
    }
    host->scale = scale;
}

static void output_geometry(void *data, struct wl_output *output, int32_t x, int32_t y,
                            int32_t physical_width, int32_t physical_height,
                            int32_t subpixel, const char *make, const char *model,
                            int32_t transform) {
    (void)data; (void)output; (void)x; (void)y; (void)physical_width;
    (void)physical_height; (void)subpixel; (void)make; (void)model; (void)transform;
}

static void output_mode(void *data, struct wl_output *output, uint32_t flags,
                        int32_t width, int32_t height, int32_t refresh) {
    (void)data; (void)output; (void)flags; (void)width; (void)height; (void)refresh;
}

static void output_done(void *data, struct wl_output *output) {
    (void)data; (void)output;
}

static void output_scale(void *data, struct wl_output *output, int32_t factor) {
    WaylandOutput *entry = data;
    (void)output;
    entry->scale = factor > 0 ? factor : 1;
    if (entry->host) wayland_update_scale(entry->host);
}

static void output_name(void *data, struct wl_output *output, const char *name) {
    (void)data; (void)output; (void)name;
}

static void output_description(void *data, struct wl_output *output,
                               const char *description) {
    (void)data; (void)output; (void)description;
}

static const struct wl_output_listener output_listener = {
    output_geometry, output_mode, output_done, output_scale,
    output_name, output_description
};

static void registry_global(void *data, struct wl_registry *registry, uint32_t name,
                            const char *interface, uint32_t version) {
    WaylandGlobals *g = data;
    if (strcmp(interface, wl_compositor_interface.name) == 0) {
        g->compositor = wl_registry_bind(registry, name, &wl_compositor_interface,
                                         version < 4 ? version : 4);
    } else if (strcmp(interface, wl_shm_interface.name) == 0) {
        g->shm = wl_registry_bind(registry, name, &wl_shm_interface, 1);
    } else if (strcmp(interface, zwlr_layer_shell_v1_interface.name) == 0) {
        g->layer_shell = wl_registry_bind(registry, name,
                                           &zwlr_layer_shell_v1_interface,
                                           version < 4 ? version : 4);
    } else if (strcmp(interface, wl_output_interface.name) == 0 &&
               g->output_count < CCLOVER_MAX_OUTPUTS) {
        WaylandOutput *entry = &g->outputs[g->output_count++];
        entry->host = g->host;
        entry->scale = 1;
        entry->output = wl_registry_bind(registry, name, &wl_output_interface,
                                         version < 4 ? version : 4);
        wl_output_add_listener(entry->output, &output_listener, entry);
    }
}

static void registry_remove(void *data, struct wl_registry *registry, uint32_t name) {
    (void)data; (void)registry; (void)name;
}

static const struct wl_registry_listener registry_listener = {
    registry_global, registry_remove
};

static void surface_enter(void *data, struct wl_surface *surface, struct wl_output *output) {
    WaylandHost *host = data;
    size_t i;
    (void)surface;
    for (i = 0; i < host->globals.output_count; ++i) {
        if (host->globals.outputs[i].output == output) {
            host->globals.outputs[i].entered = 1;
            break;
        }
    }
    wayland_update_scale(host);
}

static void surface_leave(void *data, struct wl_surface *surface, struct wl_output *output) {
    WaylandHost *host = data;
    size_t i;
    (void)surface;
    for (i = 0; i < host->globals.output_count; ++i) {
        if (host->globals.outputs[i].output == output) {
            host->globals.outputs[i].entered = 0;
            break;
        }
    }
    wayland_update_scale(host);
}

static void surface_preferred_buffer_scale(void *data, struct wl_surface *surface,
                                           int32_t factor) {
    (void)data; (void)surface; (void)factor;
}

static void surface_preferred_buffer_transform(void *data, struct wl_surface *surface,
                                               uint32_t transform) {
    (void)data; (void)surface; (void)transform;
}

static const struct wl_surface_listener surface_listener = {
    surface_enter, surface_leave, surface_preferred_buffer_scale,
    surface_preferred_buffer_transform
};

static void layer_configure(void *data, struct zwlr_layer_surface_v1 *layer,
                            uint32_t serial, uint32_t width, uint32_t height) {
    WaylandHost *host = data;
    (void)width; (void)height;
    zwlr_layer_surface_v1_ack_configure(layer, serial);
    host->configured = 1;
    host->dirty = 1;
}

static void layer_closed(void *data, struct zwlr_layer_surface_v1 *layer) {
    WaylandHost *host = data;
    (void)layer;
    host->closed = 1;
}

static const struct zwlr_layer_surface_v1_listener layer_listener = {
    layer_configure, layer_closed
};

static void buffer_release(void *data, struct wl_buffer *buffer) {
    WaylandBuffer *owned = data;
    (void)buffer;
    owned->busy = 0;
}

static const struct wl_buffer_listener buffer_listener = { buffer_release };

static int create_shm_file(size_t size) {
    char name[64];
    int fd;
    snprintf(name, sizeof(name), "/cclover-mon-%ld-%u", (long)getpid(),
             (unsigned)rand());
    fd = shm_open(name, O_RDWR | O_CREAT | O_EXCL, 0600);
    if (fd < 0) return -1;
    shm_unlink(name);
    if (ftruncate(fd, (off_t)size) < 0) {
        close(fd);
        return -1;
    }
    return fd;
}

static void wayland_buffer_destroy(WaylandBuffer *owned) {
    if (owned->cr) cairo_destroy(owned->cr);
    if (owned->image) cairo_surface_destroy(owned->image);
    if (owned->buffer) wl_buffer_destroy(owned->buffer);
    if (owned->data && owned->data != MAP_FAILED) munmap(owned->data, owned->size);
    memset(owned, 0, sizeof(*owned));
}

static void wayland_static_layer_destroy(WaylandStaticLayer *layer) {
    if (layer->cr) cairo_destroy(layer->cr);
    if (layer->image) cairo_surface_destroy(layer->image);
    free(layer->data);
    memset(layer, 0, sizeof(*layer));
}

static int wayland_static_layer_ensure(WaylandHost *host, const CcloverScene *scene,
                                       int32_t scale, int *rebuilt) {
    WaylandStaticLayer *layer = &host->static_layer;
    *rebuilt = 0;
    uint32_t buffer_width = scene->width * (uint32_t)scale;
    uint32_t buffer_height = scene->height * (uint32_t)scale;
    int stride = (int)buffer_width * 4;
    size_t size = (size_t)stride * buffer_height;
    if (layer->data && layer->width == scene->width && layer->height == scene->height &&
        layer->scale == scale && layer->revision == scene->static_revision)
        return 0;

    wayland_static_layer_destroy(layer);
    layer->data = calloc(1, size);
    if (!layer->data) return -1;
    layer->size = size;
    layer->width = scene->width;
    layer->height = scene->height;
    layer->scale = scale;
    layer->revision = scene->static_revision;
    layer->image = cairo_image_surface_create_for_data(
        layer->data, CAIRO_FORMAT_ARGB32, buffer_width, buffer_height, stride);
    if (cairo_surface_status(layer->image) != CAIRO_STATUS_SUCCESS) {
        wayland_static_layer_destroy(layer);
        return -1;
    }
    layer->cr = cairo_create(layer->image);
    if (cairo_status(layer->cr) != CAIRO_STATUS_SUCCESS) {
        wayland_static_layer_destroy(layer);
        return -1;
    }
    cclover_configure_context(layer->cr);
    cairo_scale(layer->cr, scale, scale);
    cclover_draw_scene(&host->host, layer->cr, scene, 0, CCLOVER_DRAW_STATIC);
    cairo_surface_flush(layer->image);
    *rebuilt = 1;
    return 0;
}

static int wayland_buffer_create(WaylandHost *host, WaylandBuffer *owned,
                                 const CcloverScene *scene, int32_t scale) {
    uint32_t buffer_width = scene->width * (uint32_t)scale;
    uint32_t buffer_height = scene->height * (uint32_t)scale;
    int stride = (int)buffer_width * 4;
    size_t size = (size_t)stride * buffer_height;
    int fd = create_shm_file(size);
    struct wl_shm_pool *pool;
    if (fd < 0) return -1;

    owned->size = size;
    owned->width = scene->width;
    owned->height = scene->height;
    owned->scale = scale;
    owned->data = mmap(NULL, size, PROT_READ | PROT_WRITE, MAP_SHARED, fd, 0);
    if (owned->data == MAP_FAILED) {
        owned->data = NULL;
        close(fd);
        wayland_buffer_destroy(owned);
        return -1;
    }

    pool = wl_shm_create_pool(host->globals.shm, fd, (int)size);
    owned->buffer = wl_shm_pool_create_buffer(pool, 0, buffer_width, buffer_height,
                                              stride, WL_SHM_FORMAT_ARGB8888);
    wl_shm_pool_destroy(pool);
    close(fd);
    if (!owned->buffer) {
        wayland_buffer_destroy(owned);
        return -1;
    }

    owned->image = cairo_image_surface_create_for_data(
        owned->data, CAIRO_FORMAT_ARGB32, buffer_width, buffer_height, stride);
    if (cairo_surface_status(owned->image) != CAIRO_STATUS_SUCCESS) {
        wayland_buffer_destroy(owned);
        return -1;
    }
    owned->cr = cairo_create(owned->image);
    if (cairo_status(owned->cr) != CAIRO_STATUS_SUCCESS) {
        wayland_buffer_destroy(owned);
        return -1;
    }
    cclover_configure_context(owned->cr);
    cairo_scale(owned->cr, scale, scale);
    wl_buffer_add_listener(owned->buffer, &buffer_listener, owned);
    return 0;
}

static int wayland_buffer_acquire(WaylandHost *host, const CcloverScene *scene,
                                  int32_t scale, WaylandBuffer **out) {
    size_t i;
    if (host->previous_buffer && !host->previous_buffer->busy &&
        host->previous_buffer->buffer &&
        host->previous_buffer->width == scene->width &&
        host->previous_buffer->height == scene->height &&
        host->previous_buffer->scale == scale) {
        *out = host->previous_buffer;
        return 0;
    }
    for (i = 0; i < CCLOVER_WAYLAND_BUFFER_COUNT; ++i) {
        WaylandBuffer *owned = &host->buffers[i];
        if (owned->busy) continue;
        if (owned->buffer &&
            (owned->width != scene->width || owned->height != scene->height ||
             owned->scale != scale)) {
            wayland_buffer_destroy(owned);
        }
        if (!owned->buffer && wayland_buffer_create(host, owned, scene, scale) < 0)
            return -1;
        *out = owned;
        return 0;
    }
    return 1;
}

static void wayland_restore_rect(WaylandHost *host, WaylandBuffer *owned,
                                 CcloverDamageRect rect, int32_t scale) {
    int buffer_width = (int)(host->width * (uint32_t)scale);
    int buffer_height = (int)(host->height * (uint32_t)scale);
    int x1 = (int)floorf(rect.x1 * scale);
    int y1 = (int)floorf(rect.y1 * scale);
    int x2 = (int)ceilf(rect.x2 * scale);
    int y2 = (int)ceilf(rect.y2 * scale);
    int stride = buffer_width * 4;
    int y;
    if (x1 < 0) x1 = 0;
    if (y1 < 0) y1 = 0;
    if (x2 > buffer_width) x2 = buffer_width;
    if (y2 > buffer_height) y2 = buffer_height;
    if (x2 <= x1 || y2 <= y1) return;
    for (y = y1; y < y2; ++y) {
        memcpy((uint8_t *)owned->data + (size_t)y * stride + (size_t)x1 * 4,
               (uint8_t *)host->static_layer.data + (size_t)y * stride + (size_t)x1 * 4,
               (size_t)(x2 - x1) * 4);
    }
    cairo_surface_mark_dirty_rectangle(owned->image, x1, y1, x2 - x1, y2 - y1);
}

static int wayland_draw(WaylandHost *host, const CcloverScene *scene) {
    CcloverDamageRect full_damage = { 0.0f, 0.0f, (float)scene->width, (float)scene->height };
    const CcloverDamageRect *dirty = scene->damage_rects;
    size_t dirty_count = scene->damage_count;
    int32_t scale = host->scale > 0 ? host->scale : 1;
    int profiling = getenv("CCLOVER_RENDER_PROFILE") != NULL;
    double profile_started = profiling ? cclover_profile_cpu_ms() : 0.0;
    double profile_dynamic = profiling ? cclover_profile_cpu_ms() : 0.0;
    double profile_static = 0.0;
    double profile_raster = 0.0;
    size_t i;
    int full_redraw = scene->full_redraw != 0;
    int static_rebuilt = 0;
    WaylandBuffer *owned;
    int acquired;

    if (wayland_static_layer_ensure(host, scene, scale, &static_rebuilt) < 0) return -1;
    if (static_rebuilt) full_redraw = 1;
    if (profiling) profile_static = cclover_profile_cpu_ms();
    if (full_redraw) {
        dirty = &full_damage;
        dirty_count = 1;
    } else if (dirty_count == 0) {
        return 0;
    }

    acquired = wayland_buffer_acquire(host, scene, scale, &owned);
    if (acquired != 0) return acquired;
    cairo_surface_flush(owned->image);

    if (full_redraw) {
        memcpy(owned->data, host->static_layer.data, owned->size);
        cairo_surface_mark_dirty(owned->image);
    } else {
        if (owned != host->previous_buffer) {
            if (host->previous_buffer && host->previous_buffer->size == owned->size) {
                memcpy(owned->data, host->previous_buffer->data, owned->size);
                cairo_surface_mark_dirty(owned->image);
            } else {
                memcpy(owned->data, host->static_layer.data, owned->size);
                cairo_surface_mark_dirty(owned->image);
                full_redraw = 1;
                dirty = &full_damage;
                dirty_count = 1;
            }
        }
        if (!full_redraw) {
            for (i = 0; i < dirty_count; ++i)
                wayland_restore_rect(host, owned, dirty[i], scale);
        }
    }

    cairo_save(owned->cr);
    for (i = 0; i < dirty_count; ++i) {
        cairo_rectangle(owned->cr, dirty[i].x1, dirty[i].y1,
                        dirty[i].x2 - dirty[i].x1, dirty[i].y2 - dirty[i].y1);
    }
    cairo_clip(owned->cr);
    cclover_draw_scene(&host->host, owned->cr, scene, 0, CCLOVER_DRAW_DYNAMIC);
    cairo_restore(owned->cr);
    cairo_surface_flush(owned->image);
    if (profiling) profile_raster = cclover_profile_cpu_ms();

    wl_surface_set_buffer_scale(host->surface, scale);
    wl_surface_attach(host->surface, owned->buffer, 0, 0);
    for (i = 0; i < dirty_count; ++i) {
        int x = (int)floorf(dirty[i].x1);
        int y = (int)floorf(dirty[i].y1);
        int width = (int)ceilf(dirty[i].x2) - x;
        int height = (int)ceilf(dirty[i].y2) - y;
        wl_surface_damage(host->surface, x, y, width, height);
    }
    owned->busy = 1;
    wl_surface_commit(host->surface);
    wl_display_flush(host->globals.display);
    host->previous_buffer = owned;
    if (profiling) {
        double dirty_area = 0.0;
        for (i = 0; i < dirty_count; ++i)
            dirty_area += (dirty[i].x2 - dirty[i].x1) * (dirty[i].y2 - dirty[i].y1);
        fprintf(stderr,
                "drawparts dynamic=%.3f static=%.3f raster=%.3f submit=%.3fms dirty=%zu area=%.0f\n",
                profile_dynamic - profile_started,
                profile_static - profile_dynamic,
                profile_raster - profile_static,
                cclover_profile_cpu_ms() - profile_raster,
                dirty_count, dirty_area);
    }
    return 0;
}

int cclover_linux_wayland_run(void *context, const CcloverCallbacks *callbacks) {
    WaylandHost host;
    CcloverScene scene;
    struct wl_registry *registry;
    struct wl_region *empty_input;
    size_t i;
    int fd;
    memset(&host, 0, sizeof(host));
    host.host.context = context;
    host.host.callbacks = callbacks;
    host.scale = 1;
    host.globals.host = &host;
    host.globals.display = wl_display_connect(NULL);
    if (!host.globals.display) return 201;
    registry = wl_display_get_registry(host.globals.display);
    wl_registry_add_listener(registry, &registry_listener, &host.globals);
    if (wl_display_roundtrip(host.globals.display) < 0 || !host.globals.compositor ||
        !host.globals.shm || !host.globals.layer_shell) {
        wl_registry_destroy(registry);
        wl_display_disconnect(host.globals.display);
        return 202;
    }
    /* Collect initial wl_output.scale events before the surface enters an output. */
    wl_display_roundtrip(host.globals.display);
    wayland_set_initial_scale(&host);
    if (cclover_measure_init(&host.host) < 0) {
        cclover_measure_destroy(&host.host);
        wl_registry_destroy(registry);
        wl_display_disconnect(host.globals.display);
        return 203;
    }

    cclover_scene(&host.host, &scene);
    host.width = scene.width;
    host.height = scene.height;
    host.surface = wl_compositor_create_surface(host.globals.compositor);
    wl_surface_add_listener(host.surface, &surface_listener, &host);
    host.layer_surface = zwlr_layer_shell_v1_get_layer_surface(
        host.globals.layer_shell, host.surface, NULL,
        ZWLR_LAYER_SHELL_V1_LAYER_BOTTOM, "cclover-mon");
    zwlr_layer_surface_v1_add_listener(host.layer_surface, &layer_listener, &host);
    zwlr_layer_surface_v1_set_anchor(host.layer_surface,
        ZWLR_LAYER_SURFACE_V1_ANCHOR_TOP | ZWLR_LAYER_SURFACE_V1_ANCHOR_RIGHT);
    zwlr_layer_surface_v1_set_margin(host.layer_surface, CCLOVER_MARGIN,
                                     CCLOVER_MARGIN, 0, 0);
    zwlr_layer_surface_v1_set_exclusive_zone(host.layer_surface, 0);
    zwlr_layer_surface_v1_set_keyboard_interactivity(
        host.layer_surface, ZWLR_LAYER_SURFACE_V1_KEYBOARD_INTERACTIVITY_NONE);
    zwlr_layer_surface_v1_set_size(host.layer_surface, host.width, host.height);
    empty_input = wl_compositor_create_region(host.globals.compositor);
    wl_surface_set_input_region(host.surface, empty_input);
    wl_region_destroy(empty_input);
    wl_surface_commit(host.surface);

    while (!host.configured && !host.closed) {
        if (wl_display_dispatch(host.globals.display) < 0) {
            host.closed = 1;
            break;
        }
    }
    host.dirty = 1;
    fd = wl_display_get_fd(host.globals.display);

    while (!host.closed) {
        struct pollfd pfd = { fd, POLLIN, 0 };
        uint32_t status = callbacks->poll(context);
        if (status & CCLOVER_POLL_QUIT) break;
        if (status & CCLOVER_POLL_FRAME) {
            int profiling = getenv("CCLOVER_RENDER_PROFILE") != NULL;
            double profile_started = profiling ? cclover_profile_cpu_ms() : 0.0;
            cclover_scene(&host.host, &scene);
            if (profiling)
                fprintf(stderr, "scene cpu=%.3fms\n",
                        cclover_profile_cpu_ms() - profile_started);
            if (scene.width != host.width || scene.height != host.height) {
                host.width = scene.width;
                host.height = scene.height;
                host.configured = 0;
                zwlr_layer_surface_v1_set_size(host.layer_surface, host.width, host.height);
                wl_surface_commit(host.surface);
            }
            host.dirty = 1;
        }
        if (host.configured && host.dirty) {
            int draw_result;
            int profiling = getenv("CCLOVER_RENDER_PROFILE") != NULL;
            double profile_started = profiling ? cclover_profile_cpu_ms() : 0.0;
            draw_result = wayland_draw(&host, &scene);
            if (profiling)
                fprintf(stderr, "draw cpu=%.3fms\n",
                        cclover_profile_cpu_ms() - profile_started);
            if (draw_result < 0) {
                host.closed = 1;
                break;
            }
            if (draw_result == 0) host.dirty = 0;
        }

        wl_display_flush(host.globals.display);
        if (poll(&pfd, 1, 100) > 0 && (pfd.revents & POLLIN)) {
            if (wl_display_dispatch(host.globals.display) < 0) break;
        } else {
            wl_display_dispatch_pending(host.globals.display);
        }
    }

    if (host.layer_surface) zwlr_layer_surface_v1_destroy(host.layer_surface);
    if (host.surface) wl_surface_destroy(host.surface);
    wl_display_roundtrip(host.globals.display);
    for (i = 0; i < CCLOVER_WAYLAND_BUFFER_COUNT; ++i)
        wayland_buffer_destroy(&host.buffers[i]);
    wayland_static_layer_destroy(&host.static_layer);
    for (i = 0; i < host.globals.output_count; ++i) {
        if (host.globals.outputs[i].output)
            wl_output_destroy(host.globals.outputs[i].output);
    }
    if (host.globals.layer_shell) zwlr_layer_shell_v1_destroy(host.globals.layer_shell);
    if (host.globals.shm) wl_shm_destroy(host.globals.shm);
    if (host.globals.compositor) wl_compositor_destroy(host.globals.compositor);
    wl_registry_destroy(registry);
    wl_display_roundtrip(host.globals.display);
    wl_display_disconnect(host.globals.display);
    cclover_measure_destroy(&host.host);
    return 0;
}
