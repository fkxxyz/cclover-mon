#include <linux/bpf.h>
#include <bpf/bpf_helpers.h>
#include <bpf/bpf_core_read.h>
#include <bpf/bpf_tracing.h>
char LICENSE[] SEC("license") = "GPL";
struct task_struct { struct task_struct *group_leader; __u64 start_boottime; } __attribute__((preserve_access_index));
struct super_block { __u32 s_dev; } __attribute__((preserve_access_index));
struct inode { struct super_block *i_sb; } __attribute__((preserve_access_index));
struct file { struct inode *f_inode; } __attribute__((preserve_access_index));
struct disk_key { __u32 tgid; __u32 dev; __u8 direction; __u8 pad[3]; };
struct counter_value { __u64 process_start_time; __u64 bytes; };
struct { __uint(type,BPF_MAP_TYPE_LRU_HASH); __uint(max_entries,16384); __type(key,struct disk_key); __type(value,struct counter_value); } disk_bytes SEC(".maps");
static __always_inline __u32 tgid(void){return (__u32)(bpf_get_current_pid_tgid()>>32);}
static __always_inline __u64 start_time(void){struct task_struct*t=(struct task_struct*)bpf_get_current_task_btf();struct task_struct*l=t?BPF_CORE_READ(t,group_leader):0;return l?BPF_CORE_READ(l,start_boottime):0;}
static __always_inline void add(struct file*f,__u8 d,long n){if(!f||n<=0)return;struct inode*i=BPF_CORE_READ(f,f_inode);struct super_block*sb=i?BPF_CORE_READ(i,i_sb):0;__u32 dev=sb?BPF_CORE_READ(sb,s_dev):0;if(!dev)return;struct disk_key k={.tgid=tgid(),.dev=dev,.direction=d};__u64 st=start_time();struct counter_value init={.process_start_time=st,.bytes=(__u64)n};struct counter_value*v=bpf_map_lookup_elem(&disk_bytes,&k);if(v&&v->process_start_time==st)__sync_fetch_and_add(&v->bytes,(__u64)n);else bpf_map_update_elem(&disk_bytes,&k,&init,BPF_ANY);}
SEC("fexit/vfs_read") int BPF_PROG(on_vfs_read,struct file*f,void*b,__u64 c,void*p,long ret){add(f,0,ret);return 0;}
SEC("fexit/vfs_write") int BPF_PROG(on_vfs_write,struct file*f,const void*b,__u64 c,void*p,long ret){add(f,1,ret);return 0;}
