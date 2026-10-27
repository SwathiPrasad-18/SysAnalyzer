// ============================================================
// ui.rs  –  Full TUI Renderer (ratatui)
// ============================================================
use crate::app::{App, Tab};
use crate::collector::{fmt_bytes, fmt_duration};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    symbols,
    text::{Line, Span},
    widgets::{
        Axis, BarChart, Block, Borders, Chart, Clear, Dataset, Gauge, GraphType,
        List, ListItem, Paragraph, Row, Sparkline, Table, Tabs, Wrap,
    },
    Frame,
};

const BRAND: Color = Color::Rgb(80, 200, 255);
const DIM: Color = Color::Rgb(100, 100, 100);

// ── entry point ──────────────────────────────────────────────────────────────
pub fn render(f: &mut Frame, app: &App) {
    let size = f.area();

    // ── root layout ──
    let root = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // title bar
            Constraint::Length(3), // tabs
            Constraint::Min(0),    // content
            Constraint::Length(1), // status bar
        ])
        .split(size);

    draw_title(f, app, root[0]);
    draw_tabs(f, app, root[1]);
    draw_content(f, app, root[2]);
    draw_statusbar(f, app, root[3]);

    if app.show_help {
        draw_help(f, size);
    }

    if let Some((msg, _)) = &app.export_msg {
        draw_toast(f, size, msg);
    }
}

// ── title ─────────────────────────────────────────────────────────────────────
fn draw_title(f: &mut Frame, app: &App, area: Rect) {
    let health_color = app.ai.health.color();
    let title = Line::from(vec![
        Span::styled(
            "  ⬡ AI SYSTEM ANALYZER  ",
            Style::default().fg(BRAND).add_modifier(Modifier::BOLD),
        ),
        Span::styled("│ ", Style::default().fg(DIM)),
        Span::styled(
            format!(" {} ", app.ai.health.grade),
            Style::default()
                .fg(Color::Black)
                .bg(health_color)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("  Health {:.0}%  ", app.ai.health.overall),
            Style::default().fg(health_color).add_modifier(Modifier::BOLD),
        ),
        Span::styled("│ ", Style::default().fg(DIM)),
        Span::styled(
            app.ai.health.summary.as_str(),
            Style::default().fg(Color::White),
        ),
    ]);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(BRAND))
        .title_alignment(Alignment::Left);
    let p = Paragraph::new(title).block(block);
    f.render_widget(p, area);
}

// ── tabs ──────────────────────────────────────────────────────────────────────
fn draw_tabs(f: &mut Frame, app: &App, area: Rect) {
    let titles: Vec<Line> = Tab::all()
        .iter()
        .map(|t| Line::from(format!(" {} ", t.title())))
        .collect();
    let tabs = Tabs::new(titles)
        .select(app.current_tab.index())
        .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(DIM)))
        .style(Style::default().fg(DIM))
        .highlight_style(
            Style::default()
                .fg(BRAND)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
        )
        .divider(symbols::line::VERTICAL);
    f.render_widget(tabs, area);
}

// ── content router ─────────────────────────────────────────────────────────
fn draw_content(f: &mut Frame, app: &App, area: Rect) {
    match &app.current_tab {
        Tab::Overview  => draw_overview(f, app, area),
        Tab::CPU       => draw_cpu(f, app, area),
        Tab::Memory    => draw_memory(f, app, area),
        Tab::Disk      => draw_disk(f, app, area),
        Tab::Network   => draw_network(f, app, area),
        Tab::Processes => draw_processes(f, app, area),
        Tab::AI        => draw_ai(f, app, area),
        Tab::Logs      => draw_logs(f, app, area),
    }
}

