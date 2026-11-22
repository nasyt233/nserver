use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};
use unicode_width::UnicodeWidthStr;

use crate::app::{
    App, InputTarget, NewSiteField, NewSiteState, Overlay, SettingsField, SiteSettingsState,
    ToastKind,
};
use crate::ui::widgets::{center, center_percent, clear_popup, key_span, panel, truncate};

pub fn draw_toast(frame: &mut Frame, area: Rect, app: &App) {
    let Some(toast) = &app.toast else {
        return;
    };
    if area.width < 24 || area.height < 5 {
        return;
    }
    let width = 70.min(area.width.saturating_sub(20));
    let x = area.x + 1;
    let y = area.bottom().saturating_sub(1);
    let popup = Rect::new(x, y, width, 1);
    clear_popup(frame, popup);
    let color = match toast.kind {
        ToastKind::Info => app.theme.info,
        ToastKind::Success => app.theme.success,
        ToastKind::Warning => app.theme.warning,
        ToastKind::Danger => app.theme.danger,
    };
    let label = format!(" {} {} ", toast.kind.icon(), toast.kind.label());
    let label_width = UnicodeWidthStr::width(label.as_str());
    let text = truncate(
        &toast.text,
        width.saturating_sub(label_width as u16 + 1) as usize,
    );
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                label,
                Style::default()
                    .fg(app.theme.bg)
                    .bg(color)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(format!(" {text}"), Style::default().fg(color)),
        ]))
        .style(Style::default().bg(app.theme.footer_bg)),
        popup,
    );
}

pub fn draw_overlay(frame: &mut Frame, area: Rect, app: &App) {
    match &app.overlay {
        Overlay::None => {}
        Overlay::Help { scroll } => crate::ui::help::draw(frame, area, app, *scroll),
        Overlay::Confirm(confirm) => draw_confirm(frame, area, app, confirm),
        Overlay::Output(output) => draw_output(frame, area, app, output),
        Overlay::Input(input) => draw_input(frame, area, app, input),
        Overlay::NewSite(form) => draw_new_site(frame, area, app, form),
        Overlay::SiteSettings(state) => draw_site_settings(frame, area, app, state),
    }
}

fn draw_confirm(frame: &mut Frame, area: Rect, app: &App, confirm: &crate::app::ConfirmState) {
    let theme = &app.theme;
    let width = 62.min(area.width.saturating_sub(4));
    let lines = 2 + confirm.message.len().min(4) as u16;
    let height = (lines + 5).min(area.height.saturating_sub(2));
    let popup = center(area, width, height);
    clear_popup(frame, popup);
    let color = if confirm.danger {
        theme.danger
    } else {
        theme.warning
    };
    let block = panel(theme, format!("确认 · {}", confirm.title))
        .border_style(Style::default().fg(color))
        .title_style(Style::default().fg(color).add_modifier(Modifier::BOLD));
    let inner = block.inner(popup);
    frame.render_widget(block, popup);
    let rows = Layout::vertical([Constraint::Min(0), Constraint::Length(2)]).split(inner);

    let mut message = vec![Line::from("")];
    for line in &confirm.message {
        message.push(Line::from(Span::styled(
            line.clone(),
            Style::default().fg(theme.fg),
        )));
    }
    frame.render_widget(Paragraph::new(message).wrap(Wrap { trim: true }), rows[0]);
    let hint = Line::from(vec![
        key_span(theme, "Enter"),
        Span::styled(" / ", Style::default().fg(theme.faint)),
        key_span(theme, "y"),
        Span::styled(" 确认    ", Style::default().fg(theme.muted)),
        key_span(theme, "Esc"),
        Span::styled(" / ", Style::default().fg(theme.faint)),
        key_span(theme, "n"),
        Span::styled(" 取消", Style::default().fg(theme.muted)),
    ]);
    frame.render_widget(Paragraph::new(hint).alignment(Alignment::Center), rows[1]);
}

