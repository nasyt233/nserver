use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};

use crate::app::{App, MouseAction};
use crate::ui::widgets::panel;

pub fn draw(frame: &mut Frame, area: Rect, app: &App) {
    let rows = Layout::vertical([Constraint::Min(5), Constraint::Length(4)]).split(area);
    draw_list(frame, rows[0], app);
    draw_help(frame, rows[1], app);
}

fn draw_list(frame: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;
    let block = panel(theme, "系统服务 · systemd");
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let visible = inner.height as usize;
    let selected = app
        .service_selected
        .min(app.service_units.len().saturating_sub(1));
    let start = selected.saturating_sub(visible / 2);
    let end = (start + visible.max(1)).min(app.service_units.len());
    let mut lines = Vec::new();
    for (index, unit) in app.service_units.iter().enumerate().take(end).skip(start) {
        let selected_style = if index == selected {
            Style::default()
                .bg(theme.selection_bg)
                .fg(theme.selection_fg)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.fg)
        };
        let active = if unit.active == "active" {
            Span::styled("● active ", Style::default().fg(theme.success))
        } else {
            Span::styled(
                format!("○ {:<7} ", unit.active),
                Style::default().fg(theme.faint),
            )
        };
        lines.push(Line::from(vec![
            active,
            Span::styled(format!("{:<18}", unit.name), selected_style),
            Span::styled(
                unit.description.to_string(),
                Style::default().fg(theme.muted),
            ),
            Span::styled(
                format!("  开机: {}", unit.enabled),
                Style::default().fg(theme.faint),
            ),
        ]));
    }
    if lines.is_empty() {
        lines.push(Line::from(Span::styled(
            "没有可管理的服务。",
            Style::default().fg(theme.faint),
        )));
    }
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), inner);
    for index in start..end {
        app.register_hit(
            Rect::new(inner.x, inner.y + (index - start) as u16, inner.width, 1),
            MouseAction::ServiceRow(index),
        );
    }
}

fn draw_help(frame: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;
    let block = panel(theme, "操作");
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let lines = vec![
        Line::from(Span::styled(
            "s 启动   x 停止   r 重启   e 开机启动   d 取消开机启动   u 刷新",
            Style::default().fg(theme.muted),
        )),
        Line::from(Span::styled(
            "双击服务行可重启 · 所有修改操作都会先确认 · 只读模式禁用写操作",
            Style::default().fg(theme.faint),
        )),
    ];
    frame.render_widget(Paragraph::new(lines), inner);
}
