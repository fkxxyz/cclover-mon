#ifndef CCLOVER_LINUX_CPU_H
#define CCLOVER_LINUX_CPU_H
#include <linux/types.h>
#include <asm/processor.h>
struct cpumask { u64 bits[8]; };
struct ida { int next; };
static inline void ida_init(struct ida *ida) { ida->next = 0; }
static inline int ida_alloc_max(struct ida *ida, int max, int flags) { (void)flags; return ida->next <= max ? ida->next++ : -1; }
static inline void ida_free(struct ida *ida, int id) { (void)ida; (void)id; }
static inline void ida_destroy(struct ida *ida) { (void)ida; }
static inline int topology_logical_die_id(unsigned int cpu) { (void)cpu; return 0; }
static inline int topology_max_packages(void) { return 1; }
static inline int topology_max_dies_per_package(void) { return 1; }
static inline int topology_core_id(unsigned int cpu) { return (int)cpu; }
static inline const struct cpumask *topology_sibling_cpumask(unsigned int cpu) { static struct cpumask mask = {{1}}; (void)cpu; return &mask; }
static inline int cpumask_intersects(const struct cpumask *a, const struct cpumask *b) { for (int i = 0; i < 8; ++i) if (a->bits[i] & b->bits[i]) return 1; return 0; }
static inline void cpumask_set_cpu(unsigned int cpu, struct cpumask *mask) { mask->bits[cpu / 64] |= 1ULL << (cpu % 64); }
static inline void cpumask_clear_cpu(unsigned int cpu, struct cpumask *mask) { mask->bits[cpu / 64] &= ~(1ULL << (cpu % 64)); }
static inline int cpumask_empty(const struct cpumask *mask) { for (int i = 0; i < 8; ++i) if (mask->bits[i]) return 0; return 1; }
static inline int cpumask_first(const struct cpumask *mask) { for (int i = 0; i < 512; ++i) if (mask->bits[i / 64] & (1ULL << (i % 64))) return i; return 512; }
static inline int cpumask_any_and(const struct cpumask *a, const struct cpumask *b) { for (int i = 0; i < 512; ++i) if ((a->bits[i / 64] & b->bits[i / 64]) & (1ULL << (i % 64))) return i; return 512; }
#define nr_cpu_ids 512
#define X86_FEATURE_DTHERM 0
#define X86_FEATURE_PTS 1
#define cpu_has(c, feature) (1)
extern int cpuhp_tasks_frozen;
enum cpuhp_state { CPUHP_AP_ONLINE_DYN = 0 };
static inline int cpuhp_setup_state(enum cpuhp_state state, const char *name, int (*online)(unsigned int), int (*offline)(unsigned int)) { (void)state; (void)name; (void)online; (void)offline; return 1; }
static inline void cpuhp_remove_state(enum cpuhp_state state) { (void)state; }
#endif