// ── status bar ───────────────────────────────────────────────────────────────
fn draw_statusbar(f: &mut Frame, app: &App, area: Rect) {
    let interval_ms = app.refresh_interval.as_millis();
    let paused = if app.paused { " [PAUSED]" } else { "" };
    let status = Line::from(vec![
        Span::styled(" q:Quit ", Style::default().fg(Color::Gray)),
        Span::raw("| "),
        Span::styled("Tab:Next ", Style::default().fg(Color::Gray)),
        Span::raw("| "),
        Span::styled("↑↓:Scroll ", Style::default().fg(Color::Gray)),
        Span::raw("| "),
        Span::styled("Space:Pause ", Style::default().fg(Color::Gray)),
        Span::raw("| "),
        Span::styled("e:Export ", Style::default().fg(Color::Gray)),
        Span::raw("| "),
        Span::styled("?:Help ", Style::default().fg(Color::Gray)),
        Span::raw("| "),
        Span::styled(
            format!(" Ticks: {}  Interval: {}ms{} ", app.tick_count, interval_ms, paused),
            Style::default().fg(if app.paused { Color::Yellow } else { Color::DarkGray }),
        ),
    ]);
    let p = Paragraph::new(status)
        .style(Style::default().bg(Color::Rgb(20, 20, 30)))
        .alignment(Alignment::Left);
    f.render_widget(p, area);
}

