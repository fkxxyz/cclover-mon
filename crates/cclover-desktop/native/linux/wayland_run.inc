int cclover_linux_wayland_run(void *context, const CcloverCallbacks *callbacks, int state_wake_fd, int quit_wake_fd) {
    WaylandHost host;
    CcloverScene scene;
    struct wl_registry *registry;
    struct wl_region *empty_input;
    size_t i;
    int fd;
    memset(&host, 0, sizeof(host));
    host.host.context = context;
    host.host.callbacks = callbacks;
    host.scale = 1;
    host.globals.host = &host;
    host.globals.display = wl_display_connect(NULL);
    if (!host.globals.display) return 201;
    registry = wl_display_get_registry(host.globals.display);
    wl_registry_add_listener(registry, &registry_listener, &host.globals);
    if (wl_display_roundtrip(host.globals.display) < 0 || !host.globals.compositor ||
        !host.globals.shm || !host.globals.layer_shell) {
        wl_registry_destroy(registry);
        wl_display_disconnect(host.globals.display);
        return 202;
    }
    /* Collect initial wl_output.scale events before the surface enters an output. */
    wl_display_roundtrip(host.globals.display);
    wayland_set_initial_scale(&host);

    cclover_scene(&host.host, &scene);
    host.width = scene.width;
    host.height = scene.height;
    host.surface = wl_compositor_create_surface(host.globals.compositor);
    wl_surface_add_listener(host.surface, &surface_listener, &host);
    host.layer_surface = zwlr_layer_shell_v1_get_layer_surface(
        host.globals.layer_shell, host.surface, NULL,
        ZWLR_LAYER_SHELL_V1_LAYER_BOTTOM, "cclover-mon");
    zwlr_layer_surface_v1_add_listener(host.layer_surface, &layer_listener, &host);
    zwlr_layer_surface_v1_set_anchor(host.layer_surface,
        ZWLR_LAYER_SURFACE_V1_ANCHOR_TOP | ZWLR_LAYER_SURFACE_V1_ANCHOR_RIGHT);
    zwlr_layer_surface_v1_set_margin(host.layer_surface, CCLOVER_MARGIN,
                                     CCLOVER_MARGIN, 0, 0);
    zwlr_layer_surface_v1_set_exclusive_zone(host.layer_surface, 0);
    zwlr_layer_surface_v1_set_keyboard_interactivity(
        host.layer_surface, ZWLR_LAYER_SURFACE_V1_KEYBOARD_INTERACTIVITY_NONE);
    zwlr_layer_surface_v1_set_size(host.layer_surface, host.width, host.height);
    empty_input = wl_compositor_create_region(host.globals.compositor);
    wl_surface_set_input_region(host.surface, empty_input);
    wl_region_destroy(empty_input);
    wl_surface_commit(host.surface);

    while (!host.configured && !host.closed) {
        if (wl_display_dispatch(host.globals.display) < 0) {
            host.closed = 1;
            break;
        }
    }
    host.dirty = 1;
    fd = wl_display_get_fd(host.globals.display);

    while (!host.closed) {
        struct pollfd pfds[3] = {{ fd, POLLIN, 0 }, { state_wake_fd, POLLIN, 0 }, { quit_wake_fd, POLLIN, 0 }};
        uint32_t status = callbacks->take_state(context);
        if (status & CCLOVER_STATE_CHANGED) {
            int profiling = getenv("CCLOVER_RENDER_PROFILE") != NULL;
            double profile_started = profiling ? cclover_profile_cpu_ms() : 0.0;
            cclover_scene(&host.host, &scene);
            if (profiling)
                fprintf(stderr, "scene cpu=%.3fms\n",
                        cclover_profile_cpu_ms() - profile_started);
            if (scene.width != host.width || scene.height != host.height) {
                host.width = scene.width;
                host.height = scene.height;
                host.configured = 0;
                zwlr_layer_surface_v1_set_size(host.layer_surface, host.width, host.height);
                wl_surface_commit(host.surface);
            }
            host.dirty = 1;
        }
        if (host.configured && host.dirty) {
            int draw_result;
            int profiling = getenv("CCLOVER_RENDER_PROFILE") != NULL;
            double profile_started = profiling ? cclover_profile_cpu_ms() : 0.0;
            draw_result = wayland_draw(&host, &scene);
            if (profiling)
                fprintf(stderr, "draw cpu=%.3fms\n",
                        cclover_profile_cpu_ms() - profile_started);
            if (draw_result < 0) {
                host.closed = 1;
                break;
            }
            if (draw_result == 0) host.dirty = 0;
        }

        wl_display_flush(host.globals.display);
        if (poll(pfds, 3, -1) > 0 && (pfds[0].revents & POLLIN)) {
            if (wl_display_dispatch(host.globals.display) < 0) break;
        } else {
            wl_display_dispatch_pending(host.globals.display);
        }
        if (pfds[2].revents & POLLIN) {
            cclover_drain_wake_fd(quit_wake_fd);
            break;
        }
        if (pfds[1].revents & POLLIN) cclover_drain_wake_fd(state_wake_fd);
    }

    if (host.layer_surface) zwlr_layer_surface_v1_destroy(host.layer_surface);
    if (host.surface) wl_surface_destroy(host.surface);
    wl_display_roundtrip(host.globals.display);
    for (i = 0; i < CCLOVER_WAYLAND_BUFFER_COUNT; ++i)
        wayland_buffer_destroy(&host.buffers[i]);
    wayland_static_layer_destroy(&host.static_layer);
    for (i = 0; i < host.globals.output_count; ++i) {
        if (host.globals.outputs[i].output)
            wl_output_destroy(host.globals.outputs[i].output);
    }
    if (host.globals.layer_shell) zwlr_layer_shell_v1_destroy(host.globals.layer_shell);
    if (host.globals.shm) wl_shm_destroy(host.globals.shm);
    if (host.globals.compositor) wl_compositor_destroy(host.globals.compositor);
    wl_registry_destroy(registry);
    wl_display_roundtrip(host.globals.display);
    wl_display_disconnect(host.globals.display);
    return 0;
}
