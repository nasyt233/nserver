use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Cell, Paragraph, Row, Table};

use crate::app::{App, UpstreamFocus};
use crate::config::{UpstreamInfo, file_label};
use crate::ui::widgets::{empty_state, format_age, panel_focused, truncate};

pub fn draw(frame: &mut Frame, area: Rect, app: &App) {
    if app.config.upstreams.is_empty() {
        empty_state(
            frame,
            area,
            &app.theme,
            "上游",
            "配置中没有 upstream 块。\n直接写 proxy_pass 的站点可在「站点」页查看。",
        );
        return;
    }
    let cols =
        Layout::horizontal([Constraint::Percentage(34), Constraint::Percentage(66)]).split(area);
    draw_pools(frame, cols[0], app);
    draw_backends(frame, cols[1], app);
}

fn draw_pools(frame: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;
    let title = format!("上游组  {}", app.config.upstreams.len());
    let focused = app.upstream_focus == UpstreamFocus::Pools;
    let block = panel_focused(theme, title, focused);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let visible = (inner.height as usize / 2).max(1);
    let selected = app
        .upstream_selected
        .min(app.config.upstreams.len().saturating_sub(1));
    let start = selected.saturating_sub(visible / 2);
    let end = (start + visible).min(app.config.upstreams.len());
    let mut lines = Vec::new();
    for (index, upstream) in app
        .config
        .upstreams
        .iter()
        .enumerate()
        .take(end)
        .skip(start)
    {
        let is_selected = index == selected;
        let style = if is_selected {
            Style::default()
                .bg(theme.selection_bg)
                .fg(theme.selection_fg)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.fg).add_modifier(Modifier::BOLD)
        };
        lines.push(Line::from(Span::styled(
            format!(
                " {} ",
                truncate(&upstream.name, inner.width.saturating_sub(3) as usize)
            ),
            style,
        )));
        lines.push(Line::from(Span::styled(
            format!(
                "   {} · {} 节点 · L{}",
                upstream.method,
                upstream.backends.len(),
                upstream.line
            ),
            Style::default().fg(if is_selected {
                theme.selection_fg
            } else {
                theme.faint
            }),
        )));
    }
    frame.render_widget(Paragraph::new(lines), inner);
}

fn draw_backends(frame: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;
    let Some(upstream) = app.selected_upstream() else {
        empty_state(frame, area, theme, "后端", "选择一个上游组。");
        return;
    };
    let focused = app.upstream_focus == UpstreamFocus::Backends;
    let title = format!("{} · {} 个后端", upstream.name, upstream.backends.len());
    let block = panel_focused(theme, title, focused);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.height < 2 {
        return;
    }
    let rows_area = Rect::new(inner.x, inner.y, inner.width, inner.height - 1);
    let info_area = Rect::new(inner.x, inner.y + inner.height - 1, inner.width, 1);

    if upstream.backends.is_empty() {
        frame.render_widget(
            Paragraph::new(Span::styled(
                "这个 upstream 没有 server 节点。",
                Style::default().fg(theme.warning),
            )),
            info_area,
        );
        return;
    }

    let visible = rows_area.height.saturating_sub(1) as usize;
    let selected = app
        .backend_selected
        .min(upstream.backends.len().saturating_sub(1));
    let start = selected.saturating_sub(visible / 2);
    let end = (start + visible.max(1)).min(upstream.backends.len());

    let widths = [
        Constraint::Length(2),
        Constraint::Min(14),
        Constraint::Length(11),
        Constraint::Length(9),
    ];
    let mut rows = Vec::new();
    for index in start..end {
        let backend = &upstream.backends[index];
        let probe = app.probe_for(upstream, backend);
        let (icon, color) = if backend.down {
            ("–", theme.faint)
        } else {
            match probe {
                Some(p) if p.success => ("✓", theme.success),
                Some(_) => ("×", theme.danger),
                None => ("?", theme.faint),
            }
        };
        let style = if index == selected && focused {
            Style::default()
                .bg(theme.selection_bg)
                .fg(theme.selection_fg)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.fg)
        };
        let flags = if backend.down {
            "down".to_string()
        } else if backend.backup {
            "backup".to_string()
        } else if backend.extra.is_empty() {
            "—".to_string()
        } else {
            backend.extra.join(" ")
        };
        let latency = match probe {
            Some(p) if p.success => format!("{} ms", p.latency_ms),
            Some(_) => "失败".to_string(),
            None => "—".to_string(),
        };
        rows.push(
            Row::new(vec![
                Cell::from(Span::styled(icon, Style::default().fg(color))),
                Cell::from(backend.address.clone()),
                Cell::from(Span::styled(flags, Style::default().fg(theme.muted))),
                Cell::from(Span::styled(latency, Style::default().fg(color))),
            ])
            .style(style),
        );
    }
    let table = Table::new(rows, widths)
        .header(
            Row::new(["", "后端", "标记", "延迟"]).style(
                Style::default()
                    .fg(theme.faint)
                    .add_modifier(Modifier::BOLD),
            ),
        )
        .column_spacing(1);
    frame.render_widget(table, rows_area);

    let info = selected_backend_info(app, upstream, selected);
    frame.render_widget(
        Paragraph::new(Span::styled(
            truncate(&info, info_area.width as usize),
            Style::default().fg(theme.faint),
        )),
        info_area,
    );
}

fn selected_backend_info(app: &App, upstream: &UpstreamInfo, selected: usize) -> String {
    let Some(backend) = upstream.backends.get(selected) else {
        return String::new();
    };
    let mut parts = vec![format!(
        "{} · {}",
        backend.address,
        file_label(&upstream.source, &app.config.config_dir)
    )];
    if let Some(probe) = app.probe_for(upstream, backend) {
        parts.push(format!(
            "{} · {} · {}",
            if probe.success { "在线" } else { "离线" },
            probe.message,
            format_age(probe.at)
        ));
    } else {
        parts.push("Enter 探测 · p 全部探测".to_string());
    }
    parts.join(" · ")
}
