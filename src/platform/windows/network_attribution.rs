use std::collections::{BTreeMap, HashMap, HashSet};
use std::time::{Duration, Instant};

use crate::core::model::{
    Collection, CollectionStatus, NetworkId, ProcessCounter, ProcessInstanceId,
    ProcessNetworkIoCounter,
};

use super::diagnostics::{report_issue, unavailable_from_io};
use super::ndu;
use super::network::{self, InterfaceIdentity};

const RETRY_INTERVAL: Duration = Duration::from_secs(30);

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
struct AttributionKey {
    process: ProcessInstanceId,
    network_id: NetworkId,
}

#[derive(Clone, Debug)]
struct Total {
    interface: String,
    rx_bytes: u64,
    tx_bytes: u64,
}

#[derive(Default)]
struct IntervalStats {
    process_attributions: usize,
    ambiguous_processes: usize,
    unresolved_interfaces: usize,
    accepted_interface_rows: usize,
}

pub(super) struct Collector {
    session: Option<ndu::Session>,
    retry_after: Option<(Instant, crate::core::model::CollectionUnavailable)>,
    interval_start_processes: Option<HashMap<u32, ProcessInstanceId>>,
    totals: BTreeMap<AttributionKey, Total>,
}

impl Collector {
    pub(super) fn new() -> Self {
        Self {
            session: None,
            retry_after: None,
            interval_start_processes: None,
            totals: BTreeMap::new(),
        }
    }

    pub(super) fn collect(
        &mut self,
        processes: &Collection<Vec<ProcessCounter>>,
        mut notes: Option<&mut Vec<String>>,
    ) -> Collection<Vec<ProcessNetworkIoCounter>> {
        let current_processes = complete_process_map(processes);

        if self.session.is_none() {
            if let Some((retry_after, reason)) = self.retry_after {
                if Instant::now() < retry_after {
                    return Collection::unavailable(reason);
                }
                self.retry_after = None;
            }
            match ndu::Session::open() {
                Ok(session) => {
                    self.session = Some(session);
                    self.interval_start_processes = current_processes;
                    return collection_for_process_status(processes.status(), self.rows());
                }
                Err(error) => {
                    let reason = unavailable_from_ndu_io(&error);
                    self.retry_after = Some((Instant::now() + RETRY_INTERVAL, reason));
                    report_issue(&mut notes, || format!("NDU session start failed: {error}"));
                    return Collection::unavailable(reason);
                }
            }
        }

        let snapshot = match self
            .session
            .as_ref()
            .expect("NDU session initialized")
            .query_interval()
        {
            Ok(snapshot) => snapshot,
            Err(error) => {
                let reason = unavailable_from_ndu_io(&error);
                report_issue(&mut notes, || format!("NDU interval query failed: {error}"));
                self.session = None;
                self.retry_after = Some((Instant::now() + RETRY_INTERVAL, reason));
                self.interval_start_processes = None;
                return Collection::unavailable(reason);
            }
        };

        let previous_processes = self.interval_start_processes.take();
        self.interval_start_processes = current_processes.clone();

        let Some(current_processes) = current_processes else {
            return collection_for_process_status(processes.status(), self.rows());
        };
        let Some(previous_processes) = previous_processes else {
            self.retain_active(&current_processes);
            return Collection::available(self.rows());
        };

        let attribution_count = snapshot
            .process_attributions
            .len()
            .saturating_add(snapshot.other_attribution_count);
        let stats = accumulate_interval(
            &mut self.totals,
            snapshot,
            &previous_processes,
            &current_processes,
            |luid| network::identity_for_luid(luid).ok(),
        );
        report_issue(&mut notes, || {
            format!(
                "NDU interval: {attribution_count} attributions, {} process attributions, {} accepted interface rows, {} ambiguous PID attributions, {} unresolved IfLuid rows",
                stats.process_attributions,
                stats.accepted_interface_rows,
                stats.ambiguous_processes,
                stats.unresolved_interfaces
            )
        });

        self.retain_active(&current_processes);
        let rows = self.rows();
        if stats.unresolved_interfaces > 0 {
            report_issue(&mut notes, || {
                format!(
                    "{} NDU attribution rows skipped: IfLuid could not be canonicalized",
                    stats.unresolved_interfaces
                )
            });
            Collection::degraded(rows)
        } else {
            Collection::available(rows)
        }
    }

