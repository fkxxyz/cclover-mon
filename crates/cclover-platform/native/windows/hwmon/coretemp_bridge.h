#ifndef CCLOVER_CORETEMP_BRIDGE_H
#define CCLOVER_CORETEMP_BRIDGE_H

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef int (*CcloverCoretempReadMsrFn)(void *context, uint32_t cpu, uint32_t msr, uint64_t *value);

typedef struct {
    void *context;
    CcloverCoretempReadMsrFn read_msr;
} CcloverCoretempTransport;

typedef struct CcloverCoretempState CcloverCoretempState;

int cclover_coretemp_create(uint32_t family, uint32_t model, uint32_t stepping,
                            const char *brand, CcloverCoretempTransport transport,
                            CcloverCoretempState **out_state);
void cclover_coretemp_destroy(CcloverCoretempState *state);
int cclover_coretemp_read_core_millidegrees(CcloverCoretempState *state, uint32_t cpu,
                                            long *value);
int cclover_coretemp_read_package_millidegrees(CcloverCoretempState *state, uint32_t cpu,
                                               long *value);

#ifdef __cplusplus
}
#endif

#endif
