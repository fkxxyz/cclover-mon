#ifndef CCLOVER_WINDOWS_DRAWING_H
#define CCLOVER_WINDOWS_DRAWING_H

#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include "native_scene.h"

typedef struct {
    HFONT fonts[32][2];
    UINT dpi;
    int typography_violation_reported;
} CcloverGdiRenderer;

void cclover_gdi_renderer_destroy(CcloverGdiRenderer *renderer);
int cclover_gdi_draw_scene(CcloverGdiRenderer *renderer, HDC dc,
                           const CcloverScene *scene, UINT dpi);
int cclover_gdi_validate_scene(HDC dc, const CcloverScene *scene, UINT dpi);

#endif
