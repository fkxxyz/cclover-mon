#include <assert.h>

#include "x11_lifecycle.h"

int main(void) {
    X11Lifecycle lifecycle;

    cclover_x11_lifecycle_init(&lifecycle);
    assert(lifecycle.running);
    assert(cclover_x11_can_draw(&lifecycle));
    cclover_x11_draw_completed(&lifecycle);
    assert(!cclover_x11_can_draw(&lifecycle));

    cclover_x11_state_status(&lifecycle, 1u << 4, 1u << 4);
    assert(cclover_x11_can_draw(&lifecycle));
    cclover_x11_draw_completed(&lifecycle);
    cclover_x11_exposed(&lifecycle);
    assert(cclover_x11_can_draw(&lifecycle));
    cclover_x11_draw_completed(&lifecycle);

    cclover_x11_state_status(&lifecycle, 0, 1u << 4);
    assert(!cclover_x11_can_draw(&lifecycle));
    cclover_x11_quit_requested(&lifecycle);
    assert(!lifecycle.running);
    assert(!cclover_x11_can_draw(&lifecycle));

    cclover_x11_lifecycle_init(&lifecycle);
    cclover_x11_destroyed(&lifecycle);
    assert(!lifecycle.running);
    return 0;
}
