#include <gio/gio.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>

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
} CcloverLinuxTray;

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
    "  <property name='ItemIsMenu' type='b' access='read'/>"
    "  <property name='Menu' type='o' access='read'/>"
    " </interface>"
    "</node>";

static const char MENU_XML[] =
    "<node>"
    " <interface name='com.canonical.dbusmenu'>"
    "  <method name='GetLayout'>"
    "   <arg type='i' direction='in'/><arg type='i' direction='in'/><arg type='as' direction='in'/>"
    "   <arg type='u' direction='out'/><arg type='(ia{sv}av)' direction='out'/>"
    "  </method>"
    "  <method name='GetGroupProperties'>"
    "   <arg type='ai' direction='in'/><arg type='as' direction='in'/><arg type='a(ia{sv})' direction='out'/>"
    "  </method>"
    "  <method name='GetProperty'>"
    "   <arg type='i' direction='in'/><arg type='s' direction='in'/><arg type='v' direction='out'/>"
    "  </method>"
    "  <method name='Event'>"
    "   <arg type='i' direction='in'/><arg type='s' direction='in'/><arg type='v' direction='in'/><arg type='u' direction='in'/>"
    "  </method>"
    "  <method name='AboutToShow'><arg type='i' direction='in'/><arg type='b' direction='out'/></method>"
    "  <property name='Version' type='u' access='read'/>"
    "  <property name='TextDirection' type='s' access='read'/>"
    "  <property name='Status' type='s' access='read'/>"
    "  <property name='IconThemePath' type='as' access='read'/>"
    " </interface>"
    "</node>";

static GVariant *menu_properties(int id) {
    GVariantBuilder properties;
    g_variant_builder_init(&properties, G_VARIANT_TYPE("a{sv}"));
    if (id == 1) {
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
    GVariant *item = g_variant_new("(i@a{sv}@av)", 1, menu_properties(1), g_variant_builder_end(&no_children));

    GVariantBuilder children;
    g_variant_builder_init(&children, G_VARIANT_TYPE("av"));
    g_variant_builder_add(&children, "v", item);
    return g_variant_new("(i@a{sv}@av)", 0, menu_properties(0), g_variant_builder_end(&children));
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
    (void)connection; (void)sender; (void)object_path; (void)interface_name; (void)error; (void)user_data;
    if (g_str_equal(property_name, "Category")) return g_variant_new_string("SystemServices");
    if (g_str_equal(property_name, "Id")) return g_variant_new_string("cclover-mon");
    if (g_str_equal(property_name, "Title")) return g_variant_new_string("cclover-mon");
    if (g_str_equal(property_name, "Status")) return g_variant_new_string("Active");
    if (g_str_equal(property_name, "IconName")) return g_variant_new_string("utilities-system-monitor");
    if (g_str_equal(property_name, "ItemIsMenu")) return g_variant_new_boolean(FALSE);
    if (g_str_equal(property_name, "Menu")) return g_variant_new_object_path("/Menu");
    return NULL;
}

static void menu_method_call(GDBusConnection *connection, const gchar *sender, const gchar *object_path,
                             const gchar *interface_name, const gchar *method_name, GVariant *parameters,
                             GDBusMethodInvocation *invocation, gpointer user_data) {
    (void)connection; (void)sender; (void)object_path; (void)interface_name;
    CcloverLinuxTray *tray = (CcloverLinuxTray *)user_data;
    if (g_str_equal(method_name, "GetLayout")) {
        g_dbus_method_invocation_return_value(invocation, g_variant_new("(u@(ia{sv}av))", 1u, menu_layout()));
        return;
    }
    if (g_str_equal(method_name, "GetGroupProperties")) {
        GVariantBuilder values;
        g_variant_builder_init(&values, G_VARIANT_TYPE("a(ia{sv})"));
        g_variant_builder_add(&values, "(i@a{sv})", 0, menu_properties(0));
        g_variant_builder_add(&values, "(i@a{sv})", 1, menu_properties(1));
        g_dbus_method_invocation_return_value(invocation, g_variant_new("(@a(ia{sv}))", g_variant_builder_end(&values)));
        return;
    }
    if (g_str_equal(method_name, "GetProperty")) {
        gint32 id;
        const gchar *name;
        g_variant_get(parameters, "(i&s)", &id, &name);
        GVariant *value = NULL;
        if (id == 1 && g_str_equal(name, "label")) value = g_variant_new_string("Quit");
        else if (id == 1 && g_str_equal(name, "icon-name")) value = g_variant_new_string("application-exit");
        else if (g_str_equal(name, "enabled") || g_str_equal(name, "visible")) value = g_variant_new_boolean(TRUE);
        else value = g_variant_new_string("");
        g_dbus_method_invocation_return_value(invocation, g_variant_new("(v)", value));
        return;
    }
    if (g_str_equal(method_name, "Event")) {
        gint32 id;
        const gchar *event_id;
        GVariant *data;
        guint32 timestamp;
        g_variant_get(parameters, "(i&svu)", &id, &event_id, &data, &timestamp);
        (void)data; (void)timestamp;
        if (id == 1 && g_str_equal(event_id, "clicked") && tray->quit_fn != NULL) {
            tray->quit_fn(tray->quit_context);
        }
        g_dbus_method_invocation_return_value(invocation, NULL);
        return;
    }
    if (g_str_equal(method_name, "AboutToShow")) {
        g_dbus_method_invocation_return_value(invocation, g_variant_new("(b)", FALSE));
        return;
    }
    g_dbus_method_invocation_return_dbus_error(invocation, "com.canonical.dbusmenu.Error.UnknownMethod", "unknown menu method");
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

int cclover_linux_tray_start(void *quit_context, CcloverTrayQuitFn quit_fn, CcloverLinuxTray **out) {
    if (out == NULL || quit_fn == NULL) return 1;
    *out = NULL;
    CcloverLinuxTray *tray = g_new0(CcloverLinuxTray, 1);
    tray->quit_context = quit_context;
    tray->quit_fn = quit_fn;
    tray->main_context = g_main_context_new();
    g_main_context_push_thread_default(tray->main_context);

    GError *error = NULL;
    tray->connection = g_bus_get_sync(G_BUS_TYPE_SESSION, NULL, &error);
    if (tray->connection == NULL) {
        fprintf(stderr, "cclover-mon: system tray unavailable: %s\n", error ? error->message : "session bus connection failed");
        g_clear_error(&error);
        g_main_context_pop_thread_default(tray->main_context);
        g_main_context_unref(tray->main_context);
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
    g_object_unref(tray->connection);
    g_main_context_unref(tray->main_context);
    g_free(tray);
}
