#include "x11_lifecycle.h"

void cclover_x11_lifecycle_init(X11Lifecycle *lifecycle) {
    lifecycle->running = 1;
    lifecycle->dirty = 1;
}

void cclover_x11_state_status(X11Lifecycle *lifecycle, uint32_t status,
                              uint32_t changed_flag) {
    if (status & changed_flag) lifecycle->dirty = 1;
}

void cclover_x11_exposed(X11Lifecycle *lifecycle) {
    lifecycle->dirty = 1;
}

void cclover_x11_destroyed(X11Lifecycle *lifecycle) {
    lifecycle->running = 0;
}

void cclover_x11_quit_requested(X11Lifecycle *lifecycle) {
    lifecycle->running = 0;
}

int cclover_x11_can_draw(const X11Lifecycle *lifecycle) {
    return lifecycle->running && lifecycle->dirty;
}

void cclover_x11_draw_completed(X11Lifecycle *lifecycle) {
    lifecycle->dirty = 0;
}
