use crate::ai::{Finding, Severity};
use crate::app::{App as MonitorApp, Tab};
use crate::collector::{fmt_bytes, fmt_duration, Snapshot};
use eframe::egui::{self, Color32, RichText, Stroke, Vec2};
use std::time::{Duration, Instant};

const BG: Color32 = Color32::from_rgb(11, 15, 24);
const PANEL: Color32 = Color32::from_rgb(19, 26, 39);
const PANEL_ALT: Color32 = Color32::from_rgb(24, 33, 49);
const BORDER: Color32 = Color32::from_rgb(42, 56, 77);
const TEXT: Color32 = Color32::from_rgb(228, 236, 247);
const MUTED: Color32 = Color32::from_rgb(139, 156, 179);
const CYAN: Color32 = Color32::from_rgb(67, 203, 235);
const GREEN: Color32 = Color32::from_rgb(83, 220, 161);
const YELLOW: Color32 = Color32::from_rgb(255, 195, 82);
const RED: Color32 = Color32::from_rgb(255, 100, 119);
const PURPLE: Color32 = Color32::from_rgb(172, 139, 255);

pub struct DesktopApp {
    monitor: MonitorApp,
    last_tick: Instant,
    search: String,
    process_sort: ProcessSort,
    export_status: Option<(String, Instant)>,
    last_error: Option<String>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ProcessSort { Cpu, Memory, Name }

impl DesktopApp {
    pub fn new(monitor: MonitorApp) -> Self {
        Self {
            monitor,
            last_tick: Instant::now(),
            search: String::new(),
            process_sort: ProcessSort::Cpu,
            export_status: None,
            last_error: None,
        }
    }

    fn tick_if_due(&mut self, ctx: &egui::Context) {
        if !self.monitor.paused && self.last_tick.elapsed() >= self.monitor.refresh_interval {
            match self.monitor.tick() {
                Ok(()) => self.last_error = None,
                Err(e) => self.last_error = Some(format!("{e:#}")),
            }
            self.monitor.last_refresh = Instant::now();
            self.last_tick = Instant::now();
        }
        if self.export_status.as_ref().is_some_and(|(_, t)| t.elapsed() > Duration::from_secs(4)) {
            self.export_status = None;
        }
        ctx.request_repaint_after(Duration::from_millis(100));
    }

    fn export(&mut self) {
        match self.monitor.export_json() {
            Ok(path) => self.export_status = Some((format!("Exported report: {path}"), Instant::now())),
            Err(e) => self.export_status = Some((format!("Export failed: {e}"), Instant::now())),
        }
    }

    fn page_title(&self) -> &'static str {
        match &self.monitor.current_tab {
            Tab::Overview => "System overview",
            Tab::CPU => "Processor",
            Tab::Memory => "Memory",
            Tab::Disk => "Storage",
            Tab::Network => "Network",
            Tab::Processes => "Processes",
            Tab::AI => "AI analysis",
            Tab::Logs => "Findings log",
        }
    }

    fn render_sidebar(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("sidebar")
            .exact_width(206.0)
            .frame(egui::Frame::new().fill(PANEL).inner_margin(egui::Margin::same(14)))
            .show(ctx, |ui| {
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new("◈").size(28.0).color(CYAN).strong());
                    ui.vertical(|ui| {
                        ui.label(RichText::new("AI SYSANALYZER").size(14.0).color(TEXT).strong());
                        ui.label(RichText::new("SYSTEM INTELLIGENCE").size(9.0).color(MUTED));
                    });
                });
                ui.add_space(28.0);
                ui.label(RichText::new("MONITOR").size(10.0).color(MUTED).strong());
                ui.add_space(8.0);

