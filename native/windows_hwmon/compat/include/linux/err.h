#ifndef CCLOVER_LINUX_ERR_H
#define CCLOVER_LINUX_ERR_H
#include <stdint.h>
#define IS_ERR(ptr) ((uintptr_t)(ptr) >= (uintptr_t)-4095)
#define PTR_ERR(ptr) ((long)(intptr_t)(ptr))
#define ERR_PTR(err) ((void *)(intptr_t)(err))
#define PTR_ERR_OR_ZERO(ptr) (IS_ERR(ptr) ? (int)PTR_ERR(ptr) : 0)
#endif
