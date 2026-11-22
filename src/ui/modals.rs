use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};
use unicode_width::UnicodeWidthStr;

use crate::app::{
    App, InputTarget, MouseAction, NewSiteField, NewSiteState, Overlay, PickerMode, PickerState,
    SettingsField, SiteSettingsState, ToastKind,
};
use crate::ui::widgets::{
    center, center_percent, clear_popup, key_span, panel, panel_focused, truncate,
};

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
    if !matches!(app.overlay, Overlay::None) {
        app.register_hit(
            area,
            MouseAction::Key(
                crossterm::event::KeyCode::Esc,
                crossterm::event::KeyModifiers::NONE,
            ),
        );
    }
    match &app.overlay {
        Overlay::None => {}
        Overlay::Help { scroll } => crate::ui::help::draw(frame, area, app, *scroll),
        Overlay::Confirm(confirm) => draw_confirm(frame, area, app, confirm),
        Overlay::Output(output) => draw_output(frame, area, app, output),
        Overlay::Input(input) => draw_input(frame, area, app, input),
        Overlay::NewSite(form) => draw_new_site(frame, area, app, form),
        Overlay::SiteSettings(state) => draw_site_settings(frame, area, app, state),
        Overlay::Picker(picker) => draw_picker(frame, area, app, picker),
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
    let half = rows[1].width / 2;
    app.register_hit(
        Rect::new(rows[1].x, rows[1].y, half, 1),
        MouseAction::Key(
            crossterm::event::KeyCode::Enter,
            crossterm::event::KeyModifiers::NONE,
        ),
    );
    app.register_hit(
        Rect::new(rows[1].x + half, rows[1].y, rows[1].width - half, 1),
        MouseAction::Key(
            crossterm::event::KeyCode::Esc,
            crossterm::event::KeyModifiers::NONE,
        ),
    );
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
    app.register_hit(
        footer_area,
        MouseAction::Key(
            crossterm::event::KeyCode::Esc,
            crossterm::event::KeyModifiers::NONE,
        ),
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
        InputTarget::FtpNewDir => "输入 FTP 目录名称，不能包含 / 或 ..。",
        InputTarget::FtpNewFile => "输入 FTP 文件名称，不能包含 / 或 ..。",
        InputTarget::FtpRename => "输入 FTP 项目的新名称。",
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
    let half = rows[3].width / 2;
    app.register_hit(
        Rect::new(rows[3].x, rows[3].y, half, 1),
        MouseAction::Key(
            crossterm::event::KeyCode::Enter,
            crossterm::event::KeyModifiers::NONE,
        ),
    );
    app.register_hit(
        Rect::new(rows[3].x + half, rows[3].y, rows[3].width - half, 1),
        MouseAction::Key(
            crossterm::event::KeyCode::Esc,
            crossterm::event::KeyModifiers::NONE,
        ),
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
            key_span(theme, "b"),
            Span::styled(" 浏览  ", Style::default().fg(theme.faint)),
            key_span(theme, "Ctrl+S"),
            Span::styled(" 创建  ", Style::default().fg(theme.faint)),
            key_span(theme, "Esc"),
            Span::styled(" 取消", Style::default().fg(theme.faint)),
        ])),
        footer_area,
    );
    for index in 0..fields.len() {
        let y = body.y.saturating_add(1 + index as u16);
        if y < error_area.y {
            app.register_hit(
                Rect::new(inner.x, y, inner.width, 1),
                MouseAction::NewSiteField(index),
            );
        }
    }
    let third = footer_area.width / 3;
    app.register_hit(
        Rect::new(footer_area.x, footer_area.y, third, 1),
        MouseAction::Key(
            crossterm::event::KeyCode::Char('b'),
            crossterm::event::KeyModifiers::NONE,
        ),
    );
    app.register_hit(
        Rect::new(footer_area.x + third, footer_area.y, third, 1),
        MouseAction::Key(
            crossterm::event::KeyCode::Char('s'),
            crossterm::event::KeyModifiers::CONTROL,
        ),
    );
    app.register_hit(
        Rect::new(
            footer_area.x + third * 2,
            footer_area.y,
            footer_area.width - third * 2,
            1,
        ),
        MouseAction::Key(
            crossterm::event::KeyCode::Esc,
            crossterm::event::KeyModifiers::NONE,
        ),
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
            key_span(theme, "b"),
            Span::styled(" 浏览  ", Style::default().fg(theme.faint)),
            key_span(theme, "Ctrl+S"),
            Span::styled(" 保存  ", Style::default().fg(theme.faint)),
            key_span(theme, "Esc"),
            Span::styled(" 取消", Style::default().fg(theme.faint)),
        ])),
        footer_area,
    );
    for index in 0..fields.len() {
        let y = body.y.saturating_add(1 + index as u16);
        if y < error_area.y {
            app.register_hit(
                Rect::new(inner.x, y, inner.width, 1),
                MouseAction::SiteSettingsField(index),
            );
        }
    }
    let third = footer_area.width / 3;
    app.register_hit(
        Rect::new(footer_area.x, footer_area.y, third, 1),
        MouseAction::Key(
            crossterm::event::KeyCode::Char('b'),
            crossterm::event::KeyModifiers::NONE,
        ),
    );
    app.register_hit(
        Rect::new(footer_area.x + third, footer_area.y, third, 1),
        MouseAction::Key(
            crossterm::event::KeyCode::Char('s'),
            crossterm::event::KeyModifiers::CONTROL,
        ),
    );
    app.register_hit(
        Rect::new(
            footer_area.x + third * 2,
            footer_area.y,
            footer_area.width - third * 2,
            1,
        ),
        MouseAction::Key(
            crossterm::event::KeyCode::Esc,
            crossterm::event::KeyModifiers::NONE,
        ),
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

fn draw_picker(frame: &mut Frame, area: Rect, app: &App, picker: &PickerState) {
    let theme = &app.theme;
    let width = 88.min(area.width.saturating_sub(4)).max(60);
    let height = 26.min(area.height.saturating_sub(2)).max(12);
    let popup = center(area, width, height);
    clear_popup(frame, popup);
    let title = match picker.mode {
        PickerMode::Directory => format!("{} · 网站目录", picker.mode.label()),
        PickerMode::File => format!("{} · 文件浏览器", picker.mode.label()),
    };
    let block = panel(theme, title).border_style(Style::default().fg(theme.accent));
    let inner = block.inner(popup);
    frame.render_widget(block, popup);
    if inner.height < 6 {
        return;
    }
    let rows = Layout::vertical([
        Constraint::Length(2),
        Constraint::Min(5),
        Constraint::Length(2),
    ])
    .split(inner);
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("▤ ", Style::default().fg(theme.accent)),
            Span::styled(
                truncate(
                    &picker.browser.cwd.display().to_string(),
                    rows[0].width.saturating_sub(2) as usize,
                ),
                Style::default().fg(theme.fg).add_modifier(Modifier::BOLD),
            ),
        ])),
        rows[0],
    );
    let cols =
        Layout::horizontal([Constraint::Percentage(48), Constraint::Percentage(52)]).split(rows[1]);
    let list_block = panel_focused(theme, "目录", true);
    let list_inner = list_block.inner(cols[0]);
    frame.render_widget(list_block, cols[0]);
    let visible = list_inner.height as usize;
    let start = picker.browser.selected.saturating_sub(visible / 2);
    let end = (start + visible.max(1)).min(picker.browser.entries.len());
    let mut lines = Vec::new();
    for (index, entry) in picker
        .browser
        .entries
        .iter()
        .enumerate()
        .take(end)
        .skip(start)
    {
        let selected = index == picker.browser.selected;
        let style = if selected {
            Style::default()
                .bg(theme.selection_bg)
                .fg(theme.selection_fg)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.fg)
        };
        lines.push(Line::from(vec![
            Span::styled(
                if entry.is_dir { "▸ " } else { "· " },
                Style::default().fg(if entry.is_dir {
                    theme.info
                } else {
                    theme.faint
                }),
            ),
            Span::styled(
                truncate(&entry.name, list_inner.width.saturating_sub(3) as usize),
                style,
            ),
        ]));
    }
    if lines.is_empty() {
        lines.push(Line::from(Span::styled(
            "目录为空",
            Style::default().fg(theme.faint),
        )));
    }
    frame.render_widget(Paragraph::new(lines), list_inner);
    for index in start..end {
        app.register_hit(
            Rect::new(
                list_inner.x,
                list_inner.y + (index - start) as u16,
                list_inner.width,
                1,
            ),
            MouseAction::PickerRow(index),
        );
    }

    let preview_block = panel_focused(theme, "预览", false);
    let preview_inner = preview_block.inner(cols[1]);
    frame.render_widget(preview_block, cols[1]);
    let preview = &picker.browser.preview;
    let mut preview_lines = Vec::new();
    for line in preview.lines.iter().take(preview_inner.height as usize) {
        preview_lines.push(Line::from(Span::styled(
            line.clone(),
            Style::default().fg(theme.muted),
        )));
    }
    if preview_lines.is_empty() {
        preview_lines.push(Line::from(Span::styled(
            if let Some(err) = &preview.error {
                err.clone()
            } else {
                "选择一个文件或目录".into()
            },
            Style::default().fg(theme.faint),
        )));
    }
    frame.render_widget(Paragraph::new(preview_lines), preview_inner);

    let hint = if picker.mode == PickerMode::Directory {
        "Enter 进入目录 · Ctrl+S 选择当前目录 · Backspace 上级 · Esc 取消"
    } else {
        "Enter 选择文件/进入目录 · Ctrl+S 选择文件 · Backspace 上级 · Esc 取消"
    };
    frame.render_widget(
        Paragraph::new(Line::from(vec![Span::styled(
            truncate(hint, rows[2].width as usize),
            Style::default().fg(theme.muted),
        )])),
        rows[2],
    );
    if let Some(error) = &picker.error {
        frame.render_widget(
            Paragraph::new(Span::styled(
                truncate(error, rows[2].width as usize),
                Style::default().fg(theme.danger),
            )),
            Rect::new(rows[2].x, rows[2].y, rows[2].width, 1),
        );
    }
    let third = rows[2].width / 3;
    app.register_hit(
        Rect::new(rows[2].x, rows[2].y, third, 1),
        MouseAction::Key(
            crossterm::event::KeyCode::Char('s'),
            crossterm::event::KeyModifiers::CONTROL,
        ),
    );
    app.register_hit(
        Rect::new(rows[2].x + third, rows[2].y, third, 1),
        MouseAction::Key(
            crossterm::event::KeyCode::Enter,
            crossterm::event::KeyModifiers::NONE,
        ),
    );
    app.register_hit(
        Rect::new(
            rows[2].x + third * 2,
            rows[2].y,
            rows[2].width - third * 2,
            1,
        ),
        MouseAction::Key(
            crossterm::event::KeyCode::Esc,
            crossterm::event::KeyModifiers::NONE,
        ),
    );
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