                let nav = [
                    (Tab::Overview, "◫", "Overview"),
                    (Tab::CPU, "⌁", "CPU"),
                    (Tab::Memory, "▤", "Memory"),
                    (Tab::Disk, "▰", "Storage"),
                    (Tab::Network, "⇄", "Network"),
                    (Tab::Processes, "▦", "Processes"),
                    (Tab::AI, "✳", "AI analysis"),
                    (Tab::Logs, "≡", "Findings log"),
                ];
                for (tab, glyph, label) in nav {
                    let selected = self.monitor.current_tab == tab;
                    let fill = if selected { Color32::from_rgb(28, 61, 82) } else { PANEL };
                    let text_color = if selected { CYAN } else { MUTED };
                    let button = egui::Button::new(
                        RichText::new(format!("{glyph}   {label}")).size(13.0).color(text_color)
                    ).fill(fill).stroke(if selected { Stroke::new(1.0, CYAN) } else { Stroke::NONE })
                     .corner_radius(7).min_size(Vec2::new(ui.available_width(), 36.0));
                    if ui.add(button).clicked() {
                        self.monitor.current_tab = tab;
                    }
                    ui.add_space(4.0);
                }

                ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
                    ui.separator();
                    ui.label(RichText::new(format!("Samples: {}", self.monitor.tick_count)).color(MUTED).size(11.0));
                    ui.label(RichText::new(format!("Refresh: {} ms", self.monitor.refresh_interval.as_millis())).color(MUTED).size(11.0));
                    ui.add_space(8.0);
                    let status = if self.monitor.paused { ("● PAUSED", YELLOW) } else { ("● MONITORING", GREEN) };
                    ui.label(RichText::new(status.0).color(status.1).size(11.0).strong());
                });
            });
    }

    fn render_topbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label(RichText::new(self.page_title()).size(23.0).color(TEXT).strong());
                ui.label(RichText::new("Live telemetry · local system").size(12.0).color(MUTED));
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button(if self.monitor.paused { "▶ Resume" } else { "Ⅱ Pause" }).clicked() {
                    self.monitor.paused = !self.monitor.paused;
                    self.last_tick = Instant::now();
                }
                if ui.button("⇩ Export JSON").clicked() { self.export(); }
                ui.add_space(6.0);
                let health = self.monitor.ai.health.overall;
                ui.label(RichText::new(format!("HEALTH  {:.0}/100", health)).color(score_color(health)).strong());
            });
        });
        if let Some((msg, _)) = &self.export_status {
            ui.label(RichText::new(msg).color(CYAN).size(11.0));
        }
        if let Some(err) = &self.last_error {
            ui.label(RichText::new(format!("Collector error: {err}")).color(RED).size(11.0));
        }
        ui.add_space(8.0);
        ui.separator();
        ui.add_space(10.0);
    }

    fn render_content(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            match &self.monitor.current_tab {
                Tab::Overview => self.overview(ui),
                Tab::CPU => self.cpu_page(ui),
                Tab::Memory => self.memory_page(ui),
                Tab::Disk => self.disk_page(ui),
                Tab::Network => self.network_page(ui),
                Tab::Processes => self.processes_page(ui),
                Tab::AI => self.ai_page(ui),
                Tab::Logs => self.logs_page(ui),
            }
        });
    }

    fn overview(&mut self, ui: &mut egui::Ui) {
        let Some(s) = self.monitor.latest.clone() else {
            empty_state(ui, "Waiting for the first system sample…");
            return;
        };
        let mem = pct(s.mem_used_kb, s.mem_total_kb);
        let swap = pct(s.swap_used_kb, s.swap_total_kb);
        let disk = s.disk_usage.iter().map(|d| d.usage_pct as f64).fold(0.0, f64::max);
        let temp = s.temps.iter().map(|t| t.celsius as f64).fold(0.0, f64::max);

        egui::Grid::new("metric_cards").num_columns(4).spacing([10.0, 10.0]).show(ui, |ui| {
            metric_card(ui, "CPU UTILIZATION", format!("{:.1}%", s.cpu_overall), s.cpu_overall as f64, CYAN);
            metric_card(ui, "MEMORY USED", format!("{mem:.1}%"), mem, GREEN);
            metric_card(ui, "STORAGE PEAK", format!("{disk:.1}%"), disk, PURPLE);
            metric_card(ui, "PROCESS COUNT", s.proc_count.to_string(), (s.proc_count as f64).min(100.0), YELLOW);
            ui.end_row();
        });
        ui.add_space(12.0);

        ui.columns(2, |cols| {
            panel(&mut cols[0], "PERFORMANCE HISTORY", |ui| {
                draw_chart(ui, "CPU %", &self.monitor.collector.cpu_history(), 100.0, CYAN);
                ui.add_space(10.0);
                draw_chart(ui, "Memory %", &self.monitor.collector.mem_history(), 100.0, GREEN);
            });
            panel(&mut cols[1], "SYSTEM SUMMARY", |ui| {
                info_row(ui, "Health grade", self.monitor.ai.health.grade.to_string(), score_color(self.monitor.ai.health.overall));
                info_row(ui, "Uptime", fmt_duration(s.uptime_secs), TEXT);
                info_row(ui, "CPU frequency", format!("{} MHz", s.cpu_freq_mhz), TEXT);
                info_row(ui, "Logical cores", s.cpu_per_core.len().to_string(), TEXT);
                info_row(ui, "Load average", format!("{:.2} / {:.2} / {:.2}", s.load_avg.0, s.load_avg.1, s.load_avg.2), TEXT);
                info_row(ui, "RAM", format!("{} / {}", fmt_bytes(s.mem_used_kb * 1024), fmt_bytes(s.mem_total_kb * 1024)), TEXT);
                info_row(ui, "Network RX", format!("{}/s", fmt_bytes(s.net_interfaces.iter().map(|n| n.rx_rate as u64).sum())), CYAN);
                info_row(ui, "Network TX", format!("{}/s", fmt_bytes(s.net_interfaces.iter().map(|n| n.tx_rate as u64).sum())), PURPLE);
                ui.add_space(12.0);
                ui.label(RichText::new(&self.monitor.ai.health.summary).color(MUTED));
            });
        });

        ui.add_space(12.0);
        panel(ui, "RECENT FINDINGS", |ui| {
            let recent = self.monitor.ai.recent_findings(4);
            if recent.is_empty() {
                ui.label(RichText::new("No findings recorded yet.").color(GREEN));
            } else {
                for finding in recent { finding_row(ui, finding); }
            }
        });
        let _ = swap;
    }

    fn cpu_page(&mut self, ui: &mut egui::Ui) {
        let Some(s) = self.monitor.latest.clone() else { empty_state(ui, "Waiting for CPU telemetry…"); return; };
        ui.columns(2, |cols| {
            panel(&mut cols[0], "CPU SUMMARY", |ui| {
                big_value(ui, format!("{:.1}%", s.cpu_overall), score_color(100.0 - s.cpu_overall as f64));
                info_row(ui, "Frequency", format!("{} MHz", s.cpu_freq_mhz), TEXT);
                info_row(ui, "Logical processors", s.cpu_per_core.len().to_string(), TEXT);
                info_row(ui, "Load (1 / 5 / 15m)", format!("{:.2} / {:.2} / {:.2}", s.load_avg.0, s.load_avg.1, s.load_avg.2), TEXT);
            });
            panel(&mut cols[1], "CPU HISTORY", |ui| draw_chart(ui, "Utilization (%)", &self.monitor.collector.cpu_history(), 100.0, CYAN));
        });
        ui.add_space(12.0);
        panel(ui, "PER-CORE UTILIZATION", |ui| {
            for (i, value) in s.cpu_per_core.iter().enumerate() {
                ui.horizontal(|ui| {
                    ui.set_width(62.0);
                    ui.label(RichText::new(format!("Core {i:02}")).color(MUTED).size(11.0));
                    let width = (ui.available_width() - 56.0).max(40.0);
                    ui.add(egui::ProgressBar::new((*value / 100.0).clamp(0.0, 1.0))
                        .desired_width(width).text(format!("{value:.1}%")));
                });
                ui.add_space(3.0);
            }
        });
    }

    fn memory_page(&mut self, ui: &mut egui::Ui) {
        let Some(s) = self.monitor.latest.clone() else { empty_state(ui, "Waiting for memory telemetry…"); return; };
        let mem = pct(s.mem_used_kb, s.mem_total_kb);
        let swap = pct(s.swap_used_kb, s.swap_total_kb);
        ui.columns(2, |cols| {
            panel(&mut cols[0], "PHYSICAL MEMORY", |ui| {
                big_value(ui, format!("{mem:.1}% used"), score_color(100.0 - mem));
                ui.add(egui::ProgressBar::new((mem / 100.0) as f32).desired_width(ui.available_width()).text(format!("{} / {}", fmt_bytes(s.mem_used_kb * 1024), fmt_bytes(s.mem_total_kb * 1024))));
                info_row(ui, "Free", fmt_bytes(s.mem_free_kb * 1024), GREEN);
                info_row(ui, "Available", fmt_bytes(s.mem_available_kb * 1024), CYAN);
            });
            panel(&mut cols[1], "SWAP", |ui| {
                big_value(ui, format!("{swap:.1}% used"), score_color(100.0 - swap));
                ui.add(egui::ProgressBar::new((swap / 100.0) as f32).desired_width(ui.available_width()).text(format!("{} / {}", fmt_bytes(s.swap_used_kb * 1024), fmt_bytes(s.swap_total_kb * 1024))));
            });
        });
        ui.add_space(12.0);
        panel(ui, "MEMORY TREND & FORECAST", |ui| {
            draw_chart(ui, "Memory used (%)", &self.monitor.collector.mem_history(), 100.0, GREEN);
            ui.label(RichText::new(format!("Linear trend: {:+.3}% per sample · estimated value in 60 samples: {:.1}%", self.monitor.ai.mem_stat.trend(), self.monitor.ai.mem_stat.predict(60))).color(MUTED));
        });
    }

    fn disk_page(&mut self, ui: &mut egui::Ui) {
        let Some(s) = self.monitor.latest.clone() else { empty_state(ui, "Waiting for storage telemetry…"); return; };
        ui.columns(2, |cols| {
            panel(&mut cols[0], "DISK I/O COUNTERS", |ui| {
                big_value(ui, format!("Read {}", fmt_bytes(s.disk_read_bytes)), CYAN);
                big_value(ui, format!("Write {}", fmt_bytes(s.disk_write_bytes)), PURPLE);
                ui.label(RichText::new("Counters are platform-dependent; zero values may mean the current collector does not expose disk I/O on this OS.").color(MUTED).size(10.0));
            });
            panel(&mut cols[1], "STORAGE UTILIZATION", |ui| {
                if s.disk_usage.is_empty() { ui.label(RichText::new("No mounted volumes reported.").color(MUTED)); }
                for d in &s.disk_usage {
                    ui.label(RichText::new(format!("{}  ·  {}", d.name, d.mount)).color(TEXT).strong());
                    ui.add(egui::ProgressBar::new((d.usage_pct / 100.0).clamp(0.0, 1.0)).desired_width(ui.available_width()).text(format!("{:.1}% · {} free", d.usage_pct, fmt_bytes(d.available_kb * 1024))));
                    ui.add_space(8.0);
                }
            });
        });
    }

    fn network_page(&mut self, ui: &mut egui::Ui) {
        let Some(s) = self.monitor.latest.clone() else { empty_state(ui, "Waiting for network telemetry…"); return; };
        ui.columns(2, |cols| {
            panel(&mut cols[0], "RECEIVE RATE HISTORY", |ui| draw_chart(ui, "RX (KB/s)", &self.monitor.collector.net_rx_history(), 0.0, CYAN));
            panel(&mut cols[1], "TRANSMIT RATE HISTORY", |ui| draw_chart(ui, "TX (KB/s)", &self.monitor.collector.net_tx_history(), 0.0, PURPLE));
        });
        ui.add_space(12.0);
        panel(ui, "NETWORK INTERFACES", |ui| {
            egui::Grid::new("net_table").striped(true).num_columns(4).show(ui, |ui| {
                for h in ["Interface", "Received", "Sent", "Current rate (RX / TX)"] { ui.label(RichText::new(h).color(CYAN).strong()); }
                ui.end_row();
                for n in &s.net_interfaces {
                    ui.label(RichText::new(&n.name).color(TEXT));
                    ui.label(fmt_bytes(n.rx_bytes));
                    ui.label(fmt_bytes(n.tx_bytes));
                    ui.label(format!("{}/s · {}/s", fmt_bytes(n.rx_rate as u64), fmt_bytes(n.tx_rate as u64)));
                    ui.end_row();
                }
            });
        });
    }

    fn processes_page(&mut self, ui: &mut egui::Ui) {
        let Some(s) = self.monitor.latest.clone() else { empty_state(ui, "Waiting for process telemetry…"); return; };
        ui.horizontal(|ui| {
            ui.label(RichText::new(format!("{} processes", s.proc_count)).color(MUTED));
            ui.add_space(12.0);
            ui.label("Filter");
            ui.add(egui::TextEdit::singleline(&mut self.search).hint_text("Process name or PID").desired_width(220.0));
            ui.add_space(12.0);
            ui.label("Sort");
            egui::ComboBox::from_id_salt("process_sort").selected_text(match self.process_sort { ProcessSort::Cpu => "CPU", ProcessSort::Memory => "Memory", ProcessSort::Name => "Name" }).show_ui(ui, |ui| {
                ui.selectable_value(&mut self.process_sort, ProcessSort::Cpu, "CPU");
                ui.selectable_value(&mut self.process_sort, ProcessSort::Memory, "Memory");
                ui.selectable_value(&mut self.process_sort, ProcessSort::Name, "Name");
            });
        });
        ui.add_space(8.0);
        let mut procs = s.top_procs.clone();
        match self.process_sort {
            ProcessSort::Cpu => procs.sort_by(|a,b| b.cpu_pct.total_cmp(&a.cpu_pct)),
            ProcessSort::Memory => procs.sort_by(|a,b| b.mem_kb.cmp(&a.mem_kb)),
            ProcessSort::Name => procs.sort_by(|a,b| a.name.to_lowercase().cmp(&b.name.to_lowercase())),
        }
        let needle = self.search.trim().to_lowercase();
        panel(ui, "PROCESS TABLE", |ui| {
            egui::Grid::new("process_table").striped(true).num_columns(5).spacing([12.0, 6.0]).show(ui, |ui| {
                for h in ["PID", "Process", "CPU %", "Memory", "Status"] { ui.label(RichText::new(h).color(CYAN).strong()); }
                ui.end_row();
                for p in procs.iter().filter(|p| needle.is_empty() || p.name.to_lowercase().contains(&needle) || p.pid.to_string().contains(&needle)) {
                    ui.label(RichText::new(p.pid.to_string()).color(MUTED).monospace());
                    ui.label(RichText::new(&p.name).color(TEXT));
                    ui.label(RichText::new(format!("{:.1}%", p.cpu_pct)).color(score_color(100.0 - p.cpu_pct as f64)));
                    ui.label(fmt_bytes(p.mem_kb * 1024));
                    ui.label(RichText::new(&p.status).color(MUTED));
                    ui.end_row();
                }
            });
        });
    }

    fn ai_page(&mut self, ui: &mut egui::Ui) {
        let h = self.monitor.ai.health.clone();
        ui.columns(2, |cols| {
            panel(&mut cols[0], "SYSTEM HEALTH SCORE", |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(format!("{:.0}", h.overall)).size(54.0).color(score_color(h.overall)).strong());
                    ui.vertical(|ui| {
                        ui.label(RichText::new(format!("GRADE {}", h.grade)).size(22.0).color(score_color(h.overall)).strong());
                        ui.label(RichText::new("/ 100").color(MUTED));
                    });
                });
                ui.label(RichText::new(h.summary).color(MUTED));
            });
            panel(&mut cols[1], "METRIC SCORES", |ui| {
                for (name, value) in [("CPU",h.cpu_score),("Memory",h.mem_score),("Disk",h.disk_score),("Network",h.network_score),("Temperature",h.temp_score)] {
                    info_row(ui, name, format!("{value:.1}/100"), score_color(value));
                    ui.add(egui::ProgressBar::new((value / 100.0) as f32).desired_width(ui.available_width()));
                }
            });
        });
        ui.add_space(12.0);
        panel(ui, "STATISTICAL PROFILE", |ui| {
            ui.label(RichText::new(self.monitor.ai.stats_report()).monospace().color(TEXT));
        });
        ui.add_space(12.0);
        panel(ui, "ANOMALY FINDINGS", |ui| {
            let recent = self.monitor.ai.recent_findings(30);
            if recent.is_empty() { ui.label(RichText::new("No anomaly findings so far.").color(GREEN)); }
            for f in recent { finding_row(ui, f); }
        });
    }

    fn logs_page(&mut self, ui: &mut egui::Ui) {
        panel(ui, "LATEST FINDINGS", |ui| {
            if self.monitor.ai.findings.is_empty() {
                ui.label(RichText::new("No findings recorded.").color(GREEN));
            } else {
                for finding in self.monitor.ai.findings.iter().rev() { finding_row(ui, finding); }
            }
        });
    }
}

