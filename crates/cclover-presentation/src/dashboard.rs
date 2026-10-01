use super::*;
#[derive(Clone, Copy)]
pub struct Dashboard<'a> {
    state: &'a MonitorState,
}

impl<'a> Dashboard<'a> {
    pub const fn new(state: &'a MonitorState) -> Self {
        Self { state }
    }

    pub fn history_capacity(self) -> usize {
        self.state.history_capacity.max(1)
    }

    pub fn memory(self) -> MemoryPanel<'a> {
        MemoryPanel {
            memory: self.state.snapshot.memory.value(),
            processes: self
                .state
                .snapshot
                .top_memory
                .value()
                .map(Vec::as_slice)
                .unwrap_or_default(),
            history: &self.state.history.memory_used,
        }
    }

    pub fn cpu(self) -> CpuPanel<'a> {
        CpuPanel {
            percent: self.state.snapshot.cpu_percent.value().copied(),
            processes: self
                .state
                .snapshot
                .top_cpu
                .value()
                .map(Vec::as_slice)
                .unwrap_or_default(),
            history: &self.state.history.cpu,
        }
    }

    pub fn temperature_count(self) -> usize {
        self.state.snapshot.temperatures.value().map_or(0, Vec::len)
    }

    pub fn temperature(self, index: usize) -> Option<TemperaturePanel<'a>> {
        let value = self.state.snapshot.temperatures.value()?.get(index)?;
        Some(TemperaturePanel {
            value,
            history: self.state.history.temperatures.get(&value.id),
        })
    }

    pub fn fan_count(self) -> usize {
        self.state.snapshot.fans.value().map_or(0, Vec::len)
    }

    pub fn fan(self, index: usize) -> Option<FanPanel<'a>> {
        let value = self.state.snapshot.fans.value()?.get(index)?;
        Some(FanPanel {
            value,
            history: self.state.history.fans.get(&value.id),
        })
    }

    pub fn gpu_count(self) -> usize {
        self.state.snapshot.gpus.value().map_or(0, Vec::len)
    }

    pub fn gpu(self, index: usize) -> Option<GpuPanel<'a>> {
        let value = self.state.snapshot.gpus.value()?.get(index)?;
        Some(GpuPanel {
            value,
            utilization_history: self.state.history.gpu_utilization.get(&value.id),
            memory_history: self.state.history.gpu_memory_used.get(&value.id),
            temperature_history: self.state.history.gpu_temperature.get(&value.id),
        })
    }

    pub fn disk_count(self) -> usize {
        self.state.snapshot.disks.value().map_or(0, Vec::len)
    }

    pub fn disk(self, index: usize) -> Option<DiskPanel<'a>> {
        let value = self.state.snapshot.disks.value()?.get(index)?;
        Some(DiskPanel {
            value,
            history: self.state.history.disks.get(&value.id),
            processes: &self.state.snapshot.process_disk_io,
        })
    }

    pub fn network_count(self) -> usize {
        self.state.snapshot.networks.value().map_or(0, Vec::len)
    }

    pub fn network(self, index: usize) -> Option<NetworkPanel<'a>> {
        let value = self.state.snapshot.networks.value()?.get(index)?;
        Some(NetworkPanel {
            value,
            history: self.state.history.networks.get(&value.id),
            processes: &self.state.snapshot.process_network_io,
        })
    }
}
