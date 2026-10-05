#ifndef CCLOVER_LINUX_RENDER_POLICY_H
#define CCLOVER_LINUX_RENDER_POLICY_H

typedef enum {
    CCLOVER_CAIRO_DRAW_ALL = 0,
    CCLOVER_CAIRO_DRAW_STATIC = 1,
    CCLOVER_CAIRO_DRAW_DYNAMIC = 2,
} CcloverCairoDrawMode;

static inline int cclover_cairo_command_selected(CcloverCairoDrawMode mode,
                                                 int is_static,
                                                 int cull_to_redraw_mask,
                                                 int redraw_mask_valid,
                                                 int redraw_mask_value) {
    if (mode == CCLOVER_CAIRO_DRAW_ALL) return 1;
    if (mode == CCLOVER_CAIRO_DRAW_STATIC) return is_static;
    if (mode != CCLOVER_CAIRO_DRAW_DYNAMIC || is_static) return 0;
    if (!cull_to_redraw_mask || !redraw_mask_valid) return 1;
    return redraw_mask_value != 0;
}

#endif
