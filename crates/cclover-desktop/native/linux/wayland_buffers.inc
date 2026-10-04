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

static int wayland_static_layer_ensure(WaylandHost *host, const CcloverScene *scene,
                                       int32_t scale, int *rebuilt) {
    WaylandStaticLayer *layer = &host->static_layer;
    *rebuilt = 0;
    uint32_t buffer_width = scene->width * (uint32_t)scale;
    uint32_t buffer_height = scene->height * (uint32_t)scale;
    int stride = (int)buffer_width * 4;
    size_t size = (size_t)stride * buffer_height;
    if (layer->data && layer->width == scene->width && layer->height == scene->height &&
        layer->scale == scale && layer->revision == scene->static_revision)
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
    cclover_configure_context(layer->cr);
    cairo_scale(layer->cr, scale, scale);
    cclover_draw_scene(&host->host, layer->cr, scene, 0, CCLOVER_DRAW_STATIC);
    cairo_surface_flush(layer->image);
    *rebuilt = 1;
    return 0;
}

static int wayland_buffer_create(WaylandHost *host, WaylandBuffer *owned,
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

    pool = wl_shm_create_pool(host->globals.shm, fd, (int)size);
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
    cclover_configure_context(owned->cr);
    cairo_scale(owned->cr, scale, scale);
    wl_buffer_add_listener(owned->buffer, &buffer_listener, owned);
    return 0;
}

static int wayland_buffer_acquire(WaylandHost *host, const CcloverScene *scene,
                                  int32_t scale, WaylandBuffer **out) {
    size_t i;
    if (host->previous_buffer && !host->previous_buffer->busy &&
        host->previous_buffer->buffer &&
        host->previous_buffer->width == scene->width &&
        host->previous_buffer->height == scene->height &&
        host->previous_buffer->scale == scale) {
        *out = host->previous_buffer;
        return 0;
    }
    for (i = 0; i < CCLOVER_WAYLAND_BUFFER_COUNT; ++i) {
        WaylandBuffer *owned = &host->buffers[i];
        if (owned->busy) continue;
        if (owned->buffer &&
            (owned->width != scene->width || owned->height != scene->height ||
             owned->scale != scale)) {
            wayland_buffer_destroy(owned);
        }
        if (!owned->buffer && wayland_buffer_create(host, owned, scene, scale) < 0)
            return -1;
        *out = owned;
        return 0;
    }
    return 1;
}

static void wayland_restore_rect(WaylandHost *host, WaylandBuffer *owned,
                                 CcloverDamageRect rect, int32_t scale) {
    int buffer_width = (int)(host->width * (uint32_t)scale);
    int buffer_height = (int)(host->height * (uint32_t)scale);
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
               (uint8_t *)host->static_layer.data + (size_t)y * stride + (size_t)x1 * 4,
               (size_t)(x2 - x1) * 4);
    }
    cairo_surface_mark_dirty_rectangle(owned->image, x1, y1, x2 - x1, y2 - y1);
}

static int wayland_draw(WaylandHost *host, const CcloverScene *scene) {
    CcloverDamageRect full_damage = { 0.0f, 0.0f, (float)scene->width, (float)scene->height };
    const CcloverDamageRect *dirty = scene->damage_rects;
    size_t dirty_count = scene->damage_count;
    int32_t scale = host->scale > 0 ? host->scale : 1;
    int profiling = getenv("CCLOVER_RENDER_PROFILE") != NULL;
    double profile_started = profiling ? cclover_profile_cpu_ms() : 0.0;
    double profile_dynamic = profiling ? cclover_profile_cpu_ms() : 0.0;
    double profile_static = 0.0;
    double profile_raster = 0.0;
    size_t i;
    int full_redraw = scene->full_redraw != 0 || host->previous_buffer == NULL;
    int static_rebuilt = 0;
    WaylandBuffer *owned;
    int acquired;

    if (wayland_static_layer_ensure(host, scene, scale, &static_rebuilt) < 0) return -1;
    if (static_rebuilt) full_redraw = 1;
    if (profiling) profile_static = cclover_profile_cpu_ms();
    if (full_redraw) {
        dirty = &full_damage;
        dirty_count = 1;
    } else if (dirty_count == 0) {
        return 0;
    }

    acquired = wayland_buffer_acquire(host, scene, scale, &owned);
    if (acquired != 0) return acquired;
    cairo_surface_flush(owned->image);

    if (full_redraw) {
        memcpy(owned->data, host->static_layer.data, owned->size);
        cairo_surface_mark_dirty(owned->image);
    } else {
        if (owned != host->previous_buffer) {
            if (host->previous_buffer && host->previous_buffer->size == owned->size) {
                memcpy(owned->data, host->previous_buffer->data, owned->size);
                cairo_surface_mark_dirty(owned->image);
            } else {
                memcpy(owned->data, host->static_layer.data, owned->size);
                cairo_surface_mark_dirty(owned->image);
                full_redraw = 1;
                dirty = &full_damage;
                dirty_count = 1;
            }
        }
        if (!full_redraw) {
            for (i = 0; i < dirty_count; ++i)
                wayland_restore_rect(host, owned, dirty[i], scale);
        }
    }

    cairo_save(owned->cr);
    for (i = 0; i < dirty_count; ++i) {
        cairo_rectangle(owned->cr, dirty[i].x1, dirty[i].y1,
                        dirty[i].x2 - dirty[i].x1, dirty[i].y2 - dirty[i].y1);
    }
    cairo_clip(owned->cr);
    cclover_draw_scene(&host->host, owned->cr, scene, 0, CCLOVER_DRAW_DYNAMIC);
    cairo_restore(owned->cr);
    cairo_surface_flush(owned->image);
    if (profiling) profile_raster = cclover_profile_cpu_ms();

    wl_surface_set_buffer_scale(host->surface, scale);
    wl_surface_attach(host->surface, owned->buffer, 0, 0);
    for (i = 0; i < dirty_count; ++i) {
        int x = (int)floorf(dirty[i].x1);
        int y = (int)floorf(dirty[i].y1);
        int width = (int)ceilf(dirty[i].x2) - x;
        int height = (int)ceilf(dirty[i].y2) - y;
        wl_surface_damage(host->surface, x, y, width, height);
    }
    owned->busy = 1;
    wl_surface_commit(host->surface);
    wl_display_flush(host->globals.display);
    host->previous_buffer = owned;
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

