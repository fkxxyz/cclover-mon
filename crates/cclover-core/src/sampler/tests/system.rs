use std::time::{Duration, Instant};

use super::super::*;
use super::{disk_id, disk_metadata, network_id, process_id};

#[test]
fn sample_wait_fills_only_the_remaining_interval() {
    assert_eq!(
        remaining_sample_wait(Duration::from_millis(250)),
        Duration::from_millis(750)
    );
}

#[test]
fn sample_wait_is_zero_at_or_after_the_interval() {
    assert_eq!(remaining_sample_wait(SAMPLE_INTERVAL), Duration::ZERO);
    assert_eq!(
        remaining_sample_wait(SAMPLE_INTERVAL + Duration::from_millis(1)),
        Duration::ZERO
    );
}

#[test]
fn first_sample_publishes_observed_entities_with_zero_baseline_rates() {
    let current = RawSnapshot {
        cpu: Collection::available(CpuCounter {
            total_time_units: 1_000,
            idle_time_units: 600,
            logical_cpu_count: 4,
        }),
        memory: Collection::available(MemorySnapshot {
            used_bytes: 10,
            total_bytes: 20,
            swap_used_bytes: 1,
            swap_total_bytes: 2,
        }),
        processes: Collection::available(vec![ProcessCounter {
            process: process_id(10, 1),
            name: "worker".into(),
            cpu_time_units: 100,
            rss_bytes: 4096,
        }]),
        networks: Collection::available(vec![NetworkCounter {
            id: network_id("network-a"),
            name: "eth0".into(),
            rx_bytes: 1_000,
            tx_bytes: 2_000,
        }]),
        disks: Collection::available(vec![DiskCounter {
            id: disk_id("disk-a"),
            metadata: disk_metadata("nvme0n1"),
            read_bytes: 3_000,
            write_bytes: 4_000,
        }]),
        process_disk_io: Collection::available(vec![ProcessDiskIoCounter {
            process: process_id(10, 1),
            disk_id: disk_id("disk-a"),
            device: "nvme0n1".into(),
            read_bytes: 300,
            write_bytes: 400,
        }]),
        process_network_io: Collection::available(vec![ProcessNetworkIoCounter {
            process: process_id(10, 1),
            network_id: network_id("network-a"),
            interface: "eth0".into(),
            rx_bytes: 100,
            tx_bytes: 200,
        }]),
        temperatures: Collection::available(vec![TemperatureSnapshot {
            id: TemperatureId::from_opaque_key("cpu-temp"),
            name: "CPU".into(),
            celsius: 42.0,
        }]),
        ..RawSnapshot::default()
    };

    let out = derive(None, &current);

    assert_eq!(out.cpu_percent.value().copied(), Some(0.0));
    assert_eq!(out.top_cpu.value().unwrap().len(), 1);
    assert_eq!(out.top_cpu.value().unwrap()[0].name, "worker");
    assert_eq!(out.top_cpu.value().unwrap()[0].percent, 0.0);
    assert_eq!(out.top_memory.value().unwrap()[0].name, "worker");
    assert_eq!(out.memory.value().unwrap().used_bytes, 10);
    assert_eq!(out.temperatures.value().unwrap()[0].name, "CPU");
    assert_eq!(out.networks.value().unwrap()[0].name, "eth0");
    assert_eq!(
        (
            out.networks.value().unwrap()[0].down_bytes_per_sec,
            out.networks.value().unwrap()[0].up_bytes_per_sec,
        ),
        (0.0, 0.0)
    );
    assert_eq!(
        out.disks.value().unwrap()[0].metadata.system_label,
        "nvme0n1"
    );
    assert_eq!(out.disks.value().unwrap()[0].bytes_per_sec, 0.0);

    let disk = out.process_disk_io.value().unwrap();
    assert_eq!(disk[0].device, "nvme0n1");
    assert_eq!(
        (disk[0].read_bytes_per_sec, disk[0].write_bytes_per_sec),
        (0.0, 0.0)
    );
    let network = out.process_network_io.value().unwrap();
    assert_eq!(network[0].interface, "eth0");
    assert_eq!(
        (network[0].rx_bytes_per_sec, network[0].tx_bytes_per_sec),
        (0.0, 0.0)
    );
}

#[test]
fn derives_rates_and_cpu() {
    let t = Instant::now();
    let old = RawSnapshot {
        collected_at: t,
        cpu: Collection::available(CpuCounter {
            total_time_units: 1000,
            idle_time_units: 600,
            logical_cpu_count: 4,
        }),
        processes: Collection::available(vec![ProcessCounter {
            process: process_id(1, 1),
            name: "a".into(),
            cpu_time_units: 100,
            rss_bytes: 10,
        }]),
        networks: Collection::available(vec![NetworkCounter {
            id: network_id("network-a"),
            name: "eth0".into(),
            rx_bytes: 100,
            tx_bytes: 200,
        }]),
        disks: Collection::available(vec![DiskCounter {
            id: disk_id("disk-a"),
            metadata: disk_metadata("sda"),
            read_bytes: 100,
            write_bytes: 100,
        }]),
        ..RawSnapshot::default()
    };
    let new = RawSnapshot {
        collected_at: t + Duration::from_secs(1),
        cpu: Collection::available(CpuCounter {
            total_time_units: 1100,
            idle_time_units: 650,
            logical_cpu_count: 4,
        }),
        processes: Collection::available(vec![ProcessCounter {
            process: process_id(1, 1),
            name: "a".into(),
            cpu_time_units: 110,
            rss_bytes: 20,
        }]),
        networks: Collection::available(vec![NetworkCounter {
            id: network_id("network-a"),
            name: "eth0".into(),
            rx_bytes: 300,
            tx_bytes: 500,
        }]),
        disks: Collection::available(vec![DiskCounter {
            id: disk_id("disk-a"),
            metadata: disk_metadata("sda"),
            read_bytes: 300,
            write_bytes: 500,
        }]),
        ..RawSnapshot::default()
    };
    let out = derive(Some(&old), &new);
    assert_eq!(out.cpu_percent.value().copied(), Some(50.0));
    assert_eq!(out.networks.value().unwrap()[0].down_bytes_per_sec, 200.0);
    assert_eq!(out.networks.value().unwrap()[0].up_bytes_per_sec, 300.0);
    assert_eq!(out.disks.value().unwrap()[0].bytes_per_sec, 600.0);
    assert!((out.top_cpu.value().unwrap()[0].percent - 40.0).abs() < 0.001);
}

