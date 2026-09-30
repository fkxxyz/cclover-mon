#ifndef CCLOVER_K10TEMP_BRIDGE_H
#define CCLOVER_K10TEMP_BRIDGE_H
#include <stddef.h>
#include <stdint.h>

typedef int (*cclover_k10_read_smn_fn)(void *context, uint32_t address, uint32_t *value);
typedef int (*cclover_k10_read_pci_fn)(void *context, uint32_t offset, uint32_t *value);
typedef int (*cclover_k10_read_indexed_fn)(void *context, uint32_t address, uint32_t *value);

typedef struct {
    void *context;
    cclover_k10_read_smn_fn read_smn;
    cclover_k10_read_pci_fn read_pci;
    cclover_k10_read_indexed_fn read_indexed;
} CcloverK10Transport;

typedef struct CcloverK10State CcloverK10State;

int cclover_k10_create(uint32_t family, uint32_t model, uint32_t stepping,
                       const char *brand, CcloverK10Transport transport,
                       CcloverK10State **out_state);
void cclover_k10_destroy(CcloverK10State *state);
int cclover_k10_channel_visible(CcloverK10State *state, uint32_t channel);
int cclover_k10_read_millidegrees(CcloverK10State *state, uint32_t channel, long *value);
const char *cclover_k10_channel_label(CcloverK10State *state, uint32_t channel);
uint32_t cclover_k10_max_channels(void);

#endif
