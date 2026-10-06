#include "k8temp_bridge.h"
#include "vendor_diagnostics.h"
#define boot_cpu_data cclover_k8_boot_cpu_data
#define cpuid_ebx cclover_k8_cpuid_ebx
#define pci_read_config_byte cclover_k8_pci_read_config_byte
#define pci_write_config_byte cclover_k8_pci_write_config_byte
#define pci_read_config_dword cclover_k8_pci_read_config_dword
#define devm_hwmon_device_register_with_info cclover_k8_devm_hwmon_device_register_with_info
#include <errno.h>
#include <stdlib.h>
#include <string.h>
#include <linux/hwmon.h>
#include <asm/processor.h>

_Thread_local struct cpuinfo_x86 boot_cpu_data;

struct CcloverK8State {
    struct pci_bus bus;
    struct pci_dev pdev;
    CcloverK8Transport transport;
    uint32_t cpuid_80000001_ebx;
    uint8_t selector;
};

static _Thread_local CcloverK8State *active_state;

u32 cpuid_ebx(u32 leaf) {
    if (!active_state || leaf != 0x80000001U)
        return 0;
    return active_state->cpuid_80000001_ebx;
}

int pci_read_config_byte(struct pci_dev *pdev, int where, u8 *value) {
    (void)pdev;
    if (!active_state || !value || where != 0xe4)
        return -EOPNOTSUPP;
    *value = active_state->selector;
    return 0;
}

int pci_write_config_byte(struct pci_dev *pdev, int where, u8 value) {
    (void)pdev;
    if (!active_state || where != 0xe4)
        return -EOPNOTSUPP;
    /* The signed PawnIO module exposes core selection only, not SEL_PLACE. */
    active_state->selector = value & 0x04;
    return 0;
}

int pci_read_config_dword(struct pci_dev *pdev, int where, u32 *value) {
    (void)pdev;
    if (!active_state || !value || where != 0xe4 || !active_state->transport.read_thermtrip)
        return -EOPNOTSUPP;
    uint32_t core = (active_state->selector & 0x04) ? 1U : 0U;
    if (boot_cpu_data.x86_model >= 0x40)
        core ^= 1U;
    return active_state->transport.read_thermtrip(active_state->transport.context, core, value);
}

struct device *devm_hwmon_device_register_with_info(struct device *dev, const char *name,
                                                     void *drvdata,
                                                     const struct hwmon_chip_info *info,
                                                     const void *extra_groups) {
    (void)name; (void)info; (void)extra_groups;
    dev->drvdata = drvdata;
    dev->parent = dev;
    return dev;
}

CCLOVER_HWMON_VENDOR_WARNINGS_BEGIN
#include "k8temp.c"
CCLOVER_HWMON_VENDOR_WARNINGS_END

int cclover_k8_create(uint32_t family, uint32_t model, uint32_t stepping,
                      uint32_t cpuid_80000001_ebx, CcloverK8Transport transport,
                      CcloverK8State **out_state) {
    if (!out_state || !transport.read_thermtrip)
        return -EINVAL;
    if (family != 0x0f)
        return -EOPNOTSUPP;

    CcloverK8State *state = calloc(1, sizeof(*state));
    if (!state)
        return -ENOMEM;
    state->transport = transport;
    state->cpuid_80000001_ebx = cpuid_80000001_ebx;
    state->pdev.bus = &state->bus;
    state->pdev.devfn = PCI_DEVFN(24, 3);
    state->pdev.vendor = PCI_VENDOR_ID_AMD;
    state->pdev.device = PCI_DEVICE_ID_AMD_K8_NB_MISC;

    memset(&boot_cpu_data, 0, sizeof(boot_cpu_data));
    boot_cpu_data.x86 = family;
    boot_cpu_data.x86_model = model;
    boot_cpu_data.x86_stepping = stepping;

    active_state = state;
    int result = k8temp_probe(&state->pdev, NULL);
    active_state = NULL;
    if (result) {
        free(state->pdev.dev.drvdata);
        free(state);
        return result;
    }
    *out_state = state;
    return 0;
}

void cclover_k8_destroy(CcloverK8State *state) {
    if (!state)
        return;
    free(state->pdev.dev.drvdata);
    free(state);
}

int cclover_k8_channel_visible(CcloverK8State *state, uint32_t channel) {
    if (!state || channel >= 4)
        return 0;
    struct k8temp_data *data = state->pdev.dev.drvdata;
    return k8temp_is_visible(data, hwmon_temp, hwmon_temp_input, (int)channel) != 0;
}

int cclover_k8_read_millidegrees(CcloverK8State *state, uint32_t channel, long *value) {
    if (!state || !value || channel >= 4)
        return -EINVAL;
    active_state = state;
    int result = k8temp_read(&state->pdev.dev, hwmon_temp, hwmon_temp_input, (int)channel, value);
    active_state = NULL;
    return result;
}

uint32_t cclover_k8_max_channels(void) {
    return 4;
}
