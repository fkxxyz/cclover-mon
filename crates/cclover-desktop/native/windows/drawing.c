#define UNICODE
#define _UNICODE
#include "drawing.h"
#include "windows_geometry.h"

#include <stdint.h>

static COLORREF cclover_color(uint32_t argb) {
    unsigned a = (argb >> 24) & 0xff;
    unsigned r = (argb >> 16) & 0xff;
    unsigned g = ((argb >> 8) & 0xff);
    unsigned b = argb & 0xff;
    if (a < 255) {
        const unsigned br = 0x0b, bg = 0x10, bb = 0x20;
        r = (r * a + br * (255 - a)) / 255;
        g = (g * a + bg * (255 - a)) / 255;
        b = (b * a + bb * (255 - a)) / 255;
    }
    return RGB(r, g, b);
}

static int cclover_px(UINT dpi, float logical) {
    return cclover_scale_logical(logical, dpi);
}

static void cclover_release_fonts(CcloverGdiRenderer *renderer) {
    unsigned i, j;
    for (i = 0; i < 32; ++i) {
        for (j = 0; j < 2; ++j) {
            if (renderer->fonts[i][j]) {
                DeleteObject(renderer->fonts[i][j]);
                renderer->fonts[i][j] = NULL;
            }
        }
    }
}

static void cclover_prepare_dpi(CcloverGdiRenderer *renderer, UINT dpi) {
    if (renderer->dpi && cclover_dpi_requires_resource_refresh(renderer->dpi, dpi)) {
        cclover_release_fonts(renderer);
    }
    renderer->dpi = dpi;
}

static HFONT cclover_font(CcloverGdiRenderer *renderer, uint32_t size, int bold) {
    uint32_t slot = size < 32 ? size : 31;
    HFONT cached = renderer->fonts[slot][bold ? 1 : 0];
    if (cached) return cached;
    int pixels = -cclover_px(renderer->dpi, (float)size);
    if (pixels == 0) pixels = -(int)size;
    cached = CreateFontW(pixels, 0, 0, 0, bold ? FW_BOLD : FW_NORMAL,
        FALSE, FALSE, FALSE, DEFAULT_CHARSET, OUT_DEFAULT_PRECIS, CLIP_DEFAULT_PRECIS,
        CLEARTYPE_QUALITY, FIXED_PITCH | FF_MODERN, L"Inconsolata");
    renderer->fonts[slot][bold ? 1 : 0] = cached;
    return cached;
}

static void cclover_draw_rect(CcloverGdiRenderer *renderer, HDC dc,
                              const CcloverCommand *cmd, int fill) {
    int left = cclover_px(renderer->dpi, cmd->x);
    int top = cclover_px(renderer->dpi, cmd->y);
    int right = cclover_px(renderer->dpi, cmd->x + cmd->width);
    int bottom = cclover_px(renderer->dpi, cmd->y + cmd->height);
    int diameter = cclover_px(renderer->dpi, cmd->radius * 2.0f);
    HGDIOBJ old_brush, old_pen;
    HBRUSH brush = fill ? CreateSolidBrush(cclover_color(cmd->color))
                        : (HBRUSH)GetStockObject(NULL_BRUSH);
    HPEN pen = fill ? (HPEN)GetStockObject(NULL_PEN)
                    : CreatePen(PS_SOLID,
                                max(1, cclover_px(renderer->dpi, cmd->stroke_width)),
                                cclover_color(cmd->color));
    old_brush = SelectObject(dc, brush);
    old_pen = SelectObject(dc, pen);
    if (diameter > 0) RoundRect(dc, left, top, right, bottom, diameter, diameter);
    else Rectangle(dc, left, top, right, bottom);
    SelectObject(dc, old_pen);
    SelectObject(dc, old_brush);
    if (fill) DeleteObject(brush); else DeleteObject(pen);
}