// ═══════════════════════════════════════════════════════════════════════════════
//  OVERVIEW TAB
// ═══════════════════════════════════════════════════════════════════════════════
fn draw_overview(f: &mut Frame, app: &App, area: Rect) {
    let snap = match &app.latest {
        Some(s) => s,
        None => return,
    };

    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(8), Constraint::Min(0)])
        .split(area);

    // ── top row: 5 gauges ──
    let gauge_row = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Ratio(1, 5),
            Constraint::Ratio(1, 5),
            Constraint::Ratio(1, 5),
            Constraint::Ratio(1, 5),
            Constraint::Ratio(1, 5),
        ])
        .split(outer[0]);

    // CPU gauge
    let cpu = snap.cpu_overall as f64;
    render_gauge(f, gauge_row[0], "CPU", cpu, 80.0, 95.0, "%");

    // Memory gauge
    let mem_pct = if snap.mem_total_kb > 0 {
        snap.mem_used_kb as f64 / snap.mem_total_kb as f64 * 100.0
    } else { 0.0 };
    render_gauge(f, gauge_row[1], "Memory", mem_pct, 80.0, 92.0, "%");

    // Swap gauge
    let swap_pct = if snap.swap_total_kb > 0 {
        snap.swap_used_kb as f64 / snap.swap_total_kb as f64 * 100.0
    } else { 0.0 };
    render_gauge(f, gauge_row[2], "Swap", swap_pct, 30.0, 60.0, "%");

    // Worst disk gauge
    let worst_disk = snap.disk_usage.iter().map(|d| d.usage_pct as f64).fold(0.0f64, f64::max);
    render_gauge(f, gauge_row[3], "Disk", worst_disk, 80.0, 95.0, "%");

    // Top temp
    let max_temp = snap.temps.iter().map(|t| t.celsius as f64).fold(0.0f64, f64::max);
    render_gauge(f, gauge_row[4], "Temp", max_temp, 70.0, 85.0, "°C");

    // ── bottom row: sparklines + info ──
    let bottom = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Ratio(2, 3), Constraint::Ratio(1, 3)])
        .split(outer[1]);

    // sparklines column
    let sparks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Ratio(1, 4),
            Constraint::Ratio(1, 4),
            Constraint::Ratio(1, 4),
            Constraint::Ratio(1, 4),
        ])
        .split(bottom[0]);

    render_sparkline(f, sparks[0], "CPU History", &app.collector.cpu_history(), Color::LightCyan);
    render_sparkline(f, sparks[1], "Memory History", &app.collector.mem_history(), Color::LightGreen);
    render_sparkline(f, sparks[2], "NET ↓ KB/s", &app.collector.net_rx_history(), Color::LightBlue);
    render_sparkline(f, sparks[3], "NET ↑ KB/s", &app.collector.net_tx_history(), Color::LightMagenta);

    // info panel
    let uptime = fmt_duration(snap.uptime_secs);
    let info_lines = vec![
        Line::from(vec![
            Span::styled("Uptime:  ", Style::default().fg(Color::Gray)),
            Span::styled(uptime, Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("Cores:   ", Style::default().fg(Color::Gray)),
            Span::styled(snap.cpu_per_core.len().to_string(), Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("Freq:    ", Style::default().fg(Color::Gray)),
            Span::styled(format!("{} MHz", snap.cpu_freq_mhz), Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("Load:    ", Style::default().fg(Color::Gray)),
            Span::styled(
                format!("{:.2}/{:.2}/{:.2}", snap.load_avg.0, snap.load_avg.1, snap.load_avg.2),
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(vec![
            Span::styled("Procs:   ", Style::default().fg(Color::Gray)),
            Span::styled(snap.proc_count.to_string(), Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("Mem:     ", Style::default().fg(Color::Gray)),
            Span::styled(
                format!("{} / {}", fmt_bytes(snap.mem_used_kb * 1024), fmt_bytes(snap.mem_total_kb * 1024)),
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("NET ↓:   ", Style::default().fg(Color::Gray)),
            Span::styled(fmt_bytes(snap.net_rx_bytes) + "/s", Style::default().fg(Color::LightBlue)),
        ]),
        Line::from(vec![
            Span::styled("NET ↑:   ", Style::default().fg(Color::Gray)),
            Span::styled(fmt_bytes(snap.net_tx_bytes) + "/s", Style::default().fg(Color::LightMagenta)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Disk R:  ", Style::default().fg(Color::Gray)),
            Span::styled(fmt_bytes(snap.disk_read_bytes) + "/s", Style::default().fg(Color::Yellow)),
        ]),
        Line::from(vec![
            Span::styled("Disk W:  ", Style::default().fg(Color::Gray)),
            Span::styled(fmt_bytes(snap.disk_write_bytes) + "/s", Style::default().fg(Color::LightYellow)),
        ]),
    ];
    let info = Paragraph::new(info_lines)
        .block(styled_block("System Info"))
        .wrap(Wrap { trim: false });
    f.render_widget(info, bottom[1]);
}

// ═══════════════════════════════════════════════════════════════════════════════
//  CPU TAB
// ═══════════════════════════════════════════════════════════════════════════════
fn draw_cpu(f: &mut Frame, app: &App, area: Rect) {
    let snap = match &app.latest { Some(s) => s, None => return };

    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)])
        .split(area);

    // Left: per-core bar chart – build data
    let labels_owned: Vec<String> = snap.cpu_per_core.iter().enumerate().map(|(i, _)| format!("C{}", i)).collect();
    let bars: Vec<(&str, u64)> = labels_owned
        .iter()
        .zip(snap.cpu_per_core.iter())
        .map(|(l, v)| (l.as_str(), *v as u64))
        .collect();

    let bc = BarChart::default()
        .block(styled_block("Per-Core CPU %"))
        .data(&bars)
        .bar_width(3)
        .bar_gap(1)
        .bar_style(Style::default().fg(Color::LightCyan))
        .value_style(Style::default().fg(Color::Black).bg(Color::LightCyan))
        .max(100);
    f.render_widget(bc, cols[0]);

    // Right: time-series chart
    let cpu_hist = app.collector.cpu_history();
    let ema_hist: Vec<(f64, f64)> = {
        let mut ema = 0.0f64;
        cpu_hist
            .iter()
            .enumerate()
            .map(|(i, &v)| {
                ema = 0.1 * v + 0.9 * ema;
                (i as f64, ema)
            })
            .collect()
    };
    let raw_pts: Vec<(f64, f64)> = cpu_hist.iter().enumerate().map(|(i, &v)| (i as f64, v)).collect();

    let datasets = vec![
        Dataset::default()
            .name("Raw CPU%")
            .marker(symbols::Marker::Braille)
            .graph_type(GraphType::Line)
            .style(Style::default().fg(Color::LightCyan))
            .data(&raw_pts),
        Dataset::default()
            .name("EMA (α=0.1)")
            .marker(symbols::Marker::Braille)
            .graph_type(GraphType::Line)
            .style(Style::default().fg(Color::Yellow))
            .data(&ema_hist),
    ];
    let xlen = cpu_hist.len().max(1) as f64;
    let chart = Chart::new(datasets)
        .block(styled_block("CPU History (%)"))
        .x_axis(
            Axis::default()
                .title("Ticks")
                .style(Style::default().fg(DIM))
                .bounds([0.0, xlen]),
        )
        .y_axis(
            Axis::default()
                .title("CPU %")
                .style(Style::default().fg(DIM))
                .bounds([0.0, 100.0])
                .labels(["0", "50", "100"].iter().map(|s| Span::raw(*s))),
        );
    f.render_widget(chart, cols[1]);
}

// ═══════════════════════════════════════════════════════════════════════════════
//  MEMORY TAB
// ═══════════════════════════════════════════════════════════════════════════════
fn draw_memory(f: &mut Frame, app: &App, area: Rect) {
    let snap = match &app.latest { Some(s) => s, None => return };

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(10), Constraint::Min(0)])
        .split(area);

    // gauges row
    let gauge_cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)])
        .split(rows[0]);

    let mem_pct = if snap.mem_total_kb > 0 {
        snap.mem_used_kb as f64 / snap.mem_total_kb as f64 * 100.0
    } else { 0.0 };
    render_gauge(f, gauge_cols[0], "RAM Usage", mem_pct, 70.0, 90.0, "%");

    let swap_pct = if snap.swap_total_kb > 0 {
        snap.swap_used_kb as f64 / snap.swap_total_kb as f64 * 100.0
    } else { 0.0 };
    render_gauge(f, gauge_cols[1], "Swap Usage", swap_pct, 40.0, 70.0, "%");

    // chart + stats
    let bottom = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Ratio(2, 3), Constraint::Ratio(1, 3)])
        .split(rows[1]);

    let mem_hist = app.collector.mem_history();
    let pts: Vec<(f64, f64)> = mem_hist.iter().enumerate().map(|(i, &v)| (i as f64, v)).collect();
    let xlen = mem_hist.len().max(1) as f64;
    let predicted = app.ai.mem_stat.predict(60);

    // draw predicted line
    let pred_pts: Vec<(f64, f64)> = vec![(xlen, mem_hist.last().cloned().unwrap_or(0.0)), (xlen + 60.0, predicted)];
    let datasets = vec![
        Dataset::default()
            .name("Mem Used %")
            .marker(symbols::Marker::Braille)
            .graph_type(GraphType::Line)
            .style(Style::default().fg(Color::LightGreen))
            .data(&pts),
        Dataset::default()
            .name("AI Forecast")
            .marker(symbols::Marker::Braille)
            .graph_type(GraphType::Line)
            .style(Style::default().fg(Color::LightRed))
            .data(&pred_pts),
    ];
    let chart = Chart::new(datasets)
        .block(styled_block("Memory History + AI Forecast (%)"))
        .x_axis(Axis::default().style(Style::default().fg(DIM)).bounds([0.0, xlen + 65.0]))
        .y_axis(
            Axis::default()
                .title("%")
                .style(Style::default().fg(DIM))
                .bounds([0.0, 100.0])
                .labels(["0", "50", "100"].iter().map(|s| Span::raw(*s))),
        );
    f.render_widget(chart, bottom[0]);

    // stats panel
    let trend = app.ai.mem_stat.trend();
    let lines = vec![
        Line::from(vec![
            Span::styled("Total:     ", Style::default().fg(Color::Gray)),
            Span::styled(fmt_bytes(snap.mem_total_kb * 1024), Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("Used:      ", Style::default().fg(Color::Gray)),
            Span::styled(fmt_bytes(snap.mem_used_kb * 1024), Style::default().fg(Color::LightGreen)),
        ]),
        Line::from(vec![
            Span::styled("Free:      ", Style::default().fg(Color::Gray)),
            Span::styled(fmt_bytes(snap.mem_free_kb * 1024), Style::default().fg(Color::Green)),
        ]),
        Line::from(vec![
            Span::styled("Available: ", Style::default().fg(Color::Gray)),
            Span::styled(fmt_bytes(snap.mem_available_kb * 1024), Style::default().fg(Color::Cyan)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Swap Total:", Style::default().fg(Color::Gray)),
            Span::styled(fmt_bytes(snap.swap_total_kb * 1024), Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("Swap Used: ", Style::default().fg(Color::Gray)),
            Span::styled(fmt_bytes(snap.swap_used_kb * 1024), Style::default().fg(Color::Yellow)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("AI Trend:  ", Style::default().fg(Color::Gray)),
            Span::styled(
                format!("{:+.3}%/s", trend),
                Style::default().fg(if trend > 0.5 { Color::Red } else { Color::Green }),
            ),
        ]),
        Line::from(vec![
            Span::styled("Predicted: ", Style::default().fg(Color::Gray)),
            Span::styled(
                format!("{:.1}% in 60s", predicted),
                Style::default().fg(Color::LightRed),
            ),
        ]),
        Line::from(vec![
            Span::styled("Mean:      ", Style::default().fg(Color::Gray)),
            Span::styled(format!("{:.1}%", app.ai.mem_stat.mean()), Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("Std Dev:   ", Style::default().fg(Color::Gray)),
            Span::styled(format!("{:.1}%", app.ai.mem_stat.stddev()), Style::default().fg(Color::White)),
        ]),
    ];
    let p = Paragraph::new(lines).block(styled_block("Memory Stats"));
    f.render_widget(p, bottom[1]);
}

// ═══════════════════════════════════════════════════════════════════════════════
//  DISK TAB
// ═══════════════════════════════════════════════════════════════════════════════
fn draw_disk(f: &mut Frame, app: &App, area: Rect) {
    let snap = match &app.latest { Some(s) => s, None => return };

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(7)])
        .split(area);

    // disk table
    let header = Row::new(vec!["Filesystem", "Mount", "Total", "Used", "Available", "Usage"])
        .style(Style::default().fg(BRAND).add_modifier(Modifier::BOLD))
        .height(1);

    let disk_rows: Vec<Row> = snap.disk_usage.iter().map(|d| {
        let usage_style = match d.usage_pct as u32 {
            0..=74  => Style::default().fg(Color::Green),
            75..=89 => Style::default().fg(Color::Yellow),
            _       => Style::default().fg(Color::Red),
        };
        let bar = make_bar(d.usage_pct as f64, 20);
        Row::new(vec![
            d.fs.clone(),
            d.mount.clone(),
            fmt_bytes(d.total_kb * 1024),
            fmt_bytes(d.used_kb * 1024),
            fmt_bytes(d.available_kb * 1024),
            format!("{} {:.1}%", bar, d.usage_pct),
        ])
        .style(usage_style)
        .height(1)
    }).collect();

    let table = Table::new(
        disk_rows,
        [
            Constraint::Length(10),
            Constraint::Length(14),
            Constraint::Length(10),
            Constraint::Length(10),
            Constraint::Length(10),
            Constraint::Min(25),
        ],
    )
    .header(header)
    .block(styled_block("Disk Partitions"))
    .highlight_style(Style::default().add_modifier(Modifier::REVERSED));
    f.render_widget(table, rows[0]);

    // I/O stats row
    let io_cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)])
        .split(rows[1]);

    let io_read = vec![
        Line::from(vec![
            Span::styled("Current:  ", Style::default().fg(Color::Gray)),
            Span::styled(fmt_bytes(snap.disk_read_bytes) + "/s", Style::default().fg(Color::Yellow)),
        ]),
        Line::from(vec![
            Span::styled("Mean:     ", Style::default().fg(Color::Gray)),
            Span::styled(fmt_bytes(app.ai.disk_read_stat.mean() as u64 * 1024) + "/s", Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("Peak:     ", Style::default().fg(Color::Gray)),
            Span::styled(fmt_bytes(app.ai.disk_read_stat.max() as u64 * 1024) + "/s", Style::default().fg(Color::LightYellow)),
        ]),
    ];
    let pr = Paragraph::new(io_read).block(styled_block("Disk Read I/O"));
    f.render_widget(pr, io_cols[0]);

    let io_write = vec![
        Line::from(vec![
            Span::styled("Current:  ", Style::default().fg(Color::Gray)),
            Span::styled(fmt_bytes(snap.disk_write_bytes) + "/s", Style::default().fg(Color::LightMagenta)),
        ]),
        Line::from(vec![
            Span::styled("Mean:     ", Style::default().fg(Color::Gray)),
            Span::styled(fmt_bytes(app.ai.disk_write_stat.mean() as u64 * 1024) + "/s", Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("Peak:     ", Style::default().fg(Color::Gray)),
            Span::styled(fmt_bytes(app.ai.disk_write_stat.max() as u64 * 1024) + "/s", Style::default().fg(Color::LightMagenta)),
        ]),
    ];
    let pw = Paragraph::new(io_write).block(styled_block("Disk Write I/O"));
    f.render_widget(pw, io_cols[1]);
}

// ═══════════════════════════════════════════════════════════════════════════════
//  NETWORK TAB
// ═══════════════════════════════════════════════════════════════════════════════
fn draw_network(f: &mut Frame, app: &App, area: Rect) {
    let snap = match &app.latest { Some(s) => s, None => return };

    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)])
        .split(area);

    // left: interfaces table
    let header = Row::new(vec!["Interface", "RX/s", "TX/s"])
        .style(Style::default().fg(BRAND).add_modifier(Modifier::BOLD));
    let iface_rows: Vec<Row> = snap.net_interfaces.iter().map(|n| {
        Row::new(vec![
            n.name.clone(),
            fmt_bytes(n.rx_rate as u64) + "/s",
            fmt_bytes(n.tx_rate as u64) + "/s",
        ])
    }).collect();
    let table = Table::new(iface_rows, [Constraint::Min(12), Constraint::Length(12), Constraint::Length(12)])
        .header(header)
        .block(styled_block("Network Interfaces"));
    f.render_widget(table, cols[0]);

    // right: RX/TX chart
    let rx_hist = app.collector.net_rx_history();
    let tx_hist = app.collector.net_tx_history();
    let rx_pts: Vec<(f64, f64)> = rx_hist.iter().enumerate().map(|(i, &v)| (i as f64, v)).collect();
    let tx_pts: Vec<(f64, f64)> = tx_hist.iter().enumerate().map(|(i, &v)| (i as f64, v)).collect();
    let max_y = rx_hist.iter().chain(tx_hist.iter()).cloned().fold(10.0f64, f64::max);
    let xlen = rx_hist.len().max(1) as f64;
    let net_mid = format!("{:.0}", max_y / 2.0);
    let net_top = format!("{:.0}", max_y);

    let datasets = vec![
        Dataset::default()
            .name("RX KB/s")
            .marker(symbols::Marker::Braille)
            .graph_type(GraphType::Line)
            .style(Style::default().fg(Color::LightBlue))
            .data(&rx_pts),
        Dataset::default()
            .name("TX KB/s")
            .marker(symbols::Marker::Braille)
            .graph_type(GraphType::Line)
            .style(Style::default().fg(Color::LightMagenta))
            .data(&tx_pts),
    ];
    let chart = Chart::new(datasets)
        .block(styled_block("Network Throughput (KB/s)"))
        .x_axis(Axis::default().style(Style::default().fg(DIM)).bounds([0.0, xlen]))
        .y_axis(
            Axis::default()
                .title("KB/s")
                .style(Style::default().fg(DIM))
                .bounds([0.0, max_y * 1.1])
                .labels(["0".to_string(), net_mid, net_top].into_iter().map(Span::raw)),
        );
    f.render_widget(chart, cols[1]);
}

