mod certs;
mod cleaner;
mod config;
mod docker;
mod files;
mod ftp;
mod help;
mod home;
mod logs;
mod modals;
mod monitor;
mod services;
mod settings;
mod sites;
mod store;
mod upstreams;
mod widgets;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph, Wrap};
use unicode_width::UnicodeWidthStr;

use crate::app::{App, MouseAction, Tab};
use crate::theme::Theme;
use widgets::{badge, fill, hint_line_fit, panel, truncate};

pub fn help_max_scroll(viewport: usize) -> usize {
    help::max_scroll(viewport)
}

pub fn draw(frame: &mut Frame, app: &mut App) {
    let area = frame.area();
    fill(frame, area, &app.theme);

    if area.width < 64 || area.height < 20 {
        draw_too_small(frame, area, &app.theme);
        return;
    }

    app.clear_hits();
    if app.editor.is_some() {
        draw_editor(frame, area, app);
    } else {
        draw_main(frame, area, app);
    }
    modals::draw_toast(frame, area, app);
    modals::draw_overlay(frame, area, app);
}

fn draw_main(frame: &mut Frame, area: Rect, app: &App) {
    let rows = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(0),
        Constraint::Length(1),
    ])
    .split(area);
    draw_topbar(frame, rows[0], app);
    let sidebar_width = sidebar_width(area.width, app.sidebar_collapsed);
    let body =
        Layout::horizontal([Constraint::Length(sidebar_width), Constraint::Min(0)]).split(rows[1]);
    draw_sidebar(frame, body[0], app);
    draw_content(frame, body[1], app);
    draw_footer(frame, rows[2], app);
}

fn draw_topbar(frame: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;
    frame.render_widget(
        Block::default().style(Style::default().bg(theme.header_bg)),
        Rect::new(area.x, area.y, area.width, 2),
    );
    let line1 = Rect::new(area.x + 1, area.y, area.width.saturating_sub(2), 1);
    let line2 = Rect::new(area.x + 1, area.y + 1, area.width.saturating_sub(2), 1);
    let divider = Rect::new(area.x, area.y + 2, area.width, 1);

    let right_width = if area.width >= 92 { 42 } else { 0 };
    let cols =
        Layout::horizontal([Constraint::Min(0), Constraint::Length(right_width)]).split(line1);
    let nginx = app.process.running();
    let mut spans = vec![
        Span::styled(
            " ◆ ",
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "NSERVER",
            Style::default().fg(theme.fg).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("  v{}  ", env!("CARGO_PKG_VERSION")),
            Style::default().fg(theme.faint),
        ),
        badge(
            theme,
            if nginx { "NGINX ON" } else { "NGINX OFF" },
            if nginx { theme.success } else { theme.faint },
        ),
        Span::styled("  ", Style::default()),
        badge(
            theme,
            &if app.docker.daemon_running {
                format!("DOCKER {}", app.docker.running_count())
            } else {
                "DOCKER OFF".to_string()
            },
            if app.docker.daemon_running {
                theme.info
            } else {
                theme.faint
            },
        ),
        Span::styled("  ", Style::default()),
        badge(
            theme,
            if app.ftp.running { "FTP ON" } else { "FTP OFF" },
            if app.ftp.running {
                theme.accent2
            } else {
                theme.faint
            },
        ),
    ];
    if app.read_only {
        spans.push(Span::styled("  ", Style::default()));
        spans.push(badge(theme, "READ-ONLY", theme.warning));
    }
    frame.render_widget(
        Paragraph::new(Line::from(spans)).style(Style::default().bg(theme.header_bg)),
        cols[0],
    );

    if right_width > 0 {
        let snap = &app.system.snapshot;
        let load = snap
            .load
            .map(|load| format!("load {:.2}", load.0))
            .unwrap_or_else(|| "load —".into());
        let now = chrono::Local::now().format("%H:%M:%S");
        let text = format!("{}  ·  {}  ·  {}", snap.hostname, load, now);
        frame.render_widget(
            Paragraph::new(Span::styled(
                truncate(&text, right_width.saturating_sub(1) as usize),
                Style::default().fg(theme.muted),
            ))
            .alignment(Alignment::Right)
            .style(Style::default().bg(theme.header_bg)),
            cols[1],
        );
    }

    let page_cols = Layout::horizontal([
        Constraint::Min(0),
        Constraint::Length(if area.width >= 80 { 26 } else { 0 }),
    ])
    .split(line2);
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                format!("{} ", app.tab.icon()),
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                app.tab.title(),
                Style::default().fg(theme.fg).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("   {}", app.tab.subtitle()),
                Style::default().fg(theme.muted),
            ),
        ]))
        .style(Style::default().bg(theme.header_bg)),
        page_cols[0],
    );
    frame.render_widget(
        Paragraph::new(Span::styled(
            "Ctrl+B 侧栏  ·  Tab 页面  ·  ? 帮助",
            Style::default().fg(theme.faint),
        ))
        .alignment(Alignment::Right)
        .style(Style::default().bg(theme.header_bg)),
        page_cols[1],
    );
    frame.render_widget(
        Paragraph::new(Span::styled(
            "─".repeat(divider.width as usize),
            Style::default().fg(theme.border).bg(theme.header_bg),
        )),
        divider,
    );
}

