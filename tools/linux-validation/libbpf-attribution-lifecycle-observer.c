#define _GNU_SOURCE

#include <bpf/bpf.h>
#include <dlfcn.h>
#include <errno.h>
#include <fcntl.h>
#include <linux/bpf.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

#define TRACE_ENV "CCLOVER_MON_EBPF_VALIDATION_TRACE"

typedef int (*lookup_batch_fn)(int,
                               void *,
                               void *,
                               void *,
                               void *,
                               uint32_t *,
                               const struct bpf_map_batch_opts *);
typedef int (*get_next_key_fn)(int, const void *, void *);
typedef int (*delete_elem_fn)(int, const void *);
typedef int (*obj_get_info_fn)(int, void *, uint32_t *);

static bool is_attribution_map(const char *name) {
    return strcmp(name, "disk_bytes") == 0 || strcmp(name, "network_bytes") == 0;
}

static bool attribution_map_info(int fd, char name[BPF_OBJ_NAME_LEN], uint32_t *key_size) {
    static obj_get_info_fn next_obj_get_info;
    if (next_obj_get_info == NULL) {
        next_obj_get_info = (obj_get_info_fn)dlsym(RTLD_NEXT, "bpf_obj_get_info_by_fd");
    }
    if (next_obj_get_info == NULL) {
        return false;
    }

    struct bpf_map_info info = {0};
    uint32_t info_len = sizeof(info);
    int saved_errno = errno;
    int rc = next_obj_get_info(fd, &info, &info_len);
    errno = saved_errno;
    if (rc != 0 || !is_attribution_map(info.name) || info.key_size < sizeof(uint32_t)) {
        return false;
    }

    memcpy(name, info.name, BPF_OBJ_NAME_LEN);
    name[BPF_OBJ_NAME_LEN - 1] = '\0';
    *key_size = info.key_size;
    return true;
}

static void trace_line(const char *event, const char *map_name, const uint32_t *tgid) {
    const char *path = getenv(TRACE_ENV);
    if (path == NULL || path[0] == '\0') {
        return;
    }

    char line[96];
    int length = tgid == NULL
        ? snprintf(line, sizeof(line), "%s\t%s\n", event, map_name)
        : snprintf(line, sizeof(line), "%s\t%s\t%u\n", event, map_name, *tgid);
    if (length <= 0 || (size_t)length >= sizeof(line)) {
        return;
    }

    int fd = open(path, O_WRONLY | O_APPEND | O_CLOEXEC | O_CREAT, 0600);
    if (fd < 0) {
        return;
    }
    (void)write(fd, line, (size_t)length);
    (void)close(fd);
}

static void trace_key(const char *event, const char *map_name, const void *key) {
    uint32_t tgid;
    memcpy(&tgid, key, sizeof(tgid));
    trace_line(event, map_name, &tgid);
}

int bpf_map_lookup_batch(int fd,
                         void *in_batch,
                         void *out_batch,
                         void *keys,
                         void *values,
                         uint32_t *count,
                         const struct bpf_map_batch_opts *opts) {
    static lookup_batch_fn next_lookup_batch;
    if (next_lookup_batch == NULL) {
        next_lookup_batch = (lookup_batch_fn)dlsym(RTLD_NEXT, "bpf_map_lookup_batch");
    }
    if (next_lookup_batch == NULL) {
        errno = ENOSYS;
        return -1;
    }

    char map_name[BPF_OBJ_NAME_LEN];
    uint32_t key_size = 0;
    bool target = attribution_map_info(fd, map_name, &key_size);
    int rc = next_lookup_batch(fd, in_batch, out_batch, keys, values, count, opts);
    int call_errno = errno;

    if (target) {
        trace_line("scan", map_name, NULL);
        if (keys != NULL && count != NULL && (rc == 0 || call_errno == ENOENT)) {
            for (uint32_t index = 0; index < *count; ++index) {
                const void *key = (const char *)keys + ((size_t)index * key_size);
                trace_key("seen", map_name, key);
            }
        }
    }

    errno = call_errno;
    return rc;
}

int bpf_map_get_next_key(int fd, const void *key, void *next_key) {
    static get_next_key_fn next_get_next_key;
    if (next_get_next_key == NULL) {
        next_get_next_key = (get_next_key_fn)dlsym(RTLD_NEXT, "bpf_map_get_next_key");
    }
    if (next_get_next_key == NULL) {
        errno = ENOSYS;
        return -1;
    }

    char map_name[BPF_OBJ_NAME_LEN];
    uint32_t key_size = 0;
    bool target = attribution_map_info(fd, map_name, &key_size);
    (void)key_size;
    int rc = next_get_next_key(fd, key, next_key);
    int call_errno = errno;

    if (target) {
        trace_line("scan", map_name, NULL);
        if (rc == 0 && next_key != NULL) {
            trace_key("seen", map_name, next_key);
        }
    }

    errno = call_errno;
    return rc;
}

int bpf_map_delete_elem(int fd, const void *key) {
    static delete_elem_fn next_delete_elem;
    if (next_delete_elem == NULL) {
        next_delete_elem = (delete_elem_fn)dlsym(RTLD_NEXT, "bpf_map_delete_elem");
    }
    if (next_delete_elem == NULL) {
        errno = ENOSYS;
        return -1;
    }

    char map_name[BPF_OBJ_NAME_LEN];
    uint32_t key_size = 0;
    bool target = attribution_map_info(fd, map_name, &key_size);
    (void)key_size;
    int rc = next_delete_elem(fd, key);
    int call_errno = errno;

    if (target && rc == 0 && key != NULL) {
        trace_key("delete", map_name, key);
    }

    errno = call_errno;
    return rc;
}
