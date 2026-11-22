use ratatui::Frame;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Clear, Paragraph, Wrap};
use unicode_width::UnicodeWidthStr;

use crate::theme::Theme;

pub fn fill(frame: &mut Frame, area: Rect, theme: &Theme) {
    frame.render_widget(Block::default().style(Style::default().bg(theme.bg)), area);
}

pub fn panel(theme: &Theme, title: impl Into<String>) -> Block<'static> {
    Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.border))
        .style(Style::default().bg(theme.panel_bg).fg(theme.fg))
        .title(Span::styled(
            format!(" {} ", title.into()),
            Style::default()
                .fg(theme.accent)
                .bg(theme.panel_bg)
                .add_modifier(Modifier::BOLD),
        ))
}

pub fn panel_focused(theme: &Theme, title: impl Into<String>, focused: bool) -> Block<'static> {
    let color = if focused { theme.accent } else { theme.border };
    Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(color))
        .style(Style::default().bg(theme.panel_bg).fg(theme.fg))
        .title(Span::styled(
            format!(" {} ", title.into()),
            Style::default()
                .fg(if focused { theme.accent } else { theme.muted })
                .bg(theme.panel_bg)
                .add_modifier(Modifier::BOLD),
        ))
}

pub fn key_span(theme: &Theme, key: &str) -> Span<'static> {
    Span::styled(
        format!(" {key} "),
        Style::default()
            .fg(theme.bg)
            .bg(theme.accent)
            .add_modifier(Modifier::BOLD),
    )
}

pub fn hint_line(theme: &Theme, hints: &[(&str, &str)]) -> Line<'static> {
    let mut spans = Vec::new();
    for (idx, (key, label)) in hints.iter().enumerate() {
        if idx > 0 {
            spans.push(Span::raw(" "));
        }
        spans.push(key_span(theme, key));
        spans.push(Span::styled(
            format!(" {label} "),
            Style::default().fg(theme.muted),
        ));
    }
    Line::from(spans)
}

pub fn badge(theme: &Theme, text: &str, fg: ratatui::style::Color) -> Span<'static> {
    Span::styled(
        format!(" {text} "),
        Style::default()
            .fg(theme.bg)
            .bg(fg)
            .add_modifier(Modifier::BOLD),
    )
}

pub fn center(area: Rect, width: u16, height: u16) -> Rect {
    let width = width.min(area.width.saturating_sub(2));
    let height = height.min(area.height.saturating_sub(2));
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    Rect::new(x, y, width, height)
}

pub fn center_percent(area: Rect, percent_x: u16, percent_y: u16) -> Rect {
    let width = area.width * percent_x / 100;
    let height = area.height * percent_y / 100;
    center(area, width, height)
}

pub fn truncate(text: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    if UnicodeWidthStr::width(text) <= width {
        return text.to_string();
    }
    let mut out = String::new();
    let mut used = 0;
    for ch in text.chars() {
        let w = UnicodeWidthStr::width(ch.to_string().as_str());
        if used + w + 1 > width {
            break;
        }
        out.push(ch);
        used += w;
    }
    out.push('…');
    out
}

pub fn empty_state(frame: &mut Frame, area: Rect, theme: &Theme, title: &str, message: &str) {
    let block = panel(theme, title);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let text = vec![
        Line::from(""),
        Line::from(Span::styled(
            "◇",
            Style::default()
                .fg(theme.faint)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            message.to_string(),
            Style::default().fg(theme.muted),
        )),
    ];
    frame.render_widget(
        Paragraph::new(text)
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: true }),
        inner,
    );
}

pub fn clear_popup(frame: &mut Frame, area: Rect) {
    frame.render_widget(Clear, area);
}

/// Pad using terminal display width, so Chinese labels line up in columns.
pub fn pad_display(text: &str, width: usize) -> String {
    let current = UnicodeWidthStr::width(text);
    if current >= width {
        text.to_string()
    } else {
        format!("{text}{}", " ".repeat(width - current))
    }
}

/// Keep only the hints that fit, dropping lower-priority items from the end.
pub fn hint_line_fit(theme: &Theme, hints: &[(&str, &str)], max_width: usize) -> Line<'static> {
    let mut selected = Vec::new();
    let mut used = 0usize;
    for (key, label) in hints {
        let width = 2 + UnicodeWidthStr::width(*key) + 2 + UnicodeWidthStr::width(*label) + 1;
        if !selected.is_empty() && used + width > max_width {
            break;
        }
        used += width;
        selected.push((*key, *label));
    }
    hint_line(theme, &selected)
}

pub fn format_age(instant: std::time::Instant) -> String {
    let secs = instant.elapsed().as_secs();
    if secs < 5 {
        "刚刚".into()
    } else if secs < 60 {
        format!("{secs}秒前")
    } else if secs < 3600 {
        format!("{}分前", secs / 60)
    } else {
        format!("{}时前", secs / 3600)
    }
}
