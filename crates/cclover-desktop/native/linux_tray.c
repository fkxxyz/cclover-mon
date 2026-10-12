#include <gio/gio.h>
#include <cairo.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "linux/dbusmenu_policy.h"

typedef void (*CcloverTrayQuitFn)(void *context);

typedef struct {
    GMainContext *main_context;
    GMainLoop *main_loop;
    GDBusConnection *connection;
    GDBusNodeInfo *item_info;
    GDBusNodeInfo *menu_info;
    guint item_registration;
    guint menu_registration;
    guint watcher;
    GThread *thread;
    void *quit_context;
    CcloverTrayQuitFn quit_fn;
    GVariant *icon_pixmap;
} CcloverLinuxTray;

typedef struct {
    const uint8_t *data;
    size_t length;
    size_t offset;
} CcloverPngReader;

#define CCLOVER_TRAY_ICON_SIZE 64

static cairo_status_t read_png_bytes(void *closure, unsigned char *data, unsigned int length) {
    CcloverPngReader *reader = closure;
    if (length > reader->length - reader->offset) return CAIRO_STATUS_READ_ERROR;
    memcpy(data, reader->data + reader->offset, length);
    reader->offset += length;
    return CAIRO_STATUS_SUCCESS;
}

static GVariant *tray_icon_pixmap(const uint8_t *png, size_t png_length) {
    CcloverPngReader reader = { png, png_length, 0 };
    cairo_surface_t *source = cairo_image_surface_create_from_png_stream(read_png_bytes, &reader);
    cairo_surface_t *scaled = NULL;
    cairo_t *cr = NULL;
    GVariantBuilder pixmaps;
    GVariant *result = NULL;
    uint8_t *argb = NULL;

    if (cairo_surface_status(source) != CAIRO_STATUS_SUCCESS) goto done;
    scaled = cairo_image_surface_create(CAIRO_FORMAT_ARGB32,
                                        CCLOVER_TRAY_ICON_SIZE, CCLOVER_TRAY_ICON_SIZE);
    if (cairo_surface_status(scaled) != CAIRO_STATUS_SUCCESS) goto done;
    cr = cairo_create(scaled);
    cairo_scale(cr,
                (double)CCLOVER_TRAY_ICON_SIZE / cairo_image_surface_get_width(source),
                (double)CCLOVER_TRAY_ICON_SIZE / cairo_image_surface_get_height(source));
    cairo_set_source_surface(cr, source, 0, 0);
    cairo_paint(cr);
    cairo_surface_flush(scaled);

    int stride = cairo_image_surface_get_stride(scaled);
    const uint8_t *pixels = cairo_image_surface_get_data(scaled);
    argb = g_malloc(CCLOVER_TRAY_ICON_SIZE * CCLOVER_TRAY_ICON_SIZE * 4);
    for (int y = 0; y < CCLOVER_TRAY_ICON_SIZE; ++y) {
        for (int x = 0; x < CCLOVER_TRAY_ICON_SIZE; ++x) {
            uint32_t pixel;
            memcpy(&pixel, pixels + y * stride + x * 4, sizeof(pixel));
            uint8_t alpha = (uint8_t)(pixel >> 24);
            uint8_t *out = argb + (y * CCLOVER_TRAY_ICON_SIZE + x) * 4;
            out[0] = alpha;
            out[1] = alpha ? (uint8_t)MIN(255, (((pixel >> 16) & 0xff) * 255 + alpha / 2) / alpha) : 0;
            out[2] = alpha ? (uint8_t)MIN(255, (((pixel >> 8) & 0xff) * 255 + alpha / 2) / alpha) : 0;
            out[3] = alpha ? (uint8_t)MIN(255, ((pixel & 0xff) * 255 + alpha / 2) / alpha) : 0;
        }
    }

    g_variant_builder_init(&pixmaps, G_VARIANT_TYPE("a(iiay)"));
    g_variant_builder_add(
        &pixmaps, "(ii@ay)", CCLOVER_TRAY_ICON_SIZE, CCLOVER_TRAY_ICON_SIZE,
        g_variant_new_fixed_array(G_VARIANT_TYPE_BYTE, argb,
                                  CCLOVER_TRAY_ICON_SIZE * CCLOVER_TRAY_ICON_SIZE * 4,
                                  sizeof(uint8_t)));
    result = g_variant_builder_end(&pixmaps);

done:
    g_free(argb);
    if (cr) cairo_destroy(cr);
    if (scaled) cairo_surface_destroy(scaled);
    cairo_surface_destroy(source);
    return result;
}

