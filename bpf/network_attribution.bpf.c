#include <linux/bpf.h>
#include <bpf/bpf_helpers.h>
#include <bpf/bpf_core_read.h>
#include <bpf/bpf_tracing.h>

char LICENSE[] SEC("license") = "GPL";

struct task_struct {
    struct task_struct *group_leader;
    __u64 start_boottime;
} __attribute__((preserve_access_index));
struct sock;
struct net;
struct net_device { int ifindex; } __attribute__((preserve_access_index));
struct sk_buff {
    int skb_iif;
    struct net_device *dev;
} __attribute__((preserve_access_index));
struct msghdr;
struct iov_iter;

struct network_key {
    __u32 tgid;
    __u32 ifindex;
    __u8 direction;
    __u8 pad[3];
};

struct counter_value {
    __u64 process_start_time;
    __u64 bytes;
};

struct owner {
    __u32 tgid;
    __u32 pad;
    __u64 process_start_time;
};

struct {
    __uint(type, BPF_MAP_TYPE_LRU_HASH);
    __uint(max_entries, 16384);
    __type(key, struct network_key);
    __type(value, struct counter_value);
} network_bytes SEC(".maps");

/* Long enough to bridge one sendmsg call to final interface selection. */
struct {
    __uint(type, BPF_MAP_TYPE_LRU_HASH);
    __uint(max_entries, 8192);
    __type(key, __u64);
    __type(value, struct owner);
} socket_owner SEC(".maps");

struct {
    __uint(type, BPF_MAP_TYPE_LRU_HASH);
    __uint(max_entries, 8192);
    __type(key, __u64);
    __type(value, __u32);
} socket_tx_ifindex SEC(".maps");

struct {
    __uint(type, BPF_MAP_TYPE_LRU_HASH);
    __uint(max_entries, 16384);
    __type(key, __u64);
    __type(value, __u32);
} skb_rx_ifindex SEC(".maps");

struct {
    __uint(type, BPF_MAP_TYPE_LRU_HASH);
    __uint(max_entries, 8192);
    __type(key, __u64);
    __type(value, __u32);
} datagram_rx_ifindex SEC(".maps");

static __always_inline __u32 current_tgid(void)
{
    return (__u32)(bpf_get_current_pid_tgid() >> 32);
}

static __always_inline __u64 current_process_start_time(void)
{
    struct task_struct *task = (struct task_struct *)bpf_get_current_task_btf();
    if (!task)
        return 0;
    struct task_struct *leader = BPF_CORE_READ(task, group_leader);
    if (!leader)
        leader = task;
    return BPF_CORE_READ(leader, start_boottime);
}

static __always_inline void add_network(const struct owner *owner, __u32 ifindex,
                                        __u8 direction, __u64 bytes)
{
    if (!owner || !owner->tgid || !ifindex || !bytes)
        return;

    struct network_key key = {
        .tgid = owner->tgid,
        .ifindex = ifindex,
        .direction = direction,
    };
    struct counter_value initial = {
        .process_start_time = owner->process_start_time,
        .bytes = bytes,
    };
    struct counter_value *value = bpf_map_lookup_elem(&network_bytes, &key);
    if (value && value->process_start_time == initial.process_start_time)
        __sync_fetch_and_add(&value->bytes, bytes);
    else
        bpf_map_update_elem(&network_bytes, &key, &initial, BPF_ANY);
}

static __always_inline void begin_send(struct sock *sk)
{
    if (!sk)
        return;
    __u64 sk_key = (__u64)sk;
    bpf_map_delete_elem(&socket_owner, &sk_key);
    bpf_map_delete_elem(&socket_tx_ifindex, &sk_key);
    struct owner owner = {
        .tgid = current_tgid(),
        .process_start_time = current_process_start_time(),
    };
    bpf_map_update_elem(&socket_owner, &sk_key, &owner, BPF_ANY);
}

SEC("fentry/tcp_sendmsg")
int BPF_PROG(on_tcp_sendmsg_enter, struct sock *sk, struct msghdr *msg, __u64 size)
{
    begin_send(sk);
    return 0;
}

SEC("fentry/udp_sendmsg")
int BPF_PROG(on_udp_sendmsg_enter, struct sock *sk, struct msghdr *msg, __u64 len)
{
    begin_send(sk);
    return 0;
}

static __always_inline int observe_tx_interface(struct sock *sk, struct sk_buff *skb)
{
    if (!sk || !skb)
        return 0;
    struct net_device *dev = BPF_CORE_READ(skb, dev);
    if (!dev)
        return 0;
    __u32 ifindex = BPF_CORE_READ(dev, ifindex);
    if (!ifindex)
        return 0;
    __u64 sk_key = (__u64)sk;
    bpf_map_update_elem(&socket_tx_ifindex, &sk_key, &ifindex, BPF_ANY);
    return 0;
}

SEC("fentry/ip_finish_output2")
int BPF_PROG(on_ip_finish_output2, struct net *net, struct sock *sk, struct sk_buff *skb)
{
    return observe_tx_interface(sk, skb);
}

