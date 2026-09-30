#ifndef CCLOVER_WINDOWS_GEOMETRY_H
#define CCLOVER_WINDOWS_GEOMETRY_H

#include <stdint.h>

#define CCLOVER_DEFAULT_DPI 96u

typedef struct {
    int left;
    int top;
    int right;
    int bottom;
} CcloverRectI;

typedef struct {
    int x;
    int y;
    int width;
    int height;
} CcloverWindowGeometry;

int cclover_scale_logical(float logical, uint32_t dpi);
float cclover_unscale_physical(int physical, uint32_t dpi);
int cclover_dpi_requires_resource_refresh(uint32_t old_dpi, uint32_t new_dpi);
CcloverWindowGeometry cclover_top_right_geometry(
    uint32_t logical_width,
    uint32_t logical_height,
    CcloverRectI work_area,
    uint32_t dpi,
    float logical_margin);

#endif
