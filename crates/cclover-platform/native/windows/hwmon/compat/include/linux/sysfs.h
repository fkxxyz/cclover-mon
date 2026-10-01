#ifndef CCLOVER_LINUX_SYSFS_H
#define CCLOVER_LINUX_SYSFS_H
#include <linux/pci.h>
struct attribute { const char *name; umode_t mode; };
struct device_attribute;
typedef ssize_t (*cclover_show_fn)(struct device *, struct device_attribute *, char *);
struct device_attribute { struct attribute attr; cclover_show_fn show; };
struct attribute_group { struct attribute **attrs; };
#define sysfs_attr_init(attr) ((void)(attr))
static inline int sysfs_create_group(struct kobject *kobj, const struct attribute_group *group) { (void)kobj; (void)group; return 0; }
static inline void sysfs_remove_group(struct kobject *kobj, const struct attribute_group *group) { (void)kobj; (void)group; }
#endif
