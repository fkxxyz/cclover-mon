#ifndef CCLOVER_LINUX_WAYLAND_BUFFER_POLICY_H
#define CCLOVER_LINUX_WAYLAND_BUFFER_POLICY_H

#include <stddef.h>
#include <stdint.h>

typedef enum {
    CCLOVER_WAYLAND_DRAW_SKIP = 0,
    CCLOVER_WAYLAND_DRAW_INCREMENTAL = 1,
    CCLOVER_WAYLAND_DRAW_FULL = 2,
} CcloverWaylandDrawMode;

static inline int cclover_wayland_static_layer_reusable(
    int has_data, uint32_t current_width, uint32_t current_height,
    int32_t current_scale, uint64_t current_revision, uint32_t scene_width,
    uint32_t scene_height, int32_t scene_scale, uint64_t scene_revision) {
    return has_data && current_width == scene_width &&
           current_height == scene_height && current_scale == scene_scale &&
           current_revision == scene_revision;
}

static inline int cclover_wayland_buffer_compatible(
    uint32_t current_width, uint32_t current_height, int32_t current_scale,
    uint32_t scene_width, uint32_t scene_height, int32_t scene_scale) {
    return current_width == scene_width && current_height == scene_height &&
           current_scale == scene_scale;
}

static inline int cclover_wayland_buffer_reusable(
    int busy, int realized, uint32_t current_width, uint32_t current_height,
    int32_t current_scale, uint32_t scene_width, uint32_t scene_height,
    int32_t scene_scale) {
    return !busy && realized &&
           cclover_wayland_buffer_compatible(
               current_width, current_height, current_scale, scene_width,
               scene_height, scene_scale);
}

static inline CcloverWaylandDrawMode cclover_wayland_draw_mode(
    int scene_full_redraw, int has_previous_buffer, int static_rebuilt,
    size_t damage_count) {
    if (scene_full_redraw || !has_previous_buffer || static_rebuilt)
        return CCLOVER_WAYLAND_DRAW_FULL;
    if (damage_count == 0) return CCLOVER_WAYLAND_DRAW_SKIP;
    return CCLOVER_WAYLAND_DRAW_INCREMENTAL;
}

#endif