    fn retain_active(&mut self, current: &HashMap<u32, ProcessInstanceId>) {
        let active: HashSet<_> = current.values().copied().collect();
        self.totals.retain(|key, _| active.contains(&key.process));
    }

    fn rows(&self) -> Vec<ProcessNetworkIoCounter> {
        self.totals
            .iter()
            .map(|(key, total)| ProcessNetworkIoCounter {
                process: key.process,
                network_id: key.network_id.clone(),
                interface: total.interface.clone(),
                rx_bytes: total.rx_bytes,
                tx_bytes: total.tx_bytes,
            })
            .collect()
    }
}

fn unavailable_from_ndu_io(error: &std::io::Error) -> crate::core::model::CollectionUnavailable {
    if error.kind() == std::io::ErrorKind::NotFound {
        crate::core::model::CollectionUnavailable::Unsupported
    } else {
        unavailable_from_io(error)
    }
}

fn collection_for_process_status(
    status: CollectionStatus,
    rows: Vec<ProcessNetworkIoCounter>,
) -> Collection<Vec<ProcessNetworkIoCounter>> {
    match status {
        CollectionStatus::Available => Collection::available(rows),
        CollectionStatus::Degraded => Collection::degraded(rows),
        CollectionStatus::Unavailable(reason) => Collection::unavailable(reason),
    }
}

fn complete_process_map(
    processes: &Collection<Vec<ProcessCounter>>,
) -> Option<HashMap<u32, ProcessInstanceId>> {
    match processes {
        Collection::Available(processes) => Some(
            processes
                .iter()
                .map(|process| (process.process.pid, process.process))
                .collect(),
        ),
        Collection::Degraded(_) | Collection::Unavailable(_) => None,
    }
}

fn stable_process_identity(
    pid: u32,
    previous: &HashMap<u32, ProcessInstanceId>,
    current: &HashMap<u32, ProcessInstanceId>,
) -> Option<ProcessInstanceId> {
    let previous = previous.get(&pid)?;
    let current = current.get(&pid)?;
    (previous == current).then_some(*current)
}

