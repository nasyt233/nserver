use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};

use crate::app::App;
use crate::system::{format_uptime, percent};
use crate::theme::Theme;
use crate::ui::widgets::panel;

pub fn draw(frame: &mut Frame, area: Rect, app: &App) {
    let rows = Layout::vertical([
        Constraint::Percentage(46),
        Constraint::Percentage(42),
        Constraint::Min(3),
    ])
    .split(area);
    let top =
        Layout::horizontal([Constraint::Percentage(55), Constraint::Percentage(45)]).split(rows[0]);
    paths_panel(frame, top[0], app);
    runtime_panel(frame, top[1], app);
    interface_panel(frame, rows[1], app);
    about_panel(frame, rows[2], app);
}

fn paths_panel(frame: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;
    let block = panel(theme, "路径");
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let lines = vec![
        kv(theme, "Nginx 配置", &app.config.main.display().to_string()),
        kv(
            theme,
            "NSERVER 偏好",
            &crate::user_config::UserConfig::load()
                .path
                .display()
                .to_string(),
        ),
        kv(
            theme,
            "配置目录",
            &app.config.config_dir.display().to_string(),
        ),
        kv(
            theme,
            "Nginx 程序",
            &app.bin
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| "未找到".into()),
        ),
        kv(
            theme,
            "FTP 配置",
            &app.ftp
                .config_path
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| "未找到 vsftpd".into()),
        ),
        kv(
            theme,
            "备份目录",
            &format!(
                "{}",
                app.config.config_dir.join(".nserver/backups").display()
            ),
        ),
        kv(
            theme,
            "文件回收站",
            &crate::files::trash_dir().display().to_string(),
        ),
    ];
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), inner);
}

fn runtime_panel(frame: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;
    let block = panel(theme, "运行环境");
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let snap = &app.system.snapshot;
    let lines = vec![
        kv(theme, "主机名", &snap.hostname),
        kv(theme, "系统", &snap.os),
        kv(theme, "运行", &format_uptime(snap.uptime_secs)),
        kv(
            theme,
            "资源",
            &format!(
                "CPU {:.0}% · 内存 {:.0}% · 磁盘 {:.0}%",
                snap.cpu_percent.unwrap_or(0.0),
                percent(snap.mem_used, snap.mem_total),
                percent(snap.disk_used, snap.disk_total)
            ),
        ),
        kv(theme, "Nginx", app.manager.label()),
        kv(theme, "Nginx worker", &app.process.workers.to_string()),
        kv(theme, "权限", if app.root { "root" } else { "普通用户" }),
        kv(theme, "模式", if app.read_only { "只读" } else { "可写" }),
    ];
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), inner);
}

fn interface_panel(frame: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;
    let block = panel(theme, "界面与操作");
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let lines = vec![
        kv(theme, "主题", app.theme.name),
        kv(theme, "版本", env!("CARGO_PKG_VERSION")),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                "T",
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" 切换主题    ", Style::default().fg(theme.muted)),
            Span::styled(
                "Ctrl+B",
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" 收起/展开侧栏", Style::default().fg(theme.muted)),
        ]),
        Line::from(vec![
            Span::styled(
                "e",
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" 编辑 Nginx 主配置    ", Style::default().fg(theme.muted)),
            Span::styled(
                "Ctrl+R",
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" 刷新全部", Style::default().fg(theme.muted)),
        ]),
        Line::from(vec![
            Span::styled(
                "F5",
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" 刷新全部    ", Style::default().fg(theme.muted)),
            Span::styled(
                "?",
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" 查看全部快捷键", Style::default().fg(theme.muted)),
        ]),
    ];
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), inner);
}

fn about_panel(frame: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;
    let block = panel(theme, "关于 NSERVER");
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let text = vec![
        Line::from(Span::styled(
            "NSERVER · 终端服务器管理面板",
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            "Rust + ratatui · 本地操作，不上传任何数据。所有写操作均有只读模式和备份保护。",
            Style::default().fg(theme.muted),
        )),
        Line::from(Span::styled(
            "Nginx 站点 / Docker 容器 / 文件 / FTP / SSL / 资源监控",
            Style::default().fg(theme.faint),
        )),
    ];
    frame.render_widget(Paragraph::new(text).wrap(Wrap { trim: true }), inner);
}

fn kv(theme: &Theme, key: &str, value: &str) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("{key:<10}"), Style::default().fg(theme.faint)),
        Span::styled(value.to_string(), Style::default().fg(theme.fg)),
    ])
}
