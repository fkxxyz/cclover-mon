#ifndef CCLOVER_WAYLAND_LIFECYCLE_H
#define CCLOVER_WAYLAND_LIFECYCLE_H

#include "wayland_buffer_contract.h"

#include <stddef.h>
#include <stdint.h>

#define CCLOVER_WAYLAND_MAX_OUTPUTS 16

struct wl_output;

typedef enum {
    WAYLAND_SURFACE_ABSENT,
    WAYLAND_SURFACE_WAITING_CONFIGURE,
    WAYLAND_SURFACE_ACTIVE,
    WAYLAND_SURFACE_RECREATE_PENDING,
} WaylandSurfaceState;

typedef struct {
    WaylandSurfaceState surface_state;
    int dirty;
} WaylandLifecycle;

typedef struct {
    struct wl_output *output;
    uint32_t global_name;
    int32_t scale;
    int entered;
} WaylandOutput;

void cclover_wayland_lifecycle_init(WaylandLifecycle *lifecycle);
void cclover_wayland_surface_created(WaylandLifecycle *lifecycle);
void cclover_wayland_surface_configured(WaylandLifecycle *lifecycle);
void cclover_wayland_surface_closed(WaylandLifecycle *lifecycle);
void cclover_wayland_surface_destroyed(WaylandLifecycle *lifecycle,
                                       CcloverWaylandBufferBaseline *baseline);
void cclover_wayland_state_status(WaylandLifecycle *lifecycle, uint32_t status,
                                  uint32_t changed_flag);
void cclover_wayland_state_changed(WaylandLifecycle *lifecycle);
void cclover_wayland_resize_requested(WaylandLifecycle *lifecycle);
int cclover_wayland_needs_recreate(const WaylandLifecycle *lifecycle);
int cclover_wayland_can_draw(const WaylandLifecycle *lifecycle);
void cclover_wayland_draw_completed(WaylandLifecycle *lifecycle, int completed);

WaylandOutput *cclover_wayland_output_find_free(WaylandOutput *outputs, size_t count);
WaylandOutput *cclover_wayland_output_find_global(WaylandOutput *outputs, size_t count,
                                                  uint32_t global_name);
WaylandOutput *cclover_wayland_output_find_resource(WaylandOutput *outputs, size_t count,
                                                    const struct wl_output *output);
void cclover_wayland_output_reset_state(WaylandOutput *output);
int32_t cclover_wayland_output_scale(const WaylandOutput *outputs, size_t count,
                                     int entered_only);

#endif
