#include "wayland_lifecycle.h"

#include <string.h>

void cclover_wayland_lifecycle_init(WaylandLifecycle *lifecycle) {
    lifecycle->surface_state = WAYLAND_SURFACE_ABSENT;
    lifecycle->dirty = 0;
}

void cclover_wayland_surface_created(WaylandLifecycle *lifecycle) {
    lifecycle->surface_state = WAYLAND_SURFACE_WAITING_CONFIGURE;
    lifecycle->dirty = 1;
}

void cclover_wayland_surface_configured(WaylandLifecycle *lifecycle) {
    if (lifecycle->surface_state != WAYLAND_SURFACE_ABSENT &&
        lifecycle->surface_state != WAYLAND_SURFACE_RECREATE_PENDING) {
        lifecycle->surface_state = WAYLAND_SURFACE_ACTIVE;
    }
    lifecycle->dirty = 1;
}

void cclover_wayland_surface_closed(WaylandLifecycle *lifecycle) {
    if (lifecycle->surface_state != WAYLAND_SURFACE_ABSENT)
        lifecycle->surface_state = WAYLAND_SURFACE_RECREATE_PENDING;
}

void cclover_wayland_surface_destroyed(WaylandLifecycle *lifecycle,
                                       CcloverWaylandBufferBaseline *baseline) {
    cclover_wayland_buffer_baseline_reset(baseline);
    lifecycle->surface_state = WAYLAND_SURFACE_ABSENT;
    lifecycle->dirty = 1;
}

void cclover_wayland_state_status(WaylandLifecycle *lifecycle, uint32_t status,
                                  uint32_t changed_flag) {
    if (status & changed_flag) lifecycle->dirty = 1;
}

void cclover_wayland_state_changed(WaylandLifecycle *lifecycle) {
    lifecycle->dirty = 1;
}

void cclover_wayland_resize_requested(WaylandLifecycle *lifecycle) {
    if (lifecycle->surface_state != WAYLAND_SURFACE_ABSENT &&
        lifecycle->surface_state != WAYLAND_SURFACE_RECREATE_PENDING) {
        lifecycle->surface_state = WAYLAND_SURFACE_WAITING_CONFIGURE;
    }
    lifecycle->dirty = 1;
}

int cclover_wayland_needs_recreate(const WaylandLifecycle *lifecycle) {
    return lifecycle->surface_state == WAYLAND_SURFACE_RECREATE_PENDING;
}

int cclover_wayland_can_draw(const WaylandLifecycle *lifecycle) {
    return lifecycle->surface_state == WAYLAND_SURFACE_ACTIVE && lifecycle->dirty;
}

void cclover_wayland_draw_completed(WaylandLifecycle *lifecycle, int completed) {
    if (completed) lifecycle->dirty = 0;
}

WaylandOutput *cclover_wayland_output_find_free(WaylandOutput *outputs, size_t count) {
    size_t i;
    for (i = 0; i < count; ++i) {
        if (outputs[i].output == NULL) return &outputs[i];
    }
    return NULL;
}

WaylandOutput *cclover_wayland_output_find_global(WaylandOutput *outputs, size_t count,
                                                  uint32_t global_name) {
    size_t i;
    for (i = 0; i < count; ++i) {
        if (outputs[i].output != NULL && outputs[i].global_name == global_name)
            return &outputs[i];
    }
    return NULL;
}

WaylandOutput *cclover_wayland_output_find_resource(WaylandOutput *outputs, size_t count,
                                                    const struct wl_output *output) {
    size_t i;
    if (output == NULL) return NULL;
    for (i = 0; i < count; ++i) {
        if (outputs[i].output == output) return &outputs[i];
    }
    return NULL;
}

void cclover_wayland_output_reset_state(WaylandOutput *output) {
    memset(output, 0, sizeof(*output));
}

int32_t cclover_wayland_output_scale(const WaylandOutput *outputs, size_t count,
                                     int entered_only) {
    size_t i;
    int32_t scale = 1;
    for (i = 0; i < count; ++i) {
        const WaylandOutput *output = &outputs[i];
        if (output->output == NULL || (entered_only && !output->entered)) continue;
        if (output->scale > scale) scale = output->scale;
    }
    return scale;
}
