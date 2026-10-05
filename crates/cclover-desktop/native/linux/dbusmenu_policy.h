#ifndef CCLOVER_LINUX_DBUSMENU_POLICY_H
#define CCLOVER_LINUX_DBUSMENU_POLICY_H

#include "dbusmenu_contract.h"

#include <stdint.h>

typedef enum {
#define CCLOVER_DBUSMENU_METHOD_ENUM(symbol, name, xml) CCLOVER_DBUSMENU_METHOD_##symbol,
    CCLOVER_DBUSMENU_METHODS(CCLOVER_DBUSMENU_METHOD_ENUM)
#undef CCLOVER_DBUSMENU_METHOD_ENUM
    CCLOVER_DBUSMENU_METHOD_COUNT,
    CCLOVER_DBUSMENU_METHOD_UNKNOWN = -1,
} CcloverDbusMenuMethod;

typedef enum {
    CCLOVER_DBUSMENU_ITEM_ROOT = 0,
    CCLOVER_DBUSMENU_ITEM_QUIT = 1,
} CcloverDbusMenuItem;

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
