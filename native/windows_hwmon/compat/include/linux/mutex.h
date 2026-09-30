#ifndef CCLOVER_LINUX_MUTEX_H
#define CCLOVER_LINUX_MUTEX_H
struct mutex { int unused; };
#define DEFINE_MUTEX(name) struct mutex name
#define mutex_init(lock) ((lock)->unused = 0)
#define mutex_lock(lock) ((void)(lock))
#define mutex_unlock(lock) ((void)(lock))
#endif
