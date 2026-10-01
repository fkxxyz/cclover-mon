use super::*;

impl SinkState {
    pub(super) fn new() -> Self {
        Self {
            topology: Vec::new(),
            threads: HashMap::new(),
            files: HashMap::new(),
            pending: HashMap::new(),
            interval: Interval::default(),
            file_map_overflowed: false,
        }
    }

    pub(super) fn update_topology(&mut self, topology: Vec<VolumeRoute>) {
        if self.topology == topology {
            return;
        }
        self.topology = topology;
    }

    pub(super) fn reset_epoch(&mut self) {
        self.threads.clear();
        self.files.clear();
        self.pending.clear();
        self.interval = Interval::default();
        self.file_map_overflowed = false;
    }

    pub(super) fn reset_session_state_preserving_files(&mut self) {
        self.threads.clear();
        self.pending.clear();
        self.interval = Interval::default();
    }

    fn bounded_insert<K: std::hash::Hash + Eq, V>(
        map: &mut HashMap<K, V>,
        key: K,
        value: V,
        limit: usize,
    ) -> bool {
        if map.len() >= limit && !map.contains_key(&key) {
            return false;
        }
        map.insert(key, value);
        true
    }

    fn volume_for_path(&self, path: &str) -> Option<Arc<str>> {
        let lower = path.to_ascii_lowercase();
        self.topology
            .iter()
            .filter(|route| volume_path_matches(&lower, &route.nt_path_lower))
            .max_by_key(|route| route.nt_path_lower.len())
            .map(|route| route.nt_path_lower.clone())
    }

    fn target_for_volume(&self, volume: &str) -> Option<FileTarget> {
        self.topology
            .iter()
            .find(|route| route.nt_path_lower.as_ref() == volume)
            .map(|route| route.target)
    }

    fn remember_file(&mut self, key: u64, path: &str) {
        let Some(volume) = self.volume_for_path(path) else {
            if path
                .to_ascii_lowercase()
                .starts_with(r"\device\harddiskvolume")
            {
                self.interval.stats.unresolved_local_files =
                    self.interval.stats.unresolved_local_files.saturating_add(1);
            }
            return;
        };
        if !Self::bounded_insert(&mut self.files, key, volume, MAX_FILES) {
            self.interval.stats.state_overflowed = true;
            self.file_map_overflowed = true;
        }
    }

    fn start_io(
        &mut self,
        irp: u64,
        tid: u32,
        file_object: u64,
        file_key: u64,
        direction: Direction,
        requested_bytes: u32,
        irp_flags: u32,
    ) {
        if irp_flags & IRP_PAGING_IO != 0 {
            return;
        }
        let Some(&pid) = self.threads.get(&tid) else {
            return;
        };
        let volume = self
            .files
            .get(&file_key)
            .or_else(|| self.files.get(&file_object))
            .cloned();
        let Some(volume) = volume else {
            self.interval.stats.unmapped_file_ios =
                self.interval.stats.unmapped_file_ios.saturating_add(1);
            return;
        };
        let Some(target) = self.target_for_volume(&volume) else {
            self.interval.stats.unresolved_local_files =
                self.interval.stats.unresolved_local_files.saturating_add(1);
            return;
        };
        let pending = PendingIo {
            pid,
            target,
            direction: direction.into(),
            requested_bytes,
        };
        if !Self::bounded_insert(&mut self.pending, irp, pending, MAX_PENDING) {
            self.interval.stats.state_overflowed = true;
        }
    }

    fn finish_io(&mut self, irp: u64, extra_info: u64, status: u32) {
        let Some(pending) = self.pending.remove(&irp) else {
            return;
        };
        if (status as i32) < 0 || extra_info == 0 {
            return;
        }
        if extra_info > u64::from(pending.requested_bytes) {
            self.interval.stats.invalid_completions =
                self.interval.stats.invalid_completions.saturating_add(1);
            return;
        }
        let FileTarget::SingleDisk(disk_number) = pending.target else {
            self.interval.stats.ambiguous_volume_ios =
                self.interval.stats.ambiguous_volume_ios.saturating_add(1);
            return;
        };
        let key = IntervalKey {
            pid: pending.pid,
            disk_number,
            direction: pending.direction,
        };
        if self.interval.bytes.len() >= MAX_INTERVAL_KEYS && !self.interval.bytes.contains_key(&key)
        {
            self.interval.stats.state_overflowed = true;
            return;
        }
        let bytes = self.interval.bytes.entry(key).or_default();
        *bytes = bytes.saturating_add(extra_info);
    }
}

pub(super) struct Sink {
    state: Mutex<SinkState>,
}

impl Sink {
    pub(super) fn new() -> Self {
        Self {
            state: Mutex::new(SinkState::new()),
        }
    }

    pub(super) fn update_topology(&self, topology: Vec<VolumeRoute>) {
        self.state
            .lock()
            .expect("ETW disk attribution state poisoned")
            .update_topology(topology);
    }

    pub(super) fn take_interval(&self) -> Interval {
        let mut state = self
            .state
            .lock()
            .expect("ETW disk attribution state poisoned");
        std::mem::take(&mut state.interval)
    }

    pub(super) fn reset_epoch(&self) {
        self.state
            .lock()
            .expect("ETW disk attribution state poisoned")
            .reset_epoch();
    }

    pub(super) fn reset_session_state_preserving_files(&self) {
        self.state
            .lock()
            .expect("ETW disk attribution state poisoned")
            .reset_session_state_preserving_files();
    }

    pub(super) fn file_map_incomplete(&self) -> bool {
        self.state
            .lock()
            .expect("ETW disk attribution state poisoned")
            .file_map_overflowed
    }
}

impl EventSink for Sink {
    fn on_event(&self, event: Event) {
        let mut state = self
            .state
            .lock()
            .expect("ETW disk attribution state poisoned");
        match event {
            Event::ThreadStart { pid, tid } => {
                if !SinkState::bounded_insert(&mut state.threads, tid, pid, MAX_THREADS) {
                    state.interval.stats.state_overflowed = true;
                }
            }
            Event::ThreadEnd { tid } => {
                state.threads.remove(&tid);
            }
            Event::FileName { key, path } => state.remember_file(key, &path),
            Event::FileCreate { file_object, path } => state.remember_file(file_object, &path),
            Event::FileObjectEnd { file_object } => {
                state.files.remove(&file_object);
            }
            Event::FileKeyEnd { key } => {
                state.files.remove(&key);
            }
            Event::IoStart {
                irp,
                tid,
                file_object,
                file_key,
                direction,
                requested_bytes,
                irp_flags,
            } => state.start_io(
                irp,
                tid,
                file_object,
                file_key,
                direction,
                requested_bytes,
                irp_flags,
            ),
            Event::IoEnd {
                irp,
                extra_info,
                status,
            } => state.finish_io(irp, extra_info, status),
        }
    }
}
