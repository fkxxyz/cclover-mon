#ifndef CCLOVER_LINUX_DBUSMENU_POLICY_H
#define CCLOVER_LINUX_DBUSMENU_POLICY_H

#include <stdint.h>

typedef enum {
    CCLOVER_DBUSMENU_METHOD_GET_LAYOUT,
    CCLOVER_DBUSMENU_METHOD_GET_GROUP_PROPERTIES,
    CCLOVER_DBUSMENU_METHOD_GET_PROPERTY,
    CCLOVER_DBUSMENU_METHOD_EVENT,
    CCLOVER_DBUSMENU_METHOD_EVENT_GROUP,
    CCLOVER_DBUSMENU_METHOD_ABOUT_TO_SHOW,
    CCLOVER_DBUSMENU_METHOD_ABOUT_TO_SHOW_GROUP,
    CCLOVER_DBUSMENU_METHOD_COUNT,
    CCLOVER_DBUSMENU_METHOD_UNKNOWN = -1,
} CcloverDbusMenuMethod;

typedef enum {
    CCLOVER_DBUSMENU_ACTION_NONE,
    CCLOVER_DBUSMENU_ACTION_QUIT,
} CcloverDbusMenuAction;

typedef struct {
    int accepted;
    CcloverDbusMenuAction action;
} CcloverDbusMenuEventResult;

CcloverDbusMenuMethod cclover_dbusmenu_method_from_name(const char *name);
int cclover_dbusmenu_item_exists(int32_t id);
CcloverDbusMenuEventResult cclover_dbusmenu_event(int32_t id, const char *event_id);

#endif
