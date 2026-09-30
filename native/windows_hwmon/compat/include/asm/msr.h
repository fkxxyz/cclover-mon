#ifndef CCLOVER_ASM_MSR_H
#define CCLOVER_ASM_MSR_H
#include <linux/types.h>
#define MSR_IA32_THERM_STATUS 0x19c
#define MSR_IA32_TEMPERATURE_TARGET 0x1a2
#define MSR_IA32_PACKAGE_THERM_STATUS 0x1b1
int rdmsr_safe_on_cpu(unsigned int cpu, u32 msr, u32 *low, u32 *high);
void rdmsr_on_cpu(unsigned int cpu, u32 msr, u32 *low, u32 *high);
#endif
