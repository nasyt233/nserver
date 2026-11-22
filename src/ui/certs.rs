use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Cell, Paragraph, Row, Table, Wrap};

use crate::app::App;
use crate::cert::CertStatus;
use crate::theme::Theme;
use crate::ui::widgets::{empty_state, pad_display, panel_focused, truncate};

pub fn draw(frame: &mut Frame, area: Rect, app: &App) {
    if app.certs.is_empty() {
        empty_state(
            frame,
            area,
            &app.theme,
            "SSL 证书",
            if app.config_missing {
                "配置文件不存在。"
            } else {
                "配置中没有 ssl_certificate 引用。\n按 n 生成自签名证书，或在站点配置中引用证书。"
            },
        );
        return;
    }
    let max_table = (area.height * 55 / 100).max(6);
    let table_height = (app.certs.len() as u16 + 3).min(max_table).max(6);
    let rows = Layout::vertical([Constraint::Length(table_height), Constraint::Min(0)]).split(area);
    draw_table(frame, rows[0], app);
    draw_detail(frame, rows[1], app);
}

fn draw_table(frame: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;
    let title = format!("SSL 证书  {} 张", app.certs.len());
    let block = panel_focused(theme, title, true);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let visible = inner.height.saturating_sub(1) as usize;
    let selected = app.cert_selected.min(app.certs.len().saturating_sub(1));
    let start = selected.saturating_sub(visible / 2);
    let end = (start + visible.max(1)).min(app.certs.len());

    let widths = [
        Constraint::Min(16),
        Constraint::Length(11),
        Constraint::Length(12),
        Constraint::Length(8),
    ];
    let mut rows = Vec::new();
    for (index, cert) in app.certs.iter().enumerate().take(end).skip(start) {
        let status = cert.status();
        let color = status_color(status, theme);
        let style = if index == selected {
            Style::default()
                .bg(theme.selection_bg)
                .fg(theme.selection_fg)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.fg)
        };
        let domain = if cert.domains.len() > 1 {
            format!(
                "{} +{}",
                cert.primary_domain(),
                cert.domains.len().saturating_sub(1)
            )
        } else {
            cert.primary_domain()
        };
        rows.push(
            Row::new(vec![
                Cell::from(truncate(&domain, 30)),
                Cell::from(cert.expires_label()),
                Cell::from(Span::styled(cert.days_label(), Style::default().fg(color))),
                Cell::from(Span::styled(status.label(), Style::default().fg(color))),
            ])
            .style(style),
        );
    }
    let table = Table::new(rows, widths)
        .header(
            Row::new(["域名", "到期", "剩余", "状态"]).style(
                Style::default()
                    .fg(theme.faint)
                    .add_modifier(Modifier::BOLD),
            ),
        )
        .column_spacing(1);
    frame.render_widget(table, inner);
}

fn draw_detail(frame: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;
    let Some(cert) = app.selected_cert() else {
        empty_state(frame, area, theme, "证书详情", "选择一张证书。");
        return;
    };
    let block = panel_focused(theme, "证书详情 · Enter 查看完整", false);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let status = cert.status();
    let color = status_color(status, theme);
    let mut lines = vec![
        kv(theme, "域名", &cert.display_name(), theme.accent),
        kv(
            theme,
            "状态",
            &format!("{} · {}", status.label(), cert.days_label()),
            color,
        ),
        kv(
            theme,
            "颁发",
            cert.issuer.as_deref().unwrap_or("—"),
            theme.muted,
        ),
        kv(theme, "文件", &cert.path.display().to_string(), theme.faint),
    ];
    if !cert.sans.is_empty() {
        lines.push(kv(theme, "SAN", &cert.sans.join(", "), theme.muted));
    }
    if let Some(err) = &cert.error {
        lines.push(Line::from(Span::styled(
            format!("错误：{err}"),
            Style::default().fg(theme.danger),
        )));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "n 生成自签名证书 · Enter 查看完整详情",
        Style::default().fg(theme.faint),
    )));
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), inner);
}

fn status_color(status: CertStatus, theme: &Theme) -> ratatui::style::Color {
    match status {
        CertStatus::Ok => theme.success,
        CertStatus::Warning => theme.warning,
        CertStatus::Critical | CertStatus::Expired => theme.danger,
        CertStatus::Unknown => theme.faint,
    }
}

fn kv(theme: &Theme, key: &str, value: &str, color: ratatui::style::Color) -> Line<'static> {
    Line::from(vec![
        Span::styled(pad_display(key, 6), Style::default().fg(theme.faint)),
        Span::styled(value.to_string(), Style::default().fg(color)),
    ])
}
