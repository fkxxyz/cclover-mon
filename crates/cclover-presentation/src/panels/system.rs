use crate::*;
#[derive(Clone, Copy)]
pub struct MemoryPanel<'a> {
    pub(crate) memory: Option<&'a MemorySnapshot>,
    pub(crate) processes: &'a [ProcessMemoryUsage],
    pub(crate) history: &'a VecDeque<f64>,
}

impl<'a> MemoryPanel<'a> {
    pub const TITLE: &'static str = "MEMORY";
    pub const SECONDARY_LABEL: &'static str = "SWAP";

    pub fn value(self) -> String {
        self.memory
            .map(|memory| format_bytes(memory.used_bytes))
            .unwrap_or_else(unavailable)
    }

    pub fn subtitle(self) -> String {
        self.memory
            .map(|memory| format!("/ {}", format_bytes(memory.total_bytes)))
            .unwrap_or_default()
    }

    pub fn secondary_value(self) -> String {
        self.memory
            .map(|memory| {
                format!(
                    "{} / {}",
                    format_bytes(memory.swap_used_bytes),
                    format_bytes(memory.swap_total_bytes)
                )
            })
            .unwrap_or_else(unavailable)
    }

    pub fn fraction(self) -> f32 {
        match self.memory {
            Some(memory) if memory.total_bytes > 0 => {
                memory.used_bytes as f32 / memory.total_bytes as f32
            }
            _ => 0.0,
        }
    }

    pub fn graph_values(self) -> &'a VecDeque<f64> {
        self.history
    }

    pub fn graph_max(self) -> f64 {
        self.memory
            .map(|memory| memory.total_bytes.max(1) as f64)
            .unwrap_or(1.0)
    }

    pub fn process_count(self) -> usize {
        self.processes.len()
    }

    pub fn processes(self) -> impl Iterator<Item = ProcessRow<'a>> + 'a {
        self.processes.iter().map(|process| ProcessRow {
            name: &process.name,
            value: format_bytes(process.bytes),
        })
    }
}

#[derive(Clone, Copy)]
pub struct CpuPanel<'a> {
    pub(crate) percent: Option<f64>,
    pub(crate) processes: &'a [ProcessCpuUsage],
    pub(crate) history: &'a VecDeque<f64>,
}

impl<'a> CpuPanel<'a> {
    pub const TITLE: &'static str = "CPU";

    pub fn value(self) -> String {
        self.percent.map(format_percent).unwrap_or_else(unavailable)
    }

    pub fn fraction(self) -> f32 {
        self.percent.unwrap_or(0.0) as f32 / 100.0
    }

    pub fn graph_values(self) -> &'a VecDeque<f64> {
        self.history
    }

    pub fn process_count(self) -> usize {
        self.processes.len()
    }

    pub fn processes(self) -> impl Iterator<Item = ProcessRow<'a>> + 'a {
        self.processes.iter().map(|process| ProcessRow {
            name: &process.name,
            value: format_percent(process.percent),
        })
    }
}

pub struct ProcessRow<'a> {
    pub name: &'a str,
    pub value: String,
}
