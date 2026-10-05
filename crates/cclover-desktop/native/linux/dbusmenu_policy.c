#include "dbusmenu_policy.h"

#include <string.h>

static const char *const METHOD_NAMES[] = {
#define CCLOVER_DBUSMENU_METHOD_NAME(symbol, name, xml) [CCLOVER_DBUSMENU_METHOD_##symbol] = name,
    CCLOVER_DBUSMENU_METHODS(CCLOVER_DBUSMENU_METHOD_NAME)
#undef CCLOVER_DBUSMENU_METHOD_NAME
};

_Static_assert(sizeof(METHOD_NAMES) / sizeof(METHOD_NAMES[0]) == CCLOVER_DBUSMENU_METHOD_COUNT,
               "every D-BusMenu method must have a wire name");

CcloverDbusMenuMethod cclover_dbusmenu_method_from_name(const char *name) {
    int method;
    if (name == NULL) return CCLOVER_DBUSMENU_METHOD_UNKNOWN;
    for (method = 0; method < CCLOVER_DBUSMENU_METHOD_COUNT; ++method) {
        if (strcmp(name, METHOD_NAMES[method]) == 0) return (CcloverDbusMenuMethod)method;
    }
    return CCLOVER_DBUSMENU_METHOD_UNKNOWN;
}

int cclover_dbusmenu_item_exists(int32_t id) {
    return id == CCLOVER_DBUSMENU_ITEM_ROOT || id == CCLOVER_DBUSMENU_ITEM_QUIT;
}

CcloverDbusMenuEventResult cclover_dbusmenu_event(int32_t id, const char *event_id) {
    CcloverDbusMenuEventResult result = {0, CCLOVER_DBUSMENU_ACTION_NONE};
    if (!cclover_dbusmenu_item_exists(id)) return result;
    result.accepted = 1;
    if (id == CCLOVER_DBUSMENU_ITEM_QUIT && event_id != NULL && strcmp(event_id, "clicked") == 0)
        result.action = CCLOVER_DBUSMENU_ACTION_QUIT;
    return result;
}
