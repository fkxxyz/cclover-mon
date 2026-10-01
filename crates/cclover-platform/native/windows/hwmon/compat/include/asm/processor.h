#ifndef CCLOVER_ASM_PROCESSOR_H
#define CCLOVER_ASM_PROCESSOR_H
#include <linux/types.h>
struct cpuinfo_x86 {
    u32 x86;
    u32 x86_model;
    u32 x86_stepping;
    u32 microcode;
    char x86_model_id[64];
};
extern _Thread_local struct cpuinfo_x86 boot_cpu_data;
#define cpu_data(cpu) boot_cpu_data
u32 cpuid_ebx(u32 leaf);
#endif
