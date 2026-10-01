use crate::*;
#[derive(Clone, Copy)]
pub struct DiskPanel<'a> {
    pub(crate) value: &'a DiskSnapshot,
    pub(crate) history: Option<&'a VecDeque<f64>>,
    pub(crate) processes: &'a Collection<Vec<ProcessDiskIo>>,
}

impl<'a> DiskPanel<'a> {
    pub fn name(self) -> String {
        disk_title(
            &self.value.metadata.system_label,
            &self.value.metadata.associated_labels,
        )
    }

    pub fn value(self) -> String {
        format_rate(self.value.bytes_per_sec)
    }

    pub fn history(self) -> Option<&'a VecDeque<f64>> {
        self.history
    }

    pub fn process_rows_visible(self) -> bool {
        self.processes.is_observable()
    }

    pub fn processes(self) -> impl Iterator<Item = IoProcessRow<'a>> + 'a {
        let disk_id = &self.value.id;
        self.processes
            .value()
            .into_iter()
            .flatten()
            .filter(move |process| &process.disk_id == disk_id)
            .filter(|process| process.read_bytes_per_sec + process.write_bytes_per_sec > 0.0)
            .map(|process| IoProcessRow {
                name: process.name.as_deref(),
                pid: process.process.pid,
                first_value: format_compact_rate(process.read_bytes_per_sec),
                second_value: format_compact_rate(process.write_bytes_per_sec),
            })
    }
}

pub(crate) fn disk_title(system_label: &str, associated_labels: &[String]) -> String {
    match associated_labels {
        [] => system_label.to_owned(),
        [one] => one.clone(),
        [first, second] => format!("{first} · {second}"),
        [first, second, rest @ ..] => format!("{first} · {second} +{}", rest.len()),
    }
}

#[derive(Clone, Copy)]
pub struct NetworkPanel<'a> {
    pub(crate) value: &'a NetworkSnapshot,
    pub(crate) history: Option<&'a NetworkDirectionHistory>,
    pub(crate) processes: &'a Collection<Vec<ProcessNetworkIo>>,
}

impl<'a> NetworkPanel<'a> {
    pub fn name(self) -> &'a str {
        &self.value.name
    }

    pub fn down_value(self) -> String {
        format_rate(self.value.down_bytes_per_sec)
    }

    pub fn up_value(self) -> String {
        format_rate(self.value.up_bytes_per_sec)
    }

    pub fn history(self) -> Option<&'a NetworkDirectionHistory> {
        self.history
    }

    pub fn process_rows_visible(self) -> bool {
        self.processes.is_observable()
    }

    pub fn processes(self) -> impl Iterator<Item = IoProcessRow<'a>> + 'a {
        let network_id = &self.value.id;
        self.processes
            .value()
            .into_iter()
            .flatten()
            .filter(move |process| &process.network_id == network_id)
            .filter(|process| process.rx_bytes_per_sec + process.tx_bytes_per_sec > 0.0)
            .map(|process| IoProcessRow {
                name: process.name.as_deref(),
                pid: process.process.pid,
                first_value: format_compact_rate(process.rx_bytes_per_sec),
                second_value: format_compact_rate(process.tx_bytes_per_sec),
            })
    }
}

pub struct IoProcessRow<'a> {
    pub name: Option<&'a str>,
    pub pid: u32,
    pub first_value: String,
    pub second_value: String,
}
