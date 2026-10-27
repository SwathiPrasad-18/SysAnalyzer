// ============================================================
// ai.rs  –  AI / Statistical Analysis Engine
// ============================================================
use crate::collector::{Snapshot, HISTORY_LEN};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

// ── severity levels ──────────────────────────────────────────────────────────
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Severity {
    Ok,
    Info,
    Warning,
    Critical,
}

impl Severity {
    pub fn color(&self) -> ratatui::style::Color {
        use ratatui::style::Color;
        match self {
            Severity::Ok => Color::Green,
            Severity::Info => Color::Cyan,
            Severity::Warning => Color::Yellow,
            Severity::Critical => Color::Red,
        }
    }
    pub fn label(&self) -> &'static str {
        match self {
            Severity::Ok => "OK",
            Severity::Info => "INFO",
            Severity::Warning => "WARN",
            Severity::Critical => "CRIT",
        }
    }
}

// ── an anomaly/finding ───────────────────────────────────────────────────────
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Finding {
    pub severity: Severity,
    pub category: String,
    pub message: String,
    pub value: f64,
    pub threshold: f64,
    pub ts: chrono::DateTime<chrono::Utc>,
}

// ── rolling statistics ────────────────────────────────────────────────────────
#[derive(Debug, Default, Clone)]
pub struct RollingStat {
    pub window: VecDeque<f64>,
    capacity: usize,
}

impl RollingStat {
    pub fn new(capacity: usize) -> Self {
        Self {
            window: VecDeque::with_capacity(capacity),
            capacity,
        }
    }
    pub fn push(&mut self, v: f64) {
        if self.window.len() >= self.capacity {
            self.window.pop_front();
        }
        self.window.push_back(v);
    }
    pub fn mean(&self) -> f64 {
        if self.window.is_empty() {
            return 0.0;
        }
        self.window.iter().sum::<f64>() / self.window.len() as f64
    }
    pub fn stddev(&self) -> f64 {
        if self.window.len() < 2 {
            return 0.0;
        }
        let m = self.mean();
        let var = self.window.iter().map(|x| (x - m).powi(2)).sum::<f64>()
            / (self.window.len() - 1) as f64;
        var.sqrt()
    }
    #[allow(dead_code)]
    pub fn min(&self) -> f64 {
        self.window.iter().cloned().fold(f64::INFINITY, f64::min)
    }
    pub fn max(&self) -> f64 {
        self.window
            .iter()
            .cloned()
            .fold(f64::NEG_INFINITY, f64::max)
    }
    pub fn percentile(&self, p: f64) -> f64 {
        if self.window.is_empty() {
            return 0.0;
        }
        let mut sorted: Vec<f64> = self.window.iter().cloned().collect();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let idx = (p / 100.0 * (sorted.len() - 1) as f64).round() as usize;
        sorted[idx.min(sorted.len() - 1)]
    }
    /// Z-score of the latest value
    pub fn z_score(&self) -> f64 {
        let s = self.stddev();
        if s < 1e-9 {
            return 0.0;
        }
        if let Some(&last) = self.window.back() {
            (last - self.mean()) / s
        } else {
            0.0
        }
    }
    /// Exponential moving average
    #[allow(dead_code)]
    pub fn ema(&self, alpha: f64) -> f64 {
        let mut ema = 0.0;
        let mut first = true;
        for &v in &self.window {
            if first {
                ema = v;
                first = false;
            } else {
                ema = alpha * v + (1.0 - alpha) * ema;
            }
        }
        ema
    }
    /// Simple linear trend  (slope per sample)
    pub fn trend(&self) -> f64 {
        let n = self.window.len();
        if n < 2 {
            return 0.0;
        }
        let xs: Vec<f64> = (0..n).map(|i| i as f64).collect();
        let ys: Vec<f64> = self.window.iter().cloned().collect();
        let sx: f64 = xs.iter().sum();
        let sy: f64 = ys.iter().sum();
        let sxx: f64 = xs.iter().map(|x| x * x).sum();
        let sxy: f64 = xs.iter().zip(ys.iter()).map(|(x, y)| x * y).sum();
        let nn = n as f64;
        (nn * sxy - sx * sy) / (nn * sxx - sx * sx + 1e-12)
    }
    /// Predict value N steps ahead using linear regression
    pub fn predict(&self, steps: usize) -> f64 {
        let n = self.window.len();
        if n < 2 {
            return self.window.back().cloned().unwrap_or(0.0);
        }
        let current_x = (n - 1) as f64;
        let _future_x = current_x + steps as f64;
        let slope = self.trend();
        let last = self.window.back().cloned().unwrap_or(0.0);
        (last + slope * steps as f64).clamp(0.0, 100.0)
    }
}

