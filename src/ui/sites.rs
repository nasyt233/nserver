use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Cell, Paragraph, Row, Table, Wrap};

use crate::app::{App, MouseAction};
use crate::theme::Theme;
use crate::ui::widgets::{empty_state, pad_display, panel_focused, truncate};

pub fn draw(frame: &mut Frame, area: Rect, app: &App) {
    let cols =
        Layout::horizontal([Constraint::Percentage(46), Constraint::Percentage(54)]).split(area);
    draw_list(frame, cols[0], app);
    draw_detail(frame, cols[1], app);
}

fn draw_list(frame: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;
    let sites = app.filtered_sites();
    if sites.is_empty() {
        empty_state(
            frame,
            area,
            theme,
            "站点",
            if app.config_missing {
                "配置文件不存在，无法发现站点。"
            } else if !app.site_filter.is_empty() {
                "没有匹配筛选条件的站点。按 Esc 清除筛选。"
            } else {
                "没有发现站点。按 n 创建，或检查 sites-available。"
            },
        );
        return;
    }

    let header = if app.site_filter.is_empty() {
        format!("站点  {}/{}", sites.len(), app.sites.sites.len())
    } else {
        format!(
            "站点  {}/{}  筛选: {}",
            sites.len(),
            app.sites.sites.len(),
            truncate(&app.site_filter, 18)
        )
    };
    let block = panel_focused(theme, header, true);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let visible_height = inner.height.saturating_sub(1) as usize;
    let selected = app.site_selected.min(sites.len().saturating_sub(1));
    let start = selected.saturating_sub(visible_height / 2);
    let end = (start + visible_height.max(1)).min(sites.len());

    let show_ports = inner.width >= 38;
    let widths = if show_ports {
        vec![
            Constraint::Length(2),
            Constraint::Min(12),
            Constraint::Length(9),
            Constraint::Length(4),
        ]
    } else {
        vec![Constraint::Length(2), Constraint::Min(10)]
    };

    let mut rows = Vec::new();
    for (index, site) in sites.iter().enumerate().take(end).skip(start) {
        let site = *site;
        let selected_row = index == selected;
        let mut style = if selected_row {
            Style::default()
                .bg(theme.selection_bg)
                .fg(theme.selection_fg)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.fg)
        };
        if !site.enabled && !selected_row {
            style = style.fg(theme.muted);
        }
        let icon = if site.enabled {
            Span::styled("●", Style::default().fg(theme.success))
        } else {
            Span::styled("○", Style::default().fg(theme.faint))
        };
        let mut cells = vec![Cell::from(Line::from(icon)), Cell::from(site.name.clone())];
        if show_ports {
            cells.push(Cell::from(Span::styled(
                site.listen_ports(),
                Style::default().fg(theme.info),
            )));
            cells.push(Cell::from(if site.tls() {
                Span::styled("TLS", Style::default().fg(theme.accent2))
            } else {
                Span::styled("—", Style::default().fg(theme.faint))
            }));
        }
        rows.push(Row::new(cells).style(style));
    }

    let header_cells = if show_ports {
        vec!["", "站点", "端口", "TLS"]
    } else {
        vec!["", "站点"]
    };
    let table = Table::new(rows, widths)
        .header(
            Row::new(header_cells.into_iter().map(Cell::from)).style(
                Style::default()
                    .fg(theme.faint)
                    .add_modifier(Modifier::BOLD),
            ),
        )
        .column_spacing(1);
    frame.render_widget(table, inner);
    for index in start..end {
        app.register_hit(
            Rect::new(
                inner.x,
                inner.y + 1 + (index - start) as u16,
                inner.width,
                1,
            ),
            MouseAction::SiteRow(index),
        );
    }

    if end < sites.len() {
        let hint = format!("↓ 还有 {} 个", sites.len() - end);
        if inner.height > 0 {
            frame.render_widget(
                Paragraph::new(Span::styled(hint, Style::default().fg(theme.faint))),
                Rect::new(inner.x, inner.y + inner.height - 1, inner.width, 1),
            );
        }
    }
}