SEC("fentry/ip6_finish_output2")
int BPF_PROG(on_ip6_finish_output2, struct net *net, struct sock *sk, struct sk_buff *skb)
{
    return observe_tx_interface(sk, skb);
}

static __always_inline void finish_send(struct sock *sk, int bytes)
{
    if (!sk)
        return;
    __u64 sk_key = (__u64)sk;
    struct owner *owner = bpf_map_lookup_elem(&socket_owner, &sk_key);
    __u32 *ifindex = bpf_map_lookup_elem(&socket_tx_ifindex, &sk_key);
    if (bytes > 0 && owner && ifindex)
        add_network(owner, *ifindex, 1, (__u64)bytes);
    bpf_map_delete_elem(&socket_owner, &sk_key);
    bpf_map_delete_elem(&socket_tx_ifindex, &sk_key);
}

SEC("fexit/tcp_sendmsg")
int BPF_PROG(on_tcp_sendmsg_exit, struct sock *sk, struct msghdr *msg, __u64 size, int ret)
{
    finish_send(sk, ret);
    return 0;
}

SEC("fexit/udp_sendmsg")
int BPF_PROG(on_udp_sendmsg_exit, struct sock *sk, struct msghdr *msg, __u64 len, int ret)
{
    finish_send(sk, ret);
    return 0;
}

static __always_inline void remember_rx_interface(struct sk_buff *skb)
{
    if (!skb)
        return;
    struct net_device *dev = BPF_CORE_READ(skb, dev);
    if (!dev)
        return;
    __u32 ifindex = BPF_CORE_READ(dev, ifindex);
    if (!ifindex)
        return;
    __u64 skb_key = (__u64)skb;
    bpf_map_update_elem(&skb_rx_ifindex, &skb_key, &ifindex, BPF_ANY);
}

SEC("fentry/netif_receive_skb")
int BPF_PROG(on_netif_receive_skb, struct sk_buff *skb)
{
    remember_rx_interface(skb);
    return 0;
}

SEC("fentry/netif_rx")
int BPF_PROG(on_netif_rx, struct sk_buff *skb)
{
    remember_rx_interface(skb);
    return 0;
}

SEC("fexit/__skb_recv_udp")
int BPF_PROG(on_skb_recv_udp, struct sock *sk, unsigned int flags, int *off, int *err,
             struct sk_buff *ret)
{
    if (!ret)
        return 0;
    int direct_ifindex = BPF_CORE_READ(ret, skb_iif);
    __u32 ifindex = direct_ifindex > 0 ? (__u32)direct_ifindex : 0;
    if (!ifindex) {
        struct net_device *dev = BPF_CORE_READ(ret, dev);
        if (dev)
            ifindex = (__u32)BPF_CORE_READ(dev, ifindex);
    }
    if (!ifindex)
        return 0;
    __u64 thread_key = bpf_get_current_pid_tgid();
    bpf_map_update_elem(&datagram_rx_ifindex, &thread_key, &ifindex, BPF_ANY);
    return 0;
}

SEC("fexit/udp_recvmsg")
int BPF_PROG(on_udp_recvmsg_exit, struct sock *sk, struct msghdr *msg, __u64 len,
             int flags, int ret)
{
    __u64 thread_key = bpf_get_current_pid_tgid();
    __u32 *ifindex = bpf_map_lookup_elem(&datagram_rx_ifindex, &thread_key);
    if (ret > 0 && ifindex) {
        struct owner owner = {
            .tgid = current_tgid(),
            .process_start_time = current_process_start_time(),
        };
        add_network(&owner, *ifindex, 0, (__u64)ret);
    }
    bpf_map_delete_elem(&datagram_rx_ifindex, &thread_key);
    return 0;
}

SEC("fentry/skb_copy_datagram_iter")
int BPF_PROG(on_skb_copy_datagram_iter, const struct sk_buff *skb, int offset,
             struct iov_iter *to, int len)
{
    if (!skb || len <= 0)
        return 0;
    __u64 thread_key = bpf_get_current_pid_tgid();
    if (bpf_map_lookup_elem(&datagram_rx_ifindex, &thread_key))
        return 0;
    int direct_ifindex = BPF_CORE_READ(skb, skb_iif);
    __u32 ifindex = direct_ifindex > 0 ? (__u32)direct_ifindex : 0;
    __u64 skb_key = (__u64)skb;
    if (!ifindex) {
        __u32 *remembered = bpf_map_lookup_elem(&skb_rx_ifindex, &skb_key);
        if (remembered)
            ifindex = *remembered;
    }
    bpf_map_delete_elem(&skb_rx_ifindex, &skb_key);
    if (!ifindex)
        return 0;
    struct owner owner = {
        .tgid = current_tgid(),
        .process_start_time = current_process_start_time(),
    };
    add_network(&owner, ifindex, 0, (__u64)len);
    return 0;
}
