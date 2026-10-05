#include <assert.h>
#include <stddef.h>
#include <stdint.h>

#include "wayland_lifecycle.h"

struct wl_output {
    int marker;
};

struct WaylandBuffer {
    int marker;
};

static void test_surface_recovery(void) {
    WaylandLifecycle lifecycle;
    CcloverWaylandBufferBaseline baseline = {0};
    struct WaylandBuffer previous_buffer = {0};
    int i;

    cclover_wayland_lifecycle_init(&lifecycle);
    assert(lifecycle.surface_state == WAYLAND_SURFACE_ABSENT);
    assert(!cclover_wayland_can_draw(&lifecycle));

    cclover_wayland_surface_created(&lifecycle);
    assert(lifecycle.surface_state == WAYLAND_SURFACE_WAITING_CONFIGURE);
    assert(!cclover_wayland_can_draw(&lifecycle));

    lifecycle.dirty = 0;
    cclover_wayland_state_status(&lifecycle, 0, 1u << 3);
    assert(!lifecycle.dirty);
    cclover_wayland_state_status(&lifecycle, 1u << 3, 1u << 3);
    assert(lifecycle.dirty);
    assert(!cclover_wayland_can_draw(&lifecycle));

    cclover_wayland_surface_configured(&lifecycle);
    assert(lifecycle.surface_state == WAYLAND_SURFACE_ACTIVE);
    assert(cclover_wayland_can_draw(&lifecycle));
    cclover_wayland_draw_completed(&lifecycle, 1);
    assert(!lifecycle.dirty);

    cclover_wayland_state_status(&lifecycle, 1u << 3, 1u << 3);
    assert(cclover_wayland_can_draw(&lifecycle));
    cclover_wayland_draw_completed(&lifecycle, 1);
    assert(!cclover_wayland_can_draw(&lifecycle));

    for (i = 0; i < 128; ++i) {
        cclover_wayland_surface_closed(&lifecycle);
        assert(cclover_wayland_needs_recreate(&lifecycle));
        assert(!cclover_wayland_can_draw(&lifecycle));

        cclover_wayland_surface_configured(&lifecycle);
        assert(cclover_wayland_needs_recreate(&lifecycle));

        baseline.previous_buffer = &previous_buffer;
        cclover_wayland_surface_destroyed(&lifecycle, &baseline);
        assert(baseline.previous_buffer == NULL);
        assert(lifecycle.surface_state == WAYLAND_SURFACE_ABSENT);
        cclover_wayland_surface_created(&lifecycle);
        assert(lifecycle.surface_state == WAYLAND_SURFACE_WAITING_CONFIGURE);
        cclover_wayland_surface_configured(&lifecycle);
        assert(lifecycle.surface_state == WAYLAND_SURFACE_ACTIVE);
        assert(cclover_wayland_can_draw(&lifecycle));
        cclover_wayland_draw_completed(&lifecycle, 1);
    }

    cclover_wayland_resize_requested(&lifecycle);
    assert(lifecycle.surface_state == WAYLAND_SURFACE_WAITING_CONFIGURE);
    assert(!cclover_wayland_can_draw(&lifecycle));
    cclover_wayland_surface_configured(&lifecycle);
    assert(cclover_wayland_can_draw(&lifecycle));
}

static void test_output_churn(void) {
    WaylandOutput outputs[CCLOVER_WAYLAND_MAX_OUTPUTS] = {0};
    struct wl_output resources[CCLOVER_WAYLAND_MAX_OUTPUTS] = {0};
    WaylandOutput *entry;
    size_t i;
    int cycle;

    for (i = 0; i < CCLOVER_WAYLAND_MAX_OUTPUTS; ++i) {
        entry = cclover_wayland_output_find_free(outputs, CCLOVER_WAYLAND_MAX_OUTPUTS);
        assert(entry == &outputs[i]);
        entry->output = &resources[i];
        entry->global_name = (uint32_t)(100 + i);
        entry->scale = (int32_t)(i % 3 + 1);
    }
    assert(cclover_wayland_output_find_free(outputs, CCLOVER_WAYLAND_MAX_OUTPUTS) == NULL);
    assert(cclover_wayland_output_scale(outputs, CCLOVER_WAYLAND_MAX_OUTPUTS, 0) == 3);

    outputs[3].entered = 1;
    outputs[3].scale = 2;
    outputs[7].entered = 1;
    outputs[7].scale = 3;
    assert(cclover_wayland_output_scale(outputs, CCLOVER_WAYLAND_MAX_OUTPUTS, 1) == 3);

    for (i = 0; i < CCLOVER_WAYLAND_MAX_OUTPUTS; ++i)
        cclover_wayland_output_reset_state(&outputs[i]);
    assert(cclover_wayland_output_scale(outputs, CCLOVER_WAYLAND_MAX_OUTPUTS, 0) == 1);

    for (cycle = 0; cycle < 1000; ++cycle) {
        uint32_t name = (uint32_t)(1000 + cycle);
        entry = cclover_wayland_output_find_free(outputs, CCLOVER_WAYLAND_MAX_OUTPUTS);
        assert(entry == &outputs[0]);
        entry->output = &resources[0];
        entry->global_name = name;
        entry->scale = 2;
        entry->entered = 1;
        assert(cclover_wayland_output_find_global(outputs, CCLOVER_WAYLAND_MAX_OUTPUTS, name) == entry);
        assert(cclover_wayland_output_find_resource(outputs, CCLOVER_WAYLAND_MAX_OUTPUTS,
                                                    &resources[0]) == entry);
        cclover_wayland_output_reset_state(entry);
        assert(cclover_wayland_output_find_global(outputs, CCLOVER_WAYLAND_MAX_OUTPUTS, name) == NULL);
    }
}

int main(void) {
    test_surface_recovery();
    test_output_churn();
    return 0;
}
