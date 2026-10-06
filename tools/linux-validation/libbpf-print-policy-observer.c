#define _GNU_SOURCE

#include <bpf/libbpf.h>
#include <dlfcn.h>
#include <stdarg.h>
#include <stdio.h>

typedef libbpf_print_fn_t (*set_print_fn)(libbpf_print_fn_t);

static int emit(libbpf_print_fn_t fn, enum libbpf_print_level level, const char *format, ...) {
    va_list args;
    va_start(args, format);
    int result = fn(level, format, args);
    va_end(args);
    return result;
}

libbpf_print_fn_t libbpf_set_print(libbpf_print_fn_t fn) {
    static set_print_fn next_set_print;
    if (next_set_print == NULL) {
        next_set_print = (set_print_fn)dlsym(RTLD_NEXT, "libbpf_set_print");
    }
    if (next_set_print == NULL) {
        return NULL;
    }

    libbpf_print_fn_t previous = next_set_print(fn);
    if (fn != NULL) {
        (void)emit(fn, LIBBPF_WARN, "libbpf-policy-validation: warn\n");
        (void)emit(fn, LIBBPF_INFO, "libbpf-policy-validation: info\n");
        (void)emit(fn, LIBBPF_DEBUG, "libbpf-policy-validation: debug\n");
    }
    return previous;
}