fn draw_detail(frame: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;
    let Some(site) = app.selected_site() else {
        empty_state(frame, area, theme, "站点详情", "选择一个站点查看摘录。");
        return;
    };
    let title = format!(
        "{} · {}",
        site.name,
        if site.enabled {
            "已启用"
        } else {
            "已停用"
        }
    );
    let block = panel_focused(theme, title, false);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let mut lines = Vec::new();
    lines.push(kv(theme, "入口", &site_public_url(site), theme.accent));
    lines.push(kv(
        theme,
        "状态",
        if site.enabled {
            "已启用"
        } else {
            "已停用"
        },
        if site.enabled {
            theme.success
        } else {
            theme.faint
        },
    ));
    lines.push(kv(theme, "类型", site.kind_label(), theme.info));
    lines.push(kv(
        theme,
        "文件",
        &config_relative(&site.available, &app.config.config_dir),
        theme.muted,
    ));
    lines.push(kv(
        theme,
        "修改",
        &format!(
            "{} · {}",
            site.modified_label(),
            crate::logs::format_bytes(site.size)
        ),
        theme.muted,
    ));
    lines.push(kv(
        theme,
        "server",
        &format!(
            "{} 个块 · {} location",
            site.servers.len(),
            site.servers.iter().map(|s| s.locations).sum::<usize>()
        ),
        theme.muted,
    ));
    if !site.can_toggle {
        lines.push(Line::from(Span::styled(
            "此文件不在 sites-available 中，站点切换为只读。",
            Style::default().fg(theme.faint),
        )));
    }
    let hint = match site.kind() {
        crate::sites::SiteKind::Static => "m 设置 · w 网站目录 · e 完整配置",
        crate::sites::SiteKind::Proxy | crate::sites::SiteKind::PortForward => {
            "m 设置 · p 代理目标 · l 监听端口 · e 完整配置"
        }
        _ => "m 设置 · e 完整配置",
    };
    lines.push(Line::from(Span::styled(
        hint,
        Style::default().fg(theme.faint),
    )));
    lines.push(Line::from(""));

    for (idx, server) in site.servers.iter().enumerate() {
        lines.push(Line::from(vec![
            Span::styled(
                format!("[{}] ", idx + 1),
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                server.display_names(),
                Style::default().fg(theme.fg).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("  L{}", server.line),
                Style::default().fg(theme.faint),
            ),
        ]));
        let meta = [
            server.listen_label(),
            if server.tls {
                "TLS 是".to_string()
            } else {
                "TLS 否".to_string()
            },
            format!("{} location", server.locations),
        ];
        lines.push(Line::from(Span::styled(
            format!("    {}", meta.join("  ·  ")),
            Style::default().fg(theme.muted),
        )));
        if let Some(root) = &server.root {
            lines.push(Line::from(Span::styled(
                format!("    root {root}"),
                Style::default().fg(theme.muted),
            )));
        }
        if !server.proxy_targets.is_empty() {
            lines.push(Line::from(vec![
                Span::styled("    → ", Style::default().fg(theme.accent2)),
                Span::styled(
                    server.proxy_targets.join("  "),
                    Style::default().fg(theme.accent2),
                ),
            ]));
        }
        if let Some(redirect) = &server.internal_redirect {
            lines.push(Line::from(Span::styled(
                format!("    return {redirect}"),
                Style::default().fg(theme.warning),
            )));
        }
        if let Some(cert) = &server.cert_path {
            lines.push(Line::from(Span::styled(
                format!("    证书 {}", config_relative(cert, &app.config.config_dir)),
                Style::default().fg(theme.faint),
            )));
        }
        lines.push(Line::from(""));
    }
    if site.servers.is_empty() {
        lines.push(Line::from(Span::styled(
            "没有解析到 server 块。可用 e 打开文件检查。",
            Style::default().fg(theme.warning),
        )));
    }

    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: false });
    frame.render_widget(paragraph, inner);
}

fn site_public_url(site: &crate::sites::Site) -> String {
    let Some(server) = site.servers.first() else {
        return "—".to_string();
    };
    let port = server
        .listens
        .first()
        .map(|value| value.split_whitespace().next().unwrap_or("80"))
        .unwrap_or("80")
        .trim_start_matches("0.0.0.0:")
        .to_string();
    let host = server
        .names
        .iter()
        .find(|name| name.as_str() != "_" && !name.is_empty())
        .cloned()
        .unwrap_or_else(|| "127.0.0.1".to_string());
    let scheme = if server.tls { "https" } else { "http" };
    format!("{scheme}://{host}:{port}")
}

fn kv(theme: &Theme, key: &str, value: &str, color: ratatui::style::Color) -> Line<'static> {
    Line::from(vec![
        Span::styled(pad_display(key, 7), Style::default().fg(theme.faint)),
        Span::styled(value.to_string(), Style::default().fg(color)),
    ])
}

fn config_relative(path: &std::path::Path, config_dir: &std::path::Path) -> String {
    path.strip_prefix(config_dir)
        .unwrap_or(path)
        .display()
        .to_string()
}
