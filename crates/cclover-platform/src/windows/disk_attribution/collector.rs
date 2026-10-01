use super::*;

pub(in crate::windows) struct Collector {
    sink: Arc<Sink>,
    session: Option<etw::Session>,
    retry_after: Option<(Instant, cclover_core::model::CollectionUnavailable)>,
    interval_start_processes: Option<HashMap<u32, ProcessInstanceId>>,
    totals: BTreeMap<AttributionKey, Total>,
    last_health: etw::Health,
    topology_refreshed_at: Option<Instant>,
    topology_failures: usize,
    topology_ready: bool,
    seeded: bool,
}

impl Collector {
    pub(in crate::windows) fn new() -> Self {
        Self {
            sink: Arc::new(Sink::new()),
            session: None,
            retry_after: None,
            interval_start_processes: None,
            totals: BTreeMap::new(),
            last_health: etw::Health::default(),
            topology_refreshed_at: None,
            topology_failures: 0,
            topology_ready: false,
            seeded: false,
        }
    }

    pub(in crate::windows) fn collect(
        &mut self,
        processes: &Collection<Vec<ProcessCounter>>,
        disks: &disk::Batch,
        mut notes: Option<&mut Vec<String>>,
    ) -> Collection<Vec<ProcessDiskIoCounter>> {
        if let CollectionStatus::Unavailable(reason) = disks.counters.status() {
            return Collection::unavailable(reason);
        }
        let current_processes = complete_process_map(processes);

        if let Err(error) = self.refresh_topology_if_needed() {
            report_issue(&mut notes, || {
                format!("ETW disk volume topology refresh failed: {error}")
            });
            return Collection::unavailable(unavailable_from_io(&error));
        }

        if self.session.is_none() {
            if let Some((retry_after, reason)) = self.retry_after {
                if Instant::now() < retry_after {
                    return Collection::unavailable(reason);
                }
                self.retry_after = None;
            }
            if !self.seeded {
                match etw::Session::start(self.sink.clone()) {
                    Ok(seed_session) => {
                        // FileRundown is emitted when the SystemTraceProvider session stops. The
                        // synchronous drop waits for the consumer, seeding file-key mappings for
                        // files that were already open before cclover-mon started.
                        drop(seed_session);
                        self.sink.reset_session_state_preserving_files();
                        self.seeded = true;
                    }
                    Err(error) => {
                        let reason = unavailable_from_etw_io(&error);
                        self.retry_after = Some((Instant::now() + RETRY_INTERVAL, reason));
                        report_issue(&mut notes, || {
                            format!("ETW disk attribution seed session failed: {error}")
                        });
                        return Collection::unavailable(reason);
                    }
                }
            }
            match etw::Session::start(self.sink.clone()) {
                Ok(session) => {
                    let health = match session.health() {
                        Ok(health) => health,
                        Err(error) => {
                            let reason = unavailable_from_etw_io(&error);
                            report_issue(&mut notes, || {
                                format!(
                                    "ETW disk attribution health query failed after start: {error}"
                                )
                            });
                            drop(session);
                            self.sink.reset_session_state_preserving_files();
                            self.retry_after = Some((Instant::now() + RETRY_INTERVAL, reason));
                            self.interval_start_processes = None;
                            return Collection::unavailable(reason);
                        }
                    };
                    self.last_health = health;
                    self.session = Some(session);
                    self.interval_start_processes = current_processes;
                    let rows = self.rows();
                    return match processes.status() {
                        CollectionStatus::Unavailable(reason) => Collection::unavailable(reason),
                        CollectionStatus::Degraded => Collection::degraded(rows),
                        CollectionStatus::Available
                            if self.topology_failures > 0
                                || self.sink.file_map_incomplete()
                                || disks.counters.status() == CollectionStatus::Degraded =>
                        {
                            Collection::degraded(rows)
                        }
                        CollectionStatus::Available => Collection::available(rows),
                    };
                }
                Err(error) => {
                    let reason = unavailable_from_etw_io(&error);
                    self.seeded = false;
                    self.sink.reset_epoch();
                    self.retry_after = Some((Instant::now() + RETRY_INTERVAL, reason));
                    report_issue(&mut notes, || {
                        format!("ETW disk attribution session start failed: {error}")
                    });
                    return Collection::unavailable(reason);
                }
            }
        }

        let health = match self
            .session
            .as_ref()
            .expect("ETW session initialized")
            .health()
        {
            Ok(health) => health,
            Err(error) => {
                return self.fail_session(error, &mut notes);
            }
        };
        if health.consumer_failed {
            return self.fail_session(
                io::Error::other("ETW disk attribution consumer failed"),
                &mut notes,
            );
        }
        if health.events_lost > self.last_health.events_lost
            || health.decode_errors > self.last_health.decode_errors
        {
            report_issue(&mut notes, || {
                format!(
                    "ETW disk attribution epoch reset after loss/decode failure: events_lost {}->{}, decode_errors {}->{}",
                    self.last_health.events_lost,
                    health.events_lost,
                    self.last_health.decode_errors,
                    health.decode_errors
                )
            });
            // A controlled stop emits FileRundown. Drop waits for the consumer, so keep the
            // refreshed file-key mappings but discard all interval/thread/IRP state whose
            // continuity was invalidated by the observed loss.
            self.session = None;
            self.sink.reset_session_state_preserving_files();
            self.totals.clear();
            self.interval_start_processes = None;
            self.last_health = etw::Health::default();
            return Collection::degraded(Vec::new());
        }
        self.last_health = health;

        let interval = self.sink.take_interval();
        let previous_processes = self.interval_start_processes.take();
        self.interval_start_processes = current_processes.clone();
        let Some(current_processes) = current_processes else {
            return collection_for_status(processes.status(), self.rows());
        };
        let Some(previous_processes) = previous_processes else {
            self.retain_active(&current_processes);
            let rows = self.rows();
            return if self.topology_failures > 0
                || self.sink.file_map_incomplete()
                || disks.counters.status() == CollectionStatus::Degraded
            {
                Collection::degraded(rows)
            } else {
                Collection::available(rows)
            };
        };

        let mut unresolved_disks = 0_usize;
        accumulate_interval(
            &mut self.totals,
            interval.bytes,
            &previous_processes,
            &current_processes,
            &disks.identities,
            &mut unresolved_disks,
        );
        self.retain_active(&current_processes);
        let rows = self.rows();
        let degraded = interval.stats.ambiguous_volume_ios > 0
            || interval.stats.unmapped_file_ios > 0
            || interval.stats.unresolved_local_files > 0
            || interval.stats.invalid_completions > 0
            || interval.stats.state_overflowed
            || unresolved_disks > 0
            || self.topology_failures > 0
            || self.sink.file_map_incomplete()
            || disks.counters.status() == CollectionStatus::Degraded;
        if degraded {
            report_issue(&mut notes, || {
                format!(
                    "ETW disk attribution degraded: {} ambiguous-volume I/Os, {} unmapped-file I/Os, {} unresolved local files, {} invalid completions, {} unresolved disk identities, {} volume-topology failures, file_map_incomplete={}, state_overflowed={}",
                    interval.stats.ambiguous_volume_ios,
                    interval.stats.unmapped_file_ios,
                    interval.stats.unresolved_local_files,
                    interval.stats.invalid_completions,
                    unresolved_disks,
                    self.topology_failures,
                    self.sink.file_map_incomplete(),
                    interval.stats.state_overflowed
                )
            });
            Collection::degraded(rows)
        } else {
            Collection::available(rows)
        }
    }

