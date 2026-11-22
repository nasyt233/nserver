use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Gauge, Paragraph, Sparkline};

use crate::app::App;
use crate::system::{format_bytes, format_rate, format_uptime, percent};
use crate::ui::widgets::{panel, truncate};

pub fn draw(frame: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;
    if let Some(error) = &app.system.error {
        crate::ui::widgets::empty_state(frame, area, theme, "资源监控", error);
        return;
    }
    let rows = Layout::vertical([
        Constraint::Length(7),
        Constraint::Length(7),
        Constraint::Min(6),
    ])
    .split(area);
    let top =
        Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)]).split(rows[0]);
    cpu_panel(frame, top[0], app);
    memory_panel(frame, top[1], app);

    let middle =
        Layout::horizontal([Constraint::Percentage(62), Constraint::Percentage(38)]).split(rows[1]);
    network_panel(frame, middle[0], app);
    disk_panel(frame, middle[1], app);
    processes(frame, rows[2], app);
}

fn cpu_panel(frame: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;
    let snap = &app.system.snapshot;
    let block = panel(theme, "CPU");
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.height < 3 {
        return;
    }
    let rows = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(1),
    ])
    .split(inner);
    let cpu = snap.cpu_percent.unwrap_or(0.0);
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                format!("{cpu:.1}%"),
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!(
                    "   负载 {}",
                    snap.load
                        .map(|l| format!("{:.2} / {:.2} / {:.2}", l.0, l.1, l.2))
                        .unwrap_or_else(|| "—".into())
                ),
                Style::default().fg(theme.muted),
            ),
        ])),
        rows[0],
    );
    frame.render_widget(
        Gauge::default()
            .gauge_style(Style::default().fg(theme.accent).bg(theme.border))
            .ratio((cpu / 100.0) as f64)
            .label(""),
        rows[1],
    );
    frame.render_widget(
        Sparkline::default()
            .data(app.system.cpu_history())
            .style(Style::default().fg(theme.accent)),
        rows[2],
    );
}

fn memory_panel(frame: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;
    let snap = &app.system.snapshot;
    let block = panel(theme, "内存");
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.height < 3 {
        return;
    }
    let rows = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(1),
    ])
    .split(inner);
    let pct = percent(snap.mem_used, snap.mem_total);
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                format!("{pct:.0}%"),
                Style::default()
                    .fg(theme.success)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!(
                    "   {} / {}",
                    format_bytes(snap.mem_used),
                    format_bytes(snap.mem_total)
                ),
                Style::default().fg(theme.muted),
            ),
        ])),
        rows[0],
    );
    frame.render_widget(
        Gauge::default()
            .gauge_style(Style::default().fg(theme.success).bg(theme.border))
            .ratio((pct / 100.0) as f64)
            .label(""),
        rows[1],
    );
    let mut text = vec![Line::from(Span::styled(
        format!(
            "Swap {} / {}",
            format_bytes(snap.swap_used),
            format_bytes(snap.swap_total)
        ),
        Style::default().fg(theme.faint),
    ))];
    let spark = app.system.mem_history();
    if !spark.is_empty() {
        text.push(Line::from(""));
    }
    frame.render_widget(Paragraph::new(text), rows[2]);
    if !spark.is_empty() && rows[2].height > 1 {
        frame.render_widget(
            Sparkline::default()
                .data(&spark)
                .style(Style::default().fg(theme.success)),
            Rect::new(rows[2].x, rows[2].y + 1, rows[2].width, rows[2].height - 1),
        );
    }
}

fn network_panel(frame: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;
    let snap = &app.system.snapshot;
    let block = panel(theme, "网络");
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.height < 3 {
        return;
    }
    let (rx_hist, tx_hist) = app.system.net_history();
    let rows = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(1),
    ])
    .split(inner);
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("↓ ", Style::default().fg(theme.success)),
            Span::styled(
                format_rate(snap.net_rx_rate.unwrap_or(0.0)),
                Style::default().fg(theme.fg),
            ),
            Span::styled("   ↑ ", Style::default().fg(theme.info)),
            Span::styled(
                format_rate(snap.net_tx_rate.unwrap_or(0.0)),
                Style::default().fg(theme.fg),
            ),
        ])),
        rows[0],
    );
    frame.render_widget(
        Sparkline::default()
            .data(&rx_hist)
            .style(Style::default().fg(theme.success)),
        rows[1],
    );
    frame.render_widget(
        Sparkline::default()
            .data(&tx_hist)
            .style(Style::default().fg(theme.info)),
        rows[2],
    );
}

fn disk_panel(frame: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;
    let snap = &app.system.snapshot;
    let block = panel(theme, "磁盘与运行");
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.height < 3 {
        return;
    }
    let rows = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(1),
    ])
    .split(inner);
    let pct = percent(snap.disk_used, snap.disk_total);
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                format!("{pct:.0}%"),
                Style::default()
                    .fg(theme.warning)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!(
                    "   {} / {}",
                    format_bytes(snap.disk_used),
                    format_bytes(snap.disk_total)
                ),
                Style::default().fg(theme.muted),
            ),
        ])),
        rows[0],
    );
    frame.render_widget(
        Gauge::default()
            .gauge_style(Style::default().fg(theme.warning).bg(theme.border))
            .ratio((pct / 100.0) as f64)
            .label(""),
        rows[1],
    );
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(Span::styled(
                format!("运行 {}", format_uptime(snap.uptime_secs)),
                Style::default().fg(theme.muted),
            )),
            Line::from(Span::styled(
                format!("进程 {}", snap.process_count),
                Style::default().fg(theme.faint),
            )),
            Line::from(Span::styled(
                truncate(
                    &snap.ip_addrs.join("  "),
                    rows[2].width.saturating_sub(1) as usize,
                ),
                Style::default().fg(theme.faint),
            )),
        ]),
        rows[2],
    );
}

fn processes(frame: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;
    let block = panel(theme, "高占用进程");
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let mut lines = vec![Line::from(Span::styled(
        format!(
            "{:>7}  {:<8} {:>5} {:>5}  命令",
            "PID", "用户", "CPU", "MEM"
        ),
        Style::default()
            .fg(theme.faint)
            .add_modifier(Modifier::BOLD),
    ))];
    for process in app
        .system
        .snapshot
        .top_processes
        .iter()
        .take(inner.height.saturating_sub(2) as usize)
    {
        lines.push(Line::from(vec![
            Span::styled(
                format!("{:>7}  ", process.pid),
                Style::default().fg(theme.faint),
            ),
            Span::styled(
                format!("{:<8} ", truncate(&process.user, 8)),
                Style::default().fg(theme.muted),
            ),
            Span::styled(
                format!("{:>5.1} ", process.cpu),
                Style::default().fg(if process.cpu > 50.0 {
                    theme.danger
                } else {
                    theme.warning
                }),
            ),
            Span::styled(
                format!("{:>5.1}  ", process.mem),
                Style::default().fg(theme.muted),
            ),
            Span::styled(
                truncate(&process.command, inner.width.saturating_sub(34) as usize),
                Style::default().fg(theme.fg),
            ),
        ]));
    }
    frame.render_widget(Paragraph::new(lines), inner);
}