fn draw_output(frame: &mut Frame, area: Rect, app: &App, output: &crate::app::OutputState) {
    let theme = &app.theme;
    let popup = center_percent(area, 86, 82);
    clear_popup(frame, popup);
    let color = if output.success {
        theme.success
    } else {
        theme.danger
    };
    let title = format!(
        "{} {}",
        if output.success { "✓" } else { "×" },
        output.title
    );
    let block = panel(theme, title)
        .border_style(Style::default().fg(color))
        .title_style(Style::default().fg(color).add_modifier(Modifier::BOLD));
    let inner = block.inner(popup);
    frame.render_widget(block, popup);
    if inner.height < 2 {
        return;
    }
    let body_area = Rect::new(inner.x, inner.y, inner.width, inner.height - 1);
    let footer_area = Rect::new(inner.x, inner.y + inner.height - 1, inner.width, 1);
    let height = body_area.height as usize;
    app.output_viewport.set(height);
    let max_scroll = output.lines.len().saturating_sub(height);
    let scroll = output.scroll.min(max_scroll);
    let end = (scroll + height).min(output.lines.len());

    let mut lines = Vec::new();
    for line in &output.lines[scroll..end] {
        let style = if line.starts_with("$ ") {
            Style::default().fg(theme.faint)
        } else if line.contains("成功") {
            Style::default().fg(theme.success)
        } else if line.contains("失败") || line.contains("错误") {
            Style::default().fg(theme.danger)
        } else if line.starts_with("──") {
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.fg)
        };
        lines.push(Line::from(Span::styled(line.clone(), style)));
    }
    if lines.is_empty() {
        lines.push(Line::from(Span::styled(
            "没有输出。",
            Style::default().fg(theme.faint),
        )));
    }
    frame.render_widget(Paragraph::new(lines), body_area);

    let footer = format!(
        "Esc / Enter 关闭  ·  ↑↓ 滚动  ·  PgUp/PgDn 翻页  ·  {} 行",
        output.lines.len()
    );
    frame.render_widget(
        Paragraph::new(Span::styled(
            truncate(&footer, footer_area.width as usize),
            Style::default().fg(theme.faint),
        ))
        .alignment(Alignment::Right),
        footer_area,
    );
}

fn draw_input(frame: &mut Frame, area: Rect, app: &App, input: &crate::app::InputState) {
    let theme = &app.theme;
    let popup = center(area, 62.min(area.width.saturating_sub(4)), 7);
    clear_popup(frame, popup);
    let block = panel(theme, &input.title).border_style(Style::default().fg(theme.accent));
    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    let rows = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(0),
        Constraint::Length(1),
    ])
    .split(inner);
    let hint = match input.target {
        InputTarget::Sites | InputTarget::Config | InputTarget::Logs => {
            "输入关键词，留空表示清除筛选。"
        }
        InputTarget::FilePath => "输入绝对路径，或相对当前目录的路径。",
        InputTarget::NewDir => "输入新目录名称（不能包含 / 或 ..）。",
        InputTarget::NewFile => "输入新文件名称（不能包含 / 或 ..）。",
        InputTarget::RenameFile => "输入新名称，Ctrl+U 清空。",
        InputTarget::SslDomain => "输入域名，例如 example.com；会在配置目录 ssl/ 生成证书。",
    };
    frame.render_widget(
        Paragraph::new(Span::styled(hint, Style::default().fg(theme.faint))),
        rows[0],
    );
    let available = rows[1].width.saturating_sub(2) as usize;
    let value = &input.value;
    let display = if UnicodeWidthStr::width(value.as_str()) > available {
        tail_chars(value, available)
    } else {
        value.clone()
    };
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("› ", Style::default().fg(theme.accent)),
            Span::styled(display.clone(), Style::default().fg(theme.fg)),
        ])),
        rows[1],
    );
    let cursor_x = rows[1].x + 2 + UnicodeWidthStr::width(display.as_str()) as u16;
    frame.set_cursor_position((cursor_x.min(rows[1].right().saturating_sub(1)), rows[1].y));

    frame.render_widget(
        Paragraph::new(Line::from(vec![
            key_span(theme, "Enter"),
            Span::styled(" 应用   ", Style::default().fg(theme.faint)),
            key_span(theme, "Esc"),
            Span::styled(" 取消", Style::default().fg(theme.faint)),
        ])),
        rows[3],
    );
}

