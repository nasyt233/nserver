use ratatui::Frame;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};

use crate::app::{App, MouseAction};
use crate::theme::Theme;
use crate::ui::widgets::{center_percent, clear_popup, panel};

pub fn draw(frame: &mut Frame, area: Rect, app: &App, scroll: usize) {
    let theme = &app.theme;
    let popup = center_percent(area, 86, 88);
    clear_popup(frame, popup);
    let block = panel(theme, "帮助 · 快捷键");
    let inner = block.inner(popup);
    frame.render_widget(block, popup);
    if inner.height < 2 {
        return;
    }
    let body = Rect::new(inner.x, inner.y, inner.width, inner.height - 1);
    app.help_viewport.set(body.height as usize);
    let lines = help_lines(theme);
    frame.render_widget(
        Paragraph::new(lines)
            .scroll((scroll.min(u16::MAX as usize) as u16, 0))
            .wrap(Wrap { trim: false }),
        body,
    );
    let footer = Rect::new(inner.x, inner.y + inner.height - 1, inner.width, 1);
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("↑↓", Style::default().fg(theme.accent)),
            Span::styled(" 滚动   ", Style::default().fg(theme.faint)),
            Span::styled("Esc / ?", Style::default().fg(theme.accent)),
            Span::styled(" 关闭", Style::default().fg(theme.faint)),
        ]))
        .alignment(Alignment::Right),
        footer,
    );
    app.register_hit(
        footer,
        MouseAction::Key(
            crossterm::event::KeyCode::Esc,
            crossterm::event::KeyModifiers::NONE,
        ),
    );
}

