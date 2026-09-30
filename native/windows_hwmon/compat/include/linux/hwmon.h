#ifndef CCLOVER_LINUX_HWMON_H
#define CCLOVER_LINUX_HWMON_H
#include <linux/pci.h>
enum hwmon_sensor_types { hwmon_chip, hwmon_temp };
enum hwmon_temp_attributes { hwmon_temp_input, hwmon_temp_max, hwmon_temp_crit, hwmon_temp_crit_hyst, hwmon_temp_label };
#define HWMON_T_INPUT (1U << 0)
#define HWMON_T_MAX (1U << 1)
#define HWMON_T_CRIT (1U << 2)
#define HWMON_T_CRIT_HYST (1U << 3)
#define HWMON_T_LABEL (1U << 4)
struct hwmon_ops {
    umode_t (*is_visible)(const void *, enum hwmon_sensor_types, u32, int);
    int (*read)(struct device *, enum hwmon_sensor_types, u32, int, long *);
    int (*read_string)(struct device *, enum hwmon_sensor_types, u32, int, const char **);
};
struct hwmon_channel_info { int unused; };
struct hwmon_chip_info { const struct hwmon_ops *ops; const struct hwmon_channel_info * const *info; };
#define HWMON_CHANNEL_INFO(type, ...) (&(const struct hwmon_channel_info){0})
struct device *devm_hwmon_device_register_with_info(struct device *dev, const char *name, void *drvdata, const struct hwmon_chip_info *info, const void *extra_groups);
struct attribute_group;
struct device *hwmon_device_register_with_groups(struct device *dev, const char *name, void *drvdata, const struct attribute_group **groups);
void hwmon_device_unregister(struct device *dev);
#endif