static const char ITEM_XML[] =
    "<node>"
    " <interface name='org.kde.StatusNotifierItem'>"
    "  <method name='Activate'><arg type='i' direction='in'/><arg type='i' direction='in'/></method>"
    "  <method name='SecondaryActivate'><arg type='i' direction='in'/><arg type='i' direction='in'/></method>"
    "  <method name='ContextMenu'><arg type='i' direction='in'/><arg type='i' direction='in'/></method>"
    "  <method name='Scroll'><arg type='i' direction='in'/><arg type='s' direction='in'/></method>"
    "  <property name='Category' type='s' access='read'/>"
    "  <property name='Id' type='s' access='read'/>"
    "  <property name='Title' type='s' access='read'/>"
    "  <property name='Status' type='s' access='read'/>"
    "  <property name='IconName' type='s' access='read'/>"
    "  <property name='IconPixmap' type='a(iiay)' access='read'/>"
    "  <property name='ItemIsMenu' type='b' access='read'/>"
    "  <property name='Menu' type='o' access='read'/>"
    " </interface>"
    "</node>";

static const char MENU_XML[] =
    "<node>"
    " <interface name='com.canonical.dbusmenu'>"
#define CCLOVER_DBUSMENU_METHOD_XML(symbol, name, xml) xml
    CCLOVER_DBUSMENU_METHODS(CCLOVER_DBUSMENU_METHOD_XML)
#undef CCLOVER_DBUSMENU_METHOD_XML
    "  <signal name='ItemsPropertiesUpdated'>"
    "   <arg type='a(ia{sv})'/><arg type='a(ias)'/>"
    "  </signal>"
    "  <signal name='LayoutUpdated'><arg type='u'/><arg type='i'/></signal>"
    "  <property name='Version' type='u' access='read'/>"
    "  <property name='TextDirection' type='s' access='read'/>"
    "  <property name='Status' type='s' access='read'/>"
    "  <property name='IconThemePath' type='as' access='read'/>"
    " </interface>"
    "</node>";

static GVariant *menu_properties(int id) {
    GVariantBuilder properties;
    g_variant_builder_init(&properties, G_VARIANT_TYPE("a{sv}"));
    if (id == CCLOVER_DBUSMENU_ITEM_QUIT) {
        g_variant_builder_add(&properties, "{sv}", "label", g_variant_new_string("Quit"));
        g_variant_builder_add(&properties, "{sv}", "icon-name", g_variant_new_string("application-exit"));
        g_variant_builder_add(&properties, "{sv}", "enabled", g_variant_new_boolean(TRUE));
        g_variant_builder_add(&properties, "{sv}", "visible", g_variant_new_boolean(TRUE));
    } else {
        g_variant_builder_add(&properties, "{sv}", "children-display", g_variant_new_string("submenu"));
    }
    return g_variant_builder_end(&properties);
}

static GVariant *menu_layout(void) {
    GVariantBuilder no_children;
    g_variant_builder_init(&no_children, G_VARIANT_TYPE("av"));
    GVariant *item = g_variant_new("(i@a{sv}@av)", CCLOVER_DBUSMENU_ITEM_QUIT,
                                   menu_properties(CCLOVER_DBUSMENU_ITEM_QUIT),
                                   g_variant_builder_end(&no_children));

    GVariantBuilder children;
    g_variant_builder_init(&children, G_VARIANT_TYPE("av"));
    g_variant_builder_add(&children, "v", item);
    return g_variant_new("(i@a{sv}@av)", CCLOVER_DBUSMENU_ITEM_ROOT,
                         menu_properties(CCLOVER_DBUSMENU_ITEM_ROOT),
                         g_variant_builder_end(&children));
}

