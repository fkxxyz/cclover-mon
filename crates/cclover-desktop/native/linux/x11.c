#include "host_result.h"
#include "render.h"
#include "x11_lifecycle.h"

#include <X11/Xatom.h>
#include <X11/Xlib.h>
#include <X11/Xutil.h>
#include <X11/extensions/Xrender.h>
#include <X11/extensions/shape.h>
#include <cairo/cairo-xlib.h>
#include <poll.h>
#include <stdint.h>
#include <string.h>
#include <unistd.h>

#define CCLOVER_MARGIN 16

static void x11_scene(void *context, const CcloverCallbacks *callbacks,
                      CcloverScene *scene) {
    memset(scene, 0, sizeof(*scene));
    callbacks->scene(context, scene);
}

static Atom x11_atom(Display *display, const char *name) {
    return XInternAtom(display, name, False);
}

static void x11_state(Display *display, Window window, Atom state) {
    XEvent event;
    memset(&event, 0, sizeof(event));
    event.xclient.type = ClientMessage;
    event.xclient.window = window;
    event.xclient.message_type = x11_atom(display, "_NET_WM_STATE");
    event.xclient.format = 32;
    event.xclient.data.l[0] = 1;
    event.xclient.data.l[1] = (long)state;
    XSendEvent(display, DefaultRootWindow(display), False,
               SubstructureRedirectMask | SubstructureNotifyMask, &event);
}

static void x11_place(Display *display, Window window, uint32_t width, uint32_t height) {
    int screen = DefaultScreen(display);
    int x = DisplayWidth(display, screen) - (int)width - CCLOVER_MARGIN;
    Atom moveresize = x11_atom(display, "_NET_MOVERESIZE_WINDOW");
    XEvent event;
    if (x < 0) x = 0;
    XMoveResizeWindow(display, window, x, CCLOVER_MARGIN, width, height);
    XLowerWindow(display, window);

    memset(&event, 0, sizeof(event));
    event.xclient.type = ClientMessage;
    event.xclient.window = window;
    event.xclient.message_type = moveresize;
    event.xclient.format = 32;
    event.xclient.data.l[0] = 10 | (1 << 8) | (1 << 9) | (1 << 12);
    event.xclient.data.l[1] = x;
    event.xclient.data.l[2] = CCLOVER_MARGIN;
    XSendEvent(display, DefaultRootWindow(display), False,
               SubstructureRedirectMask | SubstructureNotifyMask, &event);
}

static void x11_policy(Display *display, Window window) {
    typedef struct {
        unsigned long flags;
        unsigned long functions;
        unsigned long decorations;
        long input_mode;
        unsigned long status;
    } MotifHints;
    Atom states[3];
    Atom wm_state = x11_atom(display, "_NET_WM_STATE");
    Atom wm_desktop = x11_atom(display, "_NET_WM_DESKTOP");
    Atom motif_hints = x11_atom(display, "_MOTIF_WM_HINTS");
    unsigned long all_desktops = 0xffffffffUL;
    MotifHints motif = { 2, 0, 0, 0, 0 };
    XWMHints *hints = XAllocWMHints();

    states[0] = x11_atom(display, "_NET_WM_STATE_SKIP_TASKBAR");
    states[1] = x11_atom(display, "_NET_WM_STATE_SKIP_PAGER");
    states[2] = x11_atom(display, "_NET_WM_STATE_BELOW");
    XChangeProperty(display, window, wm_state, XA_ATOM, 32, PropModeReplace,
                    (unsigned char *)states, 3);
    XChangeProperty(display, window, wm_desktop, XA_CARDINAL, 32, PropModeReplace,
                    (unsigned char *)&all_desktops, 1);
    XChangeProperty(display, window, motif_hints, motif_hints, 32, PropModeReplace,
                    (unsigned char *)&motif, 5);
    if (hints) {
        hints->flags = InputHint;
        hints->input = False;
        XSetWMHints(display, window, hints);
        XFree(hints);
    }
    XShapeCombineRectangles(display, window, ShapeInput, 0, 0, NULL, 0,
                            ShapeSet, Unsorted);
}

static void cclover_drain_wake_fd(int fd) {
    char buffer[64];
    while (read(fd, buffer, sizeof(buffer)) > 0) {}
}

