#include "k10temp_bridge.h"
#include <errno.h>
#include <stdlib.h>
#include <string.h>
#include <linux/hwmon.h>
#include <asm/processor.h>

_Thread_local struct cpuinfo_x86 boot_cpu_data;

struct CcloverK10State {
    struct pci_bus bus;
    struct pci_dev pdev;
    CcloverK10Transport transport;
    u32 indexed_address;
};

static _Thread_local CcloverK10State *active_state;

u32 cpuid_ebx(u32 leaf) {
    (void)leaf;
    return 0;
}

int pci_read_config_dword(struct pci_dev *pdev, int where, u32 *value) {
    (void)pdev;
    if (!active_state || !active_state->transport.read_pci)
        return -EOPNOTSUPP;
    return active_state->transport.read_pci(active_state->transport.context, (u32)where, value);
}

int pci_bus_read_config_dword(struct pci_bus *bus, unsigned int devfn, int where, u32 *value) {
    (void)bus;
    if (!active_state || !value)
        return -EOPNOTSUPP;
    if (devfn == PCI_DEVFN(0, 0) && where == 0xbc && active_state->transport.read_indexed)
        return active_state->transport.read_indexed(active_state->transport.context,
                                                    active_state->indexed_address, value);
    *value = 0;
    return -EOPNOTSUPP;
}

int pci_bus_write_config_dword(struct pci_bus *bus, unsigned int devfn, int where, u32 value) {
    (void)bus;
    if (!active_state)
        return -EOPNOTSUPP;
    if (devfn == PCI_DEVFN(0, 0) && where == 0xb8) {
        active_state->indexed_address = value;
        return 0;
    }
    return -EOPNOTSUPP;
}

u16 amd_pci_dev_to_node_id(struct pci_dev *pdev) {
    (void)pdev;
    return 0;
}

int amd_smn_read(u16 node, u32 address, u32 *value) {
    (void)node;
    if (!active_state || !active_state->transport.read_smn)
        return -EOPNOTSUPP;
    return active_state->transport.read_smn(active_state->transport.context, address, value);
}

struct device *devm_hwmon_device_register_with_info(struct device *dev, const char *name,
                                                     void *drvdata,
                                                     const struct hwmon_chip_info *info,
                                                     const void *extra_groups) {
    (void)name; (void)info; (void)extra_groups;
    dev->drvdata = drvdata;
    return dev;
}

#include "k10temp.c"

static void activate(CcloverK10State *state) {
    active_state = state;
}

int cclover_k10_create(uint32_t family, uint32_t model, uint32_t stepping,
                       const char *brand, CcloverK10Transport transport,
                       CcloverK10State **out_state) {
    if (!out_state || !transport.read_smn || !transport.read_pci || !transport.read_indexed)
        return -EINVAL;
    if (family < 0x11 || family > 0x1a)
        return -EOPNOTSUPP;

    CcloverK10State *state = calloc(1, sizeof(*state));
    if (!state)
        return -ENOMEM;
    state->transport = transport;
    state->pdev.bus = &state->bus;
    state->pdev.devfn = PCI_DEVFN(0, 3);

    memset(&boot_cpu_data, 0, sizeof(boot_cpu_data));
    boot_cpu_data.x86 = family;
    boot_cpu_data.x86_model = model;
    boot_cpu_data.x86_stepping = stepping;
    if (brand) {
        size_t length = strlen(brand);
        if (length >= sizeof(boot_cpu_data.x86_model_id))
            length = sizeof(boot_cpu_data.x86_model_id) - 1;
        memcpy(boot_cpu_data.x86_model_id, brand, length);
        boot_cpu_data.x86_model_id[length] = '\0';
    }

    activate(state);
    int result = k10temp_probe(&state->pdev, NULL);
    active_state = NULL;
    if (result) {
        free(state->pdev.dev.drvdata);
        free(state);
        return result;
    }
    *out_state = state;
    return 0;
}

void cclover_k10_destroy(CcloverK10State *state) {
    if (!state)
        return;
    free(state->pdev.dev.drvdata);
    free(state);
}

int cclover_k10_channel_visible(CcloverK10State *state, uint32_t channel) {
    if (!state || channel >= 14)
        return 0;
    struct k10temp_data *data = state->pdev.dev.drvdata;
    return k10temp_is_visible(data, hwmon_temp, hwmon_temp_input, (int)channel) != 0;
}

int cclover_k10_read_millidegrees(CcloverK10State *state, uint32_t channel, long *value) {
    if (!state || !value || channel >= 14)
        return -EINVAL;
    activate(state);
    int result = k10temp_read_temp(&state->pdev.dev, hwmon_temp_input, (int)channel, value);
    active_state = NULL;
    return result;
}

const char *cclover_k10_channel_label(CcloverK10State *state, uint32_t channel) {
    const char *label = NULL;
    if (!state || channel >= 14)
        return NULL;
    if (k10temp_read_labels(&state->pdev.dev, hwmon_temp, hwmon_temp_label, (int)channel, &label))
        return NULL;
    return label;
}

uint32_t cclover_k10_max_channels(void) {
    return 14;
}