// ═══════════════════════════════════════════════════════════════════════════════
//  PROCESSES TAB
// ═══════════════════════════════════════════════════════════════════════════════
fn draw_processes(f: &mut Frame, app: &App, area: Rect) {
    let snap = match &app.latest { Some(s) => s, None => return };

    let header = Row::new(vec!["PID", "Name", "CPU%", "Memory", "Status"])
        .style(Style::default().fg(BRAND).add_modifier(Modifier::BOLD));

    let proc_rows: Vec<Row> = snap.top_procs.iter().map(|p| {
        let cpu_style = if p.cpu_pct > 50.0 { Style::default().fg(Color::Red) }
            else if p.cpu_pct > 20.0 { Style::default().fg(Color::Yellow) }
            else { Style::default().fg(Color::Green) };
        Row::new(vec![
            p.pid.to_string(),
            p.name.clone(),
            format!("{:.1}", p.cpu_pct),
            fmt_bytes(p.mem_kb * 1024),
            p.status.clone(),
        ]).style(cpu_style)
    }).collect();

    let visible_rows: Vec<Row> = proc_rows.into_iter().skip(app.proc_scroll).take(area.height as usize - 5).collect();

    let proc_title = format!("Top Processes (by CPU) – {} total", snap.proc_count);
    let table = Table::new(
        visible_rows,
        [
            Constraint::Length(8),
            Constraint::Min(20),
            Constraint::Length(8),
            Constraint::Length(12),
            Constraint::Length(12),
        ],
    )
    .header(header)
    .block(styled_block(&proc_title));
    f.render_widget(table, area);
}

