use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Cell, Paragraph, Row, Table};

use crate::app::App;
use crate::ui::widgets::{panel, truncate};

pub fn draw(frame: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;
    let layout = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(5),
        Constraint::Length(1),
    ])
    .split(area);

    let path_block = panel(theme, "文件管理");
    let inner = path_block.inner(layout[0]);
    frame.render_widget(path_block, layout[0]);
    let hidden = if app.files.show_hidden {
        "显示隐藏"
    } else {
        "隐藏隐藏项"
    };
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("▤ ", Style::default().fg(theme.accent)),
            Span::styled(
                truncate(
                    &app.files.cwd.display().to_string(),
                    inner.width.saturating_sub(18) as usize,
                ),
                Style::default().fg(theme.fg).add_modifier(Modifier::BOLD),
            ),
            Span::styled(format!("   {hidden}"), Style::default().fg(theme.faint)),
        ])),
        inner,
    );

    let block = panel(theme, format!("目录 · {} 项", app.files.entries.len()));
    let table_area = block.inner(layout[1]);
    frame.render_widget(block, layout[1]);
    if let Some(error) = &app.files.error {
        frame.render_widget(
            Paragraph::new(Span::styled(
                format!("! {error}"),
                Style::default().fg(theme.danger),
            )),
            table_area,
        );
        draw_info(frame, layout[2], app);
        return;
    }
    if app.files.entries.is_empty() {
        frame.render_widget(
            Paragraph::new(Span::styled(
                "目录为空。按 o 跳转，按 n 新建目录，按 t 新建文件。",
                Style::default().fg(theme.faint),
            )),
            table_area,
        );
        draw_info(frame, layout[2], app);
        return;
    }

    let visible = table_area.height.saturating_sub(1) as usize;
    let selected = app
        .files
        .selected
        .min(app.files.entries.len().saturating_sub(1));
    let start = selected.saturating_sub(visible / 2);
    let end = (start + visible.max(1)).min(app.files.entries.len());
    let wide = table_area.width >= 62;
    let widths = if wide {
        vec![
            Constraint::Length(2),
            Constraint::Min(12),
            Constraint::Length(8),
            Constraint::Length(10),
            Constraint::Length(10),
        ]
    } else {
        vec![
            Constraint::Length(2),
            Constraint::Min(10),
            Constraint::Length(8),
        ]
    };
    let mut rows = Vec::new();
    for (index, entry) in app.files.entries.iter().enumerate().take(end).skip(start) {
        let selected_row = index == selected;
        let style = if selected_row {
            Style::default()
                .bg(theme.selection_bg)
                .fg(theme.selection_fg)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.fg)
        };
        let icon = if entry.is_dir {
            Span::styled("▸", Style::default().fg(theme.info))
        } else if entry.is_symlink {
            Span::styled("↗", Style::default().fg(theme.accent2))
        } else {
            Span::styled("·", Style::default().fg(theme.faint))
        };
        let name = format!("{}{}", entry.name, if entry.is_dir { "/" } else { "" });
        let mut cells = vec![Cell::from(icon), Cell::from(truncate(&name, 40))];
        if wide {
            cells.push(Cell::from(entry.size_label()));
            cells.push(Cell::from(Span::styled(
                entry.mode.clone(),
                Style::default().fg(theme.faint),
            )));
            cells.push(Cell::from(Span::styled(
                entry.modified_label(),
                Style::default().fg(theme.muted),
            )));
        } else {
            cells.push(Cell::from(entry.size_label()));
        }
        rows.push(Row::new(cells).style(style));
    }
    let header = if wide {
        vec!["", "名称", "大小", "权限", "修改"]
    } else {
        vec!["", "名称", "大小"]
    };
    let table = Table::new(rows, widths)
        .header(
            Row::new(header.into_iter().map(Cell::from)).style(
                Style::default()
                    .fg(theme.faint)
                    .add_modifier(Modifier::BOLD),
            ),
        )
        .column_spacing(1);
    frame.render_widget(table, table_area);
    draw_info(frame, layout[2], app);
}

fn draw_info(frame: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;
    let text = if let Some(entry) = app.files.selected() {
        format!(
            "{}  ·  {}  ·  {}  ·  uid {}",
            entry.name,
            entry.mode,
            entry.path.display(),
            entry.uid
        )
    } else {
        format!(
            "{} 项 · 按 Enter 进入/预览 · n 建目录 · t 建文件 · r 重命名 · e 编辑 · d 回收站",
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
