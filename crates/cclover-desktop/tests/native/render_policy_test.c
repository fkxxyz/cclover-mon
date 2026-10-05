#include "render_policy.h"

#include <assert.h>

int main(void) {
    assert(cclover_cairo_command_selected(CCLOVER_CAIRO_DRAW_ALL, 0, 1, 1, 0));
    assert(cclover_cairo_command_selected(CCLOVER_CAIRO_DRAW_ALL, 1, 1, 1, 0));

    assert(cclover_cairo_command_selected(CCLOVER_CAIRO_DRAW_STATIC, 1, 0, 0, 0));
    assert(!cclover_cairo_command_selected(CCLOVER_CAIRO_DRAW_STATIC, 0, 0, 0, 0));

    assert(cclover_cairo_command_selected(CCLOVER_CAIRO_DRAW_DYNAMIC, 0, 0, 1, 0));
    assert(!cclover_cairo_command_selected(CCLOVER_CAIRO_DRAW_DYNAMIC, 1, 0, 1, 1));

    assert(cclover_cairo_command_selected(CCLOVER_CAIRO_DRAW_DYNAMIC, 0, 1, 1, 1));
    assert(!cclover_cairo_command_selected(CCLOVER_CAIRO_DRAW_DYNAMIC, 0, 1, 1, 0));

    /* Missing or malformed optimization metadata must fall back to drawing. */
    assert(cclover_cairo_command_selected(CCLOVER_CAIRO_DRAW_DYNAMIC, 0, 1, 0, 0));

    return 0;
}