fn sidebar_width(total: u16, collapsed: bool) -> u16 {
    if collapsed || total < 72 {
        5
    } else if total >= 110 {
        20
    } else if total >= 92 {
        18
    } else {
        15
    }
}

fn draw_sidebar(frame: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;
    let compact = area.width < 8;
    let title = if compact { "N" } else { "导航" };
    let block = panel(theme, title);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let visible = inner.height as usize;
    let mut lines = Vec::new();
    for (index, tab) in Tab::ALL.iter().enumerate() {
        if index >= visible {
            break;
        }
        let selected = *tab == app.tab;
        let shortcut = tab
            .shortcut()
            .map(|c| c.to_string())
            .unwrap_or_else(|| "·".into());
        let text = if compact {
            format!(" {} ", tab.icon())
        } else if area.width >= 18 {
            format!(" {}  {} {}", shortcut, tab.icon(), tab.title())
        } else {
            format!(" {} {}", shortcut, tab.title())
        };
        let style = if selected {
            Style::default()
                .bg(theme.accent)
                .fg(theme.bg)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.muted)
        };
        lines.push(Line::from(Span::styled(
            truncate(&text, inner.width as usize),
            style,
        )));
        app.register_hit(
            Rect::new(inner.x, inner.y + index as u16, inner.width, 1),
            MouseAction::Tab(*tab),
        );
    }
    frame.render_widget(Paragraph::new(lines), inner);
}

fn draw_content(frame: &mut Frame, area: Rect, app: &App) {
    match app.tab {
        Tab::Overview => home::draw(frame, area, app),
        Tab::Sites => sites::draw(frame, area, app),
        Tab::Files => files::draw(frame, area, app),
        Tab::Docker => docker::draw(frame, area, app),
        Tab::Monitor => monitor::draw(frame, area, app),
        Tab::Logs => logs::draw(frame, area, app),
        Tab::Certs => certs::draw(frame, area, app),
        Tab::Ftp => ftp::draw(frame, area, app),
        Tab::Settings => settings::draw(frame, area, app),
        Tab::Services => services::draw(frame, area, app),
        Tab::Store => store::draw(frame, area, app),
        Tab::Cleaner => cleaner::draw(frame, area, app),
        Tab::Config => config::draw(frame, area, app),
        Tab::Upstreams => upstreams::draw(frame, area, app),
    }
}

