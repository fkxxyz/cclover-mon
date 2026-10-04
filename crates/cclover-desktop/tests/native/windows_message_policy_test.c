#include <assert.h>

#include "message_policy.h"

int main(void) {
    const uint32_t state_message = 0x8008u;
    const uint32_t changed = 1u << 3;

    assert(cclover_win32_should_relay_state(0, state_message, state_message));
    assert(!cclover_win32_should_relay_state(1, state_message, state_message));
    assert(!cclover_win32_should_relay_state(0, state_message + 1, state_message));

    assert(cclover_win32_state_requires_refresh(changed, changed));
    assert(cclover_win32_state_requires_refresh(changed | 1u, changed));
    assert(!cclover_win32_state_requires_refresh(0, changed));
    assert(!cclover_win32_state_requires_refresh(1u, changed));
    return 0;
}
