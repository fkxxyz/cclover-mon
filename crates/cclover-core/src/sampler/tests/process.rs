use std::time::{Duration, Instant};

use super::super::process::{
    PROCESS_IO_TOP_N, derive_process_disk_io_rates, derive_process_network_io_rates,
};
use super::super::*;
use super::{disk_id, network_id, process_id};

#[test]
fn derives_process_io_by_pid_and_native_identity() {
    let t = Instant::now();
    let old = RawSnapshot {
        collected_at: t,
        process_disk_io: Collection::available(vec![ProcessDiskIoCounter {
            process: process_id(10, 1),
            disk_id: disk_id("disk-a"),
            device: "old-disk-name".into(),
            read_bytes: 100,
            write_bytes: 200,
        }]),
        process_network_io: Collection::available(vec![ProcessNetworkIoCounter {
            process: process_id(20, 1),
            network_id: network_id("network-a"),
            interface: "old-network-name".into(),
            rx_bytes: 300,
            tx_bytes: 400,
        }]),
        ..RawSnapshot::default()
    };
    let new = RawSnapshot {
        collected_at: t + Duration::from_secs(2),
        processes: Collection::available(vec![
            ProcessCounter {
                process: process_id(10, 1),
                name: "disk-worker".into(),
                cpu_time_units: 0,
                rss_bytes: 0,
            },
            ProcessCounter {
                process: process_id(20, 1),
                name: "network-worker".into(),
                cpu_time_units: 0,
                rss_bytes: 0,
            },
        ]),
        process_disk_io: Collection::available(vec![ProcessDiskIoCounter {
            process: process_id(10, 1),
            disk_id: disk_id("disk-a"),
            device: "nvme0n1".into(),
            read_bytes: 500,
            write_bytes: 1000,
        }]),
        process_network_io: Collection::available(vec![ProcessNetworkIoCounter {
            process: process_id(20, 1),
            network_id: network_id("network-a"),
            interface: "eth0".into(),
            rx_bytes: 900,
            tx_bytes: 1400,
        }]),
        ..RawSnapshot::default()
    };

    let out = derive(Some(&old), &new);
    let disk = &out.process_disk_io.value().unwrap()[0];
    assert_eq!(disk.process.pid, 10);
    assert_eq!(disk.name.as_deref(), Some("disk-worker"));
    assert_eq!(disk.device, "nvme0n1");
    assert_eq!(disk.read_bytes_per_sec, 200.0);
    assert_eq!(disk.write_bytes_per_sec, 400.0);
    let network = &out.process_network_io.value().unwrap()[0];
    assert_eq!(network.process.pid, 20);
    assert_eq!(network.name.as_deref(), Some("network-worker"));
    assert_eq!(network.interface, "eth0");
    assert_eq!(network.rx_bytes_per_sec, 300.0);
    assert_eq!(network.tx_bytes_per_sec, 500.0);
}

#[test]
fn process_domain_unions_process_and_attribution_identities() {
    let current = RawSnapshot {
        cpu: Collection::available(CpuCounter {
            total_time_units: 1_000,
            idle_time_units: 500,
            logical_cpu_count: 4,
        }),
        processes: Collection::available(vec![ProcessCounter {
            process: process_id(1, 10),
            name: "known".into(),
            cpu_time_units: 100,
            rss_bytes: 4096,
        }]),
        process_disk_io: Collection::available(vec![ProcessDiskIoCounter {
            process: process_id(2, 20),
            disk_id: disk_id("disk-a"),
            device: "sda".into(),
            read_bytes: 100,
            write_bytes: 200,
        }]),
        process_network_io: Collection::available(vec![ProcessNetworkIoCounter {
            process: process_id(3, 30),
            network_id: network_id("network-a"),
            interface: "eth0".into(),
            rx_bytes: 300,
            tx_bytes: 400,
        }]),
        ..RawSnapshot::default()
    };

    let out = derive(None, &current);

    assert_eq!(out.processes.by_id.len(), 3);
    let known = &out.processes.by_id[&process_id(1, 10)];
    assert_eq!(known.name.as_deref(), Some("known"));
    assert_eq!(known.memory_bytes, Some(4096));
    assert_eq!(known.cpu_percent, Some(0.0));

    let disk_only = &out.processes.by_id[&process_id(2, 20)];
    assert_eq!(disk_only.name, None);
    assert_eq!(disk_only.memory_bytes, None);
    assert_eq!(disk_only.cpu_percent, None);
    assert_eq!(disk_only.disk_io.len(), 1);

    let network_only = &out.processes.by_id[&process_id(3, 30)];
    assert_eq!(network_only.name, None);
    assert_eq!(network_only.network_io.len(), 1);
    assert_eq!(out.processes.metadata_status, CollectionStatus::Available);
    assert_eq!(out.processes.cpu_status, CollectionStatus::Available);
    assert_eq!(out.processes.memory_status, CollectionStatus::Available);
    assert_eq!(out.processes.disk_io_status, CollectionStatus::Available);
    assert_eq!(out.processes.network_io_status, CollectionStatus::Available);
}