fn draw_new_site(frame: &mut Frame, area: Rect, app: &App, form: &NewSiteState) {
    let theme = &app.theme;
    let width = 74.min(area.width.saturating_sub(4));
    let height = 18.min(area.height.saturating_sub(4));
    let popup = center(area, width, height);
    clear_popup(frame, popup);
    let block = panel(theme, "新建网站").border_style(Style::default().fg(theme.accent));
    let inner = block.inner(popup);
    frame.render_widget(block, popup);
    if inner.height < 8 {
        return;
    }
    let error_area = Rect::new(inner.x, inner.y + inner.height - 2, inner.width, 1);
    let footer_area = Rect::new(inner.x, inner.y + inner.height - 1, inner.width, 1);
    let body = Rect::new(
        inner.x,
        inner.y,
        inner.width,
        inner.height.saturating_sub(2),
    );

    let fields = form.visible_fields();
    let mut lines = vec![Line::from(Span::styled(
        "选择网站类型，填写后 Ctrl+S 创建；创建后可随时修改。",
        Style::default().fg(theme.faint),
    ))];
    let label_width = 11usize;
    for (index, field) in fields.iter().enumerate() {
        let focused = form.focus == index;
        let mut value = form.field_text(*field);
        if *field == NewSiteField::Kind {
            value = format!("‹ {value} ›");
        }
        let enabled = match field {
            NewSiteField::Websocket => form.spec.websocket,
            NewSiteField::Spa => form.spec.spa,
            NewSiteField::Enable => form.spec.enable,
            _ => false,
        };
        lines.push(form_row(
            theme,
            field.label(),
            &value,
            focused,
            field.is_toggle(),
            enabled,
            label_width,
        ));
    }
    frame.render_widget(Paragraph::new(lines), body);

    if let Some(error) = &form.error {
        frame.render_widget(
            Paragraph::new(Span::styled(
                truncate(&format!("! {error}"), error_area.width as usize),
                Style::default().fg(theme.danger),
            )),
            error_area,
        );
    }
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            key_span(theme, "Tab"),
            Span::styled(" 切换  ", Style::default().fg(theme.faint)),
            key_span(theme, "←→"),
            Span::styled(" 类型  ", Style::default().fg(theme.faint)),
            key_span(theme, "Ctrl+S"),
            Span::styled(" 创建  ", Style::default().fg(theme.faint)),
            key_span(theme, "Esc"),
            Span::styled(" 取消", Style::default().fg(theme.faint)),
        ])),
        footer_area,
    );

    if let Some((line, field)) = fields.iter().enumerate().find(|(index, field)| {
        *index == form.focus && **field != NewSiteField::Kind && !field.is_toggle()
    }) {
        let display = truncate(
            &form.field_text(*field),
            inner.width.saturating_sub(label_width as u16 + 4) as usize,
        );
        let y = body.y + 1 + line as u16;
        if y < error_area.y {
            let x = inner.x
                + 2
                + label_width as u16
                + 1
                + UnicodeWidthStr::width(display.as_str()) as u16;
            frame.set_cursor_position((x.min(inner.right().saturating_sub(2)), y));
        }
    }
}

