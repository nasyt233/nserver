use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};

use crate::app::{App, MouseAction};
use crate::files::{FileEntry, FileFocus};
use crate::ui::widgets::{panel_focused, truncate};

pub fn draw(frame: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;
    let rows = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(5),
        Constraint::Length(1),
    ])
    .split(area);

    let path_block = panel_focused(
        theme,
        format!(
            "文件管理 · {}",
            if app.files.show_hidden {
                "显示隐藏"
            } else {
                "隐藏隐藏项"
            }
        ),
        true,
    );
    let inner = path_block.inner(rows[0]);
    frame.render_widget(path_block, rows[0]);
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("▤ ", Style::default().fg(theme.accent)),
            Span::styled(
                truncate(
                    &app.files.cwd.display().to_string(),
                    inner.width.saturating_sub(2) as usize,
                ),
                Style::default().fg(theme.fg).add_modifier(Modifier::BOLD),
            ),
        ])),
        inner,
    );

    let cols = Layout::horizontal([
        Constraint::Percentage(26),
        Constraint::Percentage(39),
        Constraint::Percentage(35),
    ])
    .split(rows[1]);
    draw_parent(frame, cols[0], app);
    draw_current(frame, cols[1], app);
    draw_preview(frame, cols[2], app);
    draw_info(frame, rows[2], app);
}

fn draw_parent(frame: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;
    let parent = app.files.cwd.parent().unwrap_or(&app.files.cwd);
    let title = format!("父目录 · {}", short_path(parent, area.width));
    let focused = app.files.focus == FileFocus::Parent;
    let block = panel_focused(theme, title, focused);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    app.register_hit(area, MouseAction::FileFocus(FileFocus::Parent));
    if app.files.parent_entries.is_empty() {
        frame.render_widget(
            Paragraph::new(Span::styled("父目录为空", Style::default().fg(theme.faint))),
            inner,
        );
        return;
    }
    let visible = inner.height as usize;
    let selected = app
        .files
        .parent_selected
        .min(app.files.parent_entries.len().saturating_sub(1));
    let start = selected.saturating_sub(visible / 2);
    let end = (start + visible.max(1)).min(app.files.parent_entries.len());
    let mut lines = Vec::new();
    for (index, entry) in app
        .files
        .parent_entries
        .iter()
        .enumerate()
        .take(end)
        .skip(start)
    {
        let is_current = entry.path == app.files.cwd;
        let selected_row = index == selected && focused;
        let style = if selected_row {
            Style::default()
                .bg(theme.selection_bg)
                .fg(theme.selection_fg)
                .add_modifier(Modifier::BOLD)
        } else if is_current {
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.muted)
        };
        let prefix = if is_current {
            "● "
        } else if entry.is_dir {
            "▸ "
        } else {
            "· "
        };
        lines.push(Line::from(vec![
            Span::styled(prefix, Style::default().fg(theme.faint)),
            Span::styled(
                truncate(&entry.name, inner.width.saturating_sub(3) as usize),
                style,
            ),
        ]));
    }
    frame.render_widget(Paragraph::new(lines), inner);
    for index in start..end {
        app.register_hit(
            Rect::new(inner.x, inner.y + (index - start) as u16, inner.width, 1),
            MouseAction::FileParentRow(index),
        );
    }
}