    fn refresh_topology_if_needed(&mut self) -> io::Result<()> {
        if self
            .topology_refreshed_at
            .is_some_and(|at| at.elapsed() < TOPOLOGY_REFRESH_INTERVAL)
        {
            return if self.topology_ready {
                Ok(())
            } else {
                Err(io::Error::other(
                    "volume topology unavailable; refresh retry is rate-limited",
                ))
            };
        }
        let now = Instant::now();
        let topology = match native::volume_disk_bindings() {
            Ok(topology) => topology,
            Err(error) => {
                self.topology_refreshed_at = Some(now);
                self.topology_failures = self.topology_failures.saturating_add(1);
                return if self.topology_ready {
                    Ok(())
                } else {
                    Err(error)
                };
            }
        };
        self.topology_failures = topology.failures;
        let routes = topology
            .bindings
            .into_iter()
            .filter(|binding| !binding.disk_numbers.is_empty())
            .map(|binding| VolumeRoute {
                nt_path_lower: Arc::<str>::from(binding.nt_path.to_ascii_lowercase()),
                target: if binding.disk_numbers.len() == 1 {
                    FileTarget::SingleDisk(binding.disk_numbers[0])
                } else {
                    FileTarget::Ambiguous
                },
            })
            .collect();
        self.sink.update_topology(routes);
        self.topology_refreshed_at = Some(now);
        self.topology_ready = true;
        Ok(())
    }

    fn fail_session(
        &mut self,
        error: io::Error,
        notes: &mut Option<&mut Vec<String>>,
    ) -> Collection<Vec<ProcessDiskIoCounter>> {
        let reason = unavailable_from_etw_io(&error);
        report_issue(notes, || format!("ETW disk attribution failed: {error}"));
        self.session = None;
        self.retry_after = Some((Instant::now() + RETRY_INTERVAL, reason));
        self.interval_start_processes = None;
        self.sink.reset_epoch();
        self.seeded = false;
        self.totals.clear();
        Collection::unavailable(reason)
    }

    fn retain_active(&mut self, current: &HashMap<u32, ProcessInstanceId>) {
        let active: HashSet<_> = current.values().copied().collect();
        self.totals.retain(|key, _| active.contains(&key.process));
    }

    fn rows(&self) -> Vec<ProcessDiskIoCounter> {
        self.totals
            .iter()
            .map(|(key, total)| ProcessDiskIoCounter {
                process: key.process,
                disk_id: key.disk_id.clone(),
                device: total.device.clone(),
                read_bytes: total.read_bytes,
                write_bytes: total.write_bytes,
            })
            .collect()
    }
}
