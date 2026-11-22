use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};

use crate::app::{App, MouseAction};
use crate::system::format_bytes;
use crate::ui::widgets::{panel, truncate};

pub fn draw(frame: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;
    if app.cleaner.items.is_empty() {
        let block = panel(theme, "垃圾清理");
        let inner = block.inner(area);
        frame.render_widget(block, area);
        frame.render_widget(
            Paragraph::new(vec![
                Line::from(""),
                Line::from(Span::styled(
                    "正在等待扫描，按 u 重新扫描。",
                    Style::default().fg(theme.faint),
                )),
            ]),
            inner,
        );
        return;
    }
    let rows = Layout::vertical([Constraint::Min(7), Constraint::Length(5)]).split(area);
    let cols =
        Layout::horizontal([Constraint::Percentage(55), Constraint::Percentage(45)]).split(rows[0]);
    draw_list(frame, cols[0], app);
    draw_detail(frame, cols[1], app);
    draw_summary(frame, rows[1], app);
}

fn draw_list(frame: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;
    let block = panel(theme, "可清理项目");
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let visible = inner.height as usize;
    let selected = app
        .cleaner
        .selected
        .min(app.cleaner.items.len().saturating_sub(1));
    let start = selected.saturating_sub(visible / 2);
    let end = (start + visible.max(1)).min(app.cleaner.items.len());
    let mut lines = Vec::new();
    for (index, item) in app.cleaner.items.iter().enumerate().take(end).skip(start) {
        let is_selected = index == selected;
        let style = if is_selected {
            Style::default()
                .bg(theme.selection_bg)
                .fg(theme.selection_fg)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.fg)
        };
        let check = if item.selected { "[x] " } else { "[ ] " };
        let counter = if item.available {
            format_bytes(item.bytes)
        } else {
            "不可用".into()
        };
        lines.push(Line::from(vec![
            Span::styled(
                check,
                Style::default().fg(if item.selected {
                    theme.success
                } else {
                    theme.faint
                }),
            ),
            Span::styled(
                truncate(item.label, inner.width.saturating_sub(12) as usize),
                style,
            ),
            Span::styled(format!(" {counter:>9}"), Style::default().fg(theme.muted)),
        ]));
    }
    frame.render_widget(Paragraph::new(lines), inner);
    for index in start..end {
        let y = inner.y + (index - start) as u16;
        app.register_hit(
            Rect::new(inner.x, y, 5.min(inner.width), 1),
            MouseAction::CleanerToggle(index),
        );
        app.register_hit(
            Rect::new(
                inner.x.saturating_add(5),
                y,
                inner.width.saturating_sub(5),
                1,
            ),
            MouseAction::CleanerRow(index),
        );
    }
}

fn draw_detail(frame: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;
    let block = panel(theme, "说明");
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if let Some(item) = app.cleaner.selected() {
        let lines = vec![
            Line::from(Span::styled(
                item.label,
                Style::default().fg(theme.fg).add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(Span::styled(
                item.description,
                Style::default().fg(theme.muted),
            )),
            Line::from(""),
            Line::from(Span::styled(
                format!("可释放：{}", format_bytes(item.bytes)),
                Style::default().fg(theme.warning),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "Space 选择 · a 全选 · A 取消全选 · c 清理 · u 重新扫描",
                Style::default().fg(theme.faint),
            )),
        ];
        frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), inner);
    }
}

fn draw_summary(frame: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;
    let block = panel(theme, "清理汇总");
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let selected_count = app
        .cleaner
        .items
        .iter()
        .filter(|item| item.selected)
        .count();
    let total = app.cleaner.items.iter().map(|item| item.bytes).sum::<u64>();
    let lines = vec![
        Line::from(vec![
            Span::styled("已选择 ", Style::default().fg(theme.faint)),
            Span::styled(
                format!("{selected_count} 项"),
                Style::default().fg(theme.fg).add_modifier(Modifier::BOLD),
            ),
            Span::styled("   预计释放 ", Style::default().fg(theme.faint)),
            Span::styled(
                format_bytes(app.cleaner.selected_bytes()),
                Style::default()
                    .fg(theme.success)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(Span::styled(
            format!("可清理总量：{}", format_bytes(total)),
            Style::default().fg(theme.muted),
        )),
    ];
    frame.render_widget(Paragraph::new(lines), inner);
}