fn draw_current(frame: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;
    let title = format!("当前 · {} 项", app.files.entries.len());
    let focused = app.files.focus == FileFocus::Current;
    let block = panel_focused(theme, title, focused);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    app.register_hit(area, MouseAction::FileFocus(FileFocus::Current));
    if let Some(error) = &app.files.error {
        frame.render_widget(
            Paragraph::new(Span::styled(
                format!("! {error}"),
                Style::default().fg(theme.danger),
            ))
            .wrap(Wrap { trim: true }),
            inner,
        );
        return;
    }
    if app.files.entries.is_empty() {
        frame.render_widget(
            Paragraph::new(Span::styled(
                "目录为空 · n 建目录 · t 建文件",
                Style::default().fg(theme.faint),
            )),
            inner,
        );
        return;
    }
    let visible = inner.height as usize;
    let selected = app
        .files
        .selected
        .min(app.files.entries.len().saturating_sub(1));
    let start = selected.saturating_sub(visible / 2);
    let end = (start + visible.max(1)).min(app.files.entries.len());
    let wide = inner.width >= 26;
    let mut lines = Vec::new();
    for (index, entry) in app.files.entries.iter().enumerate().take(end).skip(start) {
        let selected_row = index == selected && focused;
        let base = if selected_row {
            Style::default()
                .bg(theme.selection_bg)
                .fg(theme.selection_fg)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.fg)
        };
        let icon = if entry.is_dir {
            Span::styled("▸ ", Style::default().fg(theme.info))
        } else if entry.is_symlink {
            Span::styled("↗ ", Style::default().fg(theme.accent2))
        } else {
            Span::styled("· ", Style::default().fg(theme.faint))
        };
        let mut spans = vec![
            icon,
            Span::styled(
                truncate(
                    &format!("{}{}", entry.name, if entry.is_dir { "/" } else { "" }),
                    if wide {
                        inner.width.saturating_sub(14) as usize
                    } else {
                        inner.width.saturating_sub(3) as usize
                    },
                ),
                base,
            ),
        ];
        if wide {
            spans.push(Span::styled(
                format!(" {:>7}", entry.size_label()),
                Style::default().fg(theme.faint),
            ));
        }
        lines.push(Line::from(spans));
    }
    frame.render_widget(Paragraph::new(lines), inner);
    for index in start..end {
        app.register_hit(
            Rect::new(inner.x, inner.y + (index - start) as u16, inner.width, 1),
            MouseAction::FileCurrentRow(index),
        );
    }
}

fn draw_preview(frame: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;
    let focused = app.files.focus == FileFocus::Preview;
    let title = if app.files.preview.title.is_empty() {
        "预览".to_string()
    } else {
        format!("预览 · {}", truncate(&app.files.preview.title, 18))
    };
    let block = panel_focused(theme, title, focused);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    app.register_hit(area, MouseAction::FileFocus(FileFocus::Preview));
    let preview = &app.files.preview;
    if let Some(entry) = app.files.selected() {
        let meta = format!(
            "{}  ·  {}\n{}",
            entry.mode,
            entry.size_label(),
            entry.modified_label()
        );
        frame.render_widget(
            Paragraph::new(Span::styled(meta, Style::default().fg(theme.faint))),
            Rect::new(inner.x, inner.y, inner.width, 2.min(inner.height)),
        );
    }
    let body = Rect::new(
        inner.x,
        inner.y.saturating_add(2).min(inner.bottom()),
        inner.width,
        inner.height.saturating_sub(2),
    );
    if let Some(err) = &preview.error {
        frame.render_widget(
            Paragraph::new(Span::styled(err.clone(), Style::default().fg(theme.danger)))
                .wrap(Wrap { trim: true }),
            body,
        );
        return;
    }
    let scroll = preview.scroll.min(preview.lines.len().saturating_sub(1));
    let visible: Vec<Line> = preview
        .lines
        .iter()
        .skip(scroll)
        .take(body.height as usize)
        .map(|line| {
            Line::from(Span::styled(
                line.clone(),
                Style::default().fg(if preview.binary {
                    theme.warning
                } else {
                    theme.muted
                }),
            ))
        })
        .collect();
    frame.render_widget(Paragraph::new(visible), body);
}

fn draw_info(frame: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;
    let text = if let Some(entry) = app.files.selected() {
        format!(
            "{}  ·  {}  ·  uid {}  ·  {}  ·  {}",
            file_kind(entry),
            entry.mode,
            entry.uid,
            entry.path.display(),
            entry.modified_label()
        )
    } else {
        format!(
            "{} 项 · Enter 进入/预览 · e 编辑 · n/t 新建 · r 重命名 · d 回收站",
            app.files.entries.len()
        )
    };
    frame.render_widget(
        Paragraph::new(Span::styled(
            truncate(&text, area.width as usize),
            Style::default().fg(theme.faint),
        )),
        area,
    );
}

fn file_kind(entry: &FileEntry) -> &'static str {
    if entry.is_dir {
        "目录"
    } else if entry.is_symlink {
        "符号链接"
    } else {
        "文件"
    }
}

fn short_path(path: &std::path::Path, width: u16) -> String {
    truncate(
        &path.display().to_string(),
        width.saturating_sub(10) as usize,
    )
}
