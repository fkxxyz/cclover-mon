#include <stdarg.h>
#include <stdio.h>

#include <bpf/libbpf.h>

static int cclover_libbpf_trace_enabled;

static int cclover_libbpf_debug_print(
    enum libbpf_print_level level,
    const char *format,
    va_list args) {
    if (level == LIBBPF_DEBUG && !cclover_libbpf_trace_enabled) {
        return 0;
    }

    flockfile(stderr);
    fputs("[cclover-mon] ", stderr);
    int written = vfprintf(stderr, format, args);
    funlockfile(stderr);
    return written;
}

void cclover_libbpf_configure_print(int debug_enabled, int trace_enabled) {
    cclover_libbpf_trace_enabled = trace_enabled != 0;
    libbpf_set_print(debug_enabled ? cclover_libbpf_debug_print : NULL);
}
