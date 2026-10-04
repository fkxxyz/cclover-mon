int cclover_linux_wayland_run(void *context, const CcloverCallbacks *callbacks, int state_wake_fd, int quit_wake_fd) {
    WaylandHost host;
    CcloverScene scene;
    struct wl_registry *registry = NULL;
    size_t i;
    int fd;
    int result = 0;
    int quitting = 0;
    memset(&host, 0, sizeof(host));
    host.host.context = context;
    host.host.callbacks = callbacks;
    host.scale = 1;
    host.globals.host = &host;
    host.globals.display = wl_display_connect(NULL);
    if (!host.globals.display) return CCLOVER_LINUX_HOST_WAYLAND_DISPLAY_CONNECT_FAILED;
    registry = wl_display_get_registry(host.globals.display);
    if (!registry) {
        result = CCLOVER_LINUX_HOST_WAYLAND_GLOBAL_DISCOVERY_FAILED;
        goto cleanup;
    }
    wl_registry_add_listener(registry, &registry_listener, &host.globals);
    if (wl_display_roundtrip(host.globals.display) < 0 || !host.globals.compositor ||
        !host.globals.shm || !host.globals.layer_shell) {
        result = CCLOVER_LINUX_HOST_WAYLAND_GLOBAL_DISCOVERY_FAILED;
        goto cleanup;
    }
    /* Collect initial wl_output.scale events before the surface enters an output. */
    if (wl_display_roundtrip(host.globals.display) < 0) {
        result = CCLOVER_LINUX_HOST_WAYLAND_DISPLAY_IO_FAILED;
        goto cleanup;
    }
    wayland_set_initial_scale(&host);

    cclover_scene(&host.host, &scene);
    host.width = scene.width;
    host.height = scene.height;
    if (wayland_surface_create(&host) < 0) {
        result = CCLOVER_LINUX_HOST_WAYLAND_SURFACE_FAILED;
        goto cleanup;
    }
    fd = wl_display_get_fd(host.globals.display);

    while (!quitting) {
        struct pollfd pfds[3] = {
            { fd, POLLIN, 0 },
            { state_wake_fd, POLLIN, 0 },
            { quit_wake_fd, POLLIN, 0 },
        };
        uint32_t status;
        int poll_result;

        status = callbacks->take_state(context);
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
                if (host.layer_surface &&
                    host.surface_state != WAYLAND_SURFACE_RECREATE_PENDING) {
                    host.surface_state = WAYLAND_SURFACE_WAITING_CONFIGURE;
                    zwlr_layer_surface_v1_set_size(host.layer_surface,
                                                   host.width, host.height);
                    wl_surface_commit(host.surface);
                }
            }
            host.dirty = 1;
        }
        if (host.surface_state == WAYLAND_SURFACE_RECREATE_PENDING) {
            if (wayland_surface_recreate(&host) < 0) {
                result = CCLOVER_LINUX_HOST_WAYLAND_SURFACE_FAILED;
                break;
            }
        }
        if (host.surface_state == WAYLAND_SURFACE_ACTIVE && host.dirty) {
            int draw_result;
            int profiling = getenv("CCLOVER_RENDER_PROFILE") != NULL;
            double profile_started = profiling ? cclover_profile_cpu_ms() : 0.0;
            draw_result = wayland_draw(&host, &scene);
            if (profiling)
                fprintf(stderr, "draw cpu=%.3fms\n",
                        cclover_profile_cpu_ms() - profile_started);
            if (draw_result < 0) {
                result = CCLOVER_LINUX_HOST_WAYLAND_RENDER_FAILED;
                break;
            }
            if (draw_result == 0) host.dirty = 0;
        }

        if (wl_display_flush(host.globals.display) < 0 && errno != EAGAIN) {
            result = CCLOVER_LINUX_HOST_WAYLAND_DISPLAY_IO_FAILED;
            break;
        }
        poll_result = poll(pfds, 3, -1);
        if (poll_result < 0) {
            if (errno == EINTR) continue;
            result = CCLOVER_LINUX_HOST_WAYLAND_POLL_FAILED;
            break;
        }
        if (pfds[2].revents & POLLIN) {
            cclover_drain_wake_fd(quit_wake_fd);
            quitting = 1;
            continue;
        }
        if (pfds[0].revents & (POLLERR | POLLHUP | POLLNVAL)) {
            result = CCLOVER_LINUX_HOST_WAYLAND_DISPLAY_IO_FAILED;
            break;
        }
        if (pfds[0].revents & POLLIN) {
            if (wl_display_dispatch(host.globals.display) < 0) {
                result = CCLOVER_LINUX_HOST_WAYLAND_DISPLAY_IO_FAILED;
                break;
            }
        } else if (wl_display_dispatch_pending(host.globals.display) < 0) {
            result = CCLOVER_LINUX_HOST_WAYLAND_DISPLAY_IO_FAILED;
            break;
        }
        if (pfds[1].revents & POLLIN) cclover_drain_wake_fd(state_wake_fd);
    }

cleanup:
    wayland_surface_destroy(&host);
    for (i = 0; i < CCLOVER_WAYLAND_BUFFER_COUNT; ++i)
        wayland_buffer_destroy(&host.buffers[i]);
    wayland_static_layer_destroy(&host.static_layer);
    for (i = 0; i < CCLOVER_MAX_OUTPUTS; ++i)
        wayland_output_reset(&host.globals.outputs[i]);
    if (host.globals.layer_shell) zwlr_layer_shell_v1_destroy(host.globals.layer_shell);
    if (host.globals.shm) wl_shm_destroy(host.globals.shm);
    if (host.globals.compositor) wl_compositor_destroy(host.globals.compositor);
    if (registry) wl_registry_destroy(registry);
    wl_display_disconnect(host.globals.display);
    return result;
}
