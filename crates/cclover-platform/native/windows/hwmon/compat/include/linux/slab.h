#ifndef CCLOVER_LINUX_SLAB_H
#define CCLOVER_LINUX_SLAB_H
#include <linux/module.h>
#define kzalloc(size, flags) calloc(1, (size))
#define kcalloc(count, size, flags) calloc((count), (size))
#define kfree(ptr) free((ptr))
#endif
