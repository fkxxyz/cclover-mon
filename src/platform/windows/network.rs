use crate::core::model::{Collection, NetworkCounter, NetworkId};

use super::diagnostics::{report_issue, unavailable_from_io};
use super::native;

pub(super) fn collect(mut notes: Option<&mut Vec<String>>) -> Collection<Vec<NetworkCounter>> {
    match native::network_interfaces() {
        Ok(mut interfaces) => {
            interfaces.sort_by(|a, b| a.name.cmp(&b.name));
            Collection::available(
                interfaces
                    .into_iter()
                    .map(|item| NetworkCounter {
                        id: NetworkId::from_opaque_key(item.guid),
                        name: item.name,
                        rx_bytes: item.rx_bytes,
                        tx_bytes: item.tx_bytes,
                    })
                    .collect(),
            )
        }
        Err(error) => {
            report_issue(&mut notes, || format!("GetIfTable2 failed: {error}"));
            Collection::unavailable(unavailable_from_io(&error))
        }
    }
}