impl eframe::App for DesktopApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.tick_if_due(ctx);
        ctx.set_visuals(egui::Visuals::dark());
        self.render_sidebar(ctx);

        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(BG).inner_margin(egui::Margin { left: 20, right: 20, top: 16, bottom: 14 }))
            .show(ctx, |ui| {
                self.render_topbar(ui);
                self.render_content(ui);
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new(if self.monitor.paused { "● Collection paused" } else { "● Live collection active" }).color(if self.monitor.paused { YELLOW } else { GREEN }).size(10.0));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(RichText::new(format!("Refresh {} ms · sample {}", self.monitor.refresh_interval.as_millis(), self.monitor.tick_count)).color(MUTED).size(10.0));
                    });
                });
            });

        if ctx.input(|i| i.key_pressed(egui::Key::Space) && !i.modifiers.ctrl && !i.modifiers.command) {
            self.monitor.paused = !self.monitor.paused;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::E) && (i.modifiers.ctrl || i.modifiers.command)) {
            self.export();
        }
    }
}

fn panel<R>(ui: &mut egui::Ui, title: &str, add_contents: impl FnOnce(&mut egui::Ui) -> R) -> R {
    egui::Frame::new().fill(PANEL).stroke(Stroke::new(1.0, BORDER))
        .corner_radius(10).inner_margin(egui::Margin::same(14))
        .show(ui, |ui| {
            ui.label(RichText::new(title).size(11.0).color(MUTED).strong());
            ui.add_space(10.0);
            add_contents(ui)
        }).inner
}

