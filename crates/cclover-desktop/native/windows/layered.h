#ifndef CCLOVER_WINDOWS_LAYERED_H
#define CCLOVER_WINDOWS_LAYERED_H

#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <stddef.h>
#include <stdint.h>

#include "drawing.h"
#include "native_scene.h"

/* Per-pixel-alpha presentation surface for one layered window. */
typedef struct {
    HDC dc[2];
    HBITMAP bitmap[2];
    HGDIOBJ previous[2];
    uint32_t *bits[2];
    int width;
    int height;
    /* Win32 error of the most recent CCLOVER_LAYERED_FAILED result. */
    DWORD last_error;
} CcloverLayeredSurface;

typedef struct {
    int x;
    int y;
} CcloverSamplePoint;

#define CCLOVER_LAYERED_OK 0
/* Typography conformance rejected the Scene; the previous frame stays on screen. */
#define CCLOVER_LAYERED_REJECTED 1
/* Surface allocation or UpdateLayeredWindow failed; the previous frame stays on screen. */
#define CCLOVER_LAYERED_FAILED 2

/*
 * Renders `scene` at `width` x `height` device pixels. On CCLOVER_LAYERED_OK,
 * surface->bits[0] holds premultiplied 0xAARRGGBB pixels.
 */
int cclover_layered_render(CcloverLayeredSurface *surface, CcloverGdiRenderer *renderer,
                           const CcloverScene *scene, UINT dpi, int width, int height);

/* Renders and publishes with UpdateLayeredWindow. The window keeps its position. */
int cclover_layered_present(CcloverLayeredSurface *surface, CcloverGdiRenderer *renderer,
                            HWND hwnd, const CcloverScene *scene, UINT dpi,
                            int width, int height);

void cclover_layered_destroy(CcloverLayeredSurface *surface);

/*
 * Validation entry point: renders `scene` on private resources and stores the
 * premultiplied ARGB at each point. Returns a CCLOVER_LAYERED_* status.
 */
int cclover_layered_sample(const CcloverScene *scene, UINT dpi, int width, int height,
                           const CcloverSamplePoint *points, uint32_t *argb, size_t count);

#endif