#[test]
fn process_io_does_not_reuse_delta_when_display_name_is_reused_by_new_identity() {
    let old_disk = [ProcessDiskIoCounter {
        process: process_id(10, 1),
        disk_id: disk_id("disk-old"),
        device: "sda".into(),
        read_bytes: 100,
        write_bytes: 200,
    }];
    let new_disk = [ProcessDiskIoCounter {
        process: process_id(10, 1),
        disk_id: disk_id("disk-new"),
        device: "sda".into(),
        read_bytes: 500,
        write_bytes: 1000,
    }];
    let old_network = [ProcessNetworkIoCounter {
        process: process_id(20, 1),
        network_id: network_id("network-old"),
        interface: "eth0".into(),
        rx_bytes: 300,
        tx_bytes: 400,
    }];
    let new_network = [ProcessNetworkIoCounter {
        process: process_id(20, 1),
        network_id: network_id("network-new"),
        interface: "eth0".into(),
        rx_bytes: 900,
        tx_bytes: 1400,
    }];

    let disk = derive_process_disk_io_rates(Some(&old_disk), &new_disk, 1.0);
    let network = derive_process_network_io_rates(Some(&old_network), &new_network, 1.0);

    assert_eq!(disk[0].1.read_bytes_per_sec, 0.0);
    assert_eq!(disk[0].1.write_bytes_per_sec, 0.0);
    assert_eq!(network[0].1.rx_bytes_per_sec, 0.0);
    assert_eq!(network[0].1.tx_bytes_per_sec, 0.0);
}

#[test]
fn process_domain_keeps_complete_io_before_card_top_n_projection() {
    let t = Instant::now();
    let mut old_disk: Vec<_> = (1..=4)
        .map(|pid| ProcessDiskIoCounter {
            process: process_id(pid, 1),
            disk_id: disk_id("disk-a"),
            device: "sda".into(),
            read_bytes: 0,
            write_bytes: 0,
        })
        .collect();
    old_disk.push(ProcessDiskIoCounter {
        process: process_id(5, 1),
        disk_id: disk_id("disk-b"),
        device: "sdb".into(),
        read_bytes: 0,
        write_bytes: 0,
    });
    let mut new_disk: Vec<_> = (1..=4)
        .map(|pid| ProcessDiskIoCounter {
            process: process_id(pid, 1),
            disk_id: disk_id("disk-a"),
            device: "sda".into(),
            read_bytes: u64::from(pid) * 100,
            write_bytes: u64::from(pid) * 10,
        })
        .collect();
    new_disk.push(ProcessDiskIoCounter {
        process: process_id(5, 1),
        disk_id: disk_id("disk-b"),
        device: "sdb".into(),
        read_bytes: 1,
        write_bytes: 0,
    });

    let mut old_network: Vec<_> = (1..=4)
        .map(|pid| ProcessNetworkIoCounter {
            process: process_id(pid, 1),
            network_id: network_id("network-a"),
            interface: "eth0".into(),
            rx_bytes: 0,
            tx_bytes: 0,
        })
        .collect();
    old_network.push(ProcessNetworkIoCounter {
        process: process_id(5, 1),
        network_id: network_id("network-b"),
        interface: "eth1".into(),
        rx_bytes: 0,
        tx_bytes: 0,
    });
    let mut new_network: Vec<_> = (1..=4)
        .map(|pid| ProcessNetworkIoCounter {
            process: process_id(pid, 1),
            network_id: network_id("network-a"),
            interface: "eth0".into(),
            rx_bytes: u64::from(pid) * 100,
            tx_bytes: u64::from(pid) * 10,
        })
        .collect();
    new_network.push(ProcessNetworkIoCounter {
        process: process_id(5, 1),
        network_id: network_id("network-b"),
        interface: "eth1".into(),
        rx_bytes: 1,
        tx_bytes: 0,
    });
    let processes = (1..=5)
        .map(|pid| ProcessCounter {
            process: process_id(pid, 1),
            name: format!("p{pid}").into(),
            cpu_time_units: 0,
            rss_bytes: u64::from(pid),
        })
        .collect();
    let old = RawSnapshot {
        collected_at: t,
        process_disk_io: Collection::available(old_disk),
        process_network_io: Collection::available(old_network),
        ..RawSnapshot::default()
    };
    let new = RawSnapshot {
        collected_at: t + Duration::from_secs(1),
        processes: Collection::available(processes),
        process_disk_io: Collection::available(new_disk),
        process_network_io: Collection::available(new_network),
        ..RawSnapshot::default()
    };

    let out = derive(Some(&old), &new);
    let disk = out.process_disk_io.value().unwrap();
    let network = out.process_network_io.value().unwrap();

    assert_eq!(disk.len(), PROCESS_IO_TOP_N + 1);
    assert_eq!(network.len(), PROCESS_IO_TOP_N + 1);
    assert_eq!(
        disk.iter()
            .filter(|row| row.disk_id == disk_id("disk-a"))
            .map(|row| row.process.pid)
            .collect::<Vec<_>>(),
        [4, 3, 2]
    );
    assert_eq!(
        network
            .iter()
            .filter(|row| row.network_id == network_id("network-a"))
            .map(|row| row.process.pid)
            .collect::<Vec<_>>(),
        [4, 3, 2]
    );
    assert_eq!(out.processes.by_id.len(), 5);
    assert_eq!(out.processes.by_id[&process_id(1, 1)].disk_io.len(), 1);
    assert_eq!(out.processes.by_id[&process_id(1, 1)].network_io.len(), 1);
    assert_eq!(disk[0].name.as_deref(), Some("p4"));
    assert_eq!(network[0].name.as_deref(), Some("p4"));
}

