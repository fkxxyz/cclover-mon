#ifndef CCLOVER_LINUX_SCHED_ISOLATION_H
#define CCLOVER_LINUX_SCHED_ISOLATION_H
#define HK_TYPE_MISC 0
static inline int housekeeping_cpu(unsigned int cpu, int type) { (void)cpu; (void)type; return 1; }
#endif
