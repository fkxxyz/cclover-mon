#include <errno.h>
#include <stdint.h>

int bpf_map_lookup_batch(int fd,
                         void *in_batch,
                         void *out_batch,
                         void *keys,
                         void *values,
                         uint32_t *count,
                         const void *opts) {
    (void)fd;
    (void)in_batch;
    (void)out_batch;
    (void)keys;
    (void)values;
    (void)count;
    (void)opts;
    errno = EOPNOTSUPP;
    return -1;
}