int cclover_linux_x11_run(void *context, const CcloverCallbacks *callbacks, int state_wake_fd, int quit_wake_fd) {
    CcloverCairoRenderer renderer = {0};
    CcloverScene scene;
    Display *display = XOpenDisplay(NULL);
    Window window;
    cairo_surface_t *surface;
    cairo_t *cr;
    int screen, fd;
    X11Lifecycle lifecycle;
    uint32_t width, height;
    XVisualInfo visual_info;
    Visual *visual;
    int depth;
    Colormap colormap;
    XSetWindowAttributes attrs;
    if (!display) return CCLOVER_LINUX_HOST_X11_DISPLAY_CONNECT_FAILED;
    cclover_x11_lifecycle_init(&lifecycle);

    x11_scene(context, callbacks, &scene);
    width = scene.width;
    height = scene.height;
    screen = DefaultScreen(display);
    visual = DefaultVisual(display, screen);
    depth = DefaultDepth(display, screen);
    if (XMatchVisualInfo(display, screen, 32, TrueColor, &visual_info)) {
        XRenderPictFormat *format = XRenderFindVisualFormat(display, visual_info.visual);
        if (format && format->type == PictTypeDirect && format->direct.alphaMask) {
            visual = visual_info.visual;
            depth = visual_info.depth;
        }
    }
    colormap = XCreateColormap(display, RootWindow(display, screen), visual, AllocNone);
    memset(&attrs, 0, sizeof(attrs));
    attrs.colormap = colormap;
    attrs.border_pixel = 0;
    attrs.background_pixel = 0;
    attrs.event_mask = ExposureMask | StructureNotifyMask;
    window = XCreateWindow(display, RootWindow(display, screen), 0, 0,
                           width, height, 0, depth, InputOutput, visual,
                           CWColormap | CWBorderPixel | CWBackPixel | CWEventMask,
                           &attrs);
    XStoreName(display, window, "cclover-mon");
    x11_policy(display, window);
    XMapWindow(display, window);
    x11_place(display, window, width, height);
    x11_state(display, window, x11_atom(display, "_NET_WM_STATE_SKIP_TASKBAR"));
    x11_state(display, window, x11_atom(display, "_NET_WM_STATE_SKIP_PAGER"));
    x11_state(display, window, x11_atom(display, "_NET_WM_STATE_BELOW"));
    XFlush(display);

    surface = cairo_xlib_surface_create(display, window, visual, width, height);
    cr = cairo_create(surface);
    cclover_cairo_configure_context(cr);
    fd = ConnectionNumber(display);

    while (lifecycle.running) {
        struct pollfd pfds[3] = {{ fd, POLLIN, 0 }, { state_wake_fd, POLLIN, 0 }, { quit_wake_fd, POLLIN, 0 }};
        uint32_t status = callbacks->take_state(context);
        cclover_x11_state_status(&lifecycle, status, CCLOVER_STATE_CHANGED);
        if (status & CCLOVER_STATE_CHANGED) {
            x11_scene(context, callbacks, &scene);
            if (scene.width != width || scene.height != height) {
                width = scene.width;
                height = scene.height;
                x11_place(display, window, width, height);
                cairo_xlib_surface_set_size(surface, width, height);
            }
        }
        while (XPending(display)) {
            XEvent event;
            XNextEvent(display, &event);
            if (event.type == Expose) cclover_x11_exposed(&lifecycle);
            if (event.type == DestroyNotify) cclover_x11_destroyed(&lifecycle);
        }
        if (!lifecycle.running) break;
        if (cclover_x11_can_draw(&lifecycle)) {
            cclover_cairo_draw_scene(&renderer, cr, &scene, 1, CCLOVER_CAIRO_DRAW_ALL);
            cairo_surface_flush(surface);
            XFlush(display);
            cclover_x11_draw_completed(&lifecycle);
        }
        if (poll(pfds, 3, -1) > 0) {
            if (pfds[2].revents & POLLIN) {
                cclover_drain_wake_fd(quit_wake_fd);
                cclover_x11_quit_requested(&lifecycle);
                continue;
            }
            if (pfds[1].revents & POLLIN) cclover_drain_wake_fd(state_wake_fd);
        }
    }

    cairo_destroy(cr);
    cairo_surface_destroy(surface);
    XDestroyWindow(display, window);
    XFreeColormap(display, colormap);
    XCloseDisplay(display);
    return 0;
}
