use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Cell, Paragraph, Row, Table, Wrap};

use crate::app::App;
use crate::ui::widgets::{empty_state, panel, truncate};

pub fn draw(frame: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;
    if !app.ftp.installed {
        let mut message =
            "没有检测到 vsftpd。\n\n安装后可在这里启停服务、查看配置和用户。".to_string();
        if let Some(error) = &app.ftp.error {
            message.push_str(&format!("\n\n{error}"));
        }
        empty_state(frame, area, theme, "FTP", &message);
        return;
    }

    let rows = Layout::vertical([
        Constraint::Length(5),
        Constraint::Min(8),
        Constraint::Length(1),
    ])
    .split(area);

    let block = panel(theme, "FTP 服务");
    let inner = block.inner(rows[0]);
    frame.render_widget(block, rows[0]);
    let running = app.ftp.running;
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(vec![
                Span::styled(
                    if running {
                        "● 运行中"
                    } else {
                        "○ 已停止"
                    },
                    Style::default()
                        .fg(if running {
                            theme.success
                        } else {
                            theme.warning
                        })
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("   端口 {}", app.ftp.setting("listen_port").unwrap_or("21")),
                    Style::default().fg(theme.muted),
                ),
            ]),
            Line::from(vec![
                Span::styled("配置  ", Style::default().fg(theme.faint)),
                Span::styled(
                    truncate(
                        &app.ftp
                            .config_path
                            .as_ref()
                            .map(|p| p.display().to_string())
                            .unwrap_or_else(|| "—".into()),
                        inner.width.saturating_sub(10) as usize,
                    ),
                    Style::default().fg(theme.muted),
                ),
            ]),
            Line::from(Span::styled(
                "s 启动 · S 停止 · r 重启 · l 重载 · e 编辑配置 · u 刷新",
                Style::default().fg(theme.faint),
            )),
        ]),
        inner,
    );

    let middle =
        Layout::horizontal([Constraint::Percentage(60), Constraint::Percentage(40)]).split(rows[1]);
    draw_settings(frame, middle[0], app);
    draw_users(frame, middle[1], app);
    if let Some(error) = &app.ftp.error {
        frame.render_widget(
            Paragraph::new(Span::styled(
                truncate(&format!("! {error}"), rows[2].width as usize),
                Style::default().fg(theme.danger),
            )),
            rows[2],
        );
    } else {
        frame.render_widget(
            Paragraph::new(Span::styled(
                "FTP 账号使用系统用户；修改配置后按 l 重载服务。",
                Style::default().fg(theme.faint),
            )),
            rows[2],
        );
    }
}

fn draw_settings(frame: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;
    let block = panel(theme, "配置摘要");
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let mut rows = Vec::new();
    let order = [
        "listen",
        "listen_port",
        "anonymous_enable",
        "local_enable",
        "write_enable",
        "chroot_local_user",
        "pasv_min_port",
        "pasv_max_port",
        "ssl_enable",
        "local_root",
    ];
    for key in order {
        if let Some(value) = app.ftp.setting(key) {
            rows.push(
                Row::new(vec![
                    Cell::from(Span::styled(
                        key.to_string(),
                        Style::default().fg(theme.muted),
                    )),
                    Cell::from(value.to_string()),
                ])
                .style(Style::default().fg(theme.fg)),
            );
        }
    }
    if rows.is_empty() {
        rows.push(Row::new(vec![
            Cell::from("配置文件没有可展示的键值。"),
            Cell::from(""),
        ]));
    }
    let table = Table::new(rows, [Constraint::Length(22), Constraint::Min(8)])
        .header(
            Row::new(["配置项", "值"]).style(
                Style::default()
                    .fg(theme.faint)
                    .add_modifier(Modifier::BOLD),
            ),
        )
        .column_spacing(1);
    frame.render_widget(table, inner);
}

fn draw_users(frame: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;
    let block = panel(theme, "系统用户");
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if app.ftp.users.is_empty() {
        frame.render_widget(
            Paragraph::new(Span::styled(
                "没有可用的本地用户。",
                Style::default().fg(theme.faint),
            )),
            inner,
        );
        return;
    }
    let mut lines = Vec::new();
    for user in &app.ftp.users {
        lines.push(Line::from(vec![
            Span::styled("● ", Style::default().fg(theme.success)),
            Span::styled(
                format!("{:<10}", truncate(&user.name, 10)),
                Style::default().fg(theme.fg),
            ),
            Span::styled(
                truncate(&user.home, inner.width.saturating_sub(14) as usize),
                Style::default().fg(theme.faint),
            ),
        ]));
        if lines.len() >= inner.height as usize {
            break;
        }
    }
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), inner);
}
