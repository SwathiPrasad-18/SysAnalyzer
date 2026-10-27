// ============================================================
// collector.rs  –  Live System Metrics Collector
// ============================================================
use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::time::Instant;
use sysinfo::{Components, Disks, Networks, System};

pub const HISTORY_LEN: usize = 120; // 2 min of 1-sec samples

// ── snapshot of one collection tick ────────────────────────────────────────
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    pub ts: DateTime<Utc>,

    // CPU
    pub cpu_overall: f32,
    pub cpu_per_core: Vec<f32>,
    pub cpu_freq_mhz: u64,
    pub load_avg: (f64, f64, f64),

    // Memory
    pub mem_total_kb: u64,
    pub mem_used_kb: u64,
    pub mem_free_kb: u64,
    pub mem_available_kb: u64,
    pub swap_total_kb: u64,
    pub swap_used_kb: u64,

    // Disk I/O
    pub disk_read_bytes: u64,
    pub disk_write_bytes: u64,
    pub disk_usage: Vec<DiskInfo>,

    // Network
    pub net_rx_bytes: u64,
    pub net_tx_bytes: u64,
    pub net_interfaces: Vec<NetInfo>,

    // Temperature
    pub temps: Vec<TempInfo>,

    // Processes
    pub proc_count: usize,
    pub top_procs: Vec<ProcInfo>,

    // Uptime
    pub uptime_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskInfo {
    pub name: String,
    pub mount: String,
    pub total_kb: u64,
    pub used_kb: u64,
    pub available_kb: u64,
    pub usage_pct: f32,
    pub fs: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetInfo {
    pub name: String,
    pub rx_bytes: u64,
    pub tx_bytes: u64,
    pub rx_rate: f64, // bytes/sec
    pub tx_rate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TempInfo {
    pub label: String,
    pub celsius: f32,
    pub critical: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcInfo {
    pub pid: u32,
    pub name: String,
    pub cpu_pct: f32,
    pub mem_kb: u64,
    pub status: String,
}

// ── collector ───────────────────────────────────────────────────────────────
pub struct Collector {
    sys: System,
    disks: Disks,
    networks: Networks,
    components: Components,
    last_net_rx: u64,
    last_net_tx: u64,
    last_disk_r: u64,
    last_disk_w: u64,
    last_tick: Instant,
    pub history: VecDeque<Snapshot>,
}

impl Collector {
    pub fn new() -> Self {
        let mut sys = System::new_all();
        sys.refresh_all();
        let disks = Disks::new_with_refreshed_list();
        let networks = Networks::new_with_refreshed_list();
        let components = Components::new_with_refreshed_list();
        Collector {
            sys,
            disks,
            networks,
            components,
            last_net_rx: 0,
            last_net_tx: 0,
            last_disk_r: 0,
            last_disk_w: 0,
            last_tick: Instant::now(),
            history: VecDeque::with_capacity(HISTORY_LEN),
        }
    }

    pub fn tick(&mut self) -> Result<Snapshot> {
        self.sys.refresh_all();
        self.disks.refresh();
        self.networks.refresh();
        self.components.refresh();

        let elapsed = self.last_tick.elapsed().as_secs_f64().max(0.001);
        self.last_tick = Instant::now();

        // ── CPU ──
        let cpu_overall = self.sys.global_cpu_usage();
        let cpu_per_core: Vec<f32> = self.sys.cpus().iter().map(|c| c.cpu_usage()).collect();
        let cpu_freq_mhz = self
            .sys
            .cpus()
            .first()
            .map(|c| c.frequency())
            .unwrap_or(0);
        let load_avg = {
            let la = System::load_average();
            (la.one, la.five, la.fifteen)
        };

        // ── Memory ──
        let mem_total_kb = self.sys.total_memory() / 1024;
        let mem_used_kb = self.sys.used_memory() / 1024;
        let mem_free_kb = self.sys.free_memory() / 1024;
        let mem_available_kb = self.sys.available_memory() / 1024;
        let swap_total_kb = self.sys.total_swap() / 1024;
        let swap_used_kb = self.sys.used_swap() / 1024;

        // ── Disk ──
        let (mut cur_r, mut cur_w) = (0u64, 0u64);
        let disk_usage: Vec<DiskInfo> = self
            .disks
            .iter()
            .map(|d| {
                let total = d.total_space();
                let avail = d.available_space();
                let used = total.saturating_sub(avail);
                let pct = if total > 0 {
                    (used as f32 / total as f32) * 100.0
                } else {
                    0.0
                };
                DiskInfo {
                    name: d.name().to_string_lossy().to_string(),
                    mount: d.mount_point().to_string_lossy().to_string(),
                    total_kb: total / 1024,
                    used_kb: used / 1024,
                    available_kb: avail / 1024,
                    usage_pct: pct,
                    fs: d.file_system().to_string_lossy().to_string(),
                }
            })
            .collect();

        // Disk I/O counters: procfs is available only on Linux.
        #[cfg(target_os = "linux")]
        {
            if let Ok(disk_stats) = procfs::diskstats() {
                for ds in &disk_stats {
                    cur_r += ds.sectors_read * 512;
                    cur_w += ds.sectors_written * 512;
                }
            }
        }

        let disk_read_bytes = cur_r.saturating_sub(self.last_disk_r);
        let disk_write_bytes = cur_w.saturating_sub(self.last_disk_w);
        self.last_disk_r = cur_r;
        self.last_disk_w = cur_w;

        // ── Network ──
        let (mut cur_rx, mut cur_tx) = (0u64, 0u64);
        let net_interfaces: Vec<NetInfo> = self
            .networks
            .iter()
            .map(|(name, data)| {
                cur_rx += data.received();
                cur_tx += data.transmitted();
                NetInfo {
                    name: name.clone(),
                    rx_bytes: data.received(),
                    tx_bytes: data.transmitted(),
                    rx_rate: data.received() as f64 / elapsed,
                    tx_rate: data.transmitted() as f64 / elapsed,
                }
            })
            .collect();

        let net_rx_bytes = cur_rx.saturating_sub(self.last_net_rx);
        let net_tx_bytes = cur_tx.saturating_sub(self.last_net_tx);
        self.last_net_rx = cur_rx;
        self.last_net_tx = cur_tx;

        // ── Temps ──
        let temps: Vec<TempInfo> = self
            .components
            .iter()
            .map(|c| TempInfo {
                label: c.label().to_string(),
                celsius: c.temperature(),
                critical: c.critical(),
            })
            .collect();

        // ── Processes ──
        let proc_count = self.sys.processes().len();
        let mut procs: Vec<ProcInfo> = self
            .sys
            .processes()
            .iter()
            .map(|(pid, p)| ProcInfo {
                pid: pid.as_u32(),
                name: p.name().to_string_lossy().to_string(),
                cpu_pct: p.cpu_usage(),
                mem_kb: p.memory() / 1024,
                status: format!("{:?}", p.status()),
            })
            .collect();
        procs.sort_by(|a, b| b.cpu_pct.partial_cmp(&a.cpu_pct).unwrap());
        let top_procs = procs.into_iter().take(10).collect();

        let snap = Snapshot {
            ts: Utc::now(),
            cpu_overall,
            cpu_per_core,
            cpu_freq_mhz,
            load_avg,
            mem_total_kb,
            mem_used_kb,
            mem_free_kb,
            mem_available_kb,
            swap_total_kb,
            swap_used_kb,
            disk_read_bytes,
            disk_write_bytes,
            disk_usage,
            net_rx_bytes,
            net_tx_bytes,
            net_interfaces,
            temps,
            proc_count,
            top_procs,
            uptime_secs: System::uptime(),
        };

        if self.history.len() >= HISTORY_LEN {
            self.history.pop_front();
        }
        self.history.push_back(snap.clone());
        Ok(snap)
    }

    /// Returns a Vec<f64> of the last N cpu_overall values for sparklines
    pub fn cpu_history(&self) -> Vec<f64> {
        self.history.iter().map(|s| s.cpu_overall as f64).collect()
    }

    pub fn mem_history(&self) -> Vec<f64> {
        self.history
            .iter()
            .map(|s| {
                if s.mem_total_kb > 0 {
                    s.mem_used_kb as f64 / s.mem_total_kb as f64 * 100.0
                } else {
                    0.0
                }
            })
            .collect()
    }

    pub fn net_rx_history(&self) -> Vec<f64> {
        self.history
            .iter()
            .map(|s| s.net_rx_bytes as f64 / 1024.0)
            .collect()
    }

    pub fn net_tx_history(&self) -> Vec<f64> {
        self.history
            .iter()
            .map(|s| s.net_tx_bytes as f64 / 1024.0)
            .collect()
    }
}

// ── helpers ──────────────────────────────────────────────────────────────────
pub fn fmt_bytes(b: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let mut v = b as f64;
    let mut i = 0;
    while v >= 1024.0 && i < UNITS.len() - 1 {
        v /= 1024.0;
        i += 1;
    }
    if i == 0 {
        format!("{} {}", b, UNITS[i])
    } else {
        format!("{:.1} {}", v, UNITS[i])
    }
}

pub fn fmt_duration(secs: u64) -> String {
    let d = secs / 86400;
    let h = (secs % 86400) / 3600;
    let m = (secs % 3600) / 60;
    let s = secs % 60;
    if d > 0 {
        format!("{}d {:02}h {:02}m {:02}s", d, h, m, s)
    } else {
        format!("{:02}h {:02}m {:02}s", h, m, s)
    }
}
