use std::sync::{Arc, Mutex};

use prometheus::{
    core::{Collector, Desc},
    proto::MetricFamily,
    Counter, Opts,
};
use sysinfo::{CpuRefreshKind, Disks, MemoryRefreshKind, RefreshKind, System};

const NAMESPACE: &str = "hardware_metrics";

pub struct HardwareMetrics {
    system: Arc<Mutex<System>>,
    pub cpu: CpuMetrics,
    pub memory: MemoryMetrics,
    pub disk: DiskMetrics,
}
impl HardwareMetrics {
    fn new() -> Self {
        let mut system = System::new_with_specifics(
            RefreshKind::nothing()
                .with_cpu(CpuRefreshKind::nothing())
                .with_memory(MemoryRefreshKind::nothing().with_ram()),
        );

        Self {
            cpu: CpuMetrics::new(&mut system),
            memory: MemoryMetrics::new(&mut system),
            disk: DiskMetrics::new(&mut system),
            system: Arc::new(Mutex::new(system)),
        }
    }
    fn update(&self) {
        let mut system = self.system.lock().unwrap();
        system.refresh_all();
        self.cpu.update(&system);
    }
}
impl Collector for HardwareMetrics {
    fn desc(&self) -> Vec<&Desc> {
        let mut descs = Vec::new();
        descs.extend(self.cpu.desc());
        descs.extend(self.memory.desc());
        descs.extend(self.disk.desc());
        descs
    }
    fn collect(&self) -> Vec<MetricFamily> {
        self.update();
        let mut mfs = Vec::new();
        mfs.extend(self.cpu.collect());
        mfs.extend(self.memory.collect());
        mfs.extend(self.disk.collect());
        mfs
    }
}

pub struct CpuMetrics {
    // #[cfg(feature = "hardware.usage")]
    // cpu_usage: Gauge,
    cpu_specs: Counter,
}
impl CpuMetrics {
    pub fn new(system: &System) -> Self {
        let cpu_vendor_id: &str = system
            .cpus()
            .first()
            .map_or("unknown_cpu_vendor_id", |cpu| cpu.vendor_id());
        let cpu_brand: &str = system
            .cpus()
            .first()
            .map_or("unknown_cpu_model", |cpu| cpu.brand());

        let cpu_specs = Counter::with_opts(
            Opts::new("cpu_info", "CPU specs (constants)")
                .const_label("cpu_vendor_id", cpu_vendor_id)
                .const_label("cpu_brand", cpu_brand)
                .const_label("cpu_arch", System::cpu_arch())
                .const_label(
                    "num_cpu_cores",
                    system
                        .physical_core_count()
                        .map(|c| c.to_string())
                        .unwrap_or_else(|| "unknown_num_cpu_cores".to_owned()),
                )
                .namespace(NAMESPACE),
        )
        .unwrap();

        // #[cfg(feature = "hardware.usage")]
        // let cpu_usage = Gauge::with_opts(
        //     Opts::new("cpu_usage", "Total user and system CPU time").namespace(NAMESPACE),
        // )
        // .unwrap();

        Self {
            // #[cfg(feature = "hardware.usage")]
            // cpu_usage,
            cpu_specs,
        }
    }
    fn update(&self, system: &System) {
        // #[cfg(feature = "hardware.usage")]
        // self.cpu_usage.set(system.global_cpu_usage() as f64);
    }
}
const CPU_METRICS_COUNT: usize = 12;
impl Collector for CpuMetrics {
    fn desc(&self) -> Vec<&Desc> {
        let mut desc = Vec::new();

        // #[cfg(feature = "hardware.usage")]
        // desc.extend(self.cpu_usage.desc());

        desc.extend(&self.cpu_specs.desc());
        desc
    }

    fn collect(&self) -> Vec<MetricFamily> {
        let mut mfs = Vec::with_capacity(CPU_METRICS_COUNT);

        // #[cfg(feature = "hardware.usage")]
        // mfs.extend(self.cpu_usage.collect());

        mfs.extend(self.cpu_specs.collect());
        mfs
    }
}