fn draw_site_settings(frame: &mut Frame, area: Rect, app: &App, state: &SiteSettingsState) {
    let theme = &app.theme;
    let width = 74.min(area.width.saturating_sub(4));
    let height = 16.min(area.height.saturating_sub(4));
    let popup = center(area, width, height);
    clear_popup(frame, popup);
    let block = panel(
        theme,
        format!(
            "网站设置 · {}",
            truncate(&state.site_name, width.saturating_sub(20) as usize)
        ),
    )
    .border_style(Style::default().fg(theme.accent));
    let inner = block.inner(popup);
    frame.render_widget(block, popup);
    if inner.height < 8 {
        return;
    }
    let error_area = Rect::new(inner.x, inner.y + inner.height - 2, inner.width, 1);
    let footer_area = Rect::new(inner.x, inner.y + inner.height - 1, inner.width, 1);
    let body = Rect::new(
        inner.x,
        inner.y,
        inner.width,
        inner.height.saturating_sub(2),
    );

    let fields = state.visible_fields();
    let mut lines = vec![Line::from(Span::styled(
        format!(
            "{} · 只改第一个 server 块；高级指令用 e。",
            state.kind.label()
        ),
        Style::default().fg(theme.faint),
    ))];
    let label_width = 11usize;
    for (index, field) in fields.iter().enumerate() {
        let focused = state.focus == index;
        let value = state.field_text(*field);
        let enabled = match field {
            SettingsField::Websocket => state.websocket,
            SettingsField::Spa => state.spa,
            _ => false,
        };
        lines.push(form_row(
            theme,
            field.label(),
            &value,
            focused,
            field.is_toggle(),
            enabled,
            label_width,
        ));
    }
    if state.kind == crate::sites::SiteKind::PortForward {
        lines.push(Line::from(Span::styled(
            "提示：端口转发通常把域名设为 _，只监听端口。",
            Style::default().fg(theme.faint),
        )));
    }
    frame.render_widget(Paragraph::new(lines), body);

    if let Some(error) = &state.error {
        frame.render_widget(
            Paragraph::new(Span::styled(
                truncate(&format!("! {error}"), error_area.width as usize),
                Style::default().fg(theme.danger),
            )),
            error_area,
        );
    }
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            key_span(theme, "Tab"),
            Span::styled(" 切换  ", Style::default().fg(theme.faint)),
            key_span(theme, "Space"),
            Span::styled(" 开关  ", Style::default().fg(theme.faint)),
            key_span(theme, "Ctrl+S"),
            Span::styled(" 保存  ", Style::default().fg(theme.faint)),
            key_span(theme, "Esc"),
            Span::styled(" 取消", Style::default().fg(theme.faint)),
        ])),
        footer_area,
    );

    if let Some((line, field)) = fields
        .iter()
        .enumerate()
        .find(|(index, field)| *index == state.focus && !field.is_toggle())
    {
        let display = truncate(
            &state.field_text(*field),
            inner.width.saturating_sub(label_width as u16 + 4) as usize,
        );
        let y = body.y + 1 + line as u16;
        if y < error_area.y {
            let x = inner.x
                + 2
                + label_width as u16
                + 1
                + UnicodeWidthStr::width(display.as_str()) as u16;
            frame.set_cursor_position((x.min(inner.right().saturating_sub(2)), y));
        }
    }
}

fn form_row(
    theme: &crate::theme::Theme,
    label: &str,
    value: &str,
    focused: bool,
    is_toggle: bool,
    enabled: bool,
    label_width: usize,
) -> Line<'static> {
    let prefix = if focused { "› " } else { "  " };
    let label_style = if focused {
        Style::default()
            .fg(theme.accent)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme.muted)
    };
    let mut value_style = if focused {
        Style::default()
            .bg(theme.selection_bg)
            .fg(theme.selection_fg)
            .add_modifier(Modifier::BOLD)
    } else if is_toggle {
        Style::default().fg(if enabled { theme.success } else { theme.faint })
    } else {
        Style::default().fg(theme.fg)
    };
    if focused && is_toggle {
        value_style = value_style.bg(theme.selection_bg);
    }
    let mut line = Line::from(vec![
        Span::styled(prefix, Style::default().fg(theme.accent)),
        Span::styled(
            crate::ui::widgets::pad_display(label, label_width),
            label_style,
        ),
        Span::styled(" ", Style::default()),
        Span::styled(value.to_string(), value_style),
    ]);
    if focused {
        line = line.style(Style::default().bg(theme.selection_bg));
    }
    line
}

fn tail_chars(value: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    let chars: Vec<char> = value.chars().collect();
    let mut out = String::new();
    let mut used = 0;
    for ch in chars.into_iter().rev() {
        let w = UnicodeWidthStr::width(ch.to_string().as_str());
        if used + w > width {
            break;
        }
        out.insert(0, ch);
        used += w;
    }
    out
}
