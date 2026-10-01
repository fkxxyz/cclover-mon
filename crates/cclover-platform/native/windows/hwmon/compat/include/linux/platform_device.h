#ifndef CCLOVER_LINUX_PLATFORM_DEVICE_H
#define CCLOVER_LINUX_PLATFORM_DEVICE_H
#include <linux/pci.h>
#include <stdlib.h>
struct platform_device { struct device dev; int id; };
static inline struct platform_device *platform_device_alloc(const char *name, int id) { (void)name; struct platform_device *pdev = calloc(1, sizeof(*pdev)); if (pdev) pdev->id = id; return pdev; }
static inline int platform_device_add(struct platform_device *pdev) { (void)pdev; return 0; }
static inline void platform_device_put(struct platform_device *pdev) { free(pdev); }
static inline void platform_device_unregister(struct platform_device *pdev) { free(pdev); }
static inline void platform_set_drvdata(struct platform_device *pdev, void *data) { pdev->dev.drvdata = data; }
static inline void *platform_get_drvdata(struct platform_device *pdev) { return pdev->dev.drvdata; }
#endif
