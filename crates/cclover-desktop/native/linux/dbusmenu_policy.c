#include "dbusmenu_policy.h"

#include <string.h>

CcloverDbusMenuMethod cclover_dbusmenu_method_from_name(const char *name) {
    if (name == NULL) return CCLOVER_DBUSMENU_METHOD_UNKNOWN;
    if (strcmp(name, "GetLayout") == 0) return CCLOVER_DBUSMENU_METHOD_GET_LAYOUT;
    if (strcmp(name, "GetGroupProperties") == 0)
        return CCLOVER_DBUSMENU_METHOD_GET_GROUP_PROPERTIES;
    if (strcmp(name, "GetProperty") == 0) return CCLOVER_DBUSMENU_METHOD_GET_PROPERTY;
    if (strcmp(name, "Event") == 0) return CCLOVER_DBUSMENU_METHOD_EVENT;
    if (strcmp(name, "EventGroup") == 0) return CCLOVER_DBUSMENU_METHOD_EVENT_GROUP;
    if (strcmp(name, "AboutToShow") == 0) return CCLOVER_DBUSMENU_METHOD_ABOUT_TO_SHOW;
    if (strcmp(name, "AboutToShowGroup") == 0)
        return CCLOVER_DBUSMENU_METHOD_ABOUT_TO_SHOW_GROUP;
    return CCLOVER_DBUSMENU_METHOD_UNKNOWN;
}

int cclover_dbusmenu_item_exists(int32_t id) {
    return id == 0 || id == 1;
}

CcloverDbusMenuEventResult cclover_dbusmenu_event(int32_t id, const char *event_id) {
    CcloverDbusMenuEventResult result = {0, CCLOVER_DBUSMENU_ACTION_NONE};
    if (!cclover_dbusmenu_item_exists(id)) return result;
    result.accepted = 1;
    if (id == 1 && event_id != NULL && strcmp(event_id, "clicked") == 0)
        result.action = CCLOVER_DBUSMENU_ACTION_QUIT;
    return result;
}
