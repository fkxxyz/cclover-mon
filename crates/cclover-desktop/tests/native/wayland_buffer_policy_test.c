#include "wayland_buffer_policy.h"

#include <assert.h>

int main(void) {
    assert(cclover_wayland_static_layer_reusable(1, 320, 200, 2, 7, 320, 200, 2, 7));
    assert(!cclover_wayland_static_layer_reusable(0, 320, 200, 2, 7, 320, 200, 2, 7));
    assert(!cclover_wayland_static_layer_reusable(1, 320, 200, 2, 7, 321, 200, 2, 7));
    assert(!cclover_wayland_static_layer_reusable(1, 320, 200, 2, 7, 320, 200, 1, 7));
    assert(!cclover_wayland_static_layer_reusable(1, 320, 200, 2, 7, 320, 200, 2, 8));

    assert(cclover_wayland_buffer_compatible(320, 200, 2, 320, 200, 2));
    assert(!cclover_wayland_buffer_compatible(320, 200, 2, 321, 200, 2));
    assert(!cclover_wayland_buffer_compatible(320, 200, 2, 320, 201, 2));
    assert(!cclover_wayland_buffer_compatible(320, 200, 2, 320, 200, 1));

    assert(cclover_wayland_buffer_reusable(0, 1, 320, 200, 2, 320, 200, 2));
    assert(!cclover_wayland_buffer_reusable(1, 1, 320, 200, 2, 320, 200, 2));
    assert(!cclover_wayland_buffer_reusable(0, 0, 320, 200, 2, 320, 200, 2));
    assert(!cclover_wayland_buffer_reusable(0, 1, 320, 200, 2, 320, 201, 2));
    assert(!cclover_wayland_buffer_reusable(0, 1, 320, 200, 2, 320, 200, 1));

    assert(cclover_wayland_draw_mode(1, 1, 0, 1) == CCLOVER_WAYLAND_DRAW_FULL);
    assert(cclover_wayland_draw_mode(0, 0, 0, 1) == CCLOVER_WAYLAND_DRAW_FULL);
    assert(cclover_wayland_draw_mode(0, 1, 1, 1) == CCLOVER_WAYLAND_DRAW_FULL);
    assert(cclover_wayland_draw_mode(0, 1, 0, 0) == CCLOVER_WAYLAND_DRAW_SKIP);
    assert(cclover_wayland_draw_mode(0, 1, 0, 1) == CCLOVER_WAYLAND_DRAW_INCREMENTAL);

    return 0;
}