// ═══════════════════════════════════════════════════════════════════════════════
//  AI ANALYSIS TAB
// ═══════════════════════════════════════════════════════════════════════════════
fn draw_ai(f: &mut Frame, app: &App, area: Rect) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)])
        .split(area);

    // left: health scorecard
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(10)])
        .split(cols[0]);

    let h = &app.ai.health;
    let score_lines = vec![
        Line::from(vec![
            Span::styled(format!("  Overall:  {:.1}%  ({})", h.overall, h.grade),
                Style::default().fg(h.color()).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(""),
        score_line("CPU",      h.cpu_score),
        score_line("Memory",   h.mem_score),
        score_line("Disk",     h.disk_score),
        score_line("Thermal",  h.temp_score),
        score_line("Load",     h.network_score),
        Line::from(""),
        Line::from(vec![
            Span::styled("  ", Style::default()),
            Span::styled(&h.summary, Style::default().fg(h.color())),
        ]),
    ];
    let sp = Paragraph::new(score_lines).block(styled_block("AI Health Scorecard"));
    f.render_widget(sp, rows[0]);

    // stats panel
    let stats_text = app.ai.stats_report();
    let sp2 = Paragraph::new(stats_text)
        .block(styled_block("Statistical Summary"))
        .wrap(Wrap { trim: false });
    f.render_widget(sp2, rows[1]);

    // right: recent findings
    let findings = app.ai.recent_findings(100);
    let items: Vec<ListItem> = findings.iter().skip(app.log_scroll).map(|f| {
        let color = f.severity.color();
        ListItem::new(Line::from(vec![
            Span::styled(
                format!("[{}] ", f.severity.label()),
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("{} – ", f.category),
                Style::default().fg(Color::Gray),
            ),
            Span::styled(&f.message, Style::default().fg(Color::White)),
        ]))
    }).collect();

    let list = List::new(items).block(styled_block("AI Findings (recent first, ↑↓ scroll)"));
    f.render_widget(list, cols[1]);
}

// ═══════════════════════════════════════════════════════════════════════════════
//  LOGS TAB (full findings history)
// ═══════════════════════════════════════════════════════════════════════════════
fn draw_logs(f: &mut Frame, app: &App, area: Rect) {
    let findings = app.ai.recent_findings(500);
    let items: Vec<ListItem> = findings.iter().skip(app.log_scroll).map(|finding| {
        let color = finding.severity.color();
        ListItem::new(Line::from(vec![
            Span::styled(
                finding.ts.format("%H:%M:%S ").to_string(),
                Style::default().fg(DIM),
            ),
            Span::styled(
                format!("[{:4}] ", finding.severity.label()),
                Style::default().fg(color),
            ),
            Span::styled(
                format!("{:8} ", finding.category),
                Style::default().fg(Color::Gray),
            ),
            Span::styled(&finding.message, Style::default().fg(Color::White)),
            Span::styled(
                format!("  [val={:.1} thr={:.1}]", finding.value, finding.threshold),
                Style::default().fg(DIM),
            ),
        ]))
    }).collect();

    let log_title = format!("AI Event Log – {} findings (↑↓ scroll)", app.ai.findings.len());
    let list = List::new(items)
        .block(styled_block(&log_title));
    f.render_widget(list, area);
}

// ═══════════════════════════════════════════════════════════════════════════════
//  HELP OVERLAY
// ═══════════════════════════════════════════════════════════════════════════════
fn draw_help(f: &mut Frame, area: Rect) {
    let w = 50u16.min(area.width);
    let h = 20u16.min(area.height);
    let x = (area.width.saturating_sub(w)) / 2;
    let y = (area.height.saturating_sub(h)) / 2;
    let popup = Rect { x, y, width: w, height: h };

    let lines = vec![
        Line::from(""),
        Line::from(Span::styled("  Keyboard Shortcuts", Style::default().fg(BRAND).add_modifier(Modifier::BOLD))),
        Line::from(""),
        Line::from("  q / Ctrl+C   Quit"),
        Line::from("  Tab / →      Next tab"),
        Line::from("  Shift+Tab / ← Prev tab"),
        Line::from("  1-8           Jump to tab"),
        Line::from("  ↑ / k         Scroll up"),
        Line::from("  ↓ / j         Scroll down"),
        Line::from("  Space         Pause/resume"),
        Line::from("  e              Export JSON"),
        Line::from("  ?              Toggle help"),
        Line::from(""),
        Line::from(Span::styled("  Tabs", Style::default().fg(Color::Yellow))),
        Line::from("  1: Overview   2: CPU"),
        Line::from("  3: Memory     4: Disk"),
        Line::from("  5: Network    6: Processes"),
        Line::from("  7: AI         8: Logs"),
        Line::from(""),
    ];

    f.render_widget(Clear, popup);
    let p = Paragraph::new(lines)
        .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(BRAND)).title(" Help "))
        .style(Style::default().bg(Color::Rgb(20, 20, 40)));
    f.render_widget(p, popup);
}

