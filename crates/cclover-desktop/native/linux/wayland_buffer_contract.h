#ifndef CCLOVER_LINUX_WAYLAND_BUFFER_CONTRACT_H
#define CCLOVER_LINUX_WAYLAND_BUFFER_CONTRACT_H

#include <stddef.h>

struct WaylandBuffer;

typedef struct {
    struct WaylandBuffer *previous_buffer;
} CcloverWaylandBufferBaseline;

static inline void cclover_wayland_buffer_baseline_reset(
    CcloverWaylandBufferBaseline *baseline) {
    baseline->previous_buffer = NULL;
}

#endif
