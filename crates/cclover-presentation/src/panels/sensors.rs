use crate::*;
#[derive(Clone, Copy)]
pub struct GpuPanel<'a> {
    pub(crate) value: &'a GpuSnapshot,
    pub(crate) utilization_history: Option<&'a VecDeque<f64>>,
    pub(crate) memory_history: Option<&'a VecDeque<f64>>,
    pub(crate) temperature_history: Option<&'a VecDeque<f64>>,
}

impl<'a> GpuPanel<'a> {
    pub fn name(self) -> &'a str {
        short_gpu_name(&self.value.name)
    }

    pub fn utilization_value(self) -> String {
        self.value
            .utilization_percent
            .map(format_percent)
            .unwrap_or_else(unavailable)
    }

    pub fn utilization_fraction(self) -> f32 {
        (self.value.utilization_percent.unwrap_or(0.0) as f32 / 100.0).clamp(0.0, 1.0)
    }

    pub fn utilization_history(self) -> Option<&'a VecDeque<f64>> {
        self.utilization_history
    }

    pub fn memory_value(self) -> String {
        match (self.value.memory_used_bytes, self.value.memory_total_bytes) {
            (Some(used), Some(total)) => {
                format!("{} / {}", format_bytes(used), format_bytes(total))
            }
            _ => unavailable(),
        }
    }

    pub fn memory_fraction(self) -> f32 {
        match (self.value.memory_used_bytes, self.value.memory_total_bytes) {
            (Some(used), Some(total)) if total > 0 => (used as f32 / total as f32).clamp(0.0, 1.0),
            _ => 0.0,
        }
    }

    pub fn memory_history(self) -> Option<&'a VecDeque<f64>> {
        self.memory_history
    }

    pub fn memory_graph_max(self) -> f64 {
        self.value.memory_total_bytes.unwrap_or(1).max(1) as f64
    }

    pub fn temperature_value(self) -> String {
        self.value
            .temperature_celsius
            .map(|value| format!("{value:.1}°C"))
            .unwrap_or_else(unavailable)
    }

    pub fn temperature_history(self) -> Option<&'a VecDeque<f64>> {
        self.temperature_history
    }

    pub fn power_value(self) -> String {
        self.value
            .power_watts
            .map(|value| format!("{value:.0} W"))
            .unwrap_or_else(unavailable)
    }

    pub fn core_clock_value(self) -> String {
        self.value
            .core_clock_mhz
            .map(|value| format!("{value} MHz"))
            .unwrap_or_else(unavailable)
    }

    pub fn fan_value(self) -> String {
        self.value
            .fan_percent
            .map(format_percent)
            .or_else(|| self.value.fan_rpm.map(|value| format!("{value} RPM")))
            .unwrap_or_else(unavailable)
    }
}

#[derive(Clone, Copy)]
pub struct TemperaturePanel<'a> {
    pub(crate) value: &'a TemperatureSnapshot,
    pub(crate) history: Option<&'a VecDeque<f64>>,
}

impl<'a> TemperaturePanel<'a> {
    pub fn name(self) -> &'a str {
        short_temperature_name(&self.value.name)
    }

    pub fn value(self) -> String {
        format!("{:.1}°C", self.value.celsius)
    }

    pub fn history(self) -> Option<&'a VecDeque<f64>> {
        self.history
    }
}

pub(crate) fn short_temperature_name(name: &str) -> &str {
    short_gpu_name(name)
}

#[derive(Clone, Copy)]
pub struct FanPanel<'a> {
    pub(crate) value: &'a FanSnapshot,
    pub(crate) history: Option<&'a VecDeque<f64>>,
}

impl<'a> FanPanel<'a> {
    pub fn name(self) -> &'a str {
        &self.value.name
    }

    pub fn value(self) -> String {
        format!("{} RPM", self.value.rpm)
    }

    pub fn history(self) -> Option<&'a VecDeque<f64>> {
        self.history
    }
}

pub(crate) fn short_gpu_name(name: &str) -> &str {
    let name = name.strip_prefix("NVIDIA ").unwrap_or(name);
    name.strip_prefix("GeForce ").unwrap_or(name)
}
