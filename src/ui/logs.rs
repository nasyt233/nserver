use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};

use crate::app::App;
use crate::config::LogKind;
use crate::logs::{access_status, error_severity, format_bytes};
use crate::theme::Theme;
use crate::ui::widgets::{empty_state, panel_focused, truncate};

pub fn draw(frame: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;
    let Some(source) = app.current_log_source() else {
        empty_state(
            frame,
            area,
            theme,
            "日志",
            if app.logs.sources.is_empty() {
                "配置中没有找到 access_log / error_log。可在配置中添加日志路径。"
            } else {
                "当前类型筛选下没有日志源。按 L 切换类型。"
            },
        );
        return;
    };

    let count = app.logs.sources.len();
    let title = format!(
        "日志 · {}  [{}/{}]",
        source.kind.label(),
        app.logs.index + 1,
        count
    );
    let block = panel_focused(theme, title, true);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.height < 2 {
        return;
    }

    let rows = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(0),
        Constraint::Length(1),
    ])
    .split(inner);
    draw_header(frame, rows[0], app, source, theme);
    draw_body(frame, rows[1], app, source.kind, theme);
    draw_footer(frame, rows[2], app, theme);
}

fn draw_header(
    frame: &mut Frame,
    area: Rect,
    app: &App,
    source: &crate::config::LogSource,
    theme: &Theme,
) {
    let (kind_text, kind_color) = match source.kind {
        LogKind::Access => ("ACCESS", theme.info),
        LogKind::Error => ("ERROR", theme.danger),
        LogKind::System => ("SYSTEM", theme.accent2),
    };
    let mut spans = vec![
        Span::styled(
            format!(" {kind_text} "),
            Style::default()
                .fg(theme.bg)
                .bg(kind_color)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(" {} ", source.label),
            Style::default().fg(theme.fg).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            truncate(&source.path.display().to_string(), 34),
            Style::default().fg(theme.faint),
        ),
    ];
    if let Some(snapshot) = &app.logs.snapshot {
        spans.push(Span::styled(
            format!("  {}", format_bytes(snapshot.total_bytes)),
            Style::default().fg(theme.muted),
        ));
    }
    if app.logs.auto {
        spans.push(Span::styled("  自动", Style::default().fg(theme.success)));
    } else {
        spans.push(Span::styled("  暂停", Style::default().fg(theme.warning)));
    }
    if app.logs.follow {
        spans.push(Span::styled("  跟随", Style::default().fg(theme.info)));
    }
    if !app.logs.filter.is_empty() {
        spans.push(Span::styled(
            format!("  /{}", truncate(&app.logs.filter, 16)),
            Style::default().fg(theme.accent2),
        ));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn draw_body(frame: &mut Frame, area: Rect, app: &App, kind: LogKind, theme: &Theme) {
    if let Some(err) = &app.logs.error {
        frame.render_widget(
            Paragraph::new(vec![
                Line::from(""),
                Line::from(Span::styled(
                    "无法读取日志",
                    Style::default().fg(theme.danger),
                )),
                Line::from(""),
                Line::from(Span::styled(err.clone(), Style::default().fg(theme.muted))),
                Line::from(""),
                Line::from(Span::styled(
                    "[ / ] 切换日志源，L 切换类型，u 重试",
                    Style::default().fg(theme.faint),
                )),
            ])
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: true }),
            area,
        );
        return;
    }
    let lines = app.log_lines();
    if lines.is_empty() {
        frame.render_widget(
            Paragraph::new(Span::styled(
                if app.logs.filter.is_empty() {
                    "日志为空。"
                } else {
                    "没有匹配筛选条件的日志。按 Esc 清除。"
                },
                Style::default().fg(theme.faint),
            ))
            .alignment(Alignment::Center),
            area,
        );
        return;
    }
    let viewport = area.height as usize;
    app.log_viewport.set(viewport);
    let max_scroll = lines.len().saturating_sub(viewport);
    let scroll = app.logs.scroll_from_bottom.min(max_scroll);
    let start = max_scroll.saturating_sub(scroll);
    let end = (start + viewport).min(lines.len());

    let mut rendered = Vec::new();
    for line in &lines[start..end] {
        let shown = if app.logs.hscroll == 0 {
            (*line).to_string()
        } else {
            line.chars().skip(app.logs.hscroll).collect()
        };
        rendered.push(log_line(&shown, kind, theme));
    }
    frame.render_widget(Paragraph::new(rendered), area);
}

fn log_line(text: &str, kind: LogKind, theme: &Theme) -> Line<'static> {
    let (prefix, color) = match kind {
        LogKind::Access => match access_status(text) {
            Some(code) if code >= 500 => ("×", theme.danger),
            Some(code) if code >= 400 => ("!", theme.warning),
            Some(code) if code >= 300 => ("→", theme.info),
            Some(_) => ("·", theme.success),
            None => ("·", theme.muted),
        },
        LogKind::Error => match error_severity(text) {
            Some("emerg" | "alert" | "crit" | "error") => ("×", theme.danger),
            Some("warn") => ("!", theme.warning),
            Some("notice") => ("i", theme.info),
            Some(_) => ("·", theme.muted),
            None => ("·", theme.faint),
        },
        LogKind::System => match error_severity(text) {
            Some("err" | "error" | "crit" | "emerg") => ("×", theme.danger),
            Some("warn") => ("!", theme.warning),
            _ => ("·", theme.muted),
        },
    };
    Line::from(vec![
        Span::styled(format!("{prefix} "), Style::default().fg(color)),
        Span::styled(
            text.to_string(),
            Style::default().fg(if color == theme.danger {
                theme.fg
            } else {
                theme.muted
            }),
        ),
    ])
}

fn draw_footer(frame: &mut Frame, area: Rect, app: &App, theme: &Theme) {
    let total = app
        .logs
        .snapshot
        .as_ref()
        .map(|s| s.lines.len())
        .unwrap_or(0);
    let shown = app.log_lines().len();
    let mut text = format!("共 {total} 行 · 显示 {shown} 行");
    if app.logs.hscroll > 0 {
        text.push_str(&format!(" · 横向 {}", app.logs.hscroll));
    }
    if app.logs.filter.is_empty() {
        text.push_str(" · [ ] 换源 · L 类型 · f 跟随 · a 自动 · u 刷新");
    } else {
        text.push_str(" · Esc 清除筛选");
    }
    frame.render_widget(
        Paragraph::new(Span::styled(
            truncate(&text, area.width as usize),
            Style::default().fg(theme.faint),
        )),
        area,
    );
}
