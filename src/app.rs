// ============================================================
// app.rs  –  Application State & Event Loop
// ============================================================
use crate::ai::AiEngine;
use crate::collector::{Collector, Snapshot};
use anyhow::Result;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, PartialEq)]
pub enum Tab {
    Overview,
    CPU,
    Memory,
    Disk,
    Network,
    Processes,
    AI,
    Logs,
}

impl Tab {
    pub fn all() -> Vec<Tab> {
        vec![
            Tab::Overview,
            Tab::CPU,
            Tab::Memory,
            Tab::Disk,
            Tab::Network,
            Tab::Processes,
            Tab::AI,
            Tab::Logs,
        ]
    }
    pub fn title(&self) -> &'static str {
        match self {
            Tab::Overview  => "Overview",
            Tab::CPU       => "CPU",
            Tab::Memory    => "Memory",
            Tab::Disk      => "Disk",
            Tab::Network   => "Network",
            Tab::Processes => "Processes",
            Tab::AI        => "AI Analysis",
            Tab::Logs      => "AI Logs",
        }
    }
    pub fn index(&self) -> usize {
        Tab::all().iter().position(|t| t == self).unwrap_or(0)
    }
}

// ── exported snapshot for the report writer ──────────────────────────────────
#[derive(serde::Serialize)]
pub struct ExportBundle<'a> {
    pub generated_at: String,
    pub health: &'a crate::ai::HealthScore,
    pub stats: String,
    pub latest: Option<&'a Snapshot>,
    pub findings_count: usize,
}

pub struct App {
    pub collector: Collector,
    pub ai: AiEngine,
    pub latest: Option<Snapshot>,
    pub current_tab: Tab,
    pub proc_scroll: usize,
    pub log_scroll: usize,
    pub should_quit: bool,
    pub paused: bool,
    pub refresh_interval: Duration,
    pub last_refresh: Instant,
    pub tick_count: u64,
    pub export_msg: Option<(String, Instant)>,
    pub show_help: bool,
}

impl App {
    pub fn new(interval_ms: u64) -> Self {
        App {
            collector: Collector::new(),
            ai: AiEngine::new(),
            latest: None,
            current_tab: Tab::Overview,
            proc_scroll: 0,
            log_scroll: 0,
            should_quit: false,
            paused: false,
            refresh_interval: Duration::from_millis(interval_ms),
            last_refresh: Instant::now(),
            tick_count: 0,
            export_msg: None,
            show_help: false,
        }
    }

    pub fn tick(&mut self) -> Result<()> {
        if !self.paused {
            let snap = self.collector.tick()?;
            self.ai.ingest(&snap);
            self.latest = Some(snap);
            self.tick_count += 1;
        }
        Ok(())
    }

    #[allow(dead_code)]
    pub fn should_refresh(&self) -> bool {
        self.last_refresh.elapsed() >= self.refresh_interval
    }

    pub fn next_tab(&mut self) {
        let tabs = Tab::all();
        let idx = (self.current_tab.index() + 1) % tabs.len();
        self.current_tab = tabs[idx].clone();
    }

    pub fn prev_tab(&mut self) {
        let tabs = Tab::all();
        let len = tabs.len();
        let idx = (self.current_tab.index() + len - 1) % len;
        self.current_tab = tabs[idx].clone();
    }

    pub fn scroll_down(&mut self) {
        match self.current_tab {
            Tab::Processes => self.proc_scroll = self.proc_scroll.saturating_add(1),
            Tab::Logs | Tab::AI => self.log_scroll = self.log_scroll.saturating_add(1),
            _ => {}
        }
    }

    pub fn scroll_up(&mut self) {
        match self.current_tab {
            Tab::Processes => self.proc_scroll = self.proc_scroll.saturating_sub(1),
            Tab::Logs | Tab::AI => self.log_scroll = self.log_scroll.saturating_sub(1),
            _ => {}
        }
    }

    pub fn export_json(&mut self) -> Result<String> {
        use std::fs;
        let fname = format!(
            "sysanalyzer_{}.json",
            chrono::Utc::now().format("%Y%m%d_%H%M%S")
        );
        let bundle = ExportBundle {
            generated_at: chrono::Utc::now().to_rfc3339(),
            health: &self.ai.health,
            stats: self.ai.stats_report(),
            latest: self.latest.as_ref(),
            findings_count: self.ai.findings.len(),
        };
        let json = serde_json::to_string_pretty(&bundle)?;
        fs::write(&fname, &json)?;
        self.export_msg = Some((format!("Exported → {}", fname), Instant::now()));
        Ok(fname)
    }
}
