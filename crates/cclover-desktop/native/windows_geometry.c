#include "windows_geometry.h"

static int cclover_round_positive(float value) {
    return (int)(value + 0.5f);
}

int cclover_scale_logical(float logical, uint32_t dpi) {
    if (dpi == 0) dpi = CCLOVER_DEFAULT_DPI;
    return cclover_round_positive(logical * (float)dpi / (float)CCLOVER_DEFAULT_DPI);
}

float cclover_unscale_physical(int physical, uint32_t dpi) {
    if (dpi == 0) dpi = CCLOVER_DEFAULT_DPI;
    return (float)physical * (float)CCLOVER_DEFAULT_DPI / (float)dpi;
}

int cclover_dpi_requires_resource_refresh(uint32_t old_dpi, uint32_t new_dpi) {
    if (old_dpi == 0) old_dpi = CCLOVER_DEFAULT_DPI;
    if (new_dpi == 0) new_dpi = CCLOVER_DEFAULT_DPI;
    return old_dpi != new_dpi;
}

CcloverWindowGeometry cclover_top_right_geometry(
    uint32_t logical_width,
    uint32_t logical_height,
    CcloverRectI work_area,
    uint32_t dpi,
    float logical_margin) {
    CcloverWindowGeometry geometry;
    int margin = cclover_scale_logical(logical_margin, dpi);
    geometry.width = cclover_scale_logical((float)logical_width, dpi);
    geometry.height = cclover_scale_logical((float)logical_height, dpi);
    geometry.x = work_area.right - geometry.width - margin;
    geometry.y = work_area.top + margin;
    if (geometry.x < work_area.left) geometry.x = work_area.left;
    return geometry;
}
