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

