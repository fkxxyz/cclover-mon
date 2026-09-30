#ifndef CCLOVER_LINUX_BITOPS_H
#define CCLOVER_LINUX_BITOPS_H
#include <linux/types.h>
#define BIT(nr) (1UL << (nr))
#define GENMASK(h, l) (((~0UL) << (l)) & (~0UL >> (sizeof(unsigned long) * 8 - 1 - (h))))
#define ARRAY_SIZE(a) (sizeof(a) / sizeof((a)[0]))
#endif
