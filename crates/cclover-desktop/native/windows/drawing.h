#ifndef CCLOVER_WINDOWS_DRAWING_H
#define CCLOVER_WINDOWS_DRAWING_H

#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include "native_scene.h"

typedef struct {
    HFONT fonts[32][2];
    UINT dpi;
} CcloverGdiRenderer;

void cclover_gdi_renderer_destroy(CcloverGdiRenderer *renderer);
void cclover_gdi_draw_scene(CcloverGdiRenderer *renderer, HDC dc,
                            const CcloverScene *scene, UINT dpi);

#endif
