#[cfg(test)]
mod tests {
    use super::*;

    fn process(pid: u32, birth_marker: u64) -> ProcessInstanceId {
        ProcessInstanceId { pid, birth_marker }
    }

    fn route(path: &str, target: FileTarget) -> VolumeRoute {
        VolumeRoute {
            nt_path_lower: Arc::<str>::from(path.to_ascii_lowercase()),
            target,
        }
    }

    #[test]
    fn completed_logical_io_is_counted_on_single_disk_volume() {
        let mut state = SinkState::new();
        state.update_topology(vec![route(
            r"\Device\HarddiskVolume3",
            FileTarget::SingleDisk(7),
        )]);
        state.threads.insert(10, 42);
        state.remember_file(100, r"\Device\HarddiskVolume3\data.bin");
        state.start_io(500, 10, 0, 100, Direction::Read, 4096, 0);
        state.finish_io(500, 1024, 0);

        assert_eq!(state.interval.bytes.len(), 1);
        assert_eq!(
            state.interval.bytes[&IntervalKey {
                pid: 42,
                disk_number: 7,
                direction: DirectionKey::Read,
            }],
            1024
        );
    }

    #[test]
    fn paging_io_is_not_product_logical_io() {
        let mut state = SinkState::new();
        state.update_topology(vec![route(
            r"\Device\HarddiskVolume3",
            FileTarget::SingleDisk(7),
        )]);
        state.threads.insert(10, 42);
        state.remember_file(100, r"\Device\HarddiskVolume3\pagefile.sys");
        state.start_io(500, 10, 0, 100, Direction::Write, 4096, IRP_PAGING_IO);
        state.finish_io(500, 4096, 0);
        assert!(state.interval.bytes.is_empty());
    }

    #[test]
    fn ambiguous_volume_is_skipped_and_degrades_interval() {
        let mut state = SinkState::new();
        state.update_topology(vec![route(
            r"\Device\HarddiskVolume9",
            FileTarget::Ambiguous,
        )]);
        state.threads.insert(10, 42);
        state.remember_file(100, r"\Device\HarddiskVolume9\stripe.bin");
        state.start_io(500, 10, 0, 100, Direction::Write, 4096, 0);
        state.finish_io(500, 4096, 0);
        assert!(state.interval.bytes.is_empty());
        assert_eq!(state.interval.stats.ambiguous_volume_ios, 1);
    }

    #[test]
    fn process_identity_must_be_stable_across_interval() {
        let previous = HashMap::from([(42, process(42, 100))]);
        let current = HashMap::from([(42, process(42, 200))]);
        assert_eq!(stable_process_identity(42, &previous, &current), None);
    }

    #[test]
    fn failed_or_impossible_completion_is_not_counted() {
        let mut state = SinkState::new();
        state.update_topology(vec![route(
            r"\Device\HarddiskVolume3",
            FileTarget::SingleDisk(7),
        )]);
        state.threads.insert(10, 42);
        state.remember_file(100, r"\Device\HarddiskVolume3\data.bin");
        state.start_io(500, 10, 0, 100, Direction::Read, 1024, 0);
        state.finish_io(500, 2048, 0);
        assert!(state.interval.bytes.is_empty());
        assert_eq!(state.interval.stats.invalid_completions, 1);
    }

    #[test]
    fn volume_prefix_requires_path_boundary() {
        assert!(volume_path_matches(
            r"\device\harddiskvolume1\file.bin",
            r"\device\harddiskvolume1"
        ));
        assert!(!volume_path_matches(
            r"\device\harddiskvolume10\file.bin",
            r"\device\harddiskvolume1"
        ));
    }

    #[test]
    fn file_object_and_file_key_have_independent_lifetimes() {
        let sink = Sink::new();
        {
            let mut state = sink.state.lock().unwrap();
            state.update_topology(vec![route(
                r"\Device\HarddiskVolume3",
                FileTarget::SingleDisk(7),
            )]);
            state.remember_file(100, r"\Device\HarddiskVolume3\object.bin");
            state.remember_file(200, r"\Device\HarddiskVolume3\key.bin");
        }

        sink.on_event(Event::FileObjectEnd { file_object: 100 });
        {
            let state = sink.state.lock().unwrap();
            assert!(!state.files.contains_key(&100));
            assert!(state.files.contains_key(&200));
        }

        sink.on_event(Event::FileKeyEnd { key: 200 });
        assert!(!sink.state.lock().unwrap().files.contains_key(&200));
    }

    #[test]
    fn session_reset_preserves_rundown_file_mapping_only() {
        let mut state = SinkState::new();
        state.update_topology(vec![route(
            r"\Device\HarddiskVolume3",
            FileTarget::SingleDisk(7),
        )]);
        state.remember_file(100, r"\Device\HarddiskVolume3\data.bin");
        state.threads.insert(10, 42);
        state.pending.insert(
            500,
            PendingIo {
                pid: 42,
                target: FileTarget::SingleDisk(7),
                direction: DirectionKey::Read,
                requested_bytes: 1,
            },
        );

        state.reset_session_state_preserving_files();

        assert!(state.files.contains_key(&100));
        assert!(state.threads.is_empty());
        assert!(state.pending.is_empty());
        assert!(state.interval.bytes.is_empty());
    }

    #[test]
    fn unmapped_file_io_is_explicit_degradation_evidence() {
        let mut state = SinkState::new();
        state.threads.insert(10, 42);
        state.start_io(500, 10, 100, 200, Direction::Read, 4096, 0);
        assert_eq!(state.interval.stats.unmapped_file_ios, 1);
        assert!(state.pending.is_empty());
    }
}