fn draw_footer(frame: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;
    frame.render_widget(
        Block::default().style(Style::default().bg(theme.footer_bg)),
        area,
    );
    let cols = Layout::horizontal([Constraint::Min(0), Constraint::Length(18)]).split(area);
    let left = Rect::new(cols[0].x + 1, cols[0].y, cols[0].width.saturating_sub(1), 1);
    let hints = footer_hints(app);
    frame.render_widget(
        Paragraph::new(hint_line_fit(theme, &hints, left.width as usize))
            .style(Style::default().bg(theme.footer_bg)),
        left,
    );
    let mut used = 0u16;
    for (key, label) in &hints {
        let width =
            2 + UnicodeWidthStr::width(*key) as u16 + 2 + UnicodeWidthStr::width(*label) as u16 + 1;
        if used > 0 && used + width > left.width {
            break;
        }
        if let Some(event) = hint_key_event(key) {
            app.register_hit(
                Rect::new(
                    left.x + used,
                    left.y,
                    width.min(left.width.saturating_sub(used)),
                    1,
                ),
                MouseAction::Key(event.code, event.modifiers),
            );
        }
        used = used.saturating_add(width);
    }
    let half = cols[1].width / 2;
    app.register_hit(
        Rect::new(cols[1].x, cols[1].y, half, 1),
        MouseAction::Key(KeyCode::Char('q'), KeyModifiers::NONE),
    );
    app.register_hit(
        Rect::new(cols[1].x + half, cols[1].y, cols[1].width - half, 1),
        MouseAction::Key(KeyCode::Char('?'), KeyModifiers::NONE),
    );
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                "q",
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" 退出  ", Style::default().fg(theme.faint)),
            Span::styled(
                "?",
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" 帮助 ", Style::default().fg(theme.faint)),
        ]))
        .alignment(Alignment::Right)
        .style(Style::default().bg(theme.footer_bg)),
        cols[1],
    );
}

fn hint_key_event(key: &str) -> Option<KeyEvent> {
    let event = match key {
        "↑↓" => KeyEvent::new(KeyCode::Up, KeyModifiers::NONE),
        "←→" => KeyEvent::new(KeyCode::Left, KeyModifiers::NONE),
        "Space" => KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE),
        "Enter" => KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
        "Esc" => KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE),
        "Tab" => KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE),
        "Ctrl+D/U" => KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL),
        "Ctrl+R" => KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL),
        "Ctrl+B" => KeyEvent::new(KeyCode::Char('b'), KeyModifiers::CONTROL),
        "Ctrl+O" => KeyEvent::new(KeyCode::Char('o'), KeyModifiers::CONTROL),
        "[/]" => KeyEvent::new(KeyCode::Char('['), KeyModifiers::NONE),
        "a/A" => KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE),
        "F5" => KeyEvent::new(KeyCode::F(5), KeyModifiers::NONE),
        value if value.chars().count() == 1 => {
            KeyEvent::new(KeyCode::Char(value.chars().next()?), KeyModifiers::NONE)
        }
        _ => return None,
    };
    Some(event)
}