static void item_method_call(GDBusConnection *connection, const gchar *sender, const gchar *object_path,
                             const gchar *interface_name, const gchar *method_name, GVariant *parameters,
                             GDBusMethodInvocation *invocation, gpointer user_data) {
    (void)connection; (void)sender; (void)object_path; (void)interface_name; (void)method_name; (void)parameters; (void)user_data;
    g_dbus_method_invocation_return_value(invocation, NULL);
}

static GVariant *item_get_property(GDBusConnection *connection, const gchar *sender, const gchar *object_path,
                                   const gchar *interface_name, const gchar *property_name, GError **error,
                                   gpointer user_data) {
    (void)connection; (void)sender; (void)object_path; (void)interface_name; (void)error;
    if (g_str_equal(property_name, "Category")) return g_variant_new_string("SystemServices");
    if (g_str_equal(property_name, "Id")) return g_variant_new_string("cclover-mon");
    if (g_str_equal(property_name, "Title")) return g_variant_new_string("cclover-mon");
    if (g_str_equal(property_name, "Status")) return g_variant_new_string("Active");
    if (g_str_equal(property_name, "IconName")) return g_variant_new_string("cclover-mon");
    if (g_str_equal(property_name, "IconPixmap"))
        return g_variant_ref(((CcloverLinuxTray *)user_data)->icon_pixmap);
    if (g_str_equal(property_name, "ItemIsMenu")) return g_variant_new_boolean(FALSE);
    if (g_str_equal(property_name, "Menu")) return g_variant_new_object_path("/Menu");
    return NULL;
}

static void menu_apply_action(CcloverLinuxTray *tray, CcloverDbusMenuAction action) {
    if (action == CCLOVER_DBUSMENU_ACTION_QUIT && tray->quit_fn != NULL) {
        tray->quit_fn(tray->quit_context);
    }
}

typedef void (*CcloverDbusMenuMethodFn)(CcloverLinuxTray *tray, GVariant *parameters,
                                        GDBusMethodInvocation *invocation);

static void menu_method_get_layout(CcloverLinuxTray *tray, GVariant *parameters,
                                   GDBusMethodInvocation *invocation) {
    (void)tray;
    (void)parameters;
    g_dbus_method_invocation_return_value(
        invocation, g_variant_new("(u@(ia{sv}av))", 1u, menu_layout()));
}

static void menu_method_get_group_properties(CcloverLinuxTray *tray, GVariant *parameters,
                                             GDBusMethodInvocation *invocation) {
    GVariantBuilder values;
    (void)tray;
    (void)parameters;
    g_variant_builder_init(&values, G_VARIANT_TYPE("a(ia{sv})"));
    g_variant_builder_add(&values, "(i@a{sv})", CCLOVER_DBUSMENU_ITEM_ROOT,
                          menu_properties(CCLOVER_DBUSMENU_ITEM_ROOT));
    g_variant_builder_add(&values, "(i@a{sv})", CCLOVER_DBUSMENU_ITEM_QUIT,
                          menu_properties(CCLOVER_DBUSMENU_ITEM_QUIT));
    g_dbus_method_invocation_return_value(
        invocation, g_variant_new("(@a(ia{sv}))", g_variant_builder_end(&values)));
}

