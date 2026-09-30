#ifndef CCLOVER_LINUX_MODULE_H
#define CCLOVER_LINUX_MODULE_H
#include <errno.h>
#include <stdlib.h>
#include <string.h>
#include <linux/types.h>
#include <linux/mutex.h>
#define MODULE_DESCRIPTION(...)
#define MODULE_AUTHOR(...)
#define MODULE_LICENSE(...)
#define MODULE_PARM_DESC(...)
#define MODULE_DEVICE_TABLE(...)
#define module_param(...)
#define module_param_named(...)
#define module_init(...)
#define module_exit(...)
#define module_pci_driver(...)
#define GFP_KERNEL 0
#define devm_kzalloc(dev, size, flags) calloc(1, (size))
#define dev_err(dev, ...) ((void)(dev))
#define dev_warn(dev, ...) ((void)(dev))
#define dev_notice(dev, ...) ((void)(dev))
#define pr_err(...) ((void)0)
#ifndef ARRAY_SIZE
#define ARRAY_SIZE(a) (sizeof(a) / sizeof((a)[0]))
#endif
#endif
