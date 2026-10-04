#ifndef CCLOVER_X11_LIFECYCLE_H
#define CCLOVER_X11_LIFECYCLE_H

#include <stdint.h>

typedef struct {
    int running;
    int dirty;
} X11Lifecycle;

void cclover_x11_lifecycle_init(X11Lifecycle *lifecycle);
void cclover_x11_state_status(X11Lifecycle *lifecycle, uint32_t status,
                              uint32_t changed_flag);
void cclover_x11_exposed(X11Lifecycle *lifecycle);
void cclover_x11_destroyed(X11Lifecycle *lifecycle);
void cclover_x11_quit_requested(X11Lifecycle *lifecycle);
int cclover_x11_can_draw(const X11Lifecycle *lifecycle);
void cclover_x11_draw_completed(X11Lifecycle *lifecycle);

#endif
