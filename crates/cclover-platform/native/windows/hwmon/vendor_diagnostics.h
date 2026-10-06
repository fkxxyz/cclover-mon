#ifndef CCLOVER_HWMON_VENDOR_DIAGNOSTICS_H
#define CCLOVER_HWMON_VENDOR_DIAGNOSTICS_H

/*
 * Keep accepted diagnostics scoped to unchanged, digest-pinned Linux hwmon
 * sources. Project-owned bridge code before and after the include retains the
 * compiler's normal warning policy.
 */
#if defined(__clang__)
#define CCLOVER_HWMON_VENDOR_WARNINGS_BEGIN                                                   \
    _Pragma("clang diagnostic push")                                                          \
    _Pragma("clang diagnostic ignored \"-Wsign-compare\"")                                    \
    _Pragma("clang diagnostic ignored \"-Wunused-parameter\"")                               \
    _Pragma("clang diagnostic ignored \"-Wunused-variable\"")                                \
    _Pragma("clang diagnostic ignored \"-Wunused-function\"")
#define CCLOVER_HWMON_VENDOR_WARNINGS_END _Pragma("clang diagnostic pop")
#else
#define CCLOVER_HWMON_VENDOR_WARNINGS_BEGIN
#define CCLOVER_HWMON_VENDOR_WARNINGS_END
#endif

#endif
