#ifndef CCLOVER_ASM_CPU_DEVICE_ID_H
#define CCLOVER_ASM_CPU_DEVICE_ID_H
struct x86_cpu_id { unsigned int vendor; unsigned int feature; const void *driver_data; };
#define X86_MATCH_VENDOR_FEATURE(vendor, feature, data) {0, (feature), (data)}
static inline const struct x86_cpu_id *x86_match_cpu(const struct x86_cpu_id *ids) { return ids; }
#endif
