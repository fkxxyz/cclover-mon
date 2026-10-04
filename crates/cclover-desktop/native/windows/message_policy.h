#ifndef CCLOVER_WINDOWS_MESSAGE_POLICY_H
#define CCLOVER_WINDOWS_MESSAGE_POLICY_H

#include <stdint.h>

int cclover_win32_should_relay_state(int has_window, uint32_t message,
                                     uint32_t state_message);
int cclover_win32_state_requires_refresh(uint32_t status, uint32_t changed_flag);

#endif
