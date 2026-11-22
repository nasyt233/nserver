use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Gauge, Paragraph, Sparkline, Wrap};

use crate::app::App;
use crate::system::{format_bytes, format_rate, format_uptime, percent};
use crate::theme::Theme;
use crate::ui::widgets::{hint_line_fit, panel, truncate};

pub fn draw(frame: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;
    let snap = &app.system.snapshot;
    let rows = Layout::vertical([
        Constraint::Length(6),
        Constraint::Min(8),
        Constraint::Length(7),
    ])
    .split(area);

    let cards = Layout::horizontal([
        Constraint::Percentage(25),
        Constraint::Percentage(25),
        Constraint::Percentage(25),
        Constraint::Percentage(25),
    ])
    .split(rows[0]);

    let cpu = snap.cpu_percent.unwrap_or(0.0);
    resource_card(
        frame,
        cards[0],
        theme,
        "CPU",
        &format!("{cpu:.1}%"),
        cpu,
        &format!(
            "负载 {}",
            snap.load
                .map(|load| format!("{:.2}", load.0))
                .unwrap_or_else(|| "—".into())
        ),
        app.system.cpu_history(),
        theme.accent,
    );

    let mem = percent(snap.mem_used, snap.mem_total);
    resource_card(
        frame,
        cards[1],
        theme,
        "内存",
        &format!("{mem:.0}%"),
        mem,
        &format!(
            "{} / {}",
            format_bytes(snap.mem_used),
            format_bytes(snap.mem_total)
        ),
        app.system.mem_history(),
        theme.success,
    );

    let disk = percent(snap.disk_used, snap.disk_total);
    resource_card(
        frame,
        cards[2],
        theme,
        "磁盘",
        &format!("{disk:.0}%"),
        disk,
        &format!(
            "{} / {}",
            format_bytes(snap.disk_used),
            format_bytes(snap.disk_total)
        ),
        Vec::new(),
        theme.warning,
    );

    let rx = snap.net_rx_rate.unwrap_or(0.0);
    let tx = snap.net_tx_rate.unwrap_or(0.0);
    network_card(frame, cards[3], app, theme, rx, tx);

    let middle =
        Layout::horizontal([Constraint::Percentage(56), Constraint::Percentage(44)]).split(rows[1]);
    system_panel(frame, middle[0], app);
    services_panel(frame, middle[1], app);
    processes_panel(frame, rows[2], app);
}

#[allow(clippy::too_many_arguments)]
fn resource_card(
    frame: &mut Frame,
    area: Rect,
    theme: &Theme,
    title: &str,
    value: &str,
    ratio: f32,
    detail: &str,
    spark: Vec<u64>,
    color: ratatui::style::Color,
) {
    let block = panel(theme, title);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.height < 2 || inner.width < 8 {
        return;
    }
    let rows = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(0),
        Constraint::Length(1),
    ])
    .split(inner);
    frame.render_widget(
        Paragraph::new(Span::styled(
            value.to_string(),
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        )),
        rows[0],
    );
    let gauge = Gauge::default()
        .gauge_style(Style::default().fg(color).bg(theme.border))
        .ratio((ratio / 100.0).clamp(0.0, 1.0) as f64)
        .label("");
    frame.render_widget(gauge, rows[1]);
    if !spark.is_empty() {
        let sparkline = Sparkline::default()
            .data(&spark)
            .style(Style::default().fg(color));
        frame.render_widget(sparkline, rows[2]);
    }
    frame.render_widget(
        Paragraph::new(Span::styled(
            truncate(detail, rows[3].width as usize),
            Style::default().fg(theme.muted),
        )),
        rows[3],
    );
}

fn network_card(frame: &mut Frame, area: Rect, app: &App, theme: &Theme, rx: f64, tx: f64) {
    let block = panel(theme, "网络");
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.height < 3 {
        return;
    }
    let rows = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(1),
        Constraint::Length(1),
    ])
    .split(inner);
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("↓ ", Style::default().fg(theme.success)),
            Span::styled(
                format_rate(rx),
                Style::default().fg(theme.fg).add_modifier(Modifier::BOLD),
            ),
        ])),
        rows[0],
    );
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("↑ ", Style::default().fg(theme.info)),
            Span::styled(format_rate(tx), Style::default().fg(theme.fg)),
        ])),
        rows[1],
    );
    let (rx_hist, _) = app.system.net_history();
    if !rx_hist.is_empty() {
        frame.render_widget(
            Sparkline::default()
                .data(&rx_hist)
                .style(Style::default().fg(theme.success)),
            rows[2],
        );
    }
    frame.render_widget(
        Paragraph::new(Span::styled(
            "最近 2 分钟",
            Style::default().fg(theme.faint),
        )),
        rows[3],
    );
}

