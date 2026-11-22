use crate::app::{App, ServiceWizard};
use crate::ui::widgets::{center, clear_popup, panel, truncate};
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};

pub fn draw_service_wizard(frame: &mut Frame, area: Rect, app: &App, wizard: &ServiceWizard) {
    let width = 82.min(area.width.saturating_sub(4));
    let height = 16.min(area.height.saturating_sub(4));
    let popup = center(area, width, height);
    clear_popup(frame, popup);
    let block =
        panel(&app.theme, "创建 systemd 服务").border_style(Style::default().fg(app.theme.accent));
    let inner = block.inner(popup);
    frame.render_widget(block, popup);
    let rows = Layout::vertical([Constraint::Min(0), Constraint::Length(2)]).split(inner);
    let values = [
        &wizard.spec.name,
        &wizard.spec.description,
        &wizard.spec.exec_start,
        &wizard.spec.working_directory,
        &wizard.spec.user,
        &wizard.spec.environment,
    ];
    let mut lines = vec![Line::from(Span::styled(
        "填写字段，Enter/Tab 下一项，最后一项 Enter 预览并确认。",
        Style::default().fg(app.theme.faint),
    ))];
    for (index, (label, value)) in ServiceWizard::fields()
        .iter()
        .zip(values.iter())
        .enumerate()
    {
        let style = if index == wizard.focus {
            Style::default()
                .fg(app.theme.selection_fg)
                .bg(app.theme.selection_bg)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(app.theme.fg)
        };
        lines.push(Line::from(vec![
            Span::styled(format!("{label:<8}"), Style::default().fg(app.theme.faint)),
            Span::styled(
                truncate(value, rows[0].width.saturating_sub(12) as usize),
                style,
            ),
        ]));
    }
    if let Some(error) = &wizard.error {
        lines.push(Line::from(Span::styled(
            format!("! {error}"),
            Style::default().fg(app.theme.danger),
        )));
    }
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), rows[0]);
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            "Enter 确认/下一项 · Tab 切换 · Esc 取消",
            Style::default().fg(app.theme.muted),
        ))),
        rows[1],
    );
    let y = rows[0].y + 1 + wizard.focus as u16;
    if y < rows[0].bottom() {
        frame.set_cursor_position((
            rows[0].x
                + 10
                + ratatui::prelude::text::Line::from(values[wizard.focus].as_str()).width() as u16,
            y,
        ));
    }
}