fn help_lines(theme: &Theme) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    lines.push(Line::from(vec![
        Span::styled(
            "NSERVER",
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("  ·  终端服务器管理面板", Style::default().fg(theme.muted)),
    ]));

    section(&mut lines, theme, "全局");
    item(
        &mut lines,
        theme,
        "1 - 9",
        "首页 / 站点 / 文件 / Docker / 监控 / 日志 / SSL / FTP / 设置",
    );
    item(
        &mut lines,
        theme,
        "Tab / Shift+Tab",
        "在全部页面间循环（含配置、上游）",
    );
    item(&mut lines, theme, "Ctrl+B", "收起 / 展开左侧栏");
    item(&mut lines, theme, "?", "打开 / 关闭帮助");
    item(
        &mut lines,
        theme,
        "q / Ctrl+C",
        "退出（编辑器有修改时会确认）",
    );
    item(&mut lines, theme, "T", "切换配色主题");
    item(&mut lines, theme, "Ctrl+R / F5", "刷新全部数据");
    item(
        &mut lines,
        theme,
        "t / r / s / R",
        "校验 / 重载 / 启停 / 重启 Nginx",
    );

    section(&mut lines, theme, "首页");
    item(&mut lines, theme, "资源卡", "CPU、内存、磁盘、网络与负载");
    item(
        &mut lines,
        theme,
        "服务",
        "Nginx、Docker、FTP 状态和快捷操作",
    );
    item(&mut lines, theme, "n", "快速新建 Nginx 网站");

    section(&mut lines, theme, "站点");
    item(&mut lines, theme, "Space", "启用 / 停用站点");
    item(&mut lines, theme, "n", "新建：静态 / 反代 / 端口转发");
    item(&mut lines, theme, "m", "网站设置：域名、端口、目录、代理");
    item(
        &mut lines,
        theme,
        "w / p / o / l",
        "快速改目录 / 代理 / 域名 / 端口",
    );
    item(&mut lines, theme, "e / Enter", "编辑完整站点配置");
    item(&mut lines, theme, "d", "移入 sites-available/.trash");
    item(&mut lines, theme, "c / u", "进入 Nginx 配置 / 上游页");

    section(&mut lines, theme, "文件");
    item(&mut lines, theme, "↑↓ / Enter", "选择；目录进入，文件预览");
    item(&mut lines, theme, "e", "编辑文本文件（自动备份）");
    item(&mut lines, theme, "b / Ctrl+O", "选择文件并打开编辑器");
    item(&mut lines, theme, "← →", "父目录 / 当前目录 / 预览三栏切换");
    item(&mut lines, theme, "n / t", "新建目录 / 文件");
    item(&mut lines, theme, "r", "重命名选中项");
    item(&mut lines, theme, "d", "移入 ~/.nserver/trash");
    item(&mut lines, theme, "o / Backspace", "跳转路径 / 返回上级");
    item(&mut lines, theme, ".", "显示 / 隐藏隐藏文件");

    section(&mut lines, theme, "Docker");
    item(&mut lines, theme, "i", "切换容器 / 镜像");
    item(&mut lines, theme, "s / S / r", "启动 / 停止 / 重启容器");
    item(&mut lines, theme, "l", "查看容器日志");
    item(&mut lines, theme, "d", "删除容器或镜像（需确认）");
    item(&mut lines, theme, "u", "刷新 Docker 数据");

    section(&mut lines, theme, "监控与日志");
    item(&mut lines, theme, "u（监控）", "立即重新采样资源");
    item(
        &mut lines,
        theme,
        "↑↓ / f / a",
        "日志滚动 / 跟随 / 自动刷新",
    );
    item(&mut lines, theme, "L / [ ]", "访问/错误/系统/全部 / 日志源");
    item(&mut lines, theme, "h / l / u", "横向滚动 / 重读日志");

    section(&mut lines, theme, "SSL / FTP");
    item(&mut lines, theme, "n（SSL）", "生成自签名证书到 ssl/");
    item(&mut lines, theme, "Enter", "查看证书完整详情");
    item(
        &mut lines,
        theme,
        "s / S / r / l",
        "FTP 启动 / 停止 / 重启 / 重载",
    );
    item(&mut lines, theme, "e（FTP）", "编辑 vsftpd.conf");

    section(&mut lines, theme, "应用商店 / 垃圾清理");
    item(
        &mut lines,
        theme,
        "Store",
        "i 安装 / x 卸载 / Enter 详情 / u 刷新",
    );
    item(
        &mut lines,
        theme,
        "Cleaner",
        "Space 勾选 / a 全选 / c 清理 / u 扫描",
    );
    item(
        &mut lines,
        theme,
        "FTP 目录",
        "FTP 页按 v 切换服务/目录，n/t/r/d 管理",
    );

    section(&mut lines, theme, "配置与上游");
    item(&mut lines, theme, "e / Enter", "编辑 Nginx 配置");
    item(&mut lines, theme, "Ctrl+D / U", "滚动配置预览");
    item(&mut lines, theme, "Enter（上游）", "TCP 探测选中后端");
    item(&mut lines, theme, "p", "并行探测全部后端");

    section(&mut lines, theme, "安全设计");
    item(&mut lines, theme, "备份", "配置保存前写入 .nserver/backups");
    item(&mut lines, theme, "站点删除", "只移入 .trash，不直接擦除");
    item(&mut lines, theme, "文件删除", "只移入 ~/.nserver/trash");
    item(&mut lines, theme, "只读", "--read-only 禁用全部写入");
    item(&mut lines, theme, "变更", "修改配置后按 t 校验、按 r 重载");
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "所有操作都在本机执行；NSERVER 不上传任何数据。",
        Style::default().fg(theme.warning),
    )));
    lines
}

fn section(lines: &mut Vec<Line<'static>>, theme: &Theme, title: &str) {
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        format!("▍{title}"),
        Style::default()
            .fg(theme.accent)
            .add_modifier(Modifier::BOLD),
    )));
}

fn item(lines: &mut Vec<Line<'static>>, theme: &Theme, key: &str, text: &str) {
    lines.push(Line::from(vec![
        Span::styled(
            format!("  {key:<11}"),
            Style::default()
                .fg(theme.accent2)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(text.to_string(), Style::default().fg(theme.fg)),
    ]));
}

pub fn max_scroll(viewport: usize) -> usize {
    let theme = Theme::default();
    help_lines(&theme).len().saturating_sub(viewport.max(1))
}
