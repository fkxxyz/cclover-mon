#include <assert.h>
#include <math.h>
#include <stdint.h>

#include "windows_geometry.h"

static void assert_close(float actual, float expected) {
    assert(fabsf(actual - expected) < 0.001f);
}

int main(void) {
    CcloverRectI work = {0, 0, 3840, 2160};
    CcloverWindowGeometry geometry;

    assert(cclover_scale_logical(16.0f, 96) == 16);
    assert(cclover_scale_logical(16.0f, 120) == 20);
    assert(cclover_scale_logical(16.0f, 144) == 24);
    assert(cclover_scale_logical(16.0f, 192) == 32);
    assert(cclover_scale_logical(0.5f, 192) == 1);
    assert(cclover_scale_logical(16.0f, 0) == 16);

    assert_close(cclover_unscale_physical(32, 192), 16.0f);
    assert_close(cclover_unscale_physical(20, 120), 16.0f);
    assert_close(cclover_unscale_physical(16, 0), 16.0f);

    assert(!cclover_dpi_requires_resource_refresh(96, 96));
    assert(cclover_dpi_requires_resource_refresh(96, 192));
    assert(!cclover_dpi_requires_resource_refresh(0, 96));
    assert(cclover_dpi_requires_resource_refresh(0, 192));

    geometry = cclover_top_right_geometry(420, 800, work, 192, 16.0f);
    assert(geometry.width == 840);
    assert(geometry.height == 1600);
    assert(geometry.x == 2968);
    assert(geometry.y == 32);

    work.left = -1920;
    work.top = 0;
    work.right = 0;
    work.bottom = 1080;
    geometry = cclover_top_right_geometry(420, 800, work, 96, 16.0f);
    assert(geometry.width == 420);
    assert(geometry.height == 800);
    assert(geometry.x == -436);
    assert(geometry.y == 16);

    work.left = 100;
    work.top = 50;
    work.right = 400;
    work.bottom = 300;
    geometry = cclover_top_right_geometry(420, 800, work, 96, 16.0f);
    assert(geometry.x == work.left);
    assert(geometry.y == 66);

    return 0;
}