fn system_panel(frame: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;
    let snap = &app.system.snapshot;
    let block = panel(theme, "系统信息");
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let ips = if snap.ip_addrs.is_empty() {
        "—".to_string()
    } else {
        snap.ip_addrs.join(", ")
    };
    let load = snap
        .load
        .map(|(a, b, c)| format!("{a:.2}  {b:.2}  {c:.2}"))
        .unwrap_or_else(|| "—".into());
    let lines = vec![
        kv(theme, "主机", &snap.hostname),
        kv(theme, "系统", &snap.os),
        kv(theme, "运行", &format_uptime(snap.uptime_secs)),
        kv(theme, "IP", &ips),
        kv(theme, "负载", &load),
        kv(theme, "进程", &snap.process_count.to_string()),
        kv(
            theme,
            "网络",
            &format!(
                "↓ {}  ↑ {}",
                format_rate(snap.net_rx_rate.unwrap_or(0.0)),
                format_rate(snap.net_tx_rate.unwrap_or(0.0))
            ),
        ),
    ];
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), inner);
}

fn services_panel(frame: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;
    let block = panel(theme, "服务与快捷操作");
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let nginx_running = app.process.running();
    let docker_count = app.docker.running_count();
    let docker_total = app.docker.containers.len();
    let ftp_running = app.ftp.running;
    let nginx_detail = if nginx_running {
        format!(
            "PID {} · {} · {}",
            app.process.master_pid.unwrap_or(0),
            crate::service::format_memory(app.process.rss_kb),
            crate::service::format_uptime(app.process.uptime_secs)
        )
    } else {
        "未运行".into()
    };
    let lines = vec![
        status_line(theme, "Nginx", nginx_running, nginx_detail),
        status_line(
            theme,
            "Docker",
            app.docker.daemon_running,
            if app.docker.daemon_running {
                format!("{docker_count} 运行 / {docker_total} 容器")
            } else if app.docker.installed {
                "守护进程未连接".into()
            } else {
                "未安装".into()
            },
        ),
        status_line(
            theme,
            "FTP",
            ftp_running,
            if app.ftp.installed {
                if ftp_running {
                    "vsftpd 运行中"
                } else {
                    "vsftpd 未运行"
                }
                .into()
            } else {
                "未安装 vsftpd".into()
            },
        ),
        Line::from(""),
        Line::from(Span::styled(
            "快捷操作",
            Style::default()
                .fg(theme.faint)
                .add_modifier(Modifier::BOLD),
        )),
        hint_line_fit(
            theme,
            &[
                ("n", "新建网站"),
                ("r", "重载 Nginx"),
                ("t", "校验"),
                ("?", "帮助"),
            ],
            inner.width as usize,
        ),
    ];
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), inner);
}

fn processes_panel(frame: &mut Frame, area: Rect, app: &App) {
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
    for process in app.system.snapshot.top_processes.iter().take(4) {
        let color = if process.cpu > 50.0 {
            theme.danger
        } else if process.cpu > 15.0 {
            theme.warning
        } else {
            theme.muted
        };
        lines.push(Line::from(vec![
            Span::styled(
                format!("{:>7}  ", process.pid),
                Style::default().fg(theme.faint),
            ),
            Span::styled(
                format!("{:<8} ", truncate(&process.user, 8)),
                Style::default().fg(theme.muted),
            ),
            Span::styled(format!("{:>5.1} ", process.cpu), Style::default().fg(color)),
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
    if app.system.snapshot.top_processes.is_empty() {
        lines.push(Line::from(Span::styled(
            "没有进程信息（/proc 不可读或 ps 不可用）。",
            Style::default().fg(theme.faint),
        )));
    }
    frame.render_widget(Paragraph::new(lines), inner);
}

fn kv(theme: &Theme, key: &str, value: &str) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("{key:<6}"), Style::default().fg(theme.faint)),
        Span::styled(value.to_string(), Style::default().fg(theme.fg)),
    ])
}

fn status_line(theme: &Theme, name: &str, ok: bool, detail: String) -> Line<'static> {
    let (icon, color) = if ok {
        ("●", theme.success)
    } else {
        ("○", theme.faint)
    };
    Line::from(vec![
        Span::styled(icon, Style::default().fg(color)),
        Span::styled(
            format!(" {name:<8}"),
            Style::default().fg(theme.fg).add_modifier(Modifier::BOLD),
        ),
        Span::styled(truncate(&detail, 28), Style::default().fg(theme.muted)),
    ])
}
