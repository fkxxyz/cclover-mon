#define _GNU_SOURCE
#include "wayland_buffers.h"
#include "wayland_buffer_policy.h"

#include <errno.h>
#include <fcntl.h>
#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/stat.h>
#include <sys/types.h>
#include <time.h>
#include <unistd.h>
#include <wayland-client.h>

static double cclover_profile_cpu_ms(void) {
    struct timespec ts;
    clock_gettime(CLOCK_THREAD_CPUTIME_ID, &ts);
    return (double)ts.tv_sec * 1000.0 + (double)ts.tv_nsec / 1000000.0;
}

static void buffer_release(void *data, struct wl_buffer *buffer) {
    WaylandBuffer *owned = data;
    (void)buffer;
    owned->busy = 0;
}

static const struct wl_buffer_listener buffer_listener = { buffer_release };

static int create_shm_file(size_t size) {
    char name[64];
    int fd;
    snprintf(name, sizeof(name), "/cclover-mon-%ld-%u", (long)getpid(),
             (unsigned)rand());
    fd = shm_open(name, O_RDWR | O_CREAT | O_EXCL, 0600);
    if (fd < 0) return -1;
    shm_unlink(name);
    if (ftruncate(fd, (off_t)size) < 0) {
        close(fd);
        return -1;
    }
    return fd;
}

static void wayland_buffer_destroy(WaylandBuffer *owned) {
    if (owned->cr) cairo_destroy(owned->cr);
    if (owned->image) cairo_surface_destroy(owned->image);
    if (owned->buffer) wl_buffer_destroy(owned->buffer);
    if (owned->data && owned->data != MAP_FAILED) munmap(owned->data, owned->size);
    memset(owned, 0, sizeof(*owned));
}

static void wayland_static_layer_destroy(WaylandStaticLayer *layer) {
    if (layer->cr) cairo_destroy(layer->cr);
    if (layer->image) cairo_surface_destroy(layer->image);
    free(layer->data);
    memset(layer, 0, sizeof(*layer));
}

static int wayland_static_layer_ensure(CcloverWaylandBuffers *buffers,
                                       CcloverCairoRenderer *renderer,
                                       const CcloverScene *scene,
                                       int32_t scale, int *rebuilt) {
    WaylandStaticLayer *layer = &buffers->static_layer;
    *rebuilt = 0;
    uint32_t buffer_width = scene->width * (uint32_t)scale;
    uint32_t buffer_height = scene->height * (uint32_t)scale;
    int stride = (int)buffer_width * 4;
    size_t size = (size_t)stride * buffer_height;
    if (cclover_wayland_static_layer_reusable(
            layer->data != NULL, layer->width, layer->height, layer->scale,
            layer->revision, scene->width, scene->height, scale,
            scene->static_revision))
        return 0;

    wayland_static_layer_destroy(layer);
    layer->data = calloc(1, size);
    if (!layer->data) return -1;
    layer->size = size;
    layer->width = scene->width;
    layer->height = scene->height;
    layer->scale = scale;
    layer->revision = scene->static_revision;
    layer->image = cairo_image_surface_create_for_data(
        layer->data, CAIRO_FORMAT_ARGB32, buffer_width, buffer_height, stride);
    if (cairo_surface_status(layer->image) != CAIRO_STATUS_SUCCESS) {
        wayland_static_layer_destroy(layer);
        return -1;
    }
    layer->cr = cairo_create(layer->image);
    if (cairo_status(layer->cr) != CAIRO_STATUS_SUCCESS) {
        wayland_static_layer_destroy(layer);
        return -1;
    }
    cclover_cairo_configure_context(layer->cr);
    cairo_scale(layer->cr, scale, scale);
    if (cclover_cairo_draw_scene(renderer, layer->cr, scene, 0,
                                 CCLOVER_CAIRO_DRAW_STATIC) != 0) {
        wayland_static_layer_destroy(layer);
        return 1;
    }
    cairo_surface_flush(layer->image);
    *rebuilt = 1;
    return 0;
}