#[test]
fn network_and_disk_rates_follow_identity_not_name() {
    let t = Instant::now();
    let old = RawSnapshot {
        collected_at: t,
        networks: Collection::available(vec![NetworkCounter {
            id: network_id("network-a"),
            name: "eth0".into(),
            rx_bytes: 100,
            tx_bytes: 200,
        }]),
        disks: Collection::available(vec![DiskCounter {
            id: disk_id("disk-a"),
            metadata: DiskMetadata {
                system_label: "sda".into(),
                associated_labels: vec!["D:".into()],
            },
            read_bytes: 100,
            write_bytes: 100,
        }]),
        ..RawSnapshot::default()
    };
    let renamed = RawSnapshot {
        collected_at: t + Duration::from_secs(1),
        networks: Collection::available(vec![NetworkCounter {
            id: network_id("network-a"),
            name: "lan0".into(),
            rx_bytes: 300,
            tx_bytes: 500,
        }]),
        disks: Collection::available(vec![DiskCounter {
            id: disk_id("disk-a"),
            metadata: DiskMetadata {
                system_label: "system-disk".into(),
                associated_labels: vec!["E:".into()],
            },
            read_bytes: 300,
            write_bytes: 500,
        }]),
        ..RawSnapshot::default()
    };

    let out = derive(Some(&old), &renamed);

    assert_eq!(out.networks.value().unwrap()[0].name, "lan0");
    assert_eq!(out.networks.value().unwrap()[0].down_bytes_per_sec, 200.0);
    assert_eq!(out.networks.value().unwrap()[0].up_bytes_per_sec, 300.0);
    assert_eq!(
        out.disks.value().unwrap()[0].metadata.system_label,
        "system-disk"
    );
    assert_eq!(
        out.disks.value().unwrap()[0].metadata.associated_labels,
        ["E:"]
    );
    assert_eq!(out.disks.value().unwrap()[0].bytes_per_sec, 600.0);
}

#[test]
fn reused_names_with_new_identity_do_not_inherit_rates() {
    let t = Instant::now();
    let old = RawSnapshot {
        collected_at: t,
        networks: Collection::available(vec![NetworkCounter {
            id: network_id("network-old"),
            name: "eth0".into(),
            rx_bytes: 10_000,
            tx_bytes: 20_000,
        }]),
        disks: Collection::available(vec![DiskCounter {
            id: disk_id("disk-old"),
            metadata: disk_metadata("sda"),
            read_bytes: 10_000,
            write_bytes: 20_000,
        }]),
        ..RawSnapshot::default()
    };
    let replacement = RawSnapshot {
        collected_at: t + Duration::from_secs(1),
        networks: Collection::available(vec![NetworkCounter {
            id: network_id("network-new"),
            name: "eth0".into(),
            rx_bytes: 100,
            tx_bytes: 200,
        }]),
        disks: Collection::available(vec![DiskCounter {
            id: disk_id("disk-new"),
            metadata: disk_metadata("sda"),
            read_bytes: 100,
            write_bytes: 200,
        }]),
        ..RawSnapshot::default()
    };

    let out = derive(Some(&old), &replacement);

    assert_eq!(out.networks.value().unwrap()[0].down_bytes_per_sec, 0.0);
    assert_eq!(out.networks.value().unwrap()[0].up_bytes_per_sec, 0.0);
    assert_eq!(out.disks.value().unwrap()[0].bytes_per_sec, 0.0);
}

#[test]
fn recovered_counter_observation_starts_a_new_rate_baseline() {
    let t = Instant::now();
    let unavailable = RawSnapshot {
        collected_at: t,
        networks: Collection::unavailable(CollectionUnavailable::Unavailable),
        disks: Collection::unavailable(CollectionUnavailable::Unavailable),
        ..RawSnapshot::default()
    };
    let recovered = RawSnapshot {
        collected_at: t + Duration::from_secs(1),
        networks: Collection::available(vec![NetworkCounter {
            id: network_id("network-a"),
            name: "eth0".into(),
            rx_bytes: 50_000,
            tx_bytes: 80_000,
        }]),
        disks: Collection::available(vec![DiskCounter {
            id: disk_id("disk-a"),
            metadata: disk_metadata("sda"),
            read_bytes: 90_000,
            write_bytes: 120_000,
        }]),
        ..RawSnapshot::default()
    };

    let out = derive(Some(&unavailable), &recovered);

    assert_eq!(out.networks.value().unwrap()[0].down_bytes_per_sec, 0.0);
    assert_eq!(out.networks.value().unwrap()[0].up_bytes_per_sec, 0.0);
    assert_eq!(out.disks.value().unwrap()[0].bytes_per_sec, 0.0);
}
