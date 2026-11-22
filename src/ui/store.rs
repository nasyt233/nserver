use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::app::{App, MouseAction};
use crate::ui::widgets::{panel, truncate};

pub fn draw(frame: &mut Frame, area: Rect, app: &App) {
    let cols =
        Layout::horizontal([Constraint::Percentage(48), Constraint::Percentage(52)]).split(area);
    draw_list(frame, cols[0], app);
    draw_detail(frame, cols[1], app);
}

fn draw_list(frame: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;
    let title = format!(
        "应用商店  {} · {}",
        app.store.manager.label(),
        if app.store.apps.iter().filter(|app| app.installed).count() > 0 {
            format!(
                "{} 已安装",
                app.store.apps.iter().filter(|app| app.installed).count()
            )
        } else {
            "未安装任何条目".into()
        }
    );
    let block = panel(theme, title);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let visible = inner.height as usize;
    let selected = app
        .store
        .selected
        .min(app.store.apps.len().saturating_sub(1));
    let start = selected.saturating_sub(visible / 2);
    let end = (start + visible.max(1)).min(app.store.apps.len());
    let mut lines = Vec::new();
    for (index, item) in app.store.apps.iter().enumerate().take(end).skip(start) {
        let is_selected = index == selected;
        let style = if is_selected {
            Style::default()
                .bg(theme.selection_bg)
                .fg(theme.selection_fg)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.fg)
        };
        let dot = if item.installed {
            Span::styled("● ", Style::default().fg(theme.success))
        } else {
            Span::styled("○ ", Style::default().fg(theme.faint))
        };
        lines.push(Line::from(vec![
            dot,
            Span::styled(format!("{:<14}", truncate(item.name, 14)), style),
            Span::styled(
                truncate(item.category, inner.width.saturating_sub(18) as usize),
                Style::default().fg(theme.faint),
            ),
        ]));
    }
    frame.render_widget(Paragraph::new(lines), inner);
    for index in start..end {
        app.register_hit(
            Rect::new(inner.x, inner.y + (index - start) as u16, inner.width, 1),
            MouseAction::StoreRow(index),
        );
    }
}

fn draw_detail(frame: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;
    let block = panel(theme, "应用详情");
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let Some(item) = app.store.selected_app() else {
        frame.render_widget(
            Paragraph::new(Span::styled("没有应用。", Style::default().fg(theme.faint))),
            inner,
        );
        return;
    };
    let status = if item.installed {
        Span::styled(
            "已安装",
            Style::default()
                .fg(theme.success)
                .add_modifier(Modifier::BOLD),
        )
    } else {
        Span::styled("未安装", Style::default().fg(theme.faint))
    };
    let lines = vec![
        Line::from(Span::styled(
            item.name.to_string(),
            Style::default().fg(theme.fg).add_modifier(Modifier::BOLD),
        )),
        Line::from(vec![
            Span::styled("软件包  ", Style::default().fg(theme.faint)),
            Span::styled(item.package.to_string(), Style::default().fg(theme.muted)),
        ]),
        Line::from(vec![
            Span::styled("分类    ", Style::default().fg(theme.faint)),
            Span::styled(item.category.to_string(), Style::default().fg(theme.muted)),
        ]),
        Line::from(vec![
            Span::styled("状态    ", Style::default().fg(theme.faint)),
            status,
        ]),
        Line::from(""),
        Line::from(Span::styled(
            item.description.to_string(),
            Style::default().fg(theme.muted),
        )),
        Line::from(""),
        Line::from(Span::styled(
            if item.installed {
                "x 卸载 · i 安装 · u 刷新"
            } else {
                "i 安装 · u 刷新"
            },
            Style::default().fg(theme.faint),
        )),
    ];
    frame.render_widget(
        Paragraph::new(lines).wrap(ratatui::widgets::Wrap { trim: true }),
        inner,
    );
}

#[allow(dead_code)]
fn _unused(_: &App) {}