fn footer_hints(app: &App) -> Vec<(&'static str, &'static str)> {
    match app.tab {
        Tab::Overview => vec![
            ("t", "校验"),
            ("r", "重载"),
            (
                "s",
                if app.process.running() {
                    "停止"
                } else {
                    "启动"
                },
            ),
            ("R", "重启"),
            ("n", "新建网站"),
        ],
        Tab::Sites => vec![
            ("↑↓", "选择"),
            ("m", "设置"),
            ("n", "新建"),
            ("Space", "启停"),
            ("w", "目录"),
            ("p", "代理"),
            ("e", "配置"),
            ("/", "筛选"),
        ],
        Tab::Files => vec![
            ("←→", "窗格"),
            ("↑↓", "选择"),
            ("Enter", "进入/预览"),
            ("e", "编辑"),
            ("n", "建目录"),
            ("t", "建文件"),
            ("r", "重命名"),
            ("d", "回收站"),
            ("o", "跳转"),
        ],
        Tab::Docker => vec![
            ("↑↓", "选择"),
            ("i", "容器/镜像"),
            ("s", "启动"),
            ("S", "停止"),
            ("r", "重启"),
            ("l", "日志"),
            ("d", "删除"),
            ("u", "刷新"),
        ],
        Tab::Monitor => vec![("u", "刷新采样")],
        Tab::Logs => vec![
            ("↑↓", "滚动"),
            ("f", "跟随"),
            ("a", "自动"),
            ("L", "类型"),
            ("[/]", "换源"),
            ("/", "筛选"),
        ],
        Tab::Certs => vec![
            ("↑↓", "选择"),
            ("Enter", "详情"),
            ("n", "自签名"),
            ("Ctrl+R", "刷新"),
        ],
        Tab::Ftp => match app.ftp_view {
            crate::app::FtpView::Service => vec![
                ("s", "启动"),
                ("S", "停止"),
                ("r", "重启"),
                ("l", "重载"),
                ("e", "配置"),
                ("v", "目录"),
                ("u", "刷新"),
            ],
            crate::app::FtpView::Directories => vec![
                ("↑↓", "选择"),
                ("Enter", "进入"),
                ("n", "新建目录"),
                ("t", "新建文件"),
                ("r", "重命名"),
                ("d", "删除"),
                ("v", "服务"),
                ("u", "刷新"),
            ],
        },
        Tab::Settings => vec![
            ("T", "主题"),
            ("e", "编辑主配置"),
            ("w", "保存 ~/.nserver"),
            ("Ctrl+B", "侧栏"),
        ],
        Tab::Services => vec![
            ("s", "启动"),
            ("x", "停止"),
            ("r", "重启"),
            ("e/d", "开机启停"),
            ("u", "刷新"),
        ],
        Tab::Store => vec![
            ("↑↓", "选择"),
            ("i", "安装"),
            ("x", "卸载"),
            ("Enter", "详情"),
            ("u", "刷新"),
        ],
        Tab::Cleaner => vec![
            ("↑↓", "选择"),
            ("Space", "勾选"),
            ("a/A", "全选/取消"),
            ("c", "清理"),
            ("u", "扫描"),
        ],
        Tab::Config => vec![
            ("↑↓", "文件"),
            ("e", "编辑"),
            ("Ctrl+D/U", "预览"),
            ("/", "筛选"),
        ],
        Tab::Upstreams => vec![
            ("↑↓", "选择"),
            ("←→", "焦点"),
            ("Enter", "探测"),
            ("p", "全部探测"),
        ],
    }
}

fn draw_editor(frame: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;
    let Some(editor) = &app.editor else {
        return;
    };
    let footer_area = Rect::new(area.x, area.y + area.height - 1, area.width, 1);
    let main_area = Rect::new(area.x, area.y, area.width, area.height - 1);
    let label = editor
        .path
        .strip_prefix(&app.config.config_dir)
        .unwrap_or(&editor.path)
        .display()
        .to_string();
    let marker = if editor.dirty {
        "● 未保存"
    } else {
        "已保存"
    };
    let title = format!(
        "编辑 · {}  ·  {}",
        truncate(&label, area.width.saturating_sub(28) as usize),
        marker
    );
    let block = panel(
        theme,
        truncate(&title, area.width.saturating_sub(3) as usize),
    );
    let inner = block.inner(main_area);
    frame.render_widget(block, main_area);
    frame.render_widget(&editor.textarea, inner);

    frame.render_widget(
        Block::default().style(Style::default().bg(theme.footer_bg)),
        footer_area,
    );
    let (line, col) = editor.textarea.cursor();
    let info = format!(
        " {}:{}  ·  {} 行  ·  Ctrl+S 保存  ·  Esc 关闭  ·  Ctrl+Z/Y 撤销/重做 ",
        line + 1,
        col + 1,
        editor.textarea.lines().len()
    );
    let status = if let Some(err) = &editor.error {
        Span::styled(format!("× {err}"), Style::default().fg(theme.danger))
    } else if editor.dirty {
        Span::styled("● 修改未保存", Style::default().fg(theme.warning))
    } else {
        Span::styled(
            "已保存 · 按 t 校验，按 r 重载",
            Style::default().fg(theme.success),
        )
    };
    let cols = Layout::horizontal([Constraint::Min(0), Constraint::Length(34)]).split(footer_area);
    frame.render_widget(
        Paragraph::new(Span::styled(info, Style::default().fg(theme.faint)))
            .style(Style::default().bg(theme.footer_bg)),
        cols[0],
    );
    frame.render_widget(
        Paragraph::new(status)
            .alignment(Alignment::Right)
            .style(Style::default().bg(theme.footer_bg)),
        cols[1],
    );
    app.register_hit(
        Rect::new(footer_area.x, footer_area.y, footer_area.width / 2, 1),
        MouseAction::Key(
            crossterm::event::KeyCode::Char('s'),
            crossterm::event::KeyModifiers::CONTROL,
        ),
    );
    app.register_hit(
        Rect::new(
            footer_area.x + footer_area.width / 2,
            footer_area.y,
            footer_area.width - footer_area.width / 2,
            1,
        ),
        MouseAction::Key(
            crossterm::event::KeyCode::Esc,
            crossterm::event::KeyModifiers::NONE,
        ),
    );
}

