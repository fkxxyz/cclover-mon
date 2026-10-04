#include "message_policy.h"

int cclover_win32_should_relay_state(int has_window, uint32_t message,
                                     uint32_t state_message) {
    return !has_window && message == state_message;
}

int cclover_win32_state_requires_refresh(uint32_t status, uint32_t changed_flag) {
    return (status & changed_flag) != 0;
}