fn accumulate_interval(
    totals: &mut BTreeMap<AttributionKey, Total>,
    snapshot: ndu::Snapshot,
    previous_processes: &HashMap<u32, ProcessInstanceId>,
    current_processes: &HashMap<u32, ProcessInstanceId>,
    mut resolve_interface: impl FnMut(u64) -> Option<InterfaceIdentity>,
) -> IntervalStats {
    let mut identities: HashMap<u64, Option<InterfaceIdentity>> = HashMap::new();
    let mut stats = IntervalStats::default();
    for attribution in snapshot.process_attributions {
        stats.process_attributions += 1;
        let Some(process) =
            stable_process_identity(attribution.pid, previous_processes, current_processes)
        else {
            stats.ambiguous_processes += 1;
            continue;
        };

        for usage in attribution.interfaces {
            if usage.rx_bytes == 0 && usage.tx_bytes == 0 {
                continue;
            }
            let identity = identities
                .entry(usage.if_luid)
                .or_insert_with(|| resolve_interface(usage.if_luid));
            let Some(identity) = identity.as_ref() else {
                stats.unresolved_interfaces = stats.unresolved_interfaces.saturating_add(1);
                continue;
            };
            stats.accepted_interface_rows += 1;
            let key = AttributionKey {
                process,
                network_id: identity.id.clone(),
            };
            let total = totals.entry(key).or_insert_with(|| Total {
                interface: identity.name.clone(),
                rx_bytes: 0,
                tx_bytes: 0,
            });
            total.interface.clone_from(&identity.name);
            total.rx_bytes = total.rx_bytes.saturating_add(usage.rx_bytes);
            total.tx_bytes = total.tx_bytes.saturating_add(usage.tx_bytes);
        }
    }
    stats
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::model::CollectionUnavailable;

    fn process(pid: u32, birth_marker: u64) -> ProcessInstanceId {
        ProcessInstanceId { pid, birth_marker }
    }

    fn snapshot(pid: u32, luid: u64, rx_bytes: u64, tx_bytes: u64) -> ndu::Snapshot {
        ndu::Snapshot {
            interfaces: Vec::new(),
            process_attributions: vec![ndu::ProcessAttribution {
                pid,
                executable: None,
                interfaces: vec![ndu::InterfaceUsage {
                    if_luid: luid,
                    profile_id: 0,
                    tx_bytes,
                    rx_bytes,
                }],
            }],
            other_attribution_count: 0,
        }
    }

    fn identity() -> InterfaceIdentity {
        InterfaceIdentity {
            id: NetworkId::from_opaque_key("network-a"),
            name: "Ethernet".into(),
        }
    }

    #[test]
    fn accepts_only_process_identity_stable_across_interval() {
        let previous = HashMap::from([(42, process(42, 100)), (43, process(43, 100))]);
        let current = HashMap::from([
            (42, process(42, 100)),
            (43, process(43, 200)),
            (44, process(44, 100)),
        ]);

        assert_eq!(
            stable_process_identity(42, &previous, &current),
            Some(process(42, 100))
        );
        assert_eq!(stable_process_identity(43, &previous, &current), None);
        assert_eq!(stable_process_identity(44, &previous, &current), None);
        assert_eq!(stable_process_identity(45, &previous, &current), None);
    }

    #[test]
    fn incomplete_process_observation_is_not_an_identity_baseline() {
        let degraded = Collection::degraded(vec![ProcessCounter {
            process: process(42, 100),
            name: "p42".into(),
            cpu_time_units: 0,
            rss_bytes: 0,
        }]);
        assert!(complete_process_map(&degraded).is_none());

        let unavailable =
            Collection::<Vec<ProcessCounter>>::unavailable(CollectionUnavailable::Unavailable);
        assert!(complete_process_map(&unavailable).is_none());
    }

    #[test]
    fn stale_totals_are_retired_only_from_complete_process_state() {
        let mut collector = Collector::new();
        let old = process(42, 100);
        collector.totals.insert(
            AttributionKey {
                process: old,
                network_id: NetworkId::from_opaque_key("net"),
            },
            Total {
                interface: "Ethernet".into(),
                rx_bytes: 10,
                tx_bytes: 20,
            },
        );

        collector.retain_active(&HashMap::from([(43, process(43, 100))]));
        assert!(collector.totals.is_empty());
    }

    #[test]
    fn interval_bytes_accumulate_into_monotonic_core_counters() {
        let processes = HashMap::from([(42, process(42, 100))]);
        let mut totals = BTreeMap::new();

        let stats = accumulate_interval(
            &mut totals,
            snapshot(42, 7, 100, 40),
            &processes,
            &processes,
            |_| Some(identity()),
        );
        assert_eq!(stats.unresolved_interfaces, 0);
        accumulate_interval(
            &mut totals,
            snapshot(42, 7, 25, 10),
            &processes,
            &processes,
            |_| Some(identity()),
        );

        let total = totals.values().next().expect("one cumulative row");
        assert_eq!((total.rx_bytes, total.tx_bytes), (125, 50));
    }

    #[test]
    fn pid_reuse_discards_the_ambiguous_interval() {
        let previous = HashMap::from([(42, process(42, 100))]);
        let current = HashMap::from([(42, process(42, 200))]);
        let mut totals = BTreeMap::new();

        accumulate_interval(
            &mut totals,
            snapshot(42, 7, 100, 40),
            &previous,
            &current,
            |_| Some(identity()),
        );

        assert!(totals.is_empty());
    }

    #[test]
    fn interface_resolution_is_cached_per_interval() {
        let processes = HashMap::from([(42, process(42, 100))]);
        let mut snapshot = snapshot(42, 7, 100, 40);
        snapshot.process_attributions[0]
            .interfaces
            .push(ndu::InterfaceUsage {
                if_luid: 7,
                profile_id: 0,
                tx_bytes: 5,
                rx_bytes: 10,
            });
        let mut totals = BTreeMap::new();
        let mut resolutions = 0;

        accumulate_interval(&mut totals, snapshot, &processes, &processes, |_| {
            resolutions += 1;
            Some(identity())
        });

        assert_eq!(resolutions, 1);
        let total = totals.values().next().unwrap();
        assert_eq!((total.rx_bytes, total.tx_bytes), (110, 45));
    }

    #[test]
    fn zero_usage_does_not_create_a_persistent_counter() {
        let processes = HashMap::from([(42, process(42, 100))]);
        let mut totals = BTreeMap::new();
        let mut resolutions = 0;

        let stats = accumulate_interval(
            &mut totals,
            snapshot(42, 7, 0, 0),
            &processes,
            &processes,
            |_| {
                resolutions += 1;
                Some(identity())
            },
        );

        assert!(totals.is_empty());
        assert_eq!(resolutions, 0);
        assert_eq!(stats.accepted_interface_rows, 0);
    }
}