static void menu_method_get_property(CcloverLinuxTray *tray, GVariant *parameters,
                                     GDBusMethodInvocation *invocation) {
    gint32 id;
    const gchar *name;
    GVariant *value = NULL;
    (void)tray;
    g_variant_get(parameters, "(i&s)", &id, &name);
    if (id == CCLOVER_DBUSMENU_ITEM_QUIT && g_str_equal(name, "label"))
        value = g_variant_new_string("Quit");
    else if (id == CCLOVER_DBUSMENU_ITEM_QUIT && g_str_equal(name, "icon-name"))
        value = g_variant_new_string("application-exit");
    else if (g_str_equal(name, "enabled") || g_str_equal(name, "visible"))
        value = g_variant_new_boolean(TRUE);
    else
        value = g_variant_new_string("");
    g_dbus_method_invocation_return_value(invocation, g_variant_new("(v)", value));
}

static void menu_method_event(CcloverLinuxTray *tray, GVariant *parameters,
                              GDBusMethodInvocation *invocation) {
    gint32 id;
    const gchar *event_id;
    GVariant *data;
    CcloverDbusMenuEventResult result;
    guint32 timestamp;
    g_variant_get(parameters, "(i&svu)", &id, &event_id, &data, &timestamp);
    (void)timestamp;
    result = cclover_dbusmenu_event(id, event_id);
    menu_apply_action(tray, result.action);
    g_variant_unref(data);
    g_dbus_method_invocation_return_value(invocation, NULL);
}

static void menu_method_event_group(CcloverLinuxTray *tray, GVariant *parameters,
                                    GDBusMethodInvocation *invocation) {
    GVariant *events = g_variant_get_child_value(parameters, 0);
    GVariantBuilder errors;
    g_variant_builder_init(&errors, G_VARIANT_TYPE("ai"));
    for (gsize i = 0; i < g_variant_n_children(events); ++i) {
        GVariant *event = g_variant_get_child_value(events, i);
        gint32 id;
        const gchar *event_id;
        GVariant *data;
        CcloverDbusMenuEventResult result;
        guint32 timestamp;
        g_variant_get(event, "(i&svu)", &id, &event_id, &data, &timestamp);
        (void)timestamp;
        result = cclover_dbusmenu_event(id, event_id);
        menu_apply_action(tray, result.action);
        if (!result.accepted) g_variant_builder_add(&errors, "i", id);
        g_variant_unref(data);
        g_variant_unref(event);
    }
    g_variant_unref(events);
    g_dbus_method_invocation_return_value(
        invocation, g_variant_new("(@ai)", g_variant_builder_end(&errors)));
}

static void menu_method_about_to_show(CcloverLinuxTray *tray, GVariant *parameters,
                                      GDBusMethodInvocation *invocation) {
    (void)tray;
    (void)parameters;
    g_dbus_method_invocation_return_value(invocation, g_variant_new("(b)", FALSE));
}

static void menu_method_about_to_show_group(CcloverLinuxTray *tray, GVariant *parameters,
                                            GDBusMethodInvocation *invocation) {
    GVariant *ids = g_variant_get_child_value(parameters, 0);
    GVariantBuilder updates;
    GVariantBuilder errors;
    (void)tray;
    g_variant_builder_init(&updates, G_VARIANT_TYPE("ai"));
    g_variant_builder_init(&errors, G_VARIANT_TYPE("ai"));
    for (gsize i = 0; i < g_variant_n_children(ids); ++i) {
        gint32 id;
        g_variant_get_child(ids, i, "i", &id);
        if (!cclover_dbusmenu_item_exists(id)) g_variant_builder_add(&errors, "i", id);
    }
    g_variant_unref(ids);
    g_dbus_method_invocation_return_value(
        invocation,
        g_variant_new("(@ai@ai)", g_variant_builder_end(&updates),
                      g_variant_builder_end(&errors)));
}

static const CcloverDbusMenuMethodFn MENU_METHOD_HANDLERS[] = {
    menu_method_get_layout,
    menu_method_get_group_properties,
    menu_method_get_property,
    menu_method_event,
    menu_method_event_group,
    menu_method_about_to_show,
    menu_method_about_to_show_group,
};

