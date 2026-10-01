#ifndef CCLOVER_K8TEMP_BRIDGE_H
#define CCLOVER_K8TEMP_BRIDGE_H
#include <stdint.h>

typedef int (*cclover_k8_read_thermtrip_fn)(void *context, uint32_t core, uint32_t *value);

typedef struct {
    void *context;
    cclover_k8_read_thermtrip_fn read_thermtrip;
} CcloverK8Transport;

typedef struct CcloverK8State CcloverK8State;

int cclover_k8_create(uint32_t family, uint32_t model, uint32_t stepping,
                      uint32_t cpuid_80000001_ebx, CcloverK8Transport transport,
                      CcloverK8State **out_state);
void cclover_k8_destroy(CcloverK8State *state);
int cclover_k8_channel_visible(CcloverK8State *state, uint32_t channel);
int cclover_k8_read_millidegrees(CcloverK8State *state, uint32_t channel, long *value);
uint32_t cclover_k8_max_channels(void);

#endif