fn metric_card(ui: &mut egui::Ui, title: &str, value: String, pct: f64, color: Color32) {
    egui::Frame::new().fill(PANEL).stroke(Stroke::new(1.0, BORDER))
        .corner_radius(9).inner_margin(egui::Margin::same(12))
        .show(ui, |ui| {
            ui.set_min_width((ui.available_width() / 4.0).max(110.0));
            ui.label(RichText::new(title).size(10.0).color(MUTED).strong());
            ui.add_space(5.0);
            ui.label(RichText::new(value).size(25.0).color(color).strong());
            ui.add(egui::ProgressBar::new((pct / 100.0).clamp(0.0, 1.0) as f32).desired_width(ui.available_width()));
        });
}

fn info_row(ui: &mut egui::Ui, label: &str, value: String, color: Color32) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(label).color(MUTED).size(12.0));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(RichText::new(value).color(color).size(12.0).strong());
        });
    });
}

fn big_value(ui: &mut egui::Ui, value: String, color: Color32) {
    ui.label(RichText::new(value).size(24.0).color(color).strong());
}

fn draw_chart(ui: &mut egui::Ui, title: &str, data: &[f64], max_y: f64, color: Color32) {
    ui.label(RichText::new(title).size(11.0).color(MUTED));
    let desired = Vec2::new(ui.available_width().max(120.0), 118.0);
    let (rect, _) = ui.allocate_exact_size(desired, egui::Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 5.0, PANEL_ALT);
    for i in 1..4 {
        let y = rect.top() + rect.height() * i as f32 / 4.0;
        painter.line_segment([egui::pos2(rect.left()+4.0,y), egui::pos2(rect.right()-4.0,y)], Stroke::new(1.0, BORDER));
    }
    if data.len() < 2 { return; }
    let actual_max = if max_y > 0.0 { max_y } else {
        data.iter().copied().fold(1.0_f64, f64::max).max(1.0) * 1.15
    };
    let n = data.len();
    let points: Vec<egui::Pos2> = data.iter().enumerate().map(|(i, v)| {
        let x = rect.left() + 5.0 + (rect.width()-10.0) * i as f32 / (n-1) as f32;
        let norm = (*v / actual_max).clamp(0.0, 1.0) as f32;
        let y = rect.bottom() - 5.0 - (rect.height()-10.0) * norm;
        egui::pos2(x,y)
    }).collect();
    painter.add(egui::Shape::line(points, Stroke::new(2.0, color)));
}

fn finding_row(ui: &mut egui::Ui, f: &Finding) {
    let (label, color) = match &f.severity {
        Severity::Ok => ("OK", GREEN),
        Severity::Info => ("INFO", CYAN),
        Severity::Warning => ("WARN", YELLOW),
        Severity::Critical => ("CRITICAL", RED),
    };
    ui.horizontal(|ui| {
        ui.label(RichText::new(format!("{label:8}")).color(color).strong().monospace());
        ui.label(RichText::new(&f.category).color(TEXT).strong());
        ui.label(RichText::new(&f.message).color(MUTED));
    });
    ui.add_space(4.0);
}

fn empty_state(ui: &mut egui::Ui, message: &str) {
    ui.add_space(40.0);
    ui.vertical_centered(|ui| {
        ui.label(RichText::new(message).size(16.0).color(MUTED));
    });
}

fn pct(used: u64, total: u64) -> f64 {
    if total == 0 { 0.0 } else { used as f64 / total as f64 * 100.0 }
}

fn score_color(value: f64) -> Color32 {
    if value >= 80.0 { GREEN } else if value >= 55.0 { YELLOW } else { RED }
}
