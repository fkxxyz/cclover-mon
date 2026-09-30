#ifndef CCLOVER_LINUX_JIFFIES_H
#define CCLOVER_LINUX_JIFFIES_H
#include <linux/types.h>
#define HZ 1000UL
extern _Thread_local unsigned long jiffies;
#define time_after(a, b) ((long)((b) - (a)) < 0)
#endif
