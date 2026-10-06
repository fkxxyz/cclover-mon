#include "coretemp_bridge.h"
#include "vendor_diagnostics.h"
#define boot_cpu_data cclover_coretemp_boot_cpu_data
#include <errno.h>
#include <stdlib.h>
#include <string.h>
#include <linux/hwmon.h>
#include <linux/jiffies.h>
#include <asm/msr.h>
#include <asm/processor.h>

_Thread_local struct cpuinfo_x86 boot_cpu_data;
_Thread_local unsigned long jiffies;
int cpuhp_tasks_frozen;

struct CcloverCoretempState {
    CcloverCoretempTransport transport;
};

static _Thread_local CcloverCoretempState *active_state;
static _Thread_local int last_msr_error;

int rdmsr_safe_on_cpu(unsigned int cpu, u32 msr, u32 *low, u32 *high) {
    uint64_t value = 0;
    if (!active_state || !active_state->transport.read_msr)
        return -EOPNOTSUPP;
    int result = active_state->transport.read_msr(active_state->transport.context, cpu, msr, &value);
    if (result)
        return result;
    *low = (u32)value;
    *high = (u32)(value >> 32);
    return 0;
}

void rdmsr_on_cpu(unsigned int cpu, u32 msr, u32 *low, u32 *high) {
    int result = rdmsr_safe_on_cpu(cpu, msr, low, high);
    if (result) {
        last_msr_error = result;
        *low = 0;
        *high = 0;
    }
}

struct pci_dev *pci_get_domain_bus_and_slot(int domain, unsigned int bus, unsigned int devfn) {
    (void)domain; (void)bus; (void)devfn;
    return NULL;
}

void pci_dev_put(struct pci_dev *pdev) { (void)pdev; }

struct device *hwmon_device_register_with_groups(struct device *dev, const char *name,
                                                  void *drvdata,
                                                  const struct attribute_group **groups) {
    (void)name; (void)groups;
    dev->drvdata = drvdata;
    return dev;
}

void hwmon_device_unregister(struct device *dev) { (void)dev; }

CCLOVER_HWMON_VENDOR_WARNINGS_BEGIN
#include "coretemp.c"
CCLOVER_HWMON_VENDOR_WARNINGS_END

static int read_temperature(CcloverCoretempState *state, uint32_t cpu, int package, long *value) {
    if (!state || !value)
        return -EINVAL;

    struct temp_data tdata;
    memset(&tdata, 0, sizeof(tdata));
    tdata.cpu = cpu;
    tdata.index = package ? -1 : (int)cpu;
    tdata.status_reg = package ? MSR_IA32_PACKAGE_THERM_STATUS : MSR_IA32_THERM_STATUS;
    mutex_init(&tdata.update_lock);

    struct device dev = {0};
    char buffer[32];
    active_state = state;
    last_msr_error = 0;
    jiffies += HZ + 1;
    ssize_t length = show_temp(&dev, &tdata.sd_attrs[ATTR_TEMP], buffer);
    active_state = NULL;
    if (last_msr_error)
        return last_msr_error;
    if (length < 0)
        return (int)length;

    char *end = NULL;
    long parsed = strtol(buffer, &end, 10);
    if (end == buffer)
        return -EIO;
    *value = parsed;
    return 0;
}

int cclover_coretemp_create(uint32_t family, uint32_t model, uint32_t stepping,
                            const char *brand, CcloverCoretempTransport transport,
                            CcloverCoretempState **out_state) {
    if (!out_state || !transport.read_msr)
        return -EINVAL;
    if (family != 6)
        return -EOPNOTSUPP;

    CcloverCoretempState *state = calloc(1, sizeof(*state));
    if (!state)
        return -ENOMEM;
    state->transport = transport;

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

    *out_state = state;
    return 0;
}

void cclover_coretemp_destroy(CcloverCoretempState *state) { free(state); }

int cclover_coretemp_read_core_millidegrees(CcloverCoretempState *state, uint32_t cpu,
                                            long *value) {
    return read_temperature(state, cpu, 0, value);
}

int cclover_coretemp_read_package_millidegrees(CcloverCoretempState *state, uint32_t cpu,
                                               long *value) {
    return read_temperature(state, cpu, 1, value);
}
