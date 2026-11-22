use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Cell, Paragraph, Row, Table, Wrap};

use crate::app::App;
use crate::docker::DockerView;
use crate::ui::widgets::{panel, truncate};

pub fn draw(frame: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;
    if !app.docker.daemon_running {
        let block = panel(theme, "Docker");
        let inner = block.inner(area);
        frame.render_widget(block, area);
        let mut lines = vec![Line::from("")];
        lines.push(Line::from(Span::styled(
            if app.docker.installed {
                "Docker 已安装，但守护进程不可用。"
            } else {
                "没有检测到 Docker。"
            },
            Style::default()
                .fg(theme.warning)
                .add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            if app.docker.installed {
                "请确认 docker 服务已启动，并检查当前用户是否有访问 docker.sock 的权限。"
            } else {
                "安装 Docker 后回到本页按 u 刷新；也可以使用 apt install docker.io。"
            },
            Style::default().fg(theme.muted),
        )));
        if let Some(error) = &app.docker.error {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                truncate(error, inner.width as usize),
                Style::default().fg(theme.faint),
            )));
        }
        frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), inner);
        return;
    }

    let view = app.docker_view;
    let title = match view {
        DockerView::Containers => format!(
            "Docker 容器 · {} 运行 / {} 总数",
            app.docker.running_count(),
            app.docker.containers.len()
        ),
        DockerView::Images => format!("Docker 镜像 · {} 个", app.docker.images.len()),
    };
    let block = panel(theme, title);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.height < 4 {
        return;
    }
    let rows = Layout::vertical([Constraint::Min(4), Constraint::Length(4)]).split(inner);
    match view {
        DockerView::Containers => draw_containers(frame, rows[0], app),
        DockerView::Images => draw_images(frame, rows[0], app),
    }
    draw_details(frame, rows[1], app);
}

fn draw_containers(frame: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;
    if app.docker.containers.is_empty() {
        frame.render_widget(
            Paragraph::new(Span::styled("没有容器。", Style::default().fg(theme.faint))),
            area,
        );
        return;
    }
    let visible = area.height.saturating_sub(1) as usize;
    let selected = app
        .docker_selected
        .min(app.docker.containers.len().saturating_sub(1));
    let start = selected.saturating_sub(visible / 2);
    let end = (start + visible.max(1)).min(app.docker.containers.len());
    let wide = area.width >= 62;
    let widths = if wide {
        vec![
            Constraint::Length(2),
            Constraint::Min(12),
            Constraint::Min(12),
            Constraint::Length(12),
            Constraint::Length(14),
        ]
    } else {
        vec![
            Constraint::Length(2),
            Constraint::Min(10),
            Constraint::Length(12),
        ]
    };
    let mut rows = Vec::new();
    for (index, container) in app
        .docker
        .containers
        .iter()
        .enumerate()
        .take(end)
        .skip(start)
    {
        let selected_row = index == selected;
        let style = if selected_row {
            Style::default()
                .bg(theme.selection_bg)
                .fg(theme.selection_fg)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.fg)
        };
        let running = container.state.eq_ignore_ascii_case("running");
        let icon = if running {
            Span::styled("●", Style::default().fg(theme.success))
        } else {
            Span::styled("○", Style::default().fg(theme.faint))
        };
        let mut cells = vec![Cell::from(icon), Cell::from(truncate(&container.name, 20))];
        if wide {
            cells.push(Cell::from(truncate(&container.image, 22)));
            cells.push(Cell::from(Span::styled(
                truncate(&container.status, 14),
                Style::default().fg(if running { theme.success } else { theme.muted }),
            )));
            cells.push(Cell::from(Span::styled(
                truncate(&container.ports, 18),
                Style::default().fg(theme.info),
            )));
        } else {
            cells.push(Cell::from(truncate(&container.status, 14)));
        }
        rows.push(Row::new(cells).style(style));
    }
    let header = if wide {
        vec!["", "名称", "镜像", "状态", "端口"]
    } else {
        vec!["", "名称", "状态"]
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
    frame.render_widget(table, area);
}

fn draw_images(frame: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;
    if app.docker.images.is_empty() {
        frame.render_widget(
            Paragraph::new(Span::styled("没有镜像。", Style::default().fg(theme.faint))),
            area,
        );
        return;
    }
    let visible = area.height.saturating_sub(1) as usize;
    let selected = app
        .docker_selected
        .min(app.docker.images.len().saturating_sub(1));
    let start = selected.saturating_sub(visible / 2);
    let end = (start + visible.max(1)).min(app.docker.images.len());
    let widths = [
        Constraint::Min(14),
        Constraint::Length(14),
        Constraint::Length(10),
        Constraint::Length(12),
    ];
    let mut rows = Vec::new();
    for (index, image) in app.docker.images.iter().enumerate().take(end).skip(start) {
        let style = if index == selected {
            Style::default()
                .bg(theme.selection_bg)
                .fg(theme.selection_fg)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.fg)
        };
        rows.push(
            Row::new(vec![
                Cell::from(truncate(&format!("{}:{}", image.repository, image.tag), 28)),
                Cell::from(Span::styled(
                    image.id.clone(),
                    Style::default().fg(theme.faint),
                )),
                Cell::from(image.size.clone()),
                Cell::from(Span::styled(
                    image.created.clone(),
                    Style::default().fg(theme.muted),
                )),
            ])
            .style(style),
        );
    }
    let table = Table::new(rows, widths)
        .header(
            Row::new(["镜像", "ID", "大小", "创建"]).style(
                Style::default()
                    .fg(theme.faint)
                    .add_modifier(Modifier::BOLD),
            ),
        )
        .column_spacing(1);
    frame.render_widget(table, area);
}

fn draw_details(frame: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;
    let block = panel(theme, "详情");
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let lines = match app.docker_view {
        DockerView::Containers => {
            if let Some(container) = app.docker.containers.get(app.docker_selected) {
                vec![
                    Line::from(vec![
                        Span::styled("ID   ", Style::default().fg(theme.faint)),
                        Span::styled(container.id.clone(), Style::default().fg(theme.muted)),
                    ]),
                    Line::from(vec![
                        Span::styled("状态 ", Style::default().fg(theme.faint)),
                        Span::styled(container.status.clone(), Style::default().fg(theme.fg)),
                    ]),
                    Line::from(vec![
                        Span::styled("端口 ", Style::default().fg(theme.faint)),
                        Span::styled(
                            if container.ports.is_empty() {
                                "—".into()
                            } else {
                                container.ports.clone()
                            },
                            Style::default().fg(theme.info),
                        ),
                    ]),
                ]
            } else {
                vec![Line::from(Span::styled(
                    "没有选中的容器。",
                    Style::default().fg(theme.faint),
                ))]
            }
        }
        DockerView::Images => vec![Line::from(Span::styled(
            "i 切换容器 / 镜像 · d 删除选中资源",
            Style::default().fg(theme.faint),
        ))],
    };
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), inner);
}