#[test]
fn attribution_counter_reset_does_not_create_a_rate_spike() {
    let t = Instant::now();
    let old = RawSnapshot {
        collected_at: t,
        process_disk_io: Collection::available(vec![ProcessDiskIoCounter {
            process: process_id(10, 1),
            disk_id: disk_id("disk-a"),
            device: "nvme0n1".into(),
            read_bytes: 10_000,
            write_bytes: 20_000,
        }]),
        process_network_io: Collection::available(vec![ProcessNetworkIoCounter {
            process: process_id(20, 1),
            network_id: network_id("network-a"),
            interface: "eth0".into(),
            rx_bytes: 30_000,
            tx_bytes: 40_000,
        }]),
        ..RawSnapshot::default()
    };
    let new = RawSnapshot {
        collected_at: t + Duration::from_secs(1),
        process_disk_io: Collection::available(vec![ProcessDiskIoCounter {
            process: process_id(10, 1),
            disk_id: disk_id("disk-a"),
            device: "nvme0n1".into(),
            read_bytes: 5,
            write_bytes: 7,
        }]),
        process_network_io: Collection::available(vec![ProcessNetworkIoCounter {
            process: process_id(20, 1),
            network_id: network_id("network-a"),
            interface: "eth0".into(),
            rx_bytes: 11,
            tx_bytes: 13,
        }]),
        ..RawSnapshot::default()
    };

    let out = derive(Some(&old), &new);
    let disk = &out.process_disk_io.value().unwrap()[0];
    assert_eq!(
        (disk.read_bytes_per_sec, disk.write_bytes_per_sec),
        (0.0, 0.0)
    );
    let network = &out.process_network_io.value().unwrap()[0];
    assert_eq!(
        (network.rx_bytes_per_sec, network.tx_bytes_per_sec),
        (0.0, 0.0)
    );
}