static int wayland_buffer_create(struct wl_shm *shm, WaylandBuffer *owned,
                                 const CcloverScene *scene, int32_t scale) {
    uint32_t buffer_width = scene->width * (uint32_t)scale;
    uint32_t buffer_height = scene->height * (uint32_t)scale;
    int stride = (int)buffer_width * 4;
    size_t size = (size_t)stride * buffer_height;
    int fd = create_shm_file(size);
    struct wl_shm_pool *pool;
    if (fd < 0) return -1;

    owned->size = size;
    owned->width = scene->width;
    owned->height = scene->height;
    owned->scale = scale;
    owned->data = mmap(NULL, size, PROT_READ | PROT_WRITE, MAP_SHARED, fd, 0);
    if (owned->data == MAP_FAILED) {
        owned->data = NULL;
        close(fd);
        wayland_buffer_destroy(owned);
        return -1;
    }

    pool = wl_shm_create_pool(shm, fd, (int)size);
    owned->buffer = wl_shm_pool_create_buffer(pool, 0, buffer_width, buffer_height,
                                              stride, WL_SHM_FORMAT_ARGB8888);
    wl_shm_pool_destroy(pool);
    close(fd);
    if (!owned->buffer) {
        wayland_buffer_destroy(owned);
        return -1;
    }

    owned->image = cairo_image_surface_create_for_data(
        owned->data, CAIRO_FORMAT_ARGB32, buffer_width, buffer_height, stride);
    if (cairo_surface_status(owned->image) != CAIRO_STATUS_SUCCESS) {
        wayland_buffer_destroy(owned);
        return -1;
    }
    owned->cr = cairo_create(owned->image);
    if (cairo_status(owned->cr) != CAIRO_STATUS_SUCCESS) {
        wayland_buffer_destroy(owned);
        return -1;
    }
    cclover_cairo_configure_context(owned->cr);
    cairo_scale(owned->cr, scale, scale);
    wl_buffer_add_listener(owned->buffer, &buffer_listener, owned);
    return 0;
}

static int wayland_buffer_acquire(CcloverWaylandBuffers *buffers, struct wl_shm *shm,
                                  const CcloverScene *scene, int32_t scale,
                                  WaylandBuffer **out) {
    size_t i;
    if (buffers->baseline.previous_buffer &&
        cclover_wayland_buffer_reusable(
            buffers->baseline.previous_buffer->busy,
            buffers->baseline.previous_buffer->buffer != NULL,
            buffers->baseline.previous_buffer->width, buffers->baseline.previous_buffer->height,
            buffers->baseline.previous_buffer->scale, scene->width, scene->height, scale)) {
        *out = buffers->baseline.previous_buffer;
        return 0;
    }
    for (i = 0; i < CCLOVER_WAYLAND_BUFFER_COUNT; ++i) {
        WaylandBuffer *owned = &buffers->buffers[i];
        if (owned->busy) continue;
        if (owned->buffer &&
            !cclover_wayland_buffer_compatible(
                owned->width, owned->height, owned->scale,
                scene->width, scene->height, scale)) {
            wayland_buffer_destroy(owned);
        }
        if (!owned->buffer && wayland_buffer_create(shm, owned, scene, scale) < 0)
            return -1;
        *out = owned;
        return 0;
    }
    return 1;
}

static void wayland_restore_rect(CcloverWaylandBuffers *buffers, WaylandBuffer *owned,
                                 const CcloverScene *scene, CcloverDamageRect rect,
                                 int32_t scale) {
    int buffer_width = (int)(scene->width * (uint32_t)scale);
    int buffer_height = (int)(scene->height * (uint32_t)scale);
    int x1 = (int)floorf(rect.x1 * scale);
    int y1 = (int)floorf(rect.y1 * scale);
    int x2 = (int)ceilf(rect.x2 * scale);
    int y2 = (int)ceilf(rect.y2 * scale);
    int stride = buffer_width * 4;
    int y;
    if (x1 < 0) x1 = 0;
    if (y1 < 0) y1 = 0;
    if (x2 > buffer_width) x2 = buffer_width;
    if (y2 > buffer_height) y2 = buffer_height;
    if (x2 <= x1 || y2 <= y1) return;
    for (y = y1; y < y2; ++y) {
        memcpy((uint8_t *)owned->data + (size_t)y * stride + (size_t)x1 * 4,
               (uint8_t *)buffers->static_layer.data + (size_t)y * stride + (size_t)x1 * 4,
               (size_t)(x2 - x1) * 4);
    }
    cairo_surface_mark_dirty_rectangle(owned->image, x1, y1, x2 - x1, y2 - y1);
}

