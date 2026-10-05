#include "render.h"

#include <assert.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>

typedef struct {
    unsigned char *pixels;
    cairo_surface_t *surface;
    cairo_t *cr;
    int stride;
    int height;
} TestImage;

static TestImage image_create(int width, int height) {
    TestImage image = {0};
    image.stride = cairo_format_stride_for_width(CAIRO_FORMAT_ARGB32, width);
    image.height = height;
    image.pixels = calloc((size_t)image.stride, (size_t)height);
    assert(image.pixels != NULL);
    image.surface = cairo_image_surface_create_for_data(
        image.pixels, CAIRO_FORMAT_ARGB32, width, height, image.stride);
    assert(cairo_surface_status(image.surface) == CAIRO_STATUS_SUCCESS);
    image.cr = cairo_create(image.surface);
    assert(cairo_status(image.cr) == CAIRO_STATUS_SUCCESS);
    cclover_cairo_configure_context(image.cr);
    return image;
}

static void image_destroy(TestImage *image) {
    cairo_destroy(image->cr);
    cairo_surface_destroy(image->surface);
    free(image->pixels);
}

static size_t image_size(const TestImage *image) {
    return (size_t)image->stride * (size_t)image->height;
}

static CcloverCommand text_command(const char *text) {
    CcloverCommand command = {0};
    command.kind = CCLOVER_CMD_TEXT;
    command.x = 8.0f;
    command.y = 8.0f;
    command.width = 96.0f;
    command.height = 24.0f;
    command.color = 0xffffffffu;
    command.text = (const uint8_t *)text;
    command.text_len = strlen(text);
    command.text_size = 16;
    command.flags = CCLOVER_TEXT_END | CCLOVER_TEXT_MUST_FIT;
    return command;
}

static CcloverScene scene_for(CcloverCommand *command) {
    CcloverScene scene = {0};
    scene.width = 112;
    scene.height = 40;
    scene.commands = command;
    scene.command_count = 1;
    return scene;
}

static void clear_image(TestImage *image) {
    memset(image->pixels, 0, image_size(image));
    cairo_surface_mark_dirty(image->surface);
}

int main(void) {
    CcloverCairoRenderer renderer = {0};
    CcloverCairoRenderer fallback_renderer = {0};
    CcloverCommand command = text_command("123456");
    CcloverScene scene = scene_for(&command);
    TestImage prepared = image_create((int)scene.width, (int)scene.height);
    TestImage fallback = image_create((int)scene.width, (int)scene.height);

    assert(cclover_cairo_scene_fits(
               &renderer, prepared.cr, &scene, CCLOVER_CAIRO_DRAW_ALL, 0) == 1);
    assert(renderer.prepared_commands == scene.commands);
    assert(renderer.prepared_command_count == scene.command_count);
    assert(renderer.text_advances != NULL);
    assert(renderer.text_advances[0].valid == 1);

    cclover_cairo_execute_validated_scene(
        &renderer, prepared.cr, &scene, 1, CCLOVER_CAIRO_DRAW_ALL, 0);
    cairo_surface_flush(prepared.surface);
    assert(renderer.prepared_commands == NULL);
    assert(renderer.prepared_command_count == 0);

    cclover_cairo_execute_validated_scene(
        &fallback_renderer, fallback.cr, &scene, 1, CCLOVER_CAIRO_DRAW_ALL, 0);
    cairo_surface_flush(fallback.surface);
    assert(memcmp(prepared.pixels, fallback.pixels, image_size(&prepared)) == 0);

    command.text = (const uint8_t *)"1";
    command.text_len = 1;
    clear_image(&prepared);
    clear_image(&fallback);

    cclover_cairo_execute_validated_scene(
        &renderer, prepared.cr, &scene, 1, CCLOVER_CAIRO_DRAW_ALL, 0);
    cairo_surface_flush(prepared.surface);
    assert(cclover_cairo_draw_scene(
               &fallback_renderer, fallback.cr, &scene, 1,
               CCLOVER_CAIRO_DRAW_ALL) == 0);
    cairo_surface_flush(fallback.surface);
    assert(memcmp(prepared.pixels, fallback.pixels, image_size(&prepared)) == 0);

    cclover_cairo_renderer_destroy(&renderer);
    cclover_cairo_renderer_destroy(&fallback_renderer);
    image_destroy(&prepared);
    image_destroy(&fallback);
    return 0;
}
