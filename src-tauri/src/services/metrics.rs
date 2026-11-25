use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System};

use super::process_tree::tree_pids;

/// Muestreador de CPU/RAM. La CPU de sysinfo necesita dos refrescos: la primera
/// muestra siempre da 0.
pub struct Sampler {
    sys: System,
}

impl Sampler {
    pub fn new() -> Self {
        Self { sys: System::new() }
    }

    pub fn refresh(&mut self) {
        self.sys.refresh_processes_specifics(
            ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::nothing().with_cpu().with_memory(),
        );
    }

    /// (CPU % sumada, RAM MB sumada con 1 decimal) del árbol de `root`.
    pub fn tree_usage(&self, root: u32) -> (f32, f64) {
        let mut cpu = 0f32;
        let mut bytes = 0u64;
        for pid in tree_pids(&self.sys, root) {
            if let Some(p) = self.sys.process(sysinfo::Pid::from_u32(pid)) {
                cpu += p.cpu_usage();
                bytes += p.memory();
            }
        }
        let mb = bytes as f64 / (1024.0 * 1024.0);
        ((cpu * 10.0).round() / 10.0, (mb * 10.0).round() / 10.0)
    }
}

impl Default for Sampler {
    fn default() -> Self {
        Self::new()
    }
}