// ── health score ─────────────────────────────────────────────────────────────
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthScore {
    pub overall: f64,        // 0 – 100
    pub cpu_score: f64,
    pub mem_score: f64,
    pub disk_score: f64,
    pub network_score: f64,
    pub temp_score: f64,
    pub grade: char,
    pub summary: String,
}

impl HealthScore {
    pub fn color(&self) -> ratatui::style::Color {
        use ratatui::style::Color;
        match self.overall as u32 {
            90..=100 => Color::Green,
            70..=89 => Color::LightGreen,
            50..=69 => Color::Yellow,
            30..=49 => Color::LightRed,
            _ => Color::Red,
        }
    }
}

// ── AI Engine ────────────────────────────────────────────────────────────────
pub struct AiEngine {
    pub cpu_stat: RollingStat,
    pub mem_stat: RollingStat,
    pub net_rx_stat: RollingStat,
    pub net_tx_stat: RollingStat,
    pub disk_read_stat: RollingStat,
    pub disk_write_stat: RollingStat,
    pub temp_stat: RollingStat,
    pub findings: VecDeque<Finding>,
    pub health: HealthScore,
}

impl AiEngine {
    pub fn new() -> Self {
        let cap = HISTORY_LEN;
        AiEngine {
            cpu_stat: RollingStat::new(cap),
            mem_stat: RollingStat::new(cap),
            net_rx_stat: RollingStat::new(cap),
            net_tx_stat: RollingStat::new(cap),
            disk_read_stat: RollingStat::new(cap),
            disk_write_stat: RollingStat::new(cap),
            temp_stat: RollingStat::new(cap),
            findings: VecDeque::with_capacity(200),
            health: HealthScore {
                overall: 100.0,
                cpu_score: 100.0,
                mem_score: 100.0,
                disk_score: 100.0,
                network_score: 100.0,
                temp_score: 100.0,
                grade: 'A',
                summary: "System is healthy".to_string(),
            },
        }
    }

    pub fn ingest(&mut self, snap: &Snapshot) {
        let mem_pct = if snap.mem_total_kb > 0 {
            snap.mem_used_kb as f64 / snap.mem_total_kb as f64 * 100.0
        } else {
            0.0
        };
        let max_temp = snap.temps.iter().map(|t| t.celsius as f64).fold(0.0f64, f64::max);

        self.cpu_stat.push(snap.cpu_overall as f64);
        self.mem_stat.push(mem_pct);
        self.net_rx_stat.push(snap.net_rx_bytes as f64 / 1024.0);
        self.net_tx_stat.push(snap.net_tx_bytes as f64 / 1024.0);
        self.disk_read_stat.push(snap.disk_read_bytes as f64 / 1024.0);
        self.disk_write_stat.push(snap.disk_write_bytes as f64 / 1024.0);
        if max_temp > 0.0 {
            self.temp_stat.push(max_temp);
        }

        self.detect_anomalies(snap, mem_pct, max_temp);
        self.compute_health(snap, mem_pct, max_temp);
    }

    fn add_finding(&mut self, f: Finding) {
        if self.findings.len() >= 200 {
            self.findings.pop_front();
        }
        self.findings.push_back(f);
    }