int cclover_wayland_buffers_draw(CcloverWaylandBuffers *buffers,
                                 CcloverCairoRenderer *renderer,
                                 struct wl_shm *shm,
                                 struct wl_surface *surface,
                                 struct wl_display *display,
                                 const CcloverScene *scene,
                                 int32_t scale) {
    CcloverDamageRect full_damage = { 0.0f, 0.0f, (float)scene->width, (float)scene->height };
    const CcloverDamageRect *dirty = scene->damage_rects;
    size_t dirty_count = scene->damage_count;
    scale = scale > 0 ? scale : 1;
    int profiling = getenv("CCLOVER_RENDER_PROFILE") != NULL;
    double profile_started = profiling ? cclover_profile_cpu_ms() : 0.0;
    double profile_dynamic = profiling ? cclover_profile_cpu_ms() : 0.0;
    double profile_static = 0.0;
    double profile_raster = 0.0;
    size_t i;
    int static_rebuilt = 0;
    CcloverWaylandDrawMode draw_mode;
    int full_redraw;
    WaylandBuffer *owned;
    int acquired;

    {
        int static_result = wayland_static_layer_ensure(buffers, renderer, scene, scale,
                                                        &static_rebuilt);
        if (static_result != 0) return static_result;
    }
    draw_mode = cclover_wayland_draw_mode(
        scene->full_redraw != 0, buffers->baseline.previous_buffer != NULL,
        static_rebuilt, dirty_count);
    full_redraw = draw_mode == CCLOVER_WAYLAND_DRAW_FULL;
    if (profiling) profile_static = cclover_profile_cpu_ms();
    if (full_redraw) {
        dirty = &full_damage;
        dirty_count = 1;
    } else if (draw_mode == CCLOVER_WAYLAND_DRAW_SKIP) {
        return 0;
    }

    acquired = wayland_buffer_acquire(buffers, shm, scene, scale, &owned);
    if (acquired != 0) return acquired;
    if (!cclover_cairo_scene_fits(renderer, owned->cr, scene,
                                  CCLOVER_CAIRO_DRAW_DYNAMIC))
        return 1;
    cairo_surface_flush(owned->image);

    if (full_redraw) {
        memcpy(owned->data, buffers->static_layer.data, owned->size);
        cairo_surface_mark_dirty(owned->image);
    } else {
        if (owned != buffers->baseline.previous_buffer) {
            if (buffers->baseline.previous_buffer &&
                buffers->baseline.previous_buffer->size == owned->size) {
                memcpy(owned->data, buffers->baseline.previous_buffer->data, owned->size);
                cairo_surface_mark_dirty(owned->image);
            } else {
                memcpy(owned->data, buffers->static_layer.data, owned->size);
                cairo_surface_mark_dirty(owned->image);
                full_redraw = 1;
                dirty = &full_damage;
                dirty_count = 1;
            }
        }
        if (!full_redraw) {
            for (i = 0; i < dirty_count; ++i)
                wayland_restore_rect(buffers, owned, scene, dirty[i], scale);
        }
    }

    cairo_save(owned->cr);
    for (i = 0; i < dirty_count; ++i) {
        cairo_rectangle(owned->cr, dirty[i].x1, dirty[i].y1,
                        dirty[i].x2 - dirty[i].x1, dirty[i].y2 - dirty[i].y1);
    }
    cairo_clip(owned->cr);
    if (cclover_cairo_draw_scene(renderer, owned->cr, scene, 0,
                                 CCLOVER_CAIRO_DRAW_DYNAMIC) != 0) {
        cairo_restore(owned->cr);
        return 1;
    }
    cairo_restore(owned->cr);
    cairo_surface_flush(owned->image);
    if (profiling) profile_raster = cclover_profile_cpu_ms();

    wl_surface_set_buffer_scale(surface, scale);
    wl_surface_attach(surface, owned->buffer, 0, 0);
    for (i = 0; i < dirty_count; ++i) {
        int x = (int)floorf(dirty[i].x1);
        int y = (int)floorf(dirty[i].y1);
        int width = (int)ceilf(dirty[i].x2) - x;
        int height = (int)ceilf(dirty[i].y2) - y;
        wl_surface_damage(surface, x, y, width, height);
    }
    owned->busy = 1;
    wl_surface_commit(surface);
    wl_display_flush(display);
    buffers->baseline.previous_buffer = owned;
    if (profiling) {
        double dirty_area = 0.0;
        for (i = 0; i < dirty_count; ++i)
            dirty_area += (dirty[i].x2 - dirty[i].x1) * (dirty[i].y2 - dirty[i].y1);
        fprintf(stderr,
                "drawparts dynamic=%.3f static=%.3f raster=%.3f submit=%.3fms dirty=%zu area=%.0f\n",
                profile_dynamic - profile_started,
                profile_static - profile_dynamic,
                profile_raster - profile_static,
                cclover_profile_cpu_ms() - profile_raster,
                dirty_count, dirty_area);
    }
    return 0;
}


void cclover_wayland_buffers_destroy(CcloverWaylandBuffers *buffers) {
    size_t i;
    for (i = 0; i < CCLOVER_WAYLAND_BUFFER_COUNT; ++i)
        wayland_buffer_destroy(&buffers->buffers[i]);
    wayland_static_layer_destroy(&buffers->static_layer);
    cclover_wayland_buffer_baseline_reset(&buffers->baseline);
}