static void cclover_draw_text(CcloverGdiRenderer *renderer, HDC dc,
                              const CcloverCommand *cmd) {
    RECT rect;
    UINT flags = DT_SINGLELINE | DT_VCENTER | DT_NOPREFIX;
    HFONT font = cclover_font(renderer, cmd->text_size,
                              (cmd->flags & CCLOVER_TEXT_BOLD) != 0);
    HGDIOBJ old = SelectObject(dc, font);
    wchar_t stack_text[256];
    wchar_t *wide = stack_text;
    int wide_len = MultiByteToWideChar(CP_UTF8, 0, (LPCCH)cmd->text,
                                       (int)cmd->text_len, NULL, 0);
    if (wide_len <= 0) {
        SelectObject(dc, old);
        return;
    }
    if (wide_len > (int)(sizeof(stack_text) / sizeof(stack_text[0]))) {
        wide = (wchar_t *)HeapAlloc(GetProcessHeap(), 0,
                                    (size_t)wide_len * sizeof(wchar_t));
        if (!wide) {
            SelectObject(dc, old);
            return;
        }
    }
    MultiByteToWideChar(CP_UTF8, 0, (LPCCH)cmd->text, (int)cmd->text_len,
                        wide, wide_len);
    rect.left = cclover_px(renderer->dpi, cmd->x);
    rect.top = cclover_px(renderer->dpi, cmd->y);
    rect.right = cclover_px(renderer->dpi, cmd->x + cmd->width);
    rect.bottom = cclover_px(renderer->dpi, cmd->y + cmd->height);
    flags |= (cmd->flags & CCLOVER_TEXT_END) ? DT_RIGHT : DT_LEFT;
    if (!(cmd->flags & CCLOVER_TEXT_CLIP)) flags |= DT_NOCLIP;
    SetTextColor(dc, cclover_color(cmd->color));
    DrawTextW(dc, wide, wide_len, &rect, flags);
    if (wide != stack_text) HeapFree(GetProcessHeap(), 0, wide);
    SelectObject(dc, old);
}

void cclover_gdi_draw_scene(CcloverGdiRenderer *renderer, HDC dc,
                            const CcloverScene *scene, UINT dpi) {
    size_t i;
    cclover_prepare_dpi(renderer, dpi);
    SetBkMode(dc, TRANSPARENT);
    for (i = 0; i < scene->command_count; ++i) {
        const CcloverCommand *cmd = &scene->commands[i];
        if (cmd->kind == CCLOVER_CMD_FILL_RECT) {
            cclover_draw_rect(renderer, dc, cmd, 1);
        } else if (cmd->kind == CCLOVER_CMD_STROKE_RECT) {
            cclover_draw_rect(renderer, dc, cmd, 0);
        } else if (cmd->kind == CCLOVER_CMD_TEXT) {
            cclover_draw_text(renderer, dc, cmd);
        } else if (cmd->kind == CCLOVER_CMD_POLYLINE ||
                   cmd->kind == CCLOVER_CMD_POLYGON) {
            size_t j;
            POINT stack_points[128];
            POINT *points = stack_points;
            if (cmd->point_count == 0 ||
                cmd->point_offset + cmd->point_count > scene->point_count) continue;
            if (cmd->point_count > 128) {
                points = (POINT *)HeapAlloc(GetProcessHeap(), 0,
                                            cmd->point_count * sizeof(POINT));
                if (!points) continue;
            }
            for (j = 0; j < cmd->point_count; ++j) {
                const CcloverPoint *source = &scene->points[cmd->point_offset + j];
                points[j].x = cclover_px(renderer->dpi, source->x);
                points[j].y = cclover_px(renderer->dpi, source->y);
            }
            if (cmd->kind == CCLOVER_CMD_POLYLINE) {
                HPEN pen = CreatePen(
                    PS_SOLID,
                    max(1, cclover_px(renderer->dpi, cmd->stroke_width)),
                    cclover_color(cmd->color));
                HGDIOBJ old_pen = SelectObject(dc, pen);
                Polyline(dc, points, (int)cmd->point_count);
                SelectObject(dc, old_pen);
                DeleteObject(pen);
            } else {
                HBRUSH brush = CreateSolidBrush(cclover_color(cmd->color));
                HGDIOBJ old_brush = SelectObject(dc, brush);
                HGDIOBJ old_pen = SelectObject(dc, GetStockObject(NULL_PEN));
                Polygon(dc, points, (int)cmd->point_count);
                SelectObject(dc, old_pen);
                SelectObject(dc, old_brush);
                DeleteObject(brush);
            }
            if (points != stack_points) HeapFree(GetProcessHeap(), 0, points);
        }
    }
}

void cclover_gdi_renderer_destroy(CcloverGdiRenderer *renderer) {
    cclover_release_fonts(renderer);
    renderer->dpi = 0;
}
