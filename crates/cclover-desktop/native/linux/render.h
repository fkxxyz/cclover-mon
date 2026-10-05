#ifndef CCLOVER_LINUX_RENDER_H
#define CCLOVER_LINUX_RENDER_H

#include "native_scene.h"
#include "render_policy.h"

#include <cairo/cairo.h>
#include <stddef.h>
#include <stdint.h>

typedef struct {
    uint32_t size;
    uint32_t bold;
    cairo_font_extents_t extents;
    int valid;
} CcloverFontMetricsCacheEntry;

typedef struct {
    double x_advance;
    uint8_t valid;
} CcloverTextAdvance;

typedef struct CcloverCairoRenderer {
    CcloverFontMetricsCacheEntry font_metrics[32];
    int typography_violation_reported;
    CcloverTextAdvance *text_advances;
    size_t text_advance_capacity;
    const CcloverCommand *prepared_commands;
    size_t prepared_command_count;
} CcloverCairoRenderer;

void cclover_cairo_configure_context(cairo_t *cr);
void cclover_cairo_renderer_destroy(CcloverCairoRenderer *renderer);
int cclover_cairo_scene_fits(CcloverCairoRenderer *renderer, cairo_t *cr,
                             const CcloverScene *scene,
                             CcloverCairoDrawMode mode,
                             int cull_to_redraw_mask);
int cclover_cairo_validate_scene(cairo_t *cr, const CcloverScene *scene,
                                 CcloverCairoDrawMode mode,
                                 int cull_to_redraw_mask);
void cclover_cairo_execute_validated_scene(CcloverCairoRenderer *renderer, cairo_t *cr,
                                           const CcloverScene *scene, int clear,
                                           CcloverCairoDrawMode mode,
                                           int cull_to_redraw_mask);
int cclover_cairo_draw_scene(CcloverCairoRenderer *renderer, cairo_t *cr,
                             const CcloverScene *scene, int clear,
                             CcloverCairoDrawMode mode);

#endif