_Static_assert(sizeof(MENU_METHOD_HANDLERS) / sizeof(MENU_METHOD_HANDLERS[0]) ==
                   CCLOVER_DBUSMENU_METHOD_COUNT,
               "every D-BusMenu method must have a production handler");

static void menu_method_call(GDBusConnection *connection, const gchar *sender, const gchar *object_path,
                             const gchar *interface_name, const gchar *method_name, GVariant *parameters,
                             GDBusMethodInvocation *invocation, gpointer user_data) {
    CcloverDbusMenuMethod method;
    (void)connection; (void)sender; (void)object_path; (void)interface_name;
    CcloverLinuxTray *tray = (CcloverLinuxTray *)user_data;
    method = cclover_dbusmenu_method_from_name(method_name);
    if (method != CCLOVER_DBUSMENU_METHOD_UNKNOWN) {
        MENU_METHOD_HANDLERS[method](tray, parameters, invocation);
        return;
    }
    g_dbus_method_invocation_return_dbus_error(
        invocation, "com.canonical.dbusmenu.Error.UnknownMethod", "unknown menu method");
}

static GVariant *menu_get_property(GDBusConnection *connection, const gchar *sender, const gchar *object_path,
                                   const gchar *interface_name, const gchar *property_name, GError **error,
                                   gpointer user_data) {
    (void)connection; (void)sender; (void)object_path; (void)interface_name; (void)error; (void)user_data;
    if (g_str_equal(property_name, "Version")) return g_variant_new_uint32(4);
    if (g_str_equal(property_name, "TextDirection")) return g_variant_new_string("ltr");
    if (g_str_equal(property_name, "Status")) return g_variant_new_string("normal");
    if (g_str_equal(property_name, "IconThemePath")) {
        GVariantBuilder paths;
        g_variant_builder_init(&paths, G_VARIANT_TYPE("as"));
        return g_variant_builder_end(&paths);
    }
    return NULL;
}

static const GDBusInterfaceVTable ITEM_VTABLE = { item_method_call, item_get_property, NULL, {0} };
static const GDBusInterfaceVTable MENU_VTABLE = { menu_method_call, menu_get_property, NULL, {0} };

static void watcher_appeared(GDBusConnection *connection, const gchar *name, const gchar *name_owner, gpointer user_data) {
    (void)name; (void)name_owner; (void)user_data;
    g_dbus_connection_call(connection, "org.kde.StatusNotifierWatcher", "/StatusNotifierWatcher",
                           "org.kde.StatusNotifierWatcher", "RegisterStatusNotifierItem",
                           g_variant_new("(s)", "/StatusNotifierItem"), NULL,
                           G_DBUS_CALL_FLAGS_NONE, -1, NULL, NULL, NULL);
}

static void watcher_vanished(GDBusConnection *connection, const gchar *name, gpointer user_data) {
    (void)connection; (void)name; (void)user_data;
}

static gpointer tray_thread(gpointer data) {
    CcloverLinuxTray *tray = (CcloverLinuxTray *)data;
    g_main_context_push_thread_default(tray->main_context);
    g_main_loop_run(tray->main_loop);
    g_main_context_pop_thread_default(tray->main_context);
    return NULL;
}

static gboolean quit_main_loop(gpointer data) {
    g_main_loop_quit((GMainLoop *)data);
    return G_SOURCE_REMOVE;
}

