use crate::*;

/// Renderer-neutral dashboard semantics for one published monitor state.
///
/// The process rankings are evaluated once per state with the card row policy owned here, so
/// renderers receive finished rows and never rank, join, or format process identity themselves.
pub struct Dashboard<'a> {
    state: &'a MonitorState,
    ranked: RankedProcesses,
}

struct RankedProcesses {
    cpu: Collection<Vec<ProcessCpuRank>>,
    memory: Collection<Vec<ProcessMemoryRank>>,
    disk_io: Collection<Vec<ProcessDiskIo>>,
    network_io: Collection<Vec<ProcessNetworkIo>>,
}

impl<'a> Dashboard<'a> {
    pub fn new(state: &'a MonitorState) -> Self {
        let processes = &state.snapshot.processes;
        Self {
            state,
            ranked: RankedProcesses {
                cpu: processes.cpu_ranking(PROCESS_CARD_ROWS),
                memory: processes.memory_ranking(PROCESS_CARD_ROWS),
                disk_io: processes.disk_io_ranking(IO_CARD_ROWS),
                network_io: processes.network_io_ranking(IO_CARD_ROWS),
            },
        }
    }

    pub fn history_capacity(&self) -> usize {
        self.state.history_capacity.max(1)
    }

    pub fn memory(&self) -> MemoryPanel<'_> {
        MemoryPanel {
            memory: self.state.snapshot.memory.value(),
            processes: self
                .ranked
                .memory
                .value()
                .map(Vec::as_slice)
                .unwrap_or_default(),
            history: &self.state.history.memory_used,
        }
    }

    pub fn cpu(&self) -> CpuPanel<'_> {
        CpuPanel {
            percent: self.state.snapshot.cpu_percent.value().copied(),
            processes: self
                .ranked
                .cpu
                .value()
                .map(Vec::as_slice)
                .unwrap_or_default(),
            history: &self.state.history.cpu,
        }
    }

    pub fn temperature_count(&self) -> usize {
        self.state.snapshot.temperatures.value().map_or(0, Vec::len)
    }

    pub fn temperature(&self, index: usize) -> Option<TemperaturePanel<'_>> {
        let value = self.state.snapshot.temperatures.value()?.get(index)?;
        Some(TemperaturePanel {
            value,
            history: self.state.history.temperatures.get(&value.id),
        })
    }

    pub fn fan_count(&self) -> usize {
        self.state.snapshot.fans.value().map_or(0, Vec::len)
    }

    pub fn fan(&self, index: usize) -> Option<FanPanel<'_>> {
        let value = self.state.snapshot.fans.value()?.get(index)?;
        Some(FanPanel {
            value,
            history: self.state.history.fans.get(&value.id),
        })
    }

    pub fn gpu_count(&self) -> usize {
        self.state.snapshot.gpus.value().map_or(0, Vec::len)
    }

    pub fn gpu(&self, index: usize) -> Option<GpuPanel<'_>> {
        let value = self.state.snapshot.gpus.value()?.get(index)?;
        Some(GpuPanel {
            value,
            utilization_history: self.state.history.gpu_utilization.get(&value.id),
            memory_history: self.state.history.gpu_memory_used.get(&value.id),
            temperature_history: self.state.history.gpu_temperature.get(&value.id),
        })
    }

    pub fn disk_count(&self) -> usize {
        self.state.snapshot.disks.value().map_or(0, Vec::len)
    }

    pub fn disk(&self, index: usize) -> Option<DiskPanel<'_>> {
        let value = self.state.snapshot.disks.value()?.get(index)?;
        Some(DiskPanel {
            value,
            history: self.state.history.disks.get(&value.id),
            processes: &self.ranked.disk_io,
        })
    }

    pub fn network_count(&self) -> usize {
        self.state.snapshot.networks.value().map_or(0, Vec::len)
    }

    pub fn network(&self, index: usize) -> Option<NetworkPanel<'_>> {
        let value = self.state.snapshot.networks.value()?.get(index)?;
        Some(NetworkPanel {
            value,
            history: self.state.history.networks.get(&value.id),
            processes: &self.ranked.network_io,
        })
    }
}
