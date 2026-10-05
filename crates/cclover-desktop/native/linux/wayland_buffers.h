#ifndef CCLOVER_LINUX_WAYLAND_BUFFERS_H
#define CCLOVER_LINUX_WAYLAND_BUFFERS_H

#include "wayland_buffer_contract.h"
#include "native_scene.h"
#include "render.h"

#include <cairo/cairo.h>
#include <stddef.h>
#include <stdint.h>

struct wl_buffer;
struct wl_display;
struct wl_shm;
struct wl_surface;

#define CCLOVER_WAYLAND_BUFFER_COUNT 2

typedef struct WaylandBuffer {
    struct wl_buffer *buffer;
    void *data;
    size_t size;
    cairo_surface_t *image;
    cairo_t *cr;
    uint32_t width;
    uint32_t height;
    int32_t scale;
    int busy;
} WaylandBuffer;

typedef struct {
    void *data;
    size_t size;
    cairo_surface_t *image;
    cairo_t *cr;
    uint32_t width;
    uint32_t height;
    int32_t scale;
    uint64_t revision;
} WaylandStaticLayer;

typedef struct {
    WaylandBuffer buffers[CCLOVER_WAYLAND_BUFFER_COUNT];
    CcloverWaylandBufferBaseline baseline;
    WaylandStaticLayer static_layer;
} CcloverWaylandBuffers;

void cclover_wayland_buffers_destroy(CcloverWaylandBuffers *buffers);
int cclover_wayland_buffers_draw(CcloverWaylandBuffers *buffers,
                                 CcloverCairoRenderer *renderer,
                                 struct wl_shm *shm,
                                 struct wl_surface *surface,
                                 struct wl_display *display,
                                 const CcloverScene *scene,
                                 int32_t scale);

#endif
