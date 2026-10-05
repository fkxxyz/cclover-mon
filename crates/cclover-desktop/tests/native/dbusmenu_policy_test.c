#include <assert.h>
#include <stddef.h>

#include "dbusmenu_policy.h"

static void test_method_dispatch(void) {
    assert(cclover_dbusmenu_method_from_name("GetLayout") ==
           CCLOVER_DBUSMENU_METHOD_GET_LAYOUT);
    assert(cclover_dbusmenu_method_from_name("GetGroupProperties") ==
           CCLOVER_DBUSMENU_METHOD_GET_GROUP_PROPERTIES);
    assert(cclover_dbusmenu_method_from_name("GetProperty") ==
           CCLOVER_DBUSMENU_METHOD_GET_PROPERTY);
    assert(cclover_dbusmenu_method_from_name("Event") == CCLOVER_DBUSMENU_METHOD_EVENT);
    assert(cclover_dbusmenu_method_from_name("EventGroup") ==
           CCLOVER_DBUSMENU_METHOD_EVENT_GROUP);
    assert(cclover_dbusmenu_method_from_name("AboutToShow") ==
           CCLOVER_DBUSMENU_METHOD_ABOUT_TO_SHOW);
    assert(cclover_dbusmenu_method_from_name("AboutToShowGroup") ==
           CCLOVER_DBUSMENU_METHOD_ABOUT_TO_SHOW_GROUP);
    assert(cclover_dbusmenu_method_from_name("Missing") == CCLOVER_DBUSMENU_METHOD_UNKNOWN);
    assert(cclover_dbusmenu_method_from_name(NULL) == CCLOVER_DBUSMENU_METHOD_UNKNOWN);
}

static void test_item_and_event_semantics(void) {
    CcloverDbusMenuEventResult result;

    assert(cclover_dbusmenu_item_exists(CCLOVER_DBUSMENU_ITEM_ROOT));
    assert(cclover_dbusmenu_item_exists(CCLOVER_DBUSMENU_ITEM_QUIT));
    assert(!cclover_dbusmenu_item_exists(2));

    result = cclover_dbusmenu_event(CCLOVER_DBUSMENU_ITEM_QUIT, "clicked");
    assert(result.accepted);
    assert(result.action == CCLOVER_DBUSMENU_ACTION_QUIT);

    result = cclover_dbusmenu_event(CCLOVER_DBUSMENU_ITEM_QUIT, "hovered");
    assert(result.accepted);
    assert(result.action == CCLOVER_DBUSMENU_ACTION_NONE);

    result = cclover_dbusmenu_event(CCLOVER_DBUSMENU_ITEM_ROOT, "clicked");
    assert(result.accepted);
    assert(result.action == CCLOVER_DBUSMENU_ACTION_NONE);

    result = cclover_dbusmenu_event(99, "clicked");
    assert(!result.accepted);
    assert(result.action == CCLOVER_DBUSMENU_ACTION_NONE);
}

int main(void) {
    test_method_dispatch();
    test_item_and_event_semantics();
    return 0;
}
