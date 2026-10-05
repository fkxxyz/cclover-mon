#ifndef CCLOVER_LINUX_RENDER_H
#define CCLOVER_LINUX_RENDER_H

#include "native_scene.h"

#include <cairo/cairo.h>
#include <stdint.h>

typedef struct {
    uint32_t size;
    uint32_t bold;
    cairo_font_extents_t extents;
    int valid;
} CcloverFontMetricsCacheEntry;

typedef struct CcloverCairoRenderer {
    CcloverFontMetricsCacheEntry font_metrics[32];
    int typography_violation_reported;
} CcloverCairoRenderer;

typedef enum {
    CCLOVER_CAIRO_DRAW_ALL = 0,
    CCLOVER_CAIRO_DRAW_STATIC = 1,
    CCLOVER_CAIRO_DRAW_DYNAMIC = 2,
} CcloverCairoDrawMode;

void cclover_cairo_configure_context(cairo_t *cr);
int cclover_cairo_scene_fits(CcloverCairoRenderer *renderer, cairo_t *cr,
                             const CcloverScene *scene,
                             CcloverCairoDrawMode mode);
int cclover_cairo_validate_scene(cairo_t *cr, const CcloverScene *scene,
                                 CcloverCairoDrawMode mode);
int cclover_cairo_draw_scene(CcloverCairoRenderer *renderer, cairo_t *cr,
                             const CcloverScene *scene, int clear,
                             CcloverCairoDrawMode mode);

#endif
