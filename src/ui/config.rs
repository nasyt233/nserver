use std::path::Path;

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};

use crate::app::App;
use crate::logs::format_bytes;
use crate::theme::Theme;
use crate::ui::widgets::{empty_state, panel_focused, truncate};

pub fn draw(frame: &mut Frame, area: Rect, app: &App) {
    let cols =
        Layout::horizontal([Constraint::Percentage(36), Constraint::Percentage(64)]).split(area);
    draw_list(frame, cols[0], app);
    draw_preview(frame, cols[1], app);
}

fn draw_list(frame: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;
    let files = app.filtered_config_files();
    if files.is_empty() {
        empty_state(
            frame,
            area,
            theme,
            "配置文件",
            if app.config_missing {
                "主配置不存在。请用 --config 指定正确路径。"
            } else if !app.config_filter.is_empty() {
                "没有匹配的文件。按 Esc 清除筛选。"
            } else {
                "没有发现配置文件。"
            },
        );
        return;
    }
    let title = if app.config_filter.is_empty() {
        format!("配置文件  {}", files.len())
    } else {
        format!(
            "配置文件  {}/{}  /{}",
            files.len(),
            app.config_files.len(),
            app.config_filter
        )
    };
    let block = panel_focused(
        theme,
        truncate(&title, area.width.saturating_sub(4) as usize),
        true,
    );
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let visible = inner.height as usize;
    let selected = app.config_selected.min(files.len().saturating_sub(1));
    let start = selected.saturating_sub(visible / 2);
    let end = (start + visible.max(1)).min(files.len());

    let mut lines = Vec::new();
    for (index, path) in files.iter().enumerate().take(end).skip(start) {
        let path = *path;
        let label = path
            .strip_prefix(&app.config.config_dir)
            .unwrap_or(path)
            .display()
            .to_string();
        let is_main = path == &app.config.main;
        let is_selected = index == selected;
        let style = if is_selected {
            Style::default()
                .bg(theme.selection_bg)
                .fg(theme.selection_fg)
                .add_modifier(Modifier::BOLD)
        } else if is_main {
            Style::default().fg(theme.accent)
        } else {
            Style::default().fg(theme.muted)
        };
        let marker = if is_main {
            Span::styled("★ ", Style::default().fg(theme.accent))
        } else {
            Span::styled("· ", Style::default().fg(theme.faint))
        };
        lines.push(Line::from(vec![
            marker,
            Span::styled(
                truncate(&label, inner.width.saturating_sub(3) as usize),
                style,
            ),
        ]));
    }
    frame.render_widget(Paragraph::new(lines), inner);
}

fn draw_preview(frame: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;
    let Some(preview) = &app.config_preview else {
        empty_state(
            frame,
            area,
            theme,
            "配置预览",
            "选择左侧文件查看内容。按 e 打开内置编辑器。",
        );
        return;
    };

    let title = preview
        .path
        .strip_prefix(&app.config.config_dir)
        .unwrap_or(&preview.path)
        .display()
        .to_string();
    let block = panel_focused(
        theme,
        truncate(
            &format!("预览 · {title}"),
            area.width.saturating_sub(3) as usize,
        ),
        false,
    );
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.height < 2 {
        return;
    }

    let (info_area, body_area) = (
        Rect::new(inner.x, inner.y, inner.width, 1),
        Rect::new(inner.x, inner.y + 1, inner.width, inner.height - 1),
    );
    let modified = preview
        .modified
        .map(|time| {
            let dt: chrono::DateTime<chrono::Local> = time.into();
            dt.format("%H:%M").to_string()
        })
        .unwrap_or_else(|| "—".to_string());
    let mut info = format!(
        "{} 行 · {} · 改 {} · {}",
        preview.lines.len(),
        format_bytes(preview.size),
        modified,
        if preview.truncated {
            "仅显示前 5000 行"
        } else {
            "按 e 编辑"
        }
    );
    if !app.config.parse_errors.is_empty() {
        info.push_str(&format!(" · {} 个解析提示", app.config.parse_errors.len()));
    }
    frame.render_widget(
        Paragraph::new(Span::styled(info, Style::default().fg(theme.faint))),
        info_area,
    );

    if let Some(err) = &preview.error {
        frame.render_widget(
            Paragraph::new(Span::styled(err.clone(), Style::default().fg(theme.danger)))
                .wrap(Wrap { trim: true }),
            body_area,
        );
        return;
    }

    let height = body_area.height as usize;
    app.preview_viewport.set(height);
    let total = preview.lines.len();
    let max_scroll = total.saturating_sub(height);
    let scroll = app.preview_scroll.min(max_scroll);
    let end = (scroll + height).min(total);
    let mut lines = Vec::new();
    for (offset, number) in (scroll..end).enumerate() {
        let line_no = scroll + offset + 1;
        let text = &preview.lines[number];
        let mut spans = vec![Span::styled(
            format!("{line_no:>4} "),
            Style::default().fg(theme.faint),
        )];
        spans.extend(highlight_line(text, theme));
        lines.push(Line::from(spans));
    }
    frame.render_widget(Paragraph::new(lines), body_area);

    if total > height {
        let pct = ((scroll + height) * 100 / total).min(100);
        let hint = format!("{pct}%  Ctrl+D/U 或 J/K 滚动");
        frame.render_widget(
            Paragraph::new(Span::styled(hint, Style::default().fg(theme.faint))),
            Rect::new(
                body_area.x,
                body_area.y + body_area.height - 1,
                body_area.width,
                1,
            ),
        );
    }
}