pub struct MemoryMetrics {
    pub specs: Counter,
}
impl MemoryMetrics {
    pub fn new(system: &System) -> Self {
        let mem_total = system.total_memory();
        println!("mem_total: {mem_total}");

        Self {
            specs: Counter::with_opts(
                Opts::new(
                    "memory_specs",
                    "Memory specs (constants: total amount, ...)",
                )
                .const_label("mem_total_ram_bytes", mem_total.to_string())
                .const_label(
                    "mem_total_ram_human",
                    format!("{}", human_fmt_bytes(mem_total)),
                ),
            )
            .unwrap(),
        }
    }
}
const MEMORY_METRICS_COUNT: usize = 3;
impl Collector for MemoryMetrics {
    fn desc(&self) -> Vec<&Desc> {
        let mut desc = Vec::new();
        desc.extend(&self.specs.desc());
        desc
    }
    fn collect(&self) -> Vec<MetricFamily> {
        let mut mfs = Vec::with_capacity(MEMORY_METRICS_COUNT);
        mfs.extend(self.specs.collect());
        mfs
    }
}

pub struct DiskMetrics {
    pub specs: Counter,
}
impl DiskMetrics {
    pub fn new(system: &System) -> Self {
        let disks = Disks::new_with_refreshed_list();
        // for disk in disks.iter() {
        //     println!("disk name: {}", disk.name().to_string_lossy());
        //     println!("disk_kind: {}", disk.kind().to_string());
        //     println!("space: {}", disk.total_space());
        // }
        let disk_total_space: u64 = disks
            .iter()
            .max_by_key(|disk| disk.total_space())
            .map(|d| d.total_space())
            .unwrap_or(0);

        Self {
            specs: Counter::with_opts(
                Opts::new(
                    "disk_specs",
                    "Constant disk specifications (total disk space, ...)",
                )
                .const_label("disk_total_space_bytes", disk_total_space.to_string())
                .const_label("disk_total_space_human", human_fmt_bytes(disk_total_space)),
            )
            .unwrap(),
        }
    }
}
const DISK_METRICS_COUNT: usize = 1;
impl Collector for DiskMetrics {
    fn desc(&self) -> Vec<&Desc> {
        let mut desc = Vec::new();
        desc.extend(&self.specs.desc());
        desc
    }
    fn collect(&self) -> Vec<MetricFamily> {
        let mut mfs = Vec::with_capacity(DISK_METRICS_COUNT);
        mfs.extend(self.specs.collect());
        mfs
    }
}

fn human_fmt_bytes(bytes: u64) -> String {
    const UNITS: [&str; 6] = ["B", "KB", "MB", "GB", "TB", "PB"];

    let mut value = bytes;
    let mut unit_idx = 0;
    // Shift right until we're below 1024^2 or reach end of units
    while value >= (1024 * 1024) && unit_idx < UNITS.len() - 2 {
        // dbg!("val_before", value);
        value >>= 10;
        // dbg!("val_after", value);
        unit_idx += 1;
    }
    let value: f64 = value as f64 / 1024.0;
    unit_idx += 1;

    format!("{:.2} {}", value, UNITS[unit_idx])
}

#[cfg(test)]
mod tests {
    use super::*;
    use prometheus::Registry;

    #[test]
    fn test_hardware_metrics() {
        let metrics_collector = HardwareMetrics::new();
        let metrics = metrics_collector.collect();
        dbg!(metrics);
    }

    #[test]
    fn test_cpu_metrics() {
        let mut system = System::new_with_specifics(
            RefreshKind::nothing().with_cpu(CpuRefreshKind::everything()),
        );
        let mut cpu_metrics = CpuMetrics::new(&mut system);

        // let r = Registry::new();
        // r.register(Box::new(cpu_metrics.clone())).unwrap();

        system.refresh_all();
        let metrics = cpu_metrics.collect();
        dbg!(&metrics);

        // assert_eq!(cpu_metrics.desc().len(), 1);
    }
}