// ── toast ─────────────────────────────────────────────────────────────────────
fn draw_toast(f: &mut Frame, area: Rect, msg: &str) {
    let w = (msg.len() as u16 + 4).min(area.width);
    let popup = Rect {
        x: area.width.saturating_sub(w + 2),
        y: area.height.saturating_sub(3),
        width: w,
        height: 3,
    };
    f.render_widget(Clear, popup);
    let p = Paragraph::new(format!(" {} ", msg))
        .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(Color::Green)))
        .style(Style::default().bg(Color::Rgb(0, 40, 0)).fg(Color::LightGreen));
    f.render_widget(p, popup);
}

// ── widget helpers ────────────────────────────────────────────────────────────
fn styled_block(title: &str) -> Block<'_> {
    Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(DIM))
        .title(Span::styled(
            format!(" {} ", title),
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
        ))
}

fn render_gauge(f: &mut Frame, area: Rect, title: &str, value: f64, warn: f64, crit: f64, unit: &str) {
    let color = if value >= crit { Color::Red }
        else if value >= warn { Color::Yellow }
        else { Color::Green };
    let ratio = (value / 100.0_f64).clamp(0.0, 1.0);
    let gauge = Gauge::default()
        .block(styled_block(title))
        .gauge_style(Style::default().fg(color).bg(Color::Rgb(30, 30, 30)))
        .ratio(ratio)
        .label(format!("{:.1}{}", value, unit));
    f.render_widget(gauge, area);
}

fn render_sparkline(f: &mut Frame, area: Rect, title: &str, data: &[f64], color: Color) {
    let u: Vec<u64> = data.iter().map(|v| *v as u64).collect();
    let s = Sparkline::default()
        .block(styled_block(title))
        .data(&u)
        .style(Style::default().fg(color));
    f.render_widget(s, area);
}

fn score_line(label: &str, score: f64) -> Line<'static> {
    let color = if score >= 90.0 { Color::Green }
        else if score >= 70.0 { Color::LightGreen }
        else if score >= 50.0 { Color::Yellow }
        else { Color::Red };
    let bar = make_bar(score, 15);
    Line::from(vec![
        Span::styled(format!("  {:8} ", label), Style::default().fg(Color::Gray)),
        Span::styled(bar, Style::default().fg(color)),
        Span::styled(format!(" {:.1}%", score), Style::default().fg(color).add_modifier(Modifier::BOLD)),
    ])
}

fn make_bar(pct: f64, width: usize) -> String {
    let filled = ((pct / 100.0) * width as f64).round() as usize;
    let empty = width.saturating_sub(filled);
    format!("[{}{}]", "█".repeat(filled), "░".repeat(empty))
}