    fn detect_anomalies(&mut self, snap: &Snapshot, mem_pct: f64, max_temp: f64) {
        let now = snap.ts;

        // ── CPU thresholds ──
        let cpu = snap.cpu_overall as f64;
        if cpu > 95.0 {
            self.add_finding(Finding {
                severity: Severity::Critical,
                category: "CPU".to_string(),
                message: format!("CPU critically high: {:.1}%", cpu),
                value: cpu,
                threshold: 95.0,
                ts: now,
            });
        } else if cpu > 80.0 {
            self.add_finding(Finding {
                severity: Severity::Warning,
                category: "CPU".to_string(),
                message: format!("CPU usage high: {:.1}%", cpu),
                value: cpu,
                threshold: 80.0,
                ts: now,
            });
        }

        // ── Z-score anomaly for CPU ──
        let z = self.cpu_stat.z_score();
        if z.abs() > 3.0 && self.cpu_stat.window.len() > 20 {
            self.add_finding(Finding {
                severity: Severity::Warning,
                category: "CPU".to_string(),
                message: format!(
                    "CPU statistical anomaly detected (z={:.2}). μ={:.1}% σ={:.1}%",
                    z,
                    self.cpu_stat.mean(),
                    self.cpu_stat.stddev()
                ),
                value: cpu,
                threshold: self.cpu_stat.mean() + 3.0 * self.cpu_stat.stddev(),
                ts: now,
            });
        }

        // ── Memory ──
        if mem_pct > 92.0 {
            self.add_finding(Finding {
                severity: Severity::Critical,
                category: "Memory".to_string(),
                message: format!("Memory critically low: {:.1}% used", mem_pct),
                value: mem_pct,
                threshold: 92.0,
                ts: now,
            });
        } else if mem_pct > 80.0 {
            self.add_finding(Finding {
                severity: Severity::Warning,
                category: "Memory".to_string(),
                message: format!("Memory pressure: {:.1}% used", mem_pct),
                value: mem_pct,
                threshold: 80.0,
                ts: now,
            });
        }

        // ── Memory trend prediction ──
        if self.mem_stat.window.len() >= 30 {
            let slope = self.mem_stat.trend();
            if slope > 0.5 {
                let predicted = self.mem_stat.predict(60);
                if predicted > 85.0 {
                    self.add_finding(Finding {
                        severity: Severity::Info,
                        category: "Memory".to_string(),
                        message: format!(
                            "AI Prediction: Memory may reach {:.1}% in ~60s (trend +{:.2}%/s)",
                            predicted, slope
                        ),
                        value: predicted,
                        threshold: 85.0,
                        ts: now,
                    });
                }
            }
        }

        // ── Swap ──
        if snap.swap_total_kb > 0 {
            let swap_pct =
                snap.swap_used_kb as f64 / snap.swap_total_kb as f64 * 100.0;
            if swap_pct > 50.0 {
                self.add_finding(Finding {
                    severity: Severity::Warning,
                    category: "Memory".to_string(),
                    message: format!("High swap usage: {:.1}%", swap_pct),
                    value: swap_pct,
                    threshold: 50.0,
                    ts: now,
                });
            }
        }

        // ── Disk ──
        for d in &snap.disk_usage {
            if d.usage_pct > 95.0 {
                self.add_finding(Finding {
                    severity: Severity::Critical,
                    category: "Disk".to_string(),
                    message: format!("Disk '{}' almost full: {:.1}%", d.mount, d.usage_pct),
                    value: d.usage_pct as f64,
                    threshold: 95.0,
                    ts: now,
                });
            } else if d.usage_pct > 85.0 {
                self.add_finding(Finding {
                    severity: Severity::Warning,
                    category: "Disk".to_string(),
                    message: format!("Disk '{}' running low: {:.1}%", d.mount, d.usage_pct),
                    value: d.usage_pct as f64,
                    threshold: 85.0,
                    ts: now,
                });
            }
        }

        // ── Temperature ──
        if max_temp > 0.0 {
            if max_temp > 90.0 {
                self.add_finding(Finding {
                    severity: Severity::Critical,
                    category: "Thermal".to_string(),
                    message: format!("CRITICAL temperature: {:.1}°C", max_temp),
                    value: max_temp,
                    threshold: 90.0,
                    ts: now,
                });
            } else if max_temp > 75.0 {
                self.add_finding(Finding {
                    severity: Severity::Warning,
                    category: "Thermal".to_string(),
                    message: format!("High temperature: {:.1}°C", max_temp),
                    value: max_temp,
                    threshold: 75.0,
                    ts: now,
                });
            }
        }

        // ── Load average ──
        let cores = snap.cpu_per_core.len().max(1) as f64;
        if snap.load_avg.0 > cores * 2.0 {
            self.add_finding(Finding {
                severity: Severity::Critical,
                category: "Load".to_string(),
                message: format!(
                    "Load avg critically high: 1m={:.2} (cores={})",
                    snap.load_avg.0, cores as usize
                ),
                value: snap.load_avg.0,
                threshold: cores * 2.0,
                ts: now,
            });
        } else if snap.load_avg.0 > cores {
            self.add_finding(Finding {
                severity: Severity::Warning,
                category: "Load".to_string(),
                message: format!(
                    "System overloaded: 1m={:.2} (cores={})",
                    snap.load_avg.0, cores as usize
                ),
                value: snap.load_avg.0,
                threshold: cores,
                ts: now,
            });
        }

        // ── Network spike ──
        let rx_z = self.net_rx_stat.z_score();
        if rx_z > 3.5 && self.net_rx_stat.window.len() > 15 {
            self.add_finding(Finding {
                severity: Severity::Info,
                category: "Network".to_string(),
                message: format!(
                    "Network RX spike detected (z={:.2}, +{:.1} KB/s above avg)",
                    rx_z,
                    snap.net_rx_bytes as f64 / 1024.0 - self.net_rx_stat.mean()
                ),
                value: snap.net_rx_bytes as f64 / 1024.0,
                threshold: self.net_rx_stat.mean() + 3.5 * self.net_rx_stat.stddev(),
                ts: now,
            });
        }
    }

