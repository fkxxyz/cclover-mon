/* ---------------- Wayland host ---------------- */

#define CCLOVER_WAYLAND_BUFFER_COUNT 2

typedef struct {
    struct wl_display *display;
    struct wl_compositor *compositor;
    struct wl_shm *shm;
    struct zwlr_layer_shell_v1 *layer_shell;
    WaylandOutput outputs[CCLOVER_WAYLAND_MAX_OUTPUTS];
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
    WaylandLifecycle lifecycle;
    int32_t scale;
    uint32_t width;
    uint32_t height;
    WaylandBuffer buffers[CCLOVER_WAYLAND_BUFFER_COUNT];
    WaylandBuffer *previous_buffer;
    WaylandStaticLayer static_layer;
};

static void wayland_update_scale(WaylandHost *host) {
    int32_t scale = cclover_wayland_output_scale(
        host->globals.outputs, CCLOVER_WAYLAND_MAX_OUTPUTS, 1);
    if (scale != host->scale) {
        host->scale = scale;
        cclover_wayland_state_changed(&host->lifecycle);
    }
}

static void wayland_set_initial_scale(WaylandHost *host) {
    host->scale = cclover_wayland_output_scale(
        host->globals.outputs, CCLOVER_WAYLAND_MAX_OUTPUTS, 0);
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

static void wayland_output_reset(WaylandOutput *entry) {
    if (entry->output) wl_output_destroy(entry->output);
    cclover_wayland_output_reset_state(entry);
}

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
    } else if (strcmp(interface, wl_output_interface.name) == 0) {
        WaylandOutput *entry = cclover_wayland_output_find_free(
            g->outputs, CCLOVER_WAYLAND_MAX_OUTPUTS);
        if (entry) {
            entry->host = g->host;
            entry->global_name = name;
            entry->scale = 1;
            entry->output = wl_registry_bind(registry, name, &wl_output_interface,
                                             version < 4 ? version : 4);
            if (entry->output)
                wl_output_add_listener(entry->output, &output_listener, entry);
            else
                wayland_output_reset(entry);
        }
    }
}

static void registry_remove(void *data, struct wl_registry *registry, uint32_t name) {
    WaylandGlobals *g = data;
    WaylandOutput *entry;
    (void)registry;
    entry = cclover_wayland_output_find_global(
        g->outputs, CCLOVER_WAYLAND_MAX_OUTPUTS, name);
    if (entry) {
        wayland_output_reset(entry);
        if (g->host) wayland_update_scale(g->host);
    }
}

static const struct wl_registry_listener registry_listener = {
    registry_global, registry_remove
};

static void surface_enter(void *data, struct wl_surface *surface, struct wl_output *output) {
    WaylandHost *host = data;
    WaylandOutput *entry;
    (void)surface;
    entry = cclover_wayland_output_find_resource(
        host->globals.outputs, CCLOVER_WAYLAND_MAX_OUTPUTS, output);
    if (entry) entry->entered = 1;
    wayland_update_scale(host);
}

static void surface_leave(void *data, struct wl_surface *surface, struct wl_output *output) {
    WaylandHost *host = data;
    WaylandOutput *entry;
    (void)surface;
    entry = cclover_wayland_output_find_resource(
        host->globals.outputs, CCLOVER_WAYLAND_MAX_OUTPUTS, output);
    if (entry) entry->entered = 0;
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
    if (layer != host->layer_surface) return;
    zwlr_layer_surface_v1_ack_configure(layer, serial);
    cclover_wayland_surface_configured(&host->lifecycle);
}

static void layer_closed(void *data, struct zwlr_layer_surface_v1 *layer) {
    WaylandHost *host = data;
    if (layer == host->layer_surface)
        cclover_wayland_surface_closed(&host->lifecycle);
}

static const struct zwlr_layer_surface_v1_listener layer_listener = {
    layer_configure, layer_closed
};

static void wayland_surface_destroy(WaylandHost *host) {
    if (host->layer_surface) zwlr_layer_surface_v1_destroy(host->layer_surface);
    if (host->surface) wl_surface_destroy(host->surface);
    host->layer_surface = NULL;
    host->surface = NULL;
    host->previous_buffer = NULL;
    cclover_wayland_surface_destroyed(&host->lifecycle);
}

static int wayland_surface_create(WaylandHost *host) {
    struct wl_region *empty_input;
    host->surface = wl_compositor_create_surface(host->globals.compositor);
    if (!host->surface) return -1;
    if (wl_surface_add_listener(host->surface, &surface_listener, host) < 0) {
        wayland_surface_destroy(host);
        return -1;
    }
    host->layer_surface = zwlr_layer_shell_v1_get_layer_surface(
        host->globals.layer_shell, host->surface, NULL,
        ZWLR_LAYER_SHELL_V1_LAYER_BOTTOM, "cclover-mon");
    if (!host->layer_surface) {
        wayland_surface_destroy(host);
        return -1;
    }
    if (zwlr_layer_surface_v1_add_listener(host->layer_surface, &layer_listener, host) < 0) {
        wayland_surface_destroy(host);
        return -1;
    }
    zwlr_layer_surface_v1_set_anchor(host->layer_surface,
        ZWLR_LAYER_SURFACE_V1_ANCHOR_TOP | ZWLR_LAYER_SURFACE_V1_ANCHOR_RIGHT);
    zwlr_layer_surface_v1_set_margin(host->layer_surface, CCLOVER_MARGIN,
                                     CCLOVER_MARGIN, 0, 0);
    zwlr_layer_surface_v1_set_exclusive_zone(host->layer_surface, 0);
    zwlr_layer_surface_v1_set_keyboard_interactivity(
        host->layer_surface, ZWLR_LAYER_SURFACE_V1_KEYBOARD_INTERACTIVITY_NONE);
    zwlr_layer_surface_v1_set_size(host->layer_surface, host->width, host->height);
    empty_input = wl_compositor_create_region(host->globals.compositor);
    if (!empty_input) {
        wayland_surface_destroy(host);
        return -1;
    }
    wl_surface_set_input_region(host->surface, empty_input);
    wl_region_destroy(empty_input);
    wl_surface_commit(host->surface);
    cclover_wayland_surface_created(&host->lifecycle);
    return 0;
}

static int wayland_surface_recreate(WaylandHost *host) {
    wayland_surface_destroy(host);
    return wayland_surface_create(host);
}

static void buffer_release(void *data, struct wl_buffer *buffer) {
    WaylandBuffer *owned = data;
    (void)buffer;
    owned->busy = 0;
}

static const struct wl_buffer_listener buffer_listener = { buffer_release };
