#define UNICODE
#define _UNICODE
#include "layered.h"
#include "alpha_policy.h"

#include <string.h>

static DWORD cclover_last_error(void) {
    DWORD error = GetLastError();
    return error ? error : ERROR_GEN_FAILURE;
}

void cclover_layered_destroy(CcloverLayeredSurface *surface) {
    int i;
    for (i = 0; i < 2; ++i) {
        if (surface->dc[i] && surface->previous[i]) {
            SelectObject(surface->dc[i], surface->previous[i]);
        }
        if (surface->bitmap[i]) DeleteObject(surface->bitmap[i]);
        if (surface->dc[i]) DeleteDC(surface->dc[i]);
    }
    memset(surface, 0, sizeof(*surface));
}

static int cclover_layered_ensure(CcloverLayeredSurface *surface, int width, int height) {
    BITMAPINFO info;
    HDC screen;
    DWORD error;
    int i;
    if (surface->dc[0] && surface->width == width && surface->height == height) return 1;

    cclover_layered_destroy(surface);
    ZeroMemory(&info, sizeof(info));
    info.bmiHeader.biSize = sizeof(info.bmiHeader);
    info.bmiHeader.biWidth = width;
    info.bmiHeader.biHeight = -height;
    info.bmiHeader.biPlanes = 1;
    info.bmiHeader.biBitCount = 32;
    info.bmiHeader.biCompression = BI_RGB;

    screen = GetDC(NULL);
    if (!screen) {
        error = cclover_last_error();
        surface->last_error = error;
        return 0;
    }
    for (i = 0; i < 2; ++i) {
        void *bits = NULL;
        surface->dc[i] = CreateCompatibleDC(screen);
        surface->bitmap[i] = CreateDIBSection(screen, &info, DIB_RGB_COLORS, &bits, NULL, 0);
        if (!surface->dc[i] || !surface->bitmap[i] || !bits) {
            error = cclover_last_error();
            ReleaseDC(NULL, screen);
            cclover_layered_destroy(surface);
            surface->last_error = error;
            return 0;
        }
        surface->bits[i] = (uint32_t *)bits;
        surface->previous[i] = SelectObject(surface->dc[i], surface->bitmap[i]);
    }
    ReleaseDC(NULL, screen);
    surface->width = width;
    surface->height = height;
    return 1;
}

int cclover_layered_render(CcloverLayeredSurface *surface, CcloverGdiRenderer *renderer,
                           const CcloverScene *scene, UINT dpi, int width, int height) {
    size_t pixels;
    if (width <= 0 || height <= 0) {
        surface->last_error = ERROR_INVALID_PARAMETER;
        return CCLOVER_LAYERED_FAILED;
    }
    if (!cclover_layered_ensure(surface, width, height)) return CCLOVER_LAYERED_FAILED;

    pixels = (size_t)width * (size_t)height;
    memset(surface->bits[0], 0x00, pixels * sizeof(uint32_t));
    memset(surface->bits[1], 0xff, pixels * sizeof(uint32_t));
    if (cclover_gdi_draw_scene(renderer, surface->dc[0], scene, dpi) != 0) {
        return CCLOVER_LAYERED_REJECTED;
    }
    cclover_gdi_paint_scene(renderer, surface->dc[1], scene, dpi);
    GdiFlush();
    cclover_alpha_recover(surface->bits[0], surface->bits[1], pixels);
    return CCLOVER_LAYERED_OK;
}

int cclover_layered_present(CcloverLayeredSurface *surface, CcloverGdiRenderer *renderer,
                            HWND hwnd, const CcloverScene *scene, UINT dpi,
                            int width, int height) {
    SIZE size;
    POINT origin = { 0, 0 };
    BLENDFUNCTION blend;
    int status = cclover_layered_render(surface, renderer, scene, dpi, width, height);
    if (status != CCLOVER_LAYERED_OK) return status;

    size.cx = width;
    size.cy = height;
    blend.BlendOp = AC_SRC_OVER;
    blend.BlendFlags = 0;
    blend.SourceConstantAlpha = 255;
    blend.AlphaFormat = AC_SRC_ALPHA;
    if (!UpdateLayeredWindow(hwnd, NULL, NULL, &size, surface->dc[0], &origin, 0,
                             &blend, ULW_ALPHA)) {
        surface->last_error = cclover_last_error();
        return CCLOVER_LAYERED_FAILED;
    }
    return CCLOVER_LAYERED_OK;
}

int cclover_layered_sample(const CcloverScene *scene, UINT dpi, int width, int height,
                           const CcloverSamplePoint *points, uint32_t *argb, size_t count) {
    CcloverGdiRenderer renderer = {0};
    CcloverLayeredSurface surface = {0};
    size_t i;
    int status = cclover_layered_render(&surface, &renderer, scene, dpi, width, height);
    if (status == CCLOVER_LAYERED_OK) {
        for (i = 0; i < count; ++i) {
            if (points[i].x < 0 || points[i].y < 0 ||
                points[i].x >= width || points[i].y >= height) {
                status = CCLOVER_LAYERED_FAILED;
                break;
            }
            argb[i] = surface.bits[0][(size_t)points[i].y * (size_t)width + (size_t)points[i].x];
        }
    }
    cclover_layered_destroy(&surface);
    cclover_gdi_renderer_destroy(&renderer);
    return status;
}