#[test]
fn pid_reuse_does_not_inherit_cpu_delta() {
    let t = Instant::now();
    let previous = RawSnapshot {
        collected_at: t,
        cpu: Collection::available(CpuCounter {
            total_time_units: 1_000,
            idle_time_units: 0,
            logical_cpu_count: 1,
        }),
        processes: Collection::available(vec![ProcessCounter {
            process: process_id(42, 100),
            name: "old".into(),
            cpu_time_units: 1_000,
            rss_bytes: 0,
        }]),
        ..RawSnapshot::default()
    };
    let current = RawSnapshot {
        collected_at: t + Duration::from_secs(1),
        cpu: Collection::available(CpuCounter {
            total_time_units: 1_100,
            idle_time_units: 0,
            logical_cpu_count: 1,
        }),
        processes: Collection::available(vec![ProcessCounter {
            process: process_id(42, 200),
            name: "new".into(),
            cpu_time_units: 25,
            rss_bytes: 0,
        }]),
        ..RawSnapshot::default()
    };

    let out = derive(Some(&previous), &current);
    let process = &out.processes.by_id[&process_id(42, 200)];
    assert_eq!(process.name.as_deref(), Some("new"));
    assert_eq!(process.cpu_percent, Some(0.0));
}

#[test]
fn pid_reuse_does_not_inherit_process_io_delta() {
    let old_disk = [ProcessDiskIoCounter {
        process: process_id(42, 100),
        disk_id: disk_id("disk-a"),
        device: "sda".into(),
        read_bytes: 10_000,
        write_bytes: 20_000,
    }];
    let new_disk = [ProcessDiskIoCounter {
        process: process_id(42, 200),
        disk_id: disk_id("disk-a"),
        device: "sda".into(),
        read_bytes: 100,
        write_bytes: 200,
    }];
    let old_network = [ProcessNetworkIoCounter {
        process: process_id(42, 100),
        network_id: network_id("network-a"),
        interface: "eth0".into(),
        rx_bytes: 30_000,
        tx_bytes: 40_000,
    }];
    let new_network = [ProcessNetworkIoCounter {
        process: process_id(42, 200),
        network_id: network_id("network-a"),
        interface: "eth0".into(),
        rx_bytes: 300,
        tx_bytes: 400,
    }];

    let disk = derive_process_disk_io_rates(Some(&old_disk), &new_disk, 1.0);
    let network = derive_process_network_io_rates(Some(&old_network), &new_network, 1.0);

    assert_eq!(disk[0].0, process_id(42, 200));
    assert_eq!(
        (disk[0].1.read_bytes_per_sec, disk[0].1.write_bytes_per_sec),
        (0.0, 0.0)
    );
    assert_eq!(network[0].0, process_id(42, 200));
    assert_eq!(
        (network[0].1.rx_bytes_per_sec, network[0].1.tx_bytes_per_sec),
        (0.0, 0.0)
    );
}

#[test]
fn preserves_attribution_unavailability() {
    let t = Instant::now();
    let old = RawSnapshot {
        collected_at: t,
        process_disk_io: Collection::available(Vec::new()),
        process_network_io: Collection::available(Vec::new()),
        ..RawSnapshot::default()
    };
    let new = RawSnapshot {
        collected_at: t + Duration::from_secs(1),
        process_disk_io: Collection::unavailable(CollectionUnavailable::PermissionDenied),
        process_network_io: Collection::unavailable(CollectionUnavailable::PermissionDenied),
        ..RawSnapshot::default()
    };

    let out = derive(Some(&old), &new);
    assert_eq!(
        out.process_disk_io.status(),
        CollectionStatus::Unavailable(CollectionUnavailable::PermissionDenied)
    );
    assert_eq!(
        out.process_network_io.status(),
        CollectionStatus::Unavailable(CollectionUnavailable::PermissionDenied)
    );
    assert_eq!(
        out.processes.disk_io_status,
        CollectionStatus::Unavailable(CollectionUnavailable::PermissionDenied)
    );
    assert_eq!(
        out.processes.network_io_status,
        CollectionStatus::Unavailable(CollectionUnavailable::PermissionDenied)
    );
}

#[test]
fn derivation_preserves_degraded_observation_status() {
    let current = RawSnapshot {
        networks: Collection::degraded(vec![NetworkCounter {
            id: network_id("network-a"),
            name: "eth0".into(),
            rx_bytes: 100,
            tx_bytes: 200,
        }]),
        process_disk_io: Collection::degraded(Vec::new()),
        ..RawSnapshot::default()
    };

    let out = derive(None, &current);

    assert_eq!(out.networks.status(), CollectionStatus::Degraded);
    assert_eq!(out.process_disk_io.status(), CollectionStatus::Degraded);
    assert_eq!(out.processes.disk_io_status, CollectionStatus::Degraded);
    assert_eq!(out.networks.value().unwrap().len(), 1);
    assert!(out.process_disk_io.value().unwrap().is_empty());
}