int cclover_linux_tray_start(const uint8_t *icon_png, size_t icon_png_len,
                             void *quit_context, CcloverTrayQuitFn quit_fn, CcloverLinuxTray **out) {
    if (out == NULL || quit_fn == NULL) return 1;
    *out = NULL;
    CcloverLinuxTray *tray = g_new0(CcloverLinuxTray, 1);
    tray->quit_context = quit_context;
    tray->quit_fn = quit_fn;
    GVariant *icon_pixmap = tray_icon_pixmap(icon_png, icon_png_len);
    tray->icon_pixmap = icon_pixmap ? g_variant_ref_sink(icon_pixmap) : NULL;
    if (!tray->icon_pixmap) {
        fprintf(stderr, "cclover-mon: failed to decode embedded tray icon\n");
        tray->icon_pixmap = g_variant_ref_sink(
            g_variant_new_array(G_VARIANT_TYPE("(iiay)"), NULL, 0));
    }
    tray->main_context = g_main_context_new();
    g_main_context_push_thread_default(tray->main_context);

    GError *error = NULL;
    tray->connection = g_bus_get_sync(G_BUS_TYPE_SESSION, NULL, &error);
    if (tray->connection == NULL) {
        fprintf(stderr, "cclover-mon: system tray unavailable: %s\n", error ? error->message : "session bus connection failed");
        g_clear_error(&error);
        g_main_context_pop_thread_default(tray->main_context);
        g_main_context_unref(tray->main_context);
        g_variant_unref(tray->icon_pixmap);
        g_free(tray);
        return 2;
    }

    tray->item_info = g_dbus_node_info_new_for_xml(ITEM_XML, &error);
    tray->menu_info = g_dbus_node_info_new_for_xml(MENU_XML, &error);
    if (tray->item_info == NULL || tray->menu_info == NULL) goto fail;

    tray->item_registration = g_dbus_connection_register_object(
        tray->connection, "/StatusNotifierItem", tray->item_info->interfaces[0], &ITEM_VTABLE, tray, NULL, &error);
    if (tray->item_registration == 0) goto fail;
    tray->menu_registration = g_dbus_connection_register_object(
        tray->connection, "/Menu", tray->menu_info->interfaces[0], &MENU_VTABLE, tray, NULL, &error);
    if (tray->menu_registration == 0) goto fail;

    tray->watcher = g_bus_watch_name_on_connection(
        tray->connection, "org.kde.StatusNotifierWatcher", G_BUS_NAME_WATCHER_FLAGS_NONE,
        watcher_appeared, watcher_vanished, tray, NULL);
    tray->main_loop = g_main_loop_new(tray->main_context, FALSE);
    g_main_context_pop_thread_default(tray->main_context);
    tray->thread = g_thread_new("cclover-tray", tray_thread, tray);
    *out = tray;
    return 0;

fail:
    fprintf(stderr, "cclover-mon: system tray unavailable: %s\n", error ? error->message : "D-Bus object registration failed");
    g_clear_error(&error);
    if (tray->menu_registration) g_dbus_connection_unregister_object(tray->connection, tray->menu_registration);
    if (tray->item_registration) g_dbus_connection_unregister_object(tray->connection, tray->item_registration);
    if (tray->menu_info) g_dbus_node_info_unref(tray->menu_info);
    if (tray->item_info) g_dbus_node_info_unref(tray->item_info);
    if (tray->icon_pixmap) g_variant_unref(tray->icon_pixmap);
    g_object_unref(tray->connection);
    g_main_context_pop_thread_default(tray->main_context);
    g_main_context_unref(tray->main_context);
    g_free(tray);
    return 3;
}

void cclover_linux_tray_stop(CcloverLinuxTray *tray) {
    if (tray == NULL) return;
    g_main_context_invoke(tray->main_context, quit_main_loop, tray->main_loop);
    if (tray->thread) g_thread_join(tray->thread);
    if (tray->watcher) g_bus_unwatch_name(tray->watcher);
    if (tray->menu_registration) g_dbus_connection_unregister_object(tray->connection, tray->menu_registration);
    if (tray->item_registration) g_dbus_connection_unregister_object(tray->connection, tray->item_registration);
    g_main_loop_unref(tray->main_loop);
    g_dbus_node_info_unref(tray->menu_info);
    g_dbus_node_info_unref(tray->item_info);
    g_variant_unref(tray->icon_pixmap);
    g_object_unref(tray->connection);
    g_main_context_unref(tray->main_context);
    g_free(tray);
}