fn draw_too_small(frame: &mut Frame, area: Rect, theme: &Theme) {
    let text = vec![
        Line::from(""),
        Line::from(Span::styled(
            "终端太小",
            Style::default()
                .fg(theme.warning)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            format!("当前 {}×{}，请至少调到 64×20。", area.width, area.height),
            Style::default().fg(theme.muted),
        )),
        Line::from(""),
        Line::from(Span::styled("按 q 退出", Style::default().fg(theme.faint))),
    ];
    frame.render_widget(
        Paragraph::new(text)
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: true }),
        area,
    );
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::Write;
    use std::os::unix::fs::symlink;
    use std::path::Path;

    use chrono::{Duration, Utc};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    use super::*;
    use crate::app::{
        ConfirmAction, ConfirmState, InputState, InputTarget, NewSiteField, NewSiteState,
        OutputState, Overlay, SiteSettingsState, SiteSettingsValues,
    };
    use crate::cert::CertInfo;
    use crate::cli::Cli;
    use crate::service::{CommandResult, Manager, ProcessInfo};
    use crate::theme::ThemeName;

    fn write(path: &Path, text: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        let mut file = fs::File::create(path).unwrap();
        file.write_all(text.as_bytes()).unwrap();
    }

    fn sample_app() -> (App, tempfile::TempDir) {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        write(
            &root.join("nginx.conf"),
            r#"
pid logs/nginx.pid;
error_log logs/error.log;
access_log logs/access.log;
http {
    upstream app_cluster {
        least_conn;
        server 127.0.0.1:3000 weight=3;
        server 127.0.0.1:3001 backup;
    }
    include sites-enabled/*.conf;
}
"#,
        );
        write(
            &root.join("sites-available/demo.conf"),
            r#"
server {
    listen 80;
    server_name demo.test www.demo.test;
    root /var/www/demo;
    location / { try_files $uri $uri/ =404; }
    location /api { proxy_pass http://app_cluster; }
}
server {
    listen 443 ssl;
    server_name secure.demo.test;
    ssl_certificate certs/demo.pem;
    location / { return 301 https://$host$request_uri; }
}
"#,
        );
        write(
            &root.join("sites-available/blog.conf"),
            "server { listen 8080; server_name blog.test; root /var/www/blog; }\n",
        );
        fs::create_dir_all(root.join("sites-enabled")).unwrap();
        fs::create_dir_all(root.join("logs")).unwrap();
        symlink(
            "../sites-available/demo.conf",
            root.join("sites-enabled/demo.conf"),
        )
        .unwrap();
        let mut errors = fs::File::create(root.join("logs/error.log")).unwrap();
        for i in 0..12 {
            writeln!(
                errors,
                "2026/11/21 12:0{i}:00 [error] 42#0: *{i} connect() failed (111: Connection refused) while connecting to upstream, client: 10.0.0.{i}, server: demo.test, request: \"GET /api HTTP/1.1\", upstream: \"http://127.0.0.1:3000/api\""
            )
            .unwrap();
        }
        let mut access = fs::File::create(root.join("logs/access.log")).unwrap();
        for i in 0..80 {
            let status = if i % 17 == 0 {
                503
            } else if i % 7 == 0 {
                404
            } else {
                200
            };
            writeln!(
                access,
                "10.0.0.{} - - [21/Nov/2026:12:00:{i:02} +0800] \"GET /api/items/{i} HTTP/1.1\" {status} 512 \"-\" \"curl/8.5.0\"",
                i % 250
            )
            .unwrap();
        }

        let cli = Cli {
            config: root.join("nginx.conf"),
            theme: Some(ThemeName::Midnight),
            ..Default::default()
        };
        let mut app = App::new(cli);
        app.process = ProcessInfo {
            master_pid: Some(4242),
            workers: 4,
            rss_kb: 28 * 1024,
            uptime_secs: Some(3 * 3600 + 12 * 60),
        };
        app.manager = Manager::Direct;
        app.last_test = Some(CommandResult {
            label: "配置校验".into(),
            command: "nginx -t -c nginx.conf".into(),
            success: true,
            code: Some(0),
            stdout: "nginx: configuration file test is successful".into(),
            stderr: String::new(),
            elapsed_ms: 4,
        });
        app.certs = vec![
            CertInfo {
                path: root.join("certs/demo.pem"),
                domains: vec!["secure.demo.test".into(), "www.demo.test".into()],
                subject: Some("CN = secure.demo.test".into()),
                issuer: Some("CN = Demo CA".into()),
                not_after: Some(Utc::now() + Duration::days(18)),
                days_left: Some(18),
                sans: vec!["secure.demo.test".into(), "www.demo.test".into()],
                error: None,
            },
            CertInfo {
                path: root.join("certs/old.pem"),
                domains: vec!["old.demo.test".into()],
                subject: Some("CN = old.demo.test".into()),
                issuer: Some("CN = Demo CA".into()),
                not_after: Some(Utc::now() - Duration::days(3)),
                days_left: Some(-3),
                sans: Vec::new(),
                error: None,
            },
        ];
        app.cert_selected = 0;
        app.toast = None;
        app.refresh_preview();
        app.refresh_log();
        (app, tmp)
    }

    fn render(app: &mut App, width: u16, height: u16) -> String {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| draw(frame, app)).unwrap();
        let buffer = terminal.backend().buffer();
        let mut out = String::new();
        for y in 0..height {
            for x in 0..width {
                out.push_str(buffer[(x, y)].symbol());
            }
            out.push('\n');
        }
        out
    }

    fn maybe_dump(name: &str, text: &str) {
        if std::env::var_os("NSERVER_DUMP").is_some() {
            let path = std::path::PathBuf::from("/tmp").join(format!("nserver-{name}.txt"));
            fs::write(&path, text).unwrap();
            eprintln!("dumped {}", path.display());
        }
    }

    #[test]
    fn all_tabs_render_at_common_sizes() {
        let (mut app, _tmp) = sample_app();
        for (width, height) in [(80, 24), (120, 40), (64, 20)] {
            for tab in Tab::ALL {
                app.tab = tab;
                let text = render(&mut app, width, height);
                assert!(!text.trim().is_empty());
                maybe_dump(&format!("{width}x{height}-{}", tab.title()), &text);
            }
        }
    }

    #[test]
    fn overlays_render_without_panicking() {
        let (mut app, _tmp) = sample_app();
        app.tab = Tab::Sites;
        app.overlay = Overlay::Help { scroll: 3 };
        maybe_dump("help", &render(&mut app, 80, 24));

        app.overlay = Overlay::Confirm(ConfirmState {
            title: "停止 Nginx？".into(),
            message: vec!["停止后所有站点将不可访问。".into()],
            action: ConfirmAction::Service(crate::service::Action::Stop),
            danger: true,
        });
        maybe_dump("confirm", &render(&mut app, 80, 24));

        app.overlay = Overlay::Output(OutputState::from_message(
            "配置校验",
            false,
            "nginx: [emerg] unknown directive \"serverr\" in /etc/nginx/nginx.conf:12",
        ));
        maybe_dump("output", &render(&mut app, 80, 24));

        app.overlay = Overlay::Input(InputState {
            target: InputTarget::Sites,
            title: "筛选站点".into(),
            value: "demo".into(),
        });
        maybe_dump("input", &render(&mut app, 80, 24));

        app.overlay = Overlay::NewSite(NewSiteState::new());
        maybe_dump("new-site-static", &render(&mut app, 80, 24));
        if let Overlay::NewSite(form) = &mut app.overlay {
            form.cycle_kind(1);
        }
        maybe_dump("new-site-proxy", &render(&mut app, 80, 24));
        if let Overlay::NewSite(form) = &mut app.overlay {
            form.cycle_kind(1);
        }
        maybe_dump("new-site-forward", &render(&mut app, 80, 24));

        let site = app.sites.sites[0].clone();
        app.overlay = Overlay::SiteSettings(SiteSettingsState {
            site_name: site.name.clone(),
            path: site.available.clone(),
            kind: site.kind(),
            domain: "demo.test".into(),
            listen: "80".into(),
            root: "/var/www/demo".into(),
            proxy: "http://app_cluster".into(),
            websocket: false,
            spa: false,
            has_root: true,
            has_proxy: true,
            original: SiteSettingsValues {
                domain: "demo.test".into(),
                listen: "80".into(),
                root: "/var/www/demo".into(),
                proxy: "http://app_cluster".into(),
                websocket: false,
                spa: false,
            },
            focus: 0,
            error: None,
        });
        maybe_dump("site-settings", &render(&mut app, 80, 24));

        app.overlay = Overlay::None;
        let mut textarea = tui_textarea::TextArea::new(vec![
            "server {".into(),
            "    listen 80;".into(),
            "    # 修改后保存".into(),
            "}".into(),
        ]);
        textarea.insert_str("    server_name demo.test;\n");
        app.editor = Some(crate::app::EditorState {
            path: app.config.main.clone(),
            textarea,
            dirty: true,
            changed_on_disk: false,
            error: None,
            had_trailing_newline: true,
            allow_outside: false,
        });
        maybe_dump("editor", &render(&mut app, 80, 24));
    }

    #[test]
    fn tiny_terminal_renders_hint() {
        let (mut app, _tmp) = sample_app();
        let text = render(&mut app, 50, 15);
        assert!(text.replace(' ', "").contains("终端太小"), "{text}");
    }

    #[test]
    fn cursor_is_only_visible_for_text_interfaces() {
        let (mut app, _tmp) = sample_app();
        assert!(!app.cursor_visible());
        app.overlay = Overlay::Input(InputState {
            target: InputTarget::Logs,
            title: "筛选日志".into(),
            value: String::new(),
        });
        assert!(app.cursor_visible());
        app.overlay = Overlay::NewSite(NewSiteState::new());
        assert!(!app.cursor_visible());
        if let Overlay::NewSite(form) = &mut app.overlay {
            form.focus = 1;
            assert_eq!(form.current_field(), NewSiteField::FileName);
        }
        assert!(app.cursor_visible());
        app.overlay = Overlay::Confirm(ConfirmState {
            title: "x".into(),
            message: Vec::new(),
            action: ConfirmAction::Quit,
            danger: true,
        });
        assert!(!app.cursor_visible());
    }
}