fn highlight_line(text: &str, theme: &Theme) -> Vec<Span<'static>> {
    let trimmed = text.trim_start();
    let indent = &text[..text.len() - trimmed.len()];
    let mut spans = Vec::new();
    if !indent.is_empty() {
        spans.push(Span::raw(indent.to_string()));
    }
    if trimmed.is_empty() {
        return spans;
    }

    let mut in_quote = false;
    let mut escaped = false;
    let mut comment_at = None;
    for (idx, ch) in trimmed.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if in_quote && ch == '\\' {
            escaped = true;
            continue;
        }
        if ch == '"' {
            in_quote = !in_quote;
        } else if ch == '#' && !in_quote {
            comment_at = Some(idx);
            break;
        }
    }
    let (code, comment) = match comment_at {
        Some(idx) => (&trimmed[..idx], Some(&trimmed[idx..])),
        None => (trimmed, None),
    };
    if code.trim_start().starts_with('#') {
        spans.push(Span::styled(
            trimmed.to_string(),
            Style::default()
                .fg(theme.faint)
                .add_modifier(Modifier::ITALIC),
        ));
        return spans;
    }
    let code_trimmed = code.trim_end();
    let mut rest = code_trimmed;
    let directive_end = rest
        .find(|c: char| c.is_whitespace() || c == '{' || c == '}' || c == ';')
        .unwrap_or(rest.len());
    let directive = &rest[..directive_end];
    if !directive.is_empty() {
        spans.push(Span::styled(
            directive.to_string(),
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ));
        rest = &rest[directive_end..];
    }

    let mut in_quote = false;
    let mut buffer = String::new();
    for ch in rest.chars() {
        match ch {
            '"' => {
                if !buffer.is_empty() {
                    spans.push(Span::styled(
                        std::mem::take(&mut buffer),
                        Style::default().fg(if in_quote { theme.success } else { theme.muted }),
                    ));
                }
                spans.push(Span::styled("\"", Style::default().fg(theme.faint)));
                in_quote = !in_quote;
            }
            '{' | '}' => {
                if !buffer.is_empty() {
                    spans.push(Span::styled(
                        std::mem::take(&mut buffer),
                        Style::default().fg(if in_quote { theme.success } else { theme.muted }),
                    ));
                }
                spans.push(Span::styled(
                    ch.to_string(),
                    Style::default()
                        .fg(theme.warning)
                        .add_modifier(Modifier::BOLD),
                ));
            }
            '$' => {
                if !buffer.is_empty() {
                    spans.push(Span::styled(
                        std::mem::take(&mut buffer),
                        Style::default().fg(if in_quote { theme.success } else { theme.muted }),
                    ));
                }
                spans.push(Span::styled("$", Style::default().fg(theme.warning)));
            }
            _ => buffer.push(ch),
        }
    }
    if !buffer.is_empty() {
        spans.push(Span::styled(
            buffer,
            Style::default().fg(if in_quote { theme.success } else { theme.muted }),
        ));
    }
    if let Some(comment) = comment {
        spans.push(Span::styled(
            comment.to_string(),
            Style::default()
                .fg(theme.faint)
                .add_modifier(Modifier::ITALIC),
        ));
    }
    spans
}

#[allow(dead_code)]
fn _path(_: &Path) {}
