use std::time::{Duration, Instant};

use crate::core::model::{Collection, NetworkCounter, NetworkId};

use super::diagnostics::{report_issue, unavailable_from_io};
use super::native::{self, NativeNetwork};
use super::network_device::{self, NetworkDeviceClass};

const PROVENANCE_RETRY_INTERVAL: Duration = Duration::from_secs(30);

pub(super) struct Collector {
    candidate_ids: Vec<String>,
    classifications: Vec<NetworkDeviceClass>,
    classification_error: Option<String>,
    classified_at: Option<Instant>,
}

impl Collector {
    pub(super) fn new() -> Self {
        Self {
            candidate_ids: Vec::new(),
            classifications: Vec::new(),
            classification_error: None,
            classified_at: None,
        }
    }

    pub(super) fn collect(
        &mut self,
        mut notes: Option<&mut Vec<String>>,
    ) -> Collection<Vec<NetworkCounter>> {
        match native::network_interfaces() {
            Ok(mut interfaces) => {
                interfaces.sort_by(|a, b| a.guid.cmp(&b.guid));
                self.refresh_classifications(&interfaces);

                let mut degraded = self.classification_error.is_some();
                if let Some(error) = &self.classification_error {
                    report_issue(&mut notes, || {
                        format!("network device provenance unavailable: {error}")
                    });
                }

                let mut selected = Vec::with_capacity(interfaces.len());
                for (item, class) in interfaces.into_iter().zip(&self.classifications) {
                    match class {
                        NetworkDeviceClass::HardwareBacked => selected.push(item),
                        NetworkDeviceClass::Software => {}
                        NetworkDeviceClass::Unknown => {
                            degraded = true;
                            report_issue(&mut notes, || {
                                format!(
                                    "network interface {} device provenance is unknown; retaining hardware candidate",
                                    item.guid
                                )
                            });
                            selected.push(item);
                        }
                    }
                }

                selected.sort_by(|a, b| a.name.cmp(&b.name));
                let values = selected
                    .into_iter()
                    .map(|item| NetworkCounter {
                        id: NetworkId::from_opaque_key(item.guid),
                        name: item.name,
                        rx_bytes: item.rx_bytes,
                        tx_bytes: item.tx_bytes,
                    })
                    .collect();
                if degraded {
                    Collection::degraded(values)
                } else {
                    Collection::available(values)
                }
            }
            Err(error) => {
                report_issue(&mut notes, || format!("GetIfTable2 failed: {error}"));
                Collection::unavailable(unavailable_from_io(&error))
            }
        }
    }

    fn refresh_classifications(&mut self, interfaces: &[NativeNetwork]) {
        let candidate_ids: Vec<_> = interfaces.iter().map(|item| item.guid.clone()).collect();
        let uncertain = self.classification_error.is_some()
            || self.classifications.contains(&NetworkDeviceClass::Unknown);
        let retry_due = uncertain
            && self
                .classified_at
                .is_none_or(|at| at.elapsed() >= PROVENANCE_RETRY_INTERVAL);
        if candidate_ids == self.candidate_ids && !retry_due {
            return;
        }

        self.candidate_ids = candidate_ids;
        self.classified_at = Some(Instant::now());
        match network_device::classify(&self.candidate_ids) {
            Ok(classifications) => {
                self.classifications = classifications;
                self.classification_error = None;
            }
            Err(error) => {
                self.classifications = vec![NetworkDeviceClass::Unknown; self.candidate_ids.len()];
                self.classification_error = Some(error.to_string());
            }
        }
    }
}