    fn score_metric(value: f64, warn: f64, crit: f64) -> f64 {
        if value <= warn {
            100.0
        } else if value >= crit {
            0.0
        } else {
            100.0 * (1.0 - (value - warn) / (crit - warn))
        }
    }

    fn compute_health(&mut self, snap: &Snapshot, mem_pct: f64, max_temp: f64) {
        let cpu = snap.cpu_overall as f64;
        let cpu_score = Self::score_metric(cpu, 60.0, 95.0);
        let mem_score = Self::score_metric(mem_pct, 70.0, 92.0);

        let worst_disk = snap
            .disk_usage
            .iter()
            .map(|d| d.usage_pct as f64)
            .fold(0.0f64, f64::max);
        let disk_score = Self::score_metric(worst_disk, 75.0, 95.0);

        let temp_score = if max_temp > 0.0 {
            Self::score_metric(max_temp, 65.0, 90.0)
        } else {
            100.0
        };

        let cores = snap.cpu_per_core.len().max(1) as f64;
        let la_ratio = snap.load_avg.0 / cores * 100.0;
        let network_score = Self::score_metric(la_ratio, 80.0, 200.0);

        let overall = cpu_score * 0.30
            + mem_score * 0.25
            + disk_score * 0.20
            + temp_score * 0.15
            + network_score * 0.10;

        let grade = match overall as u32 {
            90..=100 => 'A',
            80..=89 => 'B',
            70..=79 => 'C',
            60..=69 => 'D',
            _ => 'F',
        };

        let summary = if overall >= 90.0 {
            "System is healthy and performing well".to_string()
        } else if overall >= 70.0 {
            "System is stable with minor concerns".to_string()
        } else if overall >= 50.0 {
            "System under stress — attention recommended".to_string()
        } else if overall >= 30.0 {
            "System critically stressed — action required".to_string()
        } else {
            "SYSTEM IN CRISIS — immediate action required!".to_string()
        };

        self.health = HealthScore {
            overall,
            cpu_score,
            mem_score,
            disk_score,
            network_score,
            temp_score,
            grade,
            summary,
        };
    }

    pub fn recent_findings(&self, n: usize) -> Vec<&Finding> {
        self.findings.iter().rev().take(n).collect()
    }

    pub fn stats_report(&self) -> String {
        format!(
            "CPU  μ={:.1}%  σ={:.1}%  p95={:.1}%  trend={:+.2}%/s\n\
             MEM  μ={:.1}%  σ={:.1}%  p95={:.1}%  trend={:+.2}%/s\n\
             NET↓ μ={:.1}KB  σ={:.1}KB  p95={:.1}KB\n\
             NET↑ μ={:.1}KB  σ={:.1}KB  p95={:.1}KB",
            self.cpu_stat.mean(), self.cpu_stat.stddev(), self.cpu_stat.percentile(95.0), self.cpu_stat.trend(),
            self.mem_stat.mean(), self.mem_stat.stddev(), self.mem_stat.percentile(95.0), self.mem_stat.trend(),
            self.net_rx_stat.mean(), self.net_rx_stat.stddev(), self.net_rx_stat.percentile(95.0),
            self.net_tx_stat.mean(), self.net_tx_stat.stddev(), self.net_tx_stat.percentile(95.0),
        )
    }
}
