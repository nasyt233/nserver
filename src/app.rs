//! Application state and the keyboard-driven interaction model.

use std::cell::Cell;
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::net::{TcpStream, ToSocketAddrs};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::style::{Modifier, Style};
use tui_textarea::TextArea;

use crate::cert::{self, CertInfo};
use crate::cli::Cli;
use crate::config::{self, LogKind, LogSource, NginxConfig};
use crate::docker::{DockerSnapshot, DockerView};
use crate::files::FileBrowser;
use crate::ftp::FtpSnapshot;
use crate::logs::{self, LogSnapshot, TrafficStats};
use crate::service::{self, Action, CommandResult, Manager, ProcessInfo};
use crate::sites::{self, NewSiteSpec, Site, SiteInventory, SiteKind};
use crate::system::SystemState;
use crate::theme::{Theme, ThemeName};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Overview,
    Sites,
    Files,
    Docker,
    Monitor,
    Logs,
    Certs,
    Ftp,
    Settings,
    Config,
    Upstreams,
}

impl Tab {
    pub const ALL: [Tab; 11] = [
        Tab::Overview,
        Tab::Sites,
        Tab::Files,
        Tab::Docker,
        Tab::Monitor,
        Tab::Logs,
        Tab::Certs,
        Tab::Ftp,
        Tab::Settings,
        Tab::Config,
        Tab::Upstreams,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Tab::Overview => "首页",
            Tab::Sites => "站点",
            Tab::Files => "文件",
            Tab::Docker => "Docker",
            Tab::Monitor => "监控",
            Tab::Logs => "日志",
            Tab::Certs => "SSL",
            Tab::Ftp => "FTP",
            Tab::Settings => "设置",
            Tab::Config => "配置",
            Tab::Upstreams => "上游",
        }
    }

    pub fn subtitle(self) -> &'static str {
        match self {
            Self::Overview => "系统资源与服务概览",
            Self::Sites => "Nginx 站点与反向代理",
            Self::Files => "浏览、预览与管理服务器文件",
            Self::Docker => "容器运行状态与操作",
            Self::Monitor => "CPU、内存、磁盘与网络趋势",
            Self::Logs => "Nginx 访问与错误日志",
            Self::Certs => "证书到期与自签名证书",
            Self::Ftp => "vsftpd 服务与配置摘要",
            Self::Settings => "路径、主题与应用信息",
            Self::Config => "Nginx 配置浏览与编辑",
            Self::Upstreams => "upstream 后端与连通性",
        }
    }

    pub fn icon(self) -> &'static str {
        match self {
            Self::Overview => "⌂",
            Self::Sites => "◈",
            Self::Files => "▤",
            Self::Docker => "▣",
            Self::Monitor => "◔",
            Self::Logs => "≡",
            Self::Certs => "⛨",
            Self::Ftp => "⇅",
            Self::Settings => "⚙",
            Self::Config => "⚒",
            Self::Upstreams => "⇉",
        }
    }

    pub fn shortcut(self) -> Option<char> {
        match self {
            Self::Overview => Some('1'),
            Self::Sites => Some('2'),
            Self::Files => Some('3'),
            Self::Docker => Some('4'),
            Self::Monitor => Some('5'),
            Self::Logs => Some('6'),
            Self::Certs => Some('7'),
            Self::Ftp => Some('8'),
            Self::Settings => Some('9'),
            Self::Config | Self::Upstreams => None,
        }
    }

    pub fn index(self) -> usize {
        Self::ALL.iter().position(|t| *t == self).unwrap_or(0)
    }

    pub fn next(self) -> Self {
        Self::ALL[(self.index() + 1) % Self::ALL.len()]
    }

    pub fn previous(self) -> Self {
        Self::ALL[(self.index() + Self::ALL.len() - 1) % Self::ALL.len()]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastKind {
    Info,
    Success,
    Warning,
    Danger,
}

impl ToastKind {
    pub fn icon(self) -> &'static str {
        match self {
            Self::Info => "i",
            Self::Success => "✓",
            Self::Warning => "!",
            Self::Danger => "×",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Info => "信息",
            Self::Success => "成功",
            Self::Warning => "注意",
            Self::Danger => "错误",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Toast {
    pub kind: ToastKind,
    pub text: String,
    pub at: Instant,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpstreamFocus {
    Pools,
    Backends,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputTarget {
    Sites,
    Config,
    Logs,
    FilePath,
    NewDir,
    NewFile,
    RenameFile,
    SslDomain,
}

#[derive(Debug, Clone)]
pub struct InputState {
    pub target: InputTarget,
    pub title: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfirmAction {
    Service(Action),
    ToggleSite {
        enable: bool,
    },
    DeleteSite,
    DiscardEditor,
    Quit,
    Docker {
        id: String,
        name: String,
        verb: String,
        image: bool,
    },
    FtpAction {
        verb: String,
    },
    FileDelete {
        path: PathBuf,
        name: String,
    },
}

#[derive(Debug, Clone)]
pub struct ConfirmState {
    pub title: String,
    pub message: Vec<String>,
    pub action: ConfirmAction,
    pub danger: bool,
}

#[derive(Debug, Clone)]
pub struct OutputState {
    pub title: String,
    pub success: bool,
    pub lines: Vec<String>,
    pub scroll: usize,
}

impl OutputState {
    pub fn from_results(title: impl Into<String>, results: &[CommandResult]) -> Self {
        let success = results.iter().all(|r| r.success);
        let mut lines = Vec::new();
        for (idx, result) in results.iter().enumerate() {
            if results.len() > 1 {
                lines.push(format!(
                    "── {}/{} · {} ─────────────────────────────",
                    idx + 1,
                    results.len(),
                    result.label
                ));
            }
            if !result.command.is_empty() {
                lines.push(format!("$ {}", result.command));
            }
            lines.push(format!("{}  ({} ms)", result.summary(), result.elapsed_ms));
            for line in result.output().lines() {
                lines.push(format!("  {line}"));
            }
            if idx + 1 != results.len() {
                lines.push(String::new());
            }
        }
        Self {
            title: title.into(),
            success,
            lines,
            scroll: 0,
        }
    }

    pub fn from_message(
        title: impl Into<String>,
        success: bool,
        message: impl Into<String>,
    ) -> Self {
        Self {
            title: title.into(),
            success,
            lines: message.into().lines().map(ToString::to_string).collect(),
            scroll: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NewSiteField {
    Kind,
    FileName,
    ServerName,
    Listen,
    Root,
    Proxy,
    Websocket,
    Spa,
    Enable,
}

impl NewSiteField {
    pub fn label(self) -> &'static str {
        match self {
            Self::Kind => "网站类型",
            Self::FileName => "文件标识",
            Self::ServerName => "域名",
            Self::Listen => "监听端口",
            Self::Root => "网站目录",
            Self::Proxy => "目标地址",
            Self::Websocket => "WebSocket",
            Self::Spa => "SPA 回退",
            Self::Enable => "创建后启用",
        }
    }

    pub fn is_toggle(self) -> bool {
        matches!(self, Self::Websocket | Self::Spa | Self::Enable)
    }
}

#[derive(Debug, Clone)]
pub struct NewSiteState {
    pub spec: NewSiteSpec,
    pub focus: usize,
    pub error: Option<String>,
}

impl NewSiteState {
    pub fn new() -> Self {
        Self {
            spec: NewSiteSpec::default(),
            focus: 0,
            error: None,
        }
    }

    pub fn visible_fields(&self) -> Vec<NewSiteField> {
        let mut fields = vec![
            NewSiteField::Kind,
            NewSiteField::FileName,
            NewSiteField::ServerName,
            NewSiteField::Listen,
        ];
        match self.spec.kind {
            SiteKind::Static => {
                fields.push(NewSiteField::Root);
                fields.push(NewSiteField::Spa);
            }
            SiteKind::Proxy | SiteKind::PortForward => {
                fields.push(NewSiteField::Proxy);
                fields.push(NewSiteField::Websocket);
            }
            _ => {}
        }
        fields.push(NewSiteField::Enable);
        fields
    }

    pub fn current_field(&self) -> NewSiteField {
        let fields = self.visible_fields();
        fields[self.focus.min(fields.len().saturating_sub(1))]
    }

    pub fn field_text(&self, field: NewSiteField) -> String {
        match field {
            NewSiteField::Kind => self.spec.kind.label().to_string(),
            NewSiteField::FileName => self.spec.file_name.clone(),
            NewSiteField::ServerName => {
                if self.spec.kind == SiteKind::PortForward && self.spec.server_name.is_empty() {
                    "（空 = _）".to_string()
                } else {
                    self.spec.server_name.clone()
                }
            }
            NewSiteField::Listen => self.spec.listen.clone(),
            NewSiteField::Root => self.spec.root.clone(),
            NewSiteField::Proxy => self.spec.proxy.clone(),
            NewSiteField::Websocket => toggle_text(self.spec.websocket),
            NewSiteField::Spa => toggle_text(self.spec.spa),
            NewSiteField::Enable => toggle_text(self.spec.enable),
        }
    }

    pub fn field_mut(&mut self, field: NewSiteField) -> Option<&mut String> {
        match field {
            NewSiteField::FileName => Some(&mut self.spec.file_name),
            NewSiteField::ServerName => Some(&mut self.spec.server_name),
            NewSiteField::Listen => Some(&mut self.spec.listen),
            NewSiteField::Root => Some(&mut self.spec.root),
            NewSiteField::Proxy => Some(&mut self.spec.proxy),
            _ => None,
        }
    }

    pub fn toggle_field(&mut self, field: NewSiteField) {
        match field {
            NewSiteField::Websocket => self.spec.websocket = !self.spec.websocket,
            NewSiteField::Spa => self.spec.spa = !self.spec.spa,
            NewSiteField::Enable => self.spec.enable = !self.spec.enable,
            _ => {}
        }
    }

    pub fn cycle_kind(&mut self, delta: isize) {
        self.spec.kind = self.spec.kind.next(delta);
        self.apply_kind_defaults();
        let count = self.visible_fields().len();
        self.focus = self.focus.min(count.saturating_sub(1));
    }

    fn apply_kind_defaults(&mut self) {
        match self.spec.kind {
            SiteKind::Static => {
                if self.spec.listen == "8080" || self.spec.listen.is_empty() {
                    self.spec.listen = "80".into();
                }
                if self.spec.root.is_empty() {
                    self.spec.root = "/var/www/example.com/html".into();
                }
                if self.spec.server_name == "_" || self.spec.server_name.is_empty() {
                    self.spec.server_name = "example.com www.example.com".into();
                }
                if self.spec.file_name.is_empty() || self.spec.file_name.starts_with("forward-") {
                    self.spec.file_name = "example.com".into();
                }
            }
            SiteKind::Proxy => {
                self.spec.root.clear();
                if self.spec.listen == "8080" || self.spec.listen.is_empty() {
                    self.spec.listen = "80".into();
                }
                if self.spec.server_name == "_" || self.spec.server_name.is_empty() {
                    self.spec.server_name = "example.com www.example.com".into();
                }
                if self.spec.proxy.is_empty() {
                    self.spec.proxy = "http://127.0.0.1:3000".into();
                }
                if self.spec.file_name.is_empty() || self.spec.file_name.starts_with("forward-") {
                    self.spec.file_name = "example.com".into();
                }
            }
            SiteKind::PortForward => {
                self.spec.root.clear();
                if self.spec.listen == "80" || self.spec.listen.is_empty() {
                    self.spec.listen = "8080".into();
                }
                if self.spec.server_name == "example.com www.example.com" {
                    self.spec.server_name = "_".into();
                }
                if self.spec.proxy.is_empty() {
                    self.spec.proxy = "http://127.0.0.1:3000".into();
                }
                if self.spec.file_name.is_empty() || self.spec.file_name == "example.com" {
                    self.spec.file_name = "forward-8080".into();
                }
            }
            _ => {}
        }
    }

    pub fn move_focus(&mut self, delta: isize) {
        let len = self.visible_fields().len() as isize;
        self.focus = ((self.focus as isize + delta).rem_euclid(len)) as usize;
    }
}

fn toggle_text(value: bool) -> String {
    if value {
        "[x] 开启".to_string()
    } else {
        "[ ] 关闭".to_string()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsField {
    Domain,
    Listen,
    Root,
    Proxy,
    Websocket,
    Spa,
}

impl SettingsField {
    pub fn label(self) -> &'static str {
        match self {
            Self::Domain => "域名",
            Self::Listen => "监听端口",
            Self::Root => "网站目录",
            Self::Proxy => "代理目标",
            Self::Websocket => "WebSocket",
            Self::Spa => "SPA 回退",
        }
    }

    pub fn is_toggle(self) -> bool {
        matches!(self, Self::Websocket | Self::Spa)
    }
}

#[derive(Debug, Clone)]
pub struct SiteSettingsValues {
    pub domain: String,
    pub listen: String,
    pub root: String,
    pub proxy: String,
    pub websocket: bool,
    pub spa: bool,
}

#[derive(Debug, Clone)]
pub struct SiteSettingsState {
    pub site_name: String,
    pub path: PathBuf,
    pub kind: SiteKind,
    pub domain: String,
    pub listen: String,
    pub root: String,
    pub proxy: String,
    pub websocket: bool,
    pub spa: bool,
    pub has_root: bool,
    pub has_proxy: bool,
    pub original: SiteSettingsValues,
    pub focus: usize,
    pub error: Option<String>,
}

impl SiteSettingsState {
    pub fn visible_fields(&self) -> Vec<SettingsField> {
        let mut fields = vec![SettingsField::Domain, SettingsField::Listen];
        if self.has_root {
            fields.push(SettingsField::Root);
            fields.push(SettingsField::Spa);
        }
        if self.has_proxy {
            fields.push(SettingsField::Proxy);
            fields.push(SettingsField::Websocket);
        }
        fields
    }

    pub fn current_field(&self) -> SettingsField {
        let fields = self.visible_fields();
        fields[self.focus.min(fields.len().saturating_sub(1))]
    }

    pub fn focus_field(&mut self, field: SettingsField) {
        if let Some(index) = self.visible_fields().iter().position(|f| *f == field) {
            self.focus = index;
        }
    }

    pub fn field_text(&self, field: SettingsField) -> String {
        match field {
            SettingsField::Domain => self.domain.clone(),
            SettingsField::Listen => self.listen.clone(),
            SettingsField::Root => self.root.clone(),
            SettingsField::Proxy => self.proxy.clone(),
            SettingsField::Websocket => toggle_text(self.websocket),
            SettingsField::Spa => toggle_text(self.spa),
        }
    }

    pub fn field_mut(&mut self, field: SettingsField) -> Option<&mut String> {
        match field {
            SettingsField::Domain => Some(&mut self.domain),
            SettingsField::Listen => Some(&mut self.listen),
            SettingsField::Root => Some(&mut self.root),
            SettingsField::Proxy => Some(&mut self.proxy),
            _ => None,
        }
    }

    pub fn toggle_field(&mut self, field: SettingsField) {
        match field {
            SettingsField::Websocket => self.websocket = !self.websocket,
            SettingsField::Spa => self.spa = !self.spa,
            _ => {}
        }
    }

    pub fn move_focus(&mut self, delta: isize) {
        let len = self.visible_fields().len() as isize;
        self.focus = ((self.focus as isize + delta).rem_euclid(len)) as usize;
    }
}

pub struct EditorState {
    pub path: PathBuf,
    pub textarea: TextArea<'static>,
    pub dirty: bool,
    pub changed_on_disk: bool,
    pub error: Option<String>,
    pub had_trailing_newline: bool,
    pub allow_outside: bool,
}

impl std::fmt::Debug for EditorState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EditorState")
            .field("path", &self.path)
            .field("dirty", &self.dirty)
            .finish()
    }
}

#[derive(Default)]
pub enum Overlay {
    #[default]
    None,
    Help {
        scroll: usize,
    },
    Confirm(ConfirmState),
    Output(OutputState),
    Input(InputState),
    NewSite(NewSiteState),
    SiteSettings(SiteSettingsState),
}

pub struct LogState {
    pub sources: Vec<LogSource>,
    pub index: usize,
    pub kind_filter: Option<LogKind>,
    pub snapshot: Option<LogSnapshot>,
    pub error: Option<String>,
    pub filter: String,
    pub auto: bool,
    pub follow: bool,
    pub scroll_from_bottom: usize,
    pub hscroll: usize,
    pub last_loaded: Instant,
}

impl LogState {
    fn new() -> Self {
        Self {
            sources: Vec::new(),
            index: 0,
            kind_filter: None,
            snapshot: None,
            error: None,
            filter: String::new(),
            auto: true,
            follow: true,
            scroll_from_bottom: 0,
            hscroll: 0,
            last_loaded: Instant::now() - Duration::from_secs(10),
        }
    }
}

#[derive(Debug, Clone)]
pub struct BackendProbe {
    pub success: bool,
    pub latency_ms: u128,
    pub message: String,
    pub at: Instant,
}

#[derive(Debug, Clone)]
pub struct FilePreview {
    pub path: PathBuf,
    pub lines: Vec<String>,
    pub size: u64,
    pub modified: Option<std::time::SystemTime>,
    pub error: Option<String>,
    pub truncated: bool,
}

pub struct App {
    pub should_quit: bool,
    pub tab: Tab,
    pub theme: Theme,
    pub read_only: bool,
    pub config: NginxConfig,
    pub config_missing: bool,
    pub sites: SiteInventory,
    pub certs: Vec<CertInfo>,
    pub config_files: Vec<PathBuf>,
    pub config_selected: usize,
    pub config_filter: String,
    pub config_preview: Option<FilePreview>,
    pub preview_scroll: usize,
    pub site_selected: usize,
    pub site_filter: String,
    pub cert_selected: usize,
    pub upstream_selected: usize,
    pub backend_selected: usize,
    pub upstream_focus: UpstreamFocus,
    pub probes: HashMap<String, BackendProbe>,
    pub bin: Option<PathBuf>,
    bin_arg: Option<PathBuf>,
    pub manager: Manager,
    pub version: Option<String>,
    pub process: ProcessInfo,
    pub last_test: Option<CommandResult>,
    pub traffic: TrafficStats,
    pub recent_errors: Vec<String>,
    pub error_count: usize,
    pub warning_count: usize,
    pub root: bool,
    pub logs: LogState,
    pub system: SystemState,
    pub docker: DockerSnapshot,
    pub docker_view: DockerView,
    pub docker_selected: usize,
    pub ftp: FtpSnapshot,
    pub files: FileBrowser,
    pub sidebar_collapsed: bool,
    pub overlay: Overlay,
    pub editor: Option<EditorState>,
    pub toast: Option<Toast>,
    pub last_status: Instant,
    pub last_refresh: Instant,
    pub preview_viewport: Cell<usize>,
    pub output_viewport: Cell<usize>,
    pub help_viewport: Cell<usize>,
    pub log_viewport: Cell<usize>,
}

impl App {
    pub fn new(cli: Cli) -> Self {
        let config_dir = cli
            .config
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));
        let bin_arg = cli.bin.clone();
        let mut app = Self {
            should_quit: false,
            tab: Tab::Overview,
            theme: Theme::new(cli.theme.unwrap_or(ThemeName::Midnight)),
            read_only: cli.read_only,
            config: NginxConfig {
                main: cli.config.clone(),
                config_dir,
                ..Default::default()
            },
            config_missing: false,
            sites: SiteInventory::default(),
            certs: Vec::new(),
            config_files: Vec::new(),
            config_selected: 0,
            config_filter: String::new(),
            config_preview: None,
            preview_scroll: 0,
            site_selected: 0,
            site_filter: String::new(),
            cert_selected: 0,
            upstream_selected: 0,
            backend_selected: 0,
            upstream_focus: UpstreamFocus::Pools,
            probes: HashMap::new(),
            bin: service::detect_binary(bin_arg.as_deref()),
            bin_arg,
            manager: service::detect_manager(),
            version: None,
            process: ProcessInfo::default(),
            last_test: None,
            traffic: TrafficStats::default(),
            recent_errors: Vec::new(),
            error_count: 0,
            warning_count: 0,
            root: service::is_root(),
            logs: LogState::new(),
            system: SystemState::default(),
            docker: DockerSnapshot::default(),
            docker_view: DockerView::Containers,
            docker_selected: 0,
            ftp: FtpSnapshot::default(),
            files: FileBrowser::default(),
            sidebar_collapsed: false,
            overlay: Overlay::None,
            editor: None,
            toast: None,
            last_status: Instant::now() - Duration::from_secs(10),
            last_refresh: Instant::now() - Duration::from_secs(10),
            preview_viewport: Cell::new(12),
            output_viewport: Cell::new(18),
            help_viewport: Cell::new(18),
            log_viewport: Cell::new(18),
        };
        app.refresh_all(true);
        app.system.tick(true);
        app.refresh_docker();
        app.refresh_ftp();
        if app.config_missing {
            app.toast(
                ToastKind::Danger,
                format!("找不到配置文件：{}", app.config.main.display()),
            );
        } else {
            app.toast(ToastKind::Info, "就绪 · 按 ? 查看快捷键，1-6 切换页面");
        }
        app
    }

    pub fn toast(&mut self, kind: ToastKind, text: impl Into<String>) {
        self.toast = Some(Toast {
            kind,
            text: text.into(),
            at: Instant::now(),
        });
    }

    pub fn cursor_visible(&self) -> bool {
        match &self.overlay {
            Overlay::Input(_) => true,
            Overlay::NewSite(form) => {
                let field = form.current_field();
                field != NewSiteField::Kind && !field.is_toggle()
            }
            Overlay::SiteSettings(state) => !state.current_field().is_toggle(),
            Overlay::None => self.editor.is_some(),
            _ => false,
        }
    }

    pub fn tick(&mut self) {
        if self
            .toast
            .as_ref()
            .map(|t| t.at.elapsed() > Duration::from_secs(5))
            .unwrap_or(false)
        {
            self.toast = None;
        }
        self.system.tick(false);
        if self.last_status.elapsed() > Duration::from_secs(2) {
            self.process = service::inspect(&self.config.pid_file);
            self.last_status = Instant::now();
            if self.version.is_none() {
                self.version = self.bin.as_deref().and_then(service::version);
            }
        }
        if self.tab == Tab::Logs
            && self.logs.auto
            && self.logs.last_loaded.elapsed() > Duration::from_millis(1200)
            && self.editor.is_none()
            && matches!(self.overlay, Overlay::None)
        {
            self.refresh_log();
        }
    }

    pub fn on_key(&mut self, key: KeyEvent) {
        if !matches!(
            key.kind,
            crossterm::event::KeyEventKind::Press | crossterm::event::KeyEventKind::Repeat
        ) {
            return;
        }
        if key.modifiers.contains(KeyModifiers::CONTROL) && matches!(key.code, KeyCode::Char('c')) {
            if self.editor.as_ref().map(|e| e.dirty).unwrap_or(false) {
                self.overlay = Overlay::Confirm(ConfirmState {
                    title: "放弃未保存的修改并退出？".into(),
                    message: vec!["编辑器中的修改尚未保存。".into()],
                    action: ConfirmAction::Quit,
                    danger: true,
                });
            } else {
                self.should_quit = true;
            }
            return;
        }

        if !matches!(self.overlay, Overlay::None) {
            self.handle_overlay(key);
            return;
        }
        if self.editor.is_some() {
            self.handle_editor_key(key);
            return;
        }
        if self.handle_global_key(key) {
            return;
        }
        match self.tab {
            Tab::Overview => {}
            Tab::Sites => self.handle_sites_key(key),
            Tab::Config => self.handle_config_key(key),
            Tab::Logs => self.handle_logs_key(key),
            Tab::Certs => self.handle_certs_key(key),
            Tab::Upstreams => self.handle_upstreams_key(key),
            Tab::Files => self.handle_files_key(key),
            Tab::Docker => self.handle_docker_key(key),
            Tab::Monitor => self.handle_monitor_key(key),
            Tab::Ftp => self.handle_ftp_key(key),
            Tab::Settings => self.handle_settings_key(key),
        }
    }

    pub fn on_paste(&mut self, text: &str) {
        match &mut self.overlay {
            Overlay::Input(input) => input.value.push_str(text),
            Overlay::NewSite(form) => {
                let field = form.current_field();
                if field != NewSiteField::Kind
                    && !field.is_toggle()
                    && let Some(value) = form.field_mut(field)
                {
                    value.push_str(text);
                }
            }
            Overlay::SiteSettings(state) => {
                let field = state.current_field();
                if !field.is_toggle()
                    && let Some(value) = state.field_mut(field)
                {
                    value.push_str(text);
                }
            }
            _ => {}
        }
        if let Some(editor) = &mut self.editor
            && editor.textarea.insert_str(text)
        {
            editor.dirty = true;
        }
    }

    fn nginx_shortcuts_active(&self) -> bool {
        matches!(
            self.tab,
            Tab::Overview | Tab::Sites | Tab::Config | Tab::Logs | Tab::Certs | Tab::Upstreams
        )
    }

    fn handle_global_key(&mut self, key: KeyEvent) -> bool {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        match key.code {
            KeyCode::Char('c') if ctrl => true,
            KeyCode::F(5) => {
                self.refresh_all(false);
                true
            }
            KeyCode::Char('r') if ctrl => {
                self.refresh_all(false);
                true
            }
            KeyCode::Char('q') if !ctrl => {
                self.should_quit = true;
                true
            }
            KeyCode::Char('?') => {
                self.overlay = Overlay::Help { scroll: 0 };
                true
            }
            KeyCode::Char('T') if !ctrl => {
                self.theme.cycle();
                let name = self.theme.name;
                self.toast(ToastKind::Info, format!("主题：{name}"));
                true
            }
            KeyCode::Char(c @ '1'..='9') if !ctrl => {
                let idx = (c as u8 - b'1') as usize;
                self.set_tab(Tab::ALL[idx.min(Tab::ALL.len() - 1)]);
                true
            }
            KeyCode::Char('c') if !ctrl && self.tab == Tab::Sites => {
                self.set_tab(Tab::Config);
                true
            }
            KeyCode::Char('u') if !ctrl && self.tab == Tab::Sites => {
                self.set_tab(Tab::Upstreams);
                true
            }
            KeyCode::Char('b') if ctrl => {
                self.sidebar_collapsed = !self.sidebar_collapsed;
                true
            }
            KeyCode::Tab if !ctrl => {
                self.set_tab(self.tab.next());
                true
            }
            KeyCode::BackTab => {
                self.set_tab(self.tab.previous());
                true
            }
            KeyCode::Char('n') if !ctrl && matches!(self.tab, Tab::Overview | Tab::Sites) => {
                self.tab = Tab::Sites;
                self.open_new_site();
                true
            }
            KeyCode::Char('t') if !ctrl && self.nginx_shortcuts_active() => {
                self.run_service(Action::Test);
                true
            }
            KeyCode::Char('r') if !ctrl && self.nginx_shortcuts_active() => {
                self.run_service(Action::Reload);
                true
            }
            KeyCode::Char('s') if !ctrl && self.nginx_shortcuts_active() => {
                self.toggle_service();
                true
            }
            KeyCode::Char('R') if !ctrl && self.nginx_shortcuts_active() => {
                self.request_restart();
                true
            }
            KeyCode::Esc => {
                self.clear_current_filter();
                self.toast = None;
                true
            }
            _ => false,
        }
    }

    fn set_tab(&mut self, tab: Tab) {
        self.tab = tab;
        match tab {
            Tab::Config => {
                if self.config_preview.is_none() {
                    self.refresh_preview();
                }
            }
            Tab::Logs => {
                if self.current_log_source().is_none() {
                    self.pick_default_log();
                }
                self.refresh_log();
            }
            Tab::Overview => {
                self.process = service::inspect(&self.config.pid_file);
                self.system.tick(true);
                self.last_status = Instant::now();
            }
            Tab::Files => self.files.refresh(),
            Tab::Docker => self.refresh_docker(),
            Tab::Ftp => self.refresh_ftp(),
            Tab::Monitor => self.system.tick(true),
            _ => {}
        }
    }

    fn clear_current_filter(&mut self) {
        match self.tab {
            Tab::Sites if !self.site_filter.is_empty() => {
                self.site_filter.clear();
                self.site_selected = 0;
            }
            Tab::Config if !self.config_filter.is_empty() => {
                self.config_filter.clear();
                self.config_selected = 0;
                self.refresh_preview();
            }
            Tab::Logs => {
                if !self.logs.filter.is_empty() {
                    self.logs.filter.clear();
                    self.logs.scroll_from_bottom = 0;
                    self.logs.follow = true;
                } else if self.logs.kind_filter.is_some() {
                    self.logs.kind_filter = None;
                    self.pick_default_log();
                    self.refresh_log();
                }
            }
            _ => {}
        }
    }

    // ---------------------------------------------------------------- data

    pub fn refresh_all(&mut self, silent: bool) {
        let old_site = self.selected_site().map(|s| s.name.clone());
        let old_config = self.selected_config_file().cloned();
        let old_cert = self.selected_cert().map(|c| c.path.clone());
        let old_upstream = self.selected_upstream().map(|u| u.name.clone());
        let old_log_path = self.current_log_source().map(|s| s.path.clone());

        self.bin = service::detect_binary(self.bin_arg.as_deref());
        self.manager = service::detect_manager();
        self.version = self.bin.as_deref().and_then(service::version);
        self.config_missing = !self.config.main.is_file();
        self.config = config::load(&self.config.main);
        self.sites = SiteInventory::load(&self.config);
        self.certs = cert::discover(&self.config);
        self.last_test = None;

        let mut files: Vec<PathBuf> = self
            .config
            .files
            .iter()
            .filter(|path| {
                path.is_file()
                    && path
                        .metadata()
                        .map(|m| config::looks_like_config(path, m.len()))
                        .unwrap_or(false)
            })
            .cloned()
            .collect();
        files.sort_by(|a, b| {
            let am = a == &self.config.main;
            let bm = b == &self.config.main;
            bm.cmp(&am).then_with(|| {
                config::file_label(a, &self.config.config_dir)
                    .cmp(&config::file_label(b, &self.config.config_dir))
            })
        });
        self.config_files = files;

        self.site_selected = old_site
            .and_then(|name| {
                self.filtered_sites()
                    .iter()
                    .position(|site| site.name == name)
            })
            .unwrap_or(0);
        self.site_selected = self
            .site_selected
            .min(self.filtered_sites().len().saturating_sub(1));
        self.clamp_config_selection(old_config.as_ref());
        self.cert_selected = old_cert
            .and_then(|path| self.certs.iter().position(|cert| cert.path == path))
            .unwrap_or(0);
        self.upstream_selected = old_upstream
            .and_then(|name| {
                self.config
                    .upstreams
                    .iter()
                    .position(|upstream| upstream.name == name)
            })
            .unwrap_or(0);
        self.upstream_selected = self
            .upstream_selected
            .min(self.config.upstreams.len().saturating_sub(1));
        self.clamp_backend_selection();

        let mut sources = self.config.logs.clone();
        for path in ["/var/log/syslog", "/var/log/messages"] {
            let path = PathBuf::from(path);
            if path.is_file() && !sources.iter().any(|source| source.path == path) {
                sources.push(LogSource {
                    kind: LogKind::System,
                    label: path
                        .file_name()
                        .map(|name| name.to_string_lossy().to_string())
                        .unwrap_or_else(|| path.display().to_string()),
                    path,
                });
            }
        }
        self.logs.sources = sources;
        if let Some(index) =
            old_log_path.and_then(|path| self.logs.sources.iter().position(|s| s.path == path))
        {
            self.logs.index = index;
        } else {
            self.pick_default_log();
        }
        if let Some(kind) = self.logs.kind_filter {
            if !self.logs.sources.iter().any(|s| s.kind == kind) {
                self.logs.kind_filter = None;
            } else if self.current_log_source().is_none() {
                self.select_first_source_of_kind(kind);
            }
        }
        self.refresh_log();
        self.refresh_preview();
        self.refresh_overview_stats();
        self.process = service::inspect(&self.config.pid_file);
        self.system.tick(true);
        self.files.refresh();
        if self.tab == Tab::Docker {
            self.refresh_docker();
        }
        if self.tab == Tab::Ftp {
            self.refresh_ftp();
        }
        self.last_status = Instant::now();
        self.last_refresh = Instant::now();
        if !silent {
            self.toast(ToastKind::Success, "数据已刷新");
        }
    }

    fn clamp_config_selection(&mut self, old: Option<&PathBuf>) {
        let index =
            old.and_then(|path| self.filtered_config_files().iter().position(|p| *p == path));
        let count = self.filtered_config_files().len();
        self.config_selected = index.unwrap_or(0).min(count.saturating_sub(1));
    }

    fn refresh_overview_stats(&mut self) {
        self.traffic = TrafficStats::default();
        self.recent_errors.clear();
        self.error_count = 0;
        self.warning_count = 0;
        let mut seen = std::collections::HashSet::new();
        for source in self
            .config
            .logs
            .iter()
            .filter(|s| s.kind == LogKind::Access && s.path.is_file())
        {
            if !seen.insert(source.path.clone()) {
                continue;
            }
            if let Ok(snapshot) = logs::read_tail(&source.path, 1024 * 1024, 1000) {
                self.traffic.add(&TrafficStats::from_lines(&snapshot.lines));
            }
        }
        let mut errors = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for source in self
            .config
            .logs
            .iter()
            .filter(|s| s.kind == LogKind::Error && s.path.is_file())
        {
            if !seen.insert(source.path.clone()) {
                continue;
            }
            if let Ok(snapshot) = logs::read_tail(&source.path, 1024 * 1024, 400) {
                let (count, warnings) = logs::count_errors(&snapshot.lines);
                self.error_count += count;
                self.warning_count += warnings;
                errors.extend(
                    snapshot
                        .lines
                        .iter()
                        .filter(|line| logs::is_error_line(line))
                        .cloned(),
                );
            }
        }
        errors.sort();
        self.recent_errors = errors
            .into_iter()
            .rev()
            .take(5)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
    }

    pub fn refresh_docker(&mut self) {
        self.docker.refresh();
        let count = match self.docker_view {
            DockerView::Containers => self.docker.containers.len(),
            DockerView::Images => self.docker.images.len(),
        };
        self.docker_selected = self.docker_selected.min(count.saturating_sub(1));
    }

    pub fn refresh_ftp(&mut self) {
        self.ftp.refresh();
    }

    fn refresh_process(&mut self) {
        self.process = service::inspect(&self.config.pid_file);
        self.manager = service::detect_manager();
        if self.version.is_none() {
            self.version = self.bin.as_deref().and_then(service::version);
        }
        self.last_status = Instant::now();
    }

    pub fn refresh_preview(&mut self) {
        let Some(path) = self.selected_config_file().cloned() else {
            self.config_preview = None;
            return;
        };
        let meta = fs::metadata(&path);
        let size = meta.as_ref().map(|m| m.len()).unwrap_or(0);
        let modified = meta.as_ref().ok().and_then(|m| m.modified().ok());
        if size > 1024 * 1024 {
            self.config_preview = Some(FilePreview {
                path,
                lines: Vec::new(),
                size,
                modified,
                error: Some("文件超过 1 MB，已跳过预览。".into()),
                truncated: false,
            });
            return;
        }
        match fs::read_to_string(&path) {
            Ok(text) => {
                let mut lines: Vec<String> = text.lines().map(ToString::to_string).collect();
                let truncated = lines.len() > 5000;
                if truncated {
                    lines.truncate(5000);
                }
                self.config_preview = Some(FilePreview {
                    path,
                    lines,
                    size,
                    modified,
                    error: None,
                    truncated,
                });
            }
            Err(err) => {
                self.config_preview = Some(FilePreview {
                    path,
                    lines: Vec::new(),
                    size,
                    modified,
                    error: Some(format!("读取失败：{err}")),
                    truncated: false,
                });
            }
        }
    }

    // -------------------------------------------------------------- filters

    pub fn filtered_sites(&self) -> Vec<&Site> {
        let needle = self.site_filter.trim().to_lowercase();
        self.sites
            .sites
            .iter()
            .filter(|site| {
                needle.is_empty()
                    || site.name.to_lowercase().contains(&needle)
                    || site.server_names().to_lowercase().contains(&needle)
                    || site
                        .available
                        .display()
                        .to_string()
                        .to_lowercase()
                        .contains(&needle)
            })
            .collect()
    }

    pub fn selected_site(&self) -> Option<&Site> {
        self.filtered_sites().get(self.site_selected).copied()
    }

    pub fn filtered_config_files(&self) -> Vec<&PathBuf> {
        let needle = self.config_filter.trim().to_lowercase();
        self.config_files
            .iter()
            .filter(|path| {
                needle.is_empty()
                    || config::file_label(path, &self.config.config_dir)
                        .to_lowercase()
                        .contains(&needle)
            })
            .collect()
    }

    pub fn selected_config_file(&self) -> Option<&PathBuf> {
        self.filtered_config_files()
            .get(self.config_selected)
            .copied()
    }

    pub fn selected_cert(&self) -> Option<&CertInfo> {
        self.certs.get(self.cert_selected)
    }

    pub fn selected_upstream(&self) -> Option<&config::UpstreamInfo> {
        self.config.upstreams.get(self.upstream_selected)
    }

    pub fn backend_key(upstream: &config::UpstreamInfo, backend: &config::BackendInfo) -> String {
        format!("{}|{}", upstream.name, backend.address)
    }

    pub fn probe_for(
        &self,
        upstream: &config::UpstreamInfo,
        backend: &config::BackendInfo,
    ) -> Option<&BackendProbe> {
        self.probes.get(&Self::backend_key(upstream, backend))
    }

    fn clamp_backend_selection(&mut self) {
        let count = self
            .selected_upstream()
            .map(|up| up.backends.len())
            .unwrap_or(0);
        self.backend_selected = self.backend_selected.min(count.saturating_sub(1));
    }

    pub fn current_log_source(&self) -> Option<&LogSource> {
        let source = self.logs.sources.get(self.logs.index)?;
        self.logs
            .kind_filter
            .map(|kind| source.kind == kind)
            .unwrap_or(true)
            .then_some(source)
    }

    pub fn log_lines(&self) -> Vec<&str> {
        self.logs
            .snapshot
            .as_ref()
            .map(|snapshot| logs::filter_lines(&snapshot.lines, &self.logs.filter))
            .unwrap_or_default()
    }

    fn pick_default_log(&mut self) {
        let wants = |source: &&LogSource| {
            self.logs
                .kind_filter
                .map(|kind| source.kind == kind)
                .unwrap_or(true)
        };
        let nonempty =
            |source: &&LogSource| source.path.metadata().map(|m| m.len() > 0).unwrap_or(false);
        let best = self
            .logs
            .sources
            .iter()
            .enumerate()
            .filter(|(_, source)| {
                wants(source) && nonempty(source) && source.kind == LogKind::Error
            })
            .max_by_key(|(_, source)| source.path.metadata().and_then(|m| m.modified()).ok())
            .or_else(|| {
                self.logs
                    .sources
                    .iter()
                    .enumerate()
                    .filter(|(_, source)| wants(source) && nonempty(source))
                    .max_by_key(|(_, source)| {
                        source.path.metadata().and_then(|m| m.modified()).ok()
                    })
            })
            .or_else(|| {
                self.logs
                    .sources
                    .iter()
                    .enumerate()
                    .find(|(_, source)| wants(source) && source.kind == LogKind::Error)
            })
            .or_else(|| {
                self.logs
                    .sources
                    .iter()
                    .enumerate()
                    .find(|(_, source)| wants(source))
            });
        self.logs.index = best.map(|(index, _)| index).unwrap_or(0);
    }

    fn select_first_source_of_kind(&mut self, kind: LogKind) {
        if let Some(index) = self.logs.sources.iter().position(|s| s.kind == kind) {
            self.logs.index = index;
        }
    }

    pub fn refresh_log(&mut self) {
        self.logs.last_loaded = Instant::now();
        let Some(source) = self.current_log_source().cloned() else {
            self.logs.snapshot = None;
            self.logs.error = Some(if self.logs.sources.is_empty() {
                "配置中没有找到 access_log / error_log。".into()
            } else {
                "当前筛选下没有日志源。".into()
            });
            return;
        };
        match logs::read_tail(&source.path, 512 * 1024, 800) {
            Ok(snapshot) => {
                self.logs.snapshot = Some(snapshot);
                self.logs.error = None;
                if self.logs.follow {
                    self.logs.scroll_from_bottom = 0;
                }
            }
            Err(err) => {
                self.logs.snapshot = None;
                self.logs.error = Some(format!("{}：{err}", source.path.display()));
            }
        }
    }

    // ------------------------------------------------------------ navigation

    fn handle_sites_key(&mut self, key: KeyEvent) {
        let count = self.filtered_sites().len();
        match key.code {
            KeyCode::Down | KeyCode::Char('j') => {
                if count > 0 {
                    self.site_selected = (self.site_selected + 1).min(count - 1);
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.site_selected = self.site_selected.saturating_sub(1);
            }
            KeyCode::PageDown => {
                self.site_selected = (self.site_selected + 5).min(count.saturating_sub(1));
            }
            KeyCode::PageUp => {
                self.site_selected = self.site_selected.saturating_sub(5);
            }
            KeyCode::Char('g') | KeyCode::Home => self.site_selected = 0,
            KeyCode::Char('G') | KeyCode::End => self.site_selected = count.saturating_sub(1),
            KeyCode::Char(' ') => self.toggle_selected_site(),
            KeyCode::Char('n') => self.open_new_site(),
            KeyCode::Char('e') | KeyCode::Enter => self.edit_selected_site(),
            KeyCode::Char('m') | KeyCode::Char('o') => {
                self.open_site_settings(Some(SettingsField::Domain))
            }
            KeyCode::Char('l') => self.open_site_settings(Some(SettingsField::Listen)),
            KeyCode::Char('w') => self.open_site_settings_field(SettingsField::Root),
            KeyCode::Char('p') => self.open_site_settings_field(SettingsField::Proxy),
            KeyCode::Char('d') => self.request_delete_site(),
            KeyCode::Char('/') => self.open_input(InputTarget::Sites),
            KeyCode::Char('x') => {
                self.site_filter.clear();
                self.site_selected = 0;
            }
            _ => {}
        }
    }

    fn handle_config_key(&mut self, key: KeyEvent) {
        let count = self.filtered_config_files().len();
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        match key.code {
            KeyCode::Down | KeyCode::Char('j') => {
                if count > 0 && self.config_selected + 1 < count {
                    self.config_selected += 1;
                    self.preview_scroll = 0;
                    self.refresh_preview();
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if self.config_selected > 0 {
                    self.config_selected -= 1;
                    self.preview_scroll = 0;
                    self.refresh_preview();
                }
            }
            KeyCode::PageDown => {
                self.config_selected = (self.config_selected + 5).min(count.saturating_sub(1));
                self.preview_scroll = 0;
                self.refresh_preview();
            }
            KeyCode::PageUp => {
                self.config_selected = self.config_selected.saturating_sub(5);
                self.preview_scroll = 0;
                self.refresh_preview();
            }
            KeyCode::Char('g') if !ctrl => {
                self.config_selected = 0;
                self.preview_scroll = 0;
                self.refresh_preview();
            }
            KeyCode::Char('G') => {
                self.config_selected = count.saturating_sub(1);
                self.preview_scroll = 0;
                self.refresh_preview();
            }
            KeyCode::Char('d') if ctrl => {
                let max = self.preview_max_scroll();
                self.preview_scroll = (self.preview_scroll.min(max) + 10).min(max);
            }
            KeyCode::Char('u') if ctrl => {
                let max = self.preview_max_scroll();
                self.preview_scroll = self.preview_scroll.min(max).saturating_sub(10);
            }
            KeyCode::Char('J') => {
                let max = self.preview_max_scroll();
                self.preview_scroll = (self.preview_scroll.min(max) + 3).min(max);
            }
            KeyCode::Char('K') => {
                let max = self.preview_max_scroll();
                self.preview_scroll = self.preview_scroll.min(max).saturating_sub(3);
            }
            KeyCode::Home => self.preview_scroll = 0,
            KeyCode::End => self.preview_scroll = self.preview_max_scroll(),
            KeyCode::Char('e') | KeyCode::Enter => self.edit_selected_config(),
            KeyCode::Char('/') => self.open_input(InputTarget::Config),
            KeyCode::Char('x') => {
                self.config_filter.clear();
                self.config_selected = 0;
                self.refresh_preview();
            }
            _ => {}
        }
    }

    fn preview_max_scroll(&self) -> usize {
        let total = self
            .config_preview
            .as_ref()
            .map(|preview| preview.lines.len())
            .unwrap_or(0);
        total.saturating_sub(self.preview_viewport.get().max(1))
    }

    fn output_max_scroll(&self, lines: usize) -> usize {
        lines.saturating_sub(self.output_viewport.get().max(1))
    }

    fn log_max_scroll(&self) -> usize {
        self.log_lines()
            .len()
            .saturating_sub(self.log_viewport.get().max(1))
    }

    fn help_max_scroll(&self) -> usize {
        crate::ui::help_max_scroll(self.help_viewport.get())
    }

    fn handle_logs_key(&mut self, key: KeyEvent) {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                let max = self.log_max_scroll();
                self.logs.scroll_from_bottom = (self.logs.scroll_from_bottom.min(max) + 1).min(max);
                self.logs.follow = false;
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.logs.scroll_from_bottom = self.logs.scroll_from_bottom.saturating_sub(1);
                self.logs.follow = self.logs.scroll_from_bottom == 0;
            }
            KeyCode::PageUp => {
                let max = self.log_max_scroll();
                self.logs.scroll_from_bottom =
                    (self.logs.scroll_from_bottom.min(max) + 10).min(max);
                self.logs.follow = false;
            }
            KeyCode::PageDown => {
                self.logs.scroll_from_bottom = self.logs.scroll_from_bottom.saturating_sub(10);
                self.logs.follow = self.logs.scroll_from_bottom == 0;
            }
            KeyCode::Char('g') if !ctrl => {
                self.logs.scroll_from_bottom = self.log_max_scroll();
                self.logs.follow = false;
            }
            KeyCode::Char('G') => {
                self.logs.scroll_from_bottom = 0;
                self.logs.follow = true;
            }
            KeyCode::Home => {
                self.logs.hscroll = 0;
            }
            KeyCode::End => {
                self.logs.scroll_from_bottom = 0;
                self.logs.follow = true;
            }
            KeyCode::Char('h') if !ctrl => {
                self.logs.hscroll = self.logs.hscroll.saturating_sub(4);
            }
            KeyCode::Char('l') if !ctrl => {
                self.logs.hscroll = self.logs.hscroll.saturating_add(4).min(400);
            }
            KeyCode::Char('[') => self.cycle_source(-1),
            KeyCode::Char(']') => self.cycle_source(1),
            KeyCode::Char('L') => self.cycle_log_kind(),
            KeyCode::Char('f') => {
                self.logs.follow = !self.logs.follow;
                if self.logs.follow {
                    self.logs.scroll_from_bottom = 0;
                }
                let follow = self.logs.follow;
                self.toast(
                    ToastKind::Info,
                    if follow {
                        "日志跟随已开启"
                    } else {
                        "日志跟随已暂停"
                    },
                );
            }
            KeyCode::Char('a') => {
                self.logs.auto = !self.logs.auto;
                let auto = self.logs.auto;
                self.toast(
                    ToastKind::Info,
                    if auto {
                        "自动刷新已开启"
                    } else {
                        "自动刷新已暂停"
                    },
                );
            }
            KeyCode::Char('u') => self.refresh_log(),
            KeyCode::Char('/') => self.open_input(InputTarget::Logs),
            KeyCode::Char('x') => {
                self.logs.filter.clear();
                self.logs.scroll_from_bottom = 0;
                self.logs.follow = true;
            }
            _ => {}
        }
    }

    fn cycle_source(&mut self, direction: isize) {
        let sources: Vec<usize> = self
            .logs
            .sources
            .iter()
            .enumerate()
            .filter(|(_, s)| {
                self.logs
                    .kind_filter
                    .map(|kind| s.kind == kind)
                    .unwrap_or(true)
            })
            .map(|(idx, _)| idx)
            .collect();
        if sources.is_empty() {
            self.toast(ToastKind::Warning, "没有可用的日志源");
            return;
        }
        let current = sources.iter().position(|idx| *idx == self.logs.index);
        let next = match current {
            Some(pos) if direction < 0 => (pos + sources.len() - 1) % sources.len(),
            Some(pos) => (pos + 1) % sources.len(),
            None => 0,
        };
        self.logs.index = sources[next];
        self.logs.scroll_from_bottom = 0;
        self.logs.follow = true;
        self.refresh_log();
    }

    fn cycle_log_kind(&mut self) {
        self.logs.kind_filter = match self.logs.kind_filter {
            None => Some(LogKind::Access),
            Some(LogKind::Access) => Some(LogKind::Error),
            Some(LogKind::Error) => Some(LogKind::System),
            Some(LogKind::System) => None,
        };
        if let Some(kind) = self.logs.kind_filter {
            self.select_first_source_of_kind(kind);
        }
        self.logs.scroll_from_bottom = 0;
        self.logs.follow = true;
        self.refresh_log();
        let label = match self.logs.kind_filter {
            None => "全部日志",
            Some(LogKind::Access) => "访问日志",
            Some(LogKind::Error) => "错误日志",
            Some(LogKind::System) => "系统日志",
        };
        self.toast(ToastKind::Info, format!("日志类型：{label}"));
    }

    fn handle_certs_key(&mut self, key: KeyEvent) {
        let count = self.certs.len();
        match key.code {
            KeyCode::Down | KeyCode::Char('j') => {
                self.cert_selected = (self.cert_selected + 1).min(count.saturating_sub(1));
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.cert_selected = self.cert_selected.saturating_sub(1);
            }
            KeyCode::PageDown => {
                self.cert_selected = (self.cert_selected + 3).min(count.saturating_sub(1));
            }
            KeyCode::PageUp => {
                self.cert_selected = self.cert_selected.saturating_sub(3);
            }
            KeyCode::Char('g') | KeyCode::Home => self.cert_selected = 0,
            KeyCode::Char('G') | KeyCode::End => self.cert_selected = count.saturating_sub(1),
            KeyCode::Enter => self.open_cert_details(),
            KeyCode::Char('n') => {
                if self.read_only {
                    self.toast(ToastKind::Warning, "只读模式：不能生成证书");
                } else {
                    self.open_input(InputTarget::SslDomain);
                }
            }
            _ => {}
        }
    }

    fn handle_upstreams_key(&mut self, key: KeyEvent) {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        match key.code {
            KeyCode::Left | KeyCode::Char('h') if !ctrl => {
                self.upstream_focus = UpstreamFocus::Pools;
            }
            KeyCode::Right | KeyCode::Char('l') if !ctrl => {
                self.upstream_focus = UpstreamFocus::Backends;
            }
            KeyCode::Down | KeyCode::Char('j') => match self.upstream_focus {
                UpstreamFocus::Pools => {
                    let count = self.config.upstreams.len();
                    self.upstream_selected =
                        (self.upstream_selected + 1).min(count.saturating_sub(1));
                    self.backend_selected = 0;
                }
                UpstreamFocus::Backends => {
                    let count = self
                        .selected_upstream()
                        .map(|u| u.backends.len())
                        .unwrap_or(0);
                    self.backend_selected =
                        (self.backend_selected + 1).min(count.saturating_sub(1));
                }
            },
            KeyCode::Up | KeyCode::Char('k') => match self.upstream_focus {
                UpstreamFocus::Pools => {
                    self.upstream_selected = self.upstream_selected.saturating_sub(1);
                    self.backend_selected = 0;
                }
                UpstreamFocus::Backends => {
                    self.backend_selected = self.backend_selected.saturating_sub(1);
                }
            },
            KeyCode::Char('g') if !ctrl => match self.upstream_focus {
                UpstreamFocus::Pools => self.upstream_selected = 0,
                UpstreamFocus::Backends => self.backend_selected = 0,
            },
            KeyCode::Char('G') => match self.upstream_focus {
                UpstreamFocus::Pools => {
                    self.upstream_selected = self.config.upstreams.len().saturating_sub(1)
                }
                UpstreamFocus::Backends => {
                    self.backend_selected = self
                        .selected_upstream()
                        .map(|u| u.backends.len().saturating_sub(1))
                        .unwrap_or(0)
                }
            },
            KeyCode::Enter => self.probe_selected_backend(),
            KeyCode::Char('p') => self.probe_all_backends(),
            _ => {}
        }
    }

    fn handle_files_key(&mut self, key: KeyEvent) {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        match key.code {
            KeyCode::Down | KeyCode::Char('j') => self.files.move_selected(1),
            KeyCode::Up | KeyCode::Char('k') => self.files.move_selected(-1),
            KeyCode::PageDown => self.files.move_selected(8),
            KeyCode::PageUp => self.files.move_selected(-8),
            KeyCode::Char('g') | KeyCode::Home => self.files.selected = 0,
            KeyCode::Char('G') | KeyCode::End => {
                self.files.selected = self.files.entries.len().saturating_sub(1)
            }
            KeyCode::Enter => self.open_selected_file(),
            KeyCode::Backspace | KeyCode::Char('h') if !ctrl => self.files.go_up(),
            KeyCode::Char('.') => self.files.toggle_hidden(),
            KeyCode::Char('o') => self.open_input(InputTarget::FilePath),
            KeyCode::Char('n') => self.open_input(InputTarget::NewDir),
            KeyCode::Char('t') => self.open_input(InputTarget::NewFile),
            KeyCode::Char('r') => self.open_input(InputTarget::RenameFile),
            KeyCode::Char('v') => self.preview_selected_file(),
            KeyCode::Char('e') => self.edit_selected_file(),
            KeyCode::Char('d') => self.request_file_delete(),
            _ => {}
        }
    }

    fn handle_docker_key(&mut self, key: KeyEvent) {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let count = match self.docker_view {
            DockerView::Containers => self.docker.containers.len(),
            DockerView::Images => self.docker.images.len(),
        };
        match key.code {
            KeyCode::Down | KeyCode::Char('j') => {
                self.docker_selected = (self.docker_selected + 1).min(count.saturating_sub(1))
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.docker_selected = self.docker_selected.saturating_sub(1)
            }
            KeyCode::PageDown => {
                self.docker_selected = (self.docker_selected + 5).min(count.saturating_sub(1))
            }
            KeyCode::PageUp => self.docker_selected = self.docker_selected.saturating_sub(5),
            KeyCode::Char('g') if !ctrl => self.docker_selected = 0,
            KeyCode::Char('G') => self.docker_selected = count.saturating_sub(1),
            KeyCode::Char('i') => {
                self.docker_view = match self.docker_view {
                    DockerView::Containers => DockerView::Images,
                    DockerView::Images => DockerView::Containers,
                };
                self.docker_selected = 0;
                let label = self.docker_view.label();
                self.toast(ToastKind::Info, format!("Docker：{label}"));
            }
            KeyCode::Char('u') => {
                self.refresh_docker();
                self.toast(ToastKind::Success, "Docker 数据已刷新");
            }
            KeyCode::Char('l') => self.open_docker_logs(),
            KeyCode::Char('s') => self.docker_action_current("start"),
            KeyCode::Char('S') => self.docker_action_current("stop"),
            KeyCode::Char('r') => self.docker_action_current("restart"),
            KeyCode::Char('d') => self.request_docker_remove(),
            _ => {}
        }
    }

    fn handle_monitor_key(&mut self, key: KeyEvent) {
        if matches!(key.code, KeyCode::Char('u')) {
            self.system.tick(true);
            self.toast(ToastKind::Success, "资源采样已刷新");
        }
    }

    fn handle_ftp_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('s') => self.ftp_action("start", false),
            KeyCode::Char('S') => self.ftp_action("stop", true),
            KeyCode::Char('r') => self.ftp_action("restart", true),
            KeyCode::Char('l') => self.ftp_action("reload", false),
            KeyCode::Char('e') => {
                if let Some(path) = self.ftp.config_path.clone() {
                    self.open_file_editor(path);
                } else {
                    self.toast(ToastKind::Warning, "没有找到 vsftpd 配置文件");
                }
            }
            KeyCode::Char('u') => {
                self.refresh_ftp();
                self.toast(ToastKind::Success, "FTP 状态已刷新");
            }
            _ => {}
        }
    }

    fn handle_settings_key(&mut self, key: KeyEvent) {
        if matches!(key.code, KeyCode::Char('e')) {
            let main = self.config.main.clone();
            self.open_editor(main);
        }
    }

    fn open_selected_file(&mut self) {
        let Some(entry) = self.files.selected().cloned() else {
            return;
        };
        if entry.is_dir {
            self.files.navigate(&entry.path);
        } else {
            self.preview_selected_file();
        }
    }

    fn preview_selected_file(&mut self) {
        let Some(entry) = self.files.selected().cloned() else {
            return;
        };
        if entry.is_dir {
            self.toast(ToastKind::Info, "目录不能作为文本预览");
            return;
        }
        match crate::files::preview(&entry.path) {
            Ok((lines, binary)) => {
                let title = format!("预览 · {}", entry.name);
                self.overlay = Overlay::Output(OutputState {
                    title,
                    success: !binary,
                    lines,
                    scroll: 0,
                });
            }
            Err(err) => self.toast(ToastKind::Danger, err),
        }
    }

    fn edit_selected_file(&mut self) {
        let Some(entry) = self.files.selected().cloned() else {
            return;
        };
        if entry.is_dir {
            self.toast(ToastKind::Info, "目录不能编辑");
            return;
        }
        self.open_file_editor(entry.path);
    }

    fn request_file_delete(&mut self) {
        if self.read_only {
            self.toast(ToastKind::Warning, "只读模式：不能删除文件");
            return;
        }
        let Some(entry) = self.files.selected().cloned() else {
            return;
        };
        self.overlay = Overlay::Confirm(ConfirmState {
            title: format!("删除 {}？", entry.name),
            message: vec![
                entry.path.display().to_string(),
                "文件会移入 ~/.nserver/trash，可以手动恢复。".into(),
            ],
            action: ConfirmAction::FileDelete {
                path: entry.path,
                name: entry.name,
            },
            danger: true,
        });
    }

    fn selected_docker_id(&self) -> Option<(String, String)> {
        match self.docker_view {
            DockerView::Containers => self
                .docker
                .containers
                .get(self.docker_selected)
                .map(|container| (container.id.clone(), container.name.clone())),
            DockerView::Images => self.docker.images.get(self.docker_selected).map(|image| {
                (
                    image.id.clone(),
                    format!("{}:{}", image.repository, image.tag),
                )
            }),
        }
    }

    fn docker_action_current(&mut self, verb: &str) {
        if self.read_only {
            self.toast(ToastKind::Warning, "只读模式：不能操作 Docker 容器");
            return;
        }
        if self.docker_view != DockerView::Containers {
            self.toast(ToastKind::Info, "镜像没有启动/停止操作");
            return;
        }
        let Some((id, name)) = self.selected_docker_id() else {
            return;
        };
        if matches!(verb, "stop" | "restart") {
            self.overlay = Overlay::Confirm(ConfirmState {
                title: format!(
                    "{}容器 {name}？",
                    if verb == "stop" { "停止" } else { "重启" }
                ),
                message: vec!["会立即影响容器内的服务。".into()],
                action: ConfirmAction::Docker {
                    id,
                    name,
                    verb: verb.to_string(),
                    image: false,
                },
                danger: verb == "stop",
            });
        } else {
            match crate::docker::action(&id, verb) {
                Ok(text) => {
                    self.refresh_docker();
                    self.toast(
                        ToastKind::Success,
                        format!(
                            "{name} {}",
                            if verb == "start" {
                                "已启动"
                            } else {
                                "操作完成"
                            }
                        ),
                    );
                    let _ = text;
                }
                Err(err) => self.toast(ToastKind::Danger, err),
            }
        }
    }

    fn request_docker_remove(&mut self) {
        if self.read_only {
            self.toast(ToastKind::Warning, "只读模式：不能删除 Docker 资源");
            return;
        }
        let Some((id, name)) = self.selected_docker_id() else {
            return;
        };
        let image = self.docker_view == DockerView::Images;
        self.overlay = Overlay::Confirm(ConfirmState {
            title: format!("删除{} {name}？", if image { "镜像" } else { "容器" }),
            message: vec![if image {
                "镜像删除后需要重新拉取或构建。".into()
            } else {
                "容器会被强制删除，数据卷不会被自动删除。".into()
            }],
            action: ConfirmAction::Docker {
                id,
                name,
                verb: "remove".into(),
                image,
            },
            danger: true,
        });
    }

    fn open_docker_logs(&mut self) {
        if self.docker_view != DockerView::Containers {
            self.toast(ToastKind::Info, "镜像没有运行日志");
            return;
        }
        let Some((id, name)) = self.selected_docker_id() else {
            return;
        };
        match crate::docker::logs(&id, 300) {
            Ok(text) => {
                self.overlay = Overlay::Output(OutputState {
                    title: format!("Docker 日志 · {name}"),
                    success: true,
                    lines: text.lines().map(ToString::to_string).collect(),
                    scroll: 0,
                });
            }
            Err(err) => self.toast(ToastKind::Danger, err),
        }
    }

    fn ftp_action(&mut self, verb: &str, confirm: bool) {
        if self.read_only {
            self.toast(ToastKind::Warning, "只读模式：不能修改 FTP 服务");
            return;
        }
        if !self.ftp.installed {
            self.toast(ToastKind::Warning, "没有检测到 vsftpd");
            return;
        }
        if confirm {
            self.overlay = Overlay::Confirm(ConfirmState {
                title: format!("FTP 服务：{}？", verb),
                message: vec!["会短暂中断 FTP 连接。".into()],
                action: ConfirmAction::FtpAction {
                    verb: verb.to_string(),
                },
                danger: verb == "stop",
            });
        } else {
            match crate::ftp::service_action(verb) {
                Ok(text) => {
                    self.refresh_ftp();
                    self.toast(ToastKind::Success, format!("FTP {verb} 完成"));
                    let _ = text;
                }
                Err(err) => self.toast(ToastKind::Danger, err),
            }
        }
    }

    // ---------------------------------------------------------------- input

    fn open_input(&mut self, target: InputTarget) {
        let (title, value) = match target {
            InputTarget::Sites => ("筛选站点", self.site_filter.clone()),
            InputTarget::Config => ("筛选配置文件", self.config_filter.clone()),
            InputTarget::Logs => ("筛选日志", self.logs.filter.clone()),
            InputTarget::FilePath => ("跳转路径", self.files.cwd.display().to_string()),
            InputTarget::NewDir => ("新建目录名称", String::new()),
            InputTarget::NewFile => ("新建文件名称", String::new()),
            InputTarget::RenameFile => (
                "重命名",
                self.files
                    .selected()
                    .map(|entry| entry.name.clone())
                    .unwrap_or_default(),
            ),
            InputTarget::SslDomain => ("自签名证书域名", String::new()),
        };
        self.overlay = Overlay::Input(InputState {
            target,
            title: title.into(),
            value,
        });
    }

    fn apply_input(&mut self, input: &InputState) {
        match input.target {
            InputTarget::Sites => {
                self.site_filter = input.value.trim().to_string();
                self.site_selected = 0;
            }
            InputTarget::Config => {
                self.config_filter = input.value.trim().to_string();
                self.config_selected = 0;
                self.preview_scroll = 0;
                self.refresh_preview();
            }
            InputTarget::Logs => {
                self.logs.filter = input.value.trim().to_string();
                self.logs.scroll_from_bottom = 0;
                self.logs.follow = true;
            }
            InputTarget::FilePath => {
                if !input.value.trim().is_empty() {
                    self.files.navigate(Path::new(input.value.trim()));
                }
            }
            InputTarget::NewDir | InputTarget::NewFile => {
                if self.read_only {
                    self.toast(ToastKind::Warning, "只读模式：不能创建文件或目录");
                    return;
                }
                let name = input.value.trim();
                let result = if input.target == InputTarget::NewDir {
                    self.files.create_dir(name)
                } else {
                    self.files.create_file(name)
                };
                match result {
                    Ok(_) => self.toast(ToastKind::Success, "已创建"),
                    Err(err) => self.toast(ToastKind::Danger, err),
                }
            }
            InputTarget::RenameFile if self.read_only => {
                self.toast(ToastKind::Warning, "只读模式：不能重命名");
            }
            InputTarget::RenameFile => match self.files.rename_selected(input.value.trim()) {
                Ok(_) => self.toast(ToastKind::Success, "已重命名"),
                Err(err) => self.toast(ToastKind::Danger, err),
            },
            InputTarget::SslDomain => {
                match crate::cert::generate_self_signed(
                    input.value.trim(),
                    &self.config.config_dir,
                    365,
                ) {
                    Ok((cert, key)) => {
                        let lines = vec![
                            "已生成自签名证书（365 天）。".to_string(),
                            String::new(),
                            format!("certificate: {}", cert.display()),
                            format!("key:         {}", key.display()),
                            String::new(),
                            "接下来在站点配置的 server 块中添加：".to_string(),
                            format!("    ssl_certificate     {};", cert.display()),
                            format!("    ssl_certificate_key {};", key.display()),
                            "并添加 listen 443 ssl;，保存后按 t 校验。".to_string(),
                        ];
                        self.certs = crate::cert::discover(&self.config);
                        self.overlay = Overlay::Output(OutputState {
                            title: "自签名证书".into(),
                            success: true,
                            lines,
                            scroll: 0,
                        });
                    }
                    Err(err) => self.toast(ToastKind::Danger, err),
                }
            }
        }
    }

    fn handle_overlay(&mut self, key: KeyEvent) {
        let overlay = std::mem::replace(&mut self.overlay, Overlay::None);
        match overlay {
            Overlay::None => {}
            Overlay::Help { mut scroll } => match key.code {
                KeyCode::Esc | KeyCode::Char('?') | KeyCode::Char('q') | KeyCode::Enter => {}
                KeyCode::Down | KeyCode::Char('j') => {
                    let max = self.help_max_scroll();
                    scroll = (scroll.min(max) + 1).min(max);
                    self.overlay = Overlay::Help { scroll };
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    let max = self.help_max_scroll();
                    scroll = scroll.min(max).saturating_sub(1);
                    self.overlay = Overlay::Help { scroll };
                }
                KeyCode::PageDown | KeyCode::Char(' ') => {
                    let max = self.help_max_scroll();
                    scroll = (scroll.min(max) + 8).min(max);
                    self.overlay = Overlay::Help { scroll };
                }
                KeyCode::PageUp => {
                    let max = self.help_max_scroll();
                    scroll = scroll.min(max).saturating_sub(8);
                    self.overlay = Overlay::Help { scroll };
                }
                KeyCode::Char('g') | KeyCode::Home => {
                    self.overlay = Overlay::Help { scroll: 0 };
                }
                KeyCode::Char('G') | KeyCode::End => {
                    self.overlay = Overlay::Help {
                        scroll: self.help_max_scroll(),
                    };
                }
                _ => self.overlay = Overlay::Help { scroll },
            },
            Overlay::Confirm(confirm) => match key.code {
                KeyCode::Enter | KeyCode::Char('y') | KeyCode::Char('Y') => {
                    self.execute_confirm(confirm.action);
                }
                KeyCode::Esc | KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Char('q') => {
                    if matches!(confirm.action, ConfirmAction::DiscardEditor) {
                        self.toast(ToastKind::Info, "继续编辑");
                    }
                }
                _ => self.overlay = Overlay::Confirm(confirm),
            },
            Overlay::Output(mut output) => match key.code {
                KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q') => {}
                KeyCode::Down | KeyCode::Char('j') => {
                    let max = self.output_max_scroll(output.lines.len());
                    output.scroll = (output.scroll.min(max) + 1).min(max);
                    self.overlay = Overlay::Output(output);
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    let max = self.output_max_scroll(output.lines.len());
                    output.scroll = output.scroll.min(max).saturating_sub(1);
                    self.overlay = Overlay::Output(output);
                }
                KeyCode::PageDown | KeyCode::Char(' ') => {
                    let max = self.output_max_scroll(output.lines.len());
                    output.scroll = (output.scroll.min(max) + 10).min(max);
                    self.overlay = Overlay::Output(output);
                }
                KeyCode::PageUp => {
                    let max = self.output_max_scroll(output.lines.len());
                    output.scroll = output.scroll.min(max).saturating_sub(10);
                    self.overlay = Overlay::Output(output);
                }
                KeyCode::Char('g') | KeyCode::Home => {
                    output.scroll = 0;
                    self.overlay = Overlay::Output(output);
                }
                KeyCode::Char('G') | KeyCode::End => {
                    let max = self.output_max_scroll(output.lines.len());
                    output.scroll = max;
                    self.overlay = Overlay::Output(output);
                }
                _ => self.overlay = Overlay::Output(output),
            },
            Overlay::Input(mut input) => match key.code {
                KeyCode::Esc => {}
                KeyCode::Enter => {
                    self.apply_input(&input);
                    self.toast(ToastKind::Info, "筛选已应用");
                }
                KeyCode::Backspace => {
                    input.value.pop();
                    self.overlay = Overlay::Input(input);
                }
                KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    input.value.clear();
                    self.overlay = Overlay::Input(input);
                }
                KeyCode::Char(c)
                    if !key.modifiers.contains(KeyModifiers::CONTROL)
                        && !key.modifiers.contains(KeyModifiers::ALT) =>
                {
                    input.value.push(c);
                    self.overlay = Overlay::Input(input);
                }
                _ => self.overlay = Overlay::Input(input),
            },
            Overlay::NewSite(mut form) => {
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    && matches!(key.code, KeyCode::Char('s'))
                {
                    if let Err(err) = self.try_create_site(&mut form) {
                        form.error = Some(err);
                        self.overlay = Overlay::NewSite(form);
                    }
                    return;
                }
                let field = form.current_field();
                match key.code {
                    KeyCode::Esc => {}
                    KeyCode::Tab | KeyCode::Down => {
                        form.move_focus(1);
                        self.overlay = Overlay::NewSite(form);
                    }
                    KeyCode::BackTab | KeyCode::Up => {
                        form.move_focus(-1);
                        self.overlay = Overlay::NewSite(form);
                    }
                    KeyCode::Left if field == NewSiteField::Kind => {
                        form.cycle_kind(-1);
                        self.overlay = Overlay::NewSite(form);
                    }
                    KeyCode::Right if field == NewSiteField::Kind => {
                        form.cycle_kind(1);
                        self.overlay = Overlay::NewSite(form);
                    }
                    KeyCode::Char(' ') if field == NewSiteField::Kind => {
                        form.cycle_kind(1);
                        self.overlay = Overlay::NewSite(form);
                    }
                    KeyCode::Enter => {
                        if field == NewSiteField::Enable {
                            if let Err(err) = self.try_create_site(&mut form) {
                                form.error = Some(err);
                                self.overlay = Overlay::NewSite(form);
                            }
                        } else if field.is_toggle() {
                            form.toggle_field(field);
                            self.overlay = Overlay::NewSite(form);
                        } else {
                            form.move_focus(1);
                            self.overlay = Overlay::NewSite(form);
                        }
                    }
                    KeyCode::Char(' ') if field.is_toggle() => {
                        form.toggle_field(field);
                        self.overlay = Overlay::NewSite(form);
                    }
                    KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        if let Some(value) = form.field_mut(field) {
                            value.clear();
                        }
                        form.error = None;
                        self.overlay = Overlay::NewSite(form);
                    }
                    KeyCode::Backspace => {
                        if let Some(value) = form.field_mut(field) {
                            value.pop();
                        }
                        form.error = None;
                        self.overlay = Overlay::NewSite(form);
                    }
                    KeyCode::Char(c)
                        if !key.modifiers.contains(KeyModifiers::CONTROL)
                            && !key.modifiers.contains(KeyModifiers::ALT)
                            && field != NewSiteField::Kind
                            && !field.is_toggle() =>
                    {
                        if let Some(value) = form.field_mut(field) {
                            value.push(c);
                        }
                        form.error = None;
                        self.overlay = Overlay::NewSite(form);
                    }
                    _ => self.overlay = Overlay::NewSite(form),
                }
            }
            Overlay::SiteSettings(mut state) => {
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    && matches!(key.code, KeyCode::Char('s'))
                {
                    if let Err(err) = self.try_save_site_settings(&mut state) {
                        state.error = Some(err);
                        self.overlay = Overlay::SiteSettings(state);
                    }
                    return;
                }
                let field = state.current_field();
                match key.code {
                    KeyCode::Esc => {}
                    KeyCode::Tab | KeyCode::Down => {
                        state.move_focus(1);
                        self.overlay = Overlay::SiteSettings(state);
                    }
                    KeyCode::BackTab | KeyCode::Up => {
                        state.move_focus(-1);
                        self.overlay = Overlay::SiteSettings(state);
                    }
                    KeyCode::Enter => {
                        if field.is_toggle() {
                            state.toggle_field(field);
                            self.overlay = Overlay::SiteSettings(state);
                        } else if state.focus + 1 >= state.visible_fields().len() {
                            if let Err(err) = self.try_save_site_settings(&mut state) {
                                state.error = Some(err);
                                self.overlay = Overlay::SiteSettings(state);
                            }
                        } else {
                            state.move_focus(1);
                            self.overlay = Overlay::SiteSettings(state);
                        }
                    }
                    KeyCode::Char(' ') if field.is_toggle() => {
                        state.toggle_field(field);
                        self.overlay = Overlay::SiteSettings(state);
                    }
                    KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        if let Some(value) = state.field_mut(field) {
                            value.clear();
                        }
                        state.error = None;
                        self.overlay = Overlay::SiteSettings(state);
                    }
                    KeyCode::Backspace => {
                        if let Some(value) = state.field_mut(field) {
                            value.pop();
                        }
                        state.error = None;
                        self.overlay = Overlay::SiteSettings(state);
                    }
                    KeyCode::Char(c)
                        if !key.modifiers.contains(KeyModifiers::CONTROL)
                            && !key.modifiers.contains(KeyModifiers::ALT)
                            && !field.is_toggle() =>
                    {
                        if let Some(value) = state.field_mut(field) {
                            value.push(c);
                        }
                        state.error = None;
                        self.overlay = Overlay::SiteSettings(state);
                    }
                    _ => self.overlay = Overlay::SiteSettings(state),
                }
            }
        }
    }

    // -------------------------------------------------------------- service

    fn toggle_service(&mut self) {
        if self.process.running() {
            if self.read_only {
                self.toast(ToastKind::Warning, "只读模式：不能停止 Nginx");
                return;
            }
            self.overlay = Overlay::Confirm(ConfirmState {
                title: "停止 Nginx？".into(),
                message: vec![
                    "停止后所有站点将立即不可访问。".into(),
                    "配置文件不会被修改。".into(),
                ],
                action: ConfirmAction::Service(Action::Stop),
                danger: true,
            });
        } else {
            self.run_service(Action::Start);
        }
    }

    fn request_restart(&mut self) {
        if self.read_only {
            self.toast(ToastKind::Warning, "只读模式：不能重启 Nginx");
            return;
        }
        if !self.process.running() {
            self.toast(ToastKind::Warning, "Nginx 当前未运行，请按 s 启动");
            return;
        }
        self.overlay = Overlay::Confirm(ConfirmState {
            title: "重启 Nginx？".into(),
            message: vec![
                "重启会短暂中断所有站点的访问。".into(),
                "会先执行 nginx -t 校验配置。".into(),
            ],
            action: ConfirmAction::Service(Action::Restart),
            danger: true,
        });
    }

    fn run_service(&mut self, action: Action) {
        if action != Action::Test && self.read_only {
            self.toast(ToastKind::Warning, "只读模式：操作已禁用");
            return;
        }
        if action == Action::Reload && !self.process.running() {
            self.toast(ToastKind::Warning, "Nginx 未运行，请按 s 启动");
            return;
        }
        if self.config_missing {
            self.overlay = Overlay::Output(OutputState::from_message(
                format!("{}已阻止", action.label()),
                false,
                format!("配置文件不存在：{}", self.config.main.display()),
            ));
            return;
        }

        let bin = self.bin.clone();
        let config = self.config.main.clone();
        let pid_file = self.config.pid_file.clone();
        let manager = self.manager;
        let mut results = Vec::new();

        if matches!(
            action,
            Action::Test | Action::Reload | Action::Restart | Action::Start
        ) {
            let test = service::test_config(bin.as_deref(), &config);
            let test_ok = test.success;
            self.last_test = Some(test.clone());
            results.push(test);
            if !test_ok && action != Action::Test {
                self.overlay = Overlay::Output(OutputState::from_results(
                    format!("{}已阻止：配置校验失败", action.label()),
                    &results,
                ));
                self.toast(ToastKind::Danger, "配置校验失败，操作已阻止");
                return;
            }
        }
        if action != Action::Test {
            results.extend(service::perform(
                action,
                bin.as_deref(),
                &config,
                &pid_file,
                manager,
            ));
        }
        self.refresh_process();
        let title = action.label();
        self.overlay = Overlay::Output(OutputState::from_results(title, &results));
        if results.iter().all(|r| r.success) {
            self.toast(ToastKind::Success, format!("{}完成", action.label()));
        } else {
            self.toast(ToastKind::Danger, format!("{}未完成", action.label()));
        }
    }

    fn execute_confirm(&mut self, action: ConfirmAction) {
        match action {
            ConfirmAction::Service(action) => self.run_service(action),
            ConfirmAction::ToggleSite { enable } => self.perform_site_toggle(enable),
            ConfirmAction::DeleteSite => self.perform_delete_site(),
            ConfirmAction::DiscardEditor => {
                let had_saved_changes = self
                    .editor
                    .as_ref()
                    .map(|editor| editor.changed_on_disk)
                    .unwrap_or(false);
                self.editor = None;
                if had_saved_changes {
                    self.refresh_all(true);
                }
                self.toast(ToastKind::Info, "已放弃未保存的修改");
            }
            ConfirmAction::Quit => self.should_quit = true,
            ConfirmAction::Docker {
                id,
                name,
                verb,
                image,
            } => {
                let result = if image && verb == "remove" {
                    crate::docker::remove_image(&id)
                } else if verb == "remove" {
                    crate::docker::remove_container(&id)
                } else {
                    crate::docker::action(&id, &verb)
                };
                match result {
                    Ok(_) => {
                        self.refresh_docker();
                        self.toast(ToastKind::Success, format!("{name}：{verb} 完成"));
                    }
                    Err(err) => self.toast(ToastKind::Danger, err),
                }
            }
            ConfirmAction::FtpAction { verb } => match crate::ftp::service_action(&verb) {
                Ok(_) => {
                    self.refresh_ftp();
                    self.toast(ToastKind::Success, format!("FTP {verb} 完成"));
                }
                Err(err) => self.toast(ToastKind::Danger, err),
            },
            ConfirmAction::FileDelete { path, name } => {
                match crate::files::delete_path_to_trash(&path) {
                    Ok(target) => {
                        self.files.refresh();
                        self.toast(
                            ToastKind::Success,
                            format!("{name} 已移入 {}", target.display()),
                        );
                    }
                    Err(err) => self.toast(ToastKind::Danger, err),
                }
            }
        }
    }

    // ---------------------------------------------------------------- sites

    fn toggle_selected_site(&mut self) {
        if self.read_only {
            self.toast(ToastKind::Warning, "只读模式：不能启用或停用站点");
            return;
        }
        let Some(site) = self.selected_site().cloned() else {
            return;
        };
        if !site.can_toggle {
            self.toast(ToastKind::Info, "这个配置不在 sites-available 中，无法切换");
            return;
        }
        if site.enabled {
            self.overlay = Overlay::Confirm(ConfirmState {
                title: format!("停用站点 {}？", site.name),
                message: vec![
                    "将删除 sites-enabled 中的符号链接。".into(),
                    "配置文件会保留，按 r 热重载后生效。".into(),
                ],
                action: ConfirmAction::ToggleSite { enable: false },
                danger: false,
            });
        } else {
            self.perform_site_toggle(true);
        }
    }

    fn perform_site_toggle(&mut self, enable: bool) {
        if self.read_only {
            self.toast(ToastKind::Warning, "只读模式：操作已禁用");
            return;
        }
        let Some(site) = self.selected_site().cloned() else {
            return;
        };
        let name = site.name.clone();
        match self.sites.toggle(&site, enable) {
            Ok(message) => {
                self.last_test = None;
                self.refresh_all(true);
                self.site_selected = self
                    .filtered_sites()
                    .iter()
                    .position(|s| s.name == name)
                    .unwrap_or(0);
                self.toast(ToastKind::Success, message);
            }
            Err(err) => self.toast(ToastKind::Danger, err),
        }
    }

    fn request_delete_site(&mut self) {
        if self.read_only {
            self.toast(ToastKind::Warning, "只读模式：不能删除站点");
            return;
        }
        let Some(site) = self.selected_site().cloned() else {
            return;
        };
        if site.enabled {
            self.toast(ToastKind::Warning, "站点仍在启用中，请先按 Space 停用");
            return;
        }
        if !site.can_toggle {
            self.toast(ToastKind::Info, "只能管理 sites-available 中的站点");
            return;
        }
        self.overlay = Overlay::Confirm(ConfirmState {
            title: format!("删除站点 {}？", site.name),
            message: vec![
                "配置文件会被移入 sites-available/.trash。".into(),
                "之后可以手动恢复，但 Nginx 不再读取它。".into(),
            ],
            action: ConfirmAction::DeleteSite,
            danger: true,
        });
    }

    fn perform_delete_site(&mut self) {
        let Some(site) = self.selected_site().cloned() else {
            return;
        };
        let name = site.name.clone();
        match self.sites.delete_to_trash(&site) {
            Ok(trash) => {
                self.last_test = None;
                self.refresh_all(true);
                self.site_selected = self
                    .filtered_sites()
                    .iter()
                    .position(|s| s.name == name)
                    .unwrap_or(0);
                self.toast(
                    ToastKind::Success,
                    format!("已移入回收站：{}", trash.display()),
                );
            }
            Err(err) => self.toast(ToastKind::Danger, err),
        }
    }

    fn open_new_site(&mut self) {
        if self.read_only {
            self.toast(ToastKind::Warning, "只读模式：不能新建站点");
            return;
        }
        if self.sites.available_dir.is_none() || self.sites.enabled_dir.is_none() {
            self.toast(
                ToastKind::Warning,
                "当前 Nginx 不是 sites-available/sites-enabled 布局",
            );
            return;
        }
        self.overlay = Overlay::NewSite(NewSiteState::new());
    }

    fn try_create_site(&mut self, form: &mut NewSiteState) -> Result<(), String> {
        if self.read_only {
            return Err("只读模式：不能新建站点。".into());
        }
        let path = sites::create_site(&self.config, &self.sites, &form.spec)?;
        self.last_test = None;
        self.refresh_all(true);
        self.tab = Tab::Sites;
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        self.site_selected = self
            .filtered_sites()
            .iter()
            .position(|site| site.name == name)
            .unwrap_or(0);
        self.toast(
            ToastKind::Success,
            format!(
                "已创建 {} · {name}；按 m 设置，按 r 热重载生效",
                form.spec.kind.label()
            ),
        );
        Ok(())
    }

    fn open_site_settings_field(&mut self, field: SettingsField) {
        let Some(site) = self.selected_site() else {
            return;
        };
        let has_root = site.servers.iter().any(|server| server.root.is_some());
        let has_proxy = site
            .servers
            .iter()
            .any(|server| !server.proxy_targets.is_empty());
        let valid = match field {
            SettingsField::Root => has_root,
            SettingsField::Proxy | SettingsField::Websocket => has_proxy,
            _ => true,
        };
        if !valid {
            let message = if field == SettingsField::Root {
                "该站点不是静态网站，没有独立网站目录。"
            } else {
                "该站点没有代理目标；请新建反向代理/端口转发站点。"
            };
            self.toast(ToastKind::Info, message);
            return;
        }
        self.open_site_settings(Some(field));
    }

    fn open_site_settings(&mut self, focus: Option<SettingsField>) {
        if self.read_only {
            self.toast(ToastKind::Warning, "只读模式：不能修改站点");
            return;
        }
        let Some(site) = self.selected_site().cloned() else {
            return;
        };
        let is_available = site
            .available
            .parent()
            .and_then(Path::file_name)
            .map(|name| name == "sites-available")
            .unwrap_or(false);
        if !is_available {
            self.toast(
                ToastKind::Info,
                "这个配置不在 sites-available，按 e 用完整编辑器修改",
            );
            return;
        }
        let path = fs::canonicalize(&site.available).unwrap_or_else(|_| site.available.clone());
        let text = match fs::read_to_string(&path) {
            Ok(text) => text,
            Err(err) => {
                self.toast(ToastKind::Danger, format!("读取站点失败：{err}"));
                return;
            }
        };
        let (domain, listen, root, proxy) = sites::site_settings_values(&text);
        let domain = if domain.trim().is_empty() {
            site.server_names()
        } else {
            domain
        };
        let listen = listen.split_whitespace().next().unwrap_or("80").to_string();
        let kind = site.kind();
        let mut state = SiteSettingsState {
            site_name: site.name.clone(),
            path,
            kind,
            domain: if kind == SiteKind::PortForward && domain == "—" {
                "_".to_string()
            } else {
                domain
            },
            listen,
            root,
            proxy,
            websocket: sites::detect_websocket(&text),
            spa: sites::detect_spa(&text),
            has_root: site.servers.iter().any(|server| server.root.is_some()),
            has_proxy: site
                .servers
                .iter()
                .any(|server| !server.proxy_targets.is_empty()),
            original: SiteSettingsValues {
                domain: String::new(),
                listen: String::new(),
                root: String::new(),
                proxy: String::new(),
                websocket: false,
                spa: false,
            },
            focus: 0,
            error: None,
        };
        state.original = SiteSettingsValues {
            domain: state.domain.clone(),
            listen: state.listen.clone(),
            root: state.root.clone(),
            proxy: state.proxy.clone(),
            websocket: state.websocket,
            spa: state.spa,
        };
        if let Some(field) = focus {
            state.focus_field(field);
        }
        self.overlay = Overlay::SiteSettings(state);
    }

    fn try_save_site_settings(&mut self, state: &mut SiteSettingsState) -> Result<(), String> {
        if self.read_only {
            return Err("只读模式：不能修改站点。".into());
        }
        let config_root = fs::canonicalize(&self.config.config_dir)
            .unwrap_or_else(|_| self.config.config_dir.clone());
        let path = fs::canonicalize(&state.path).unwrap_or_else(|_| state.path.clone());
        if !path.starts_with(&config_root) {
            return Err("只能修改 Nginx 配置目录内的站点。".into());
        }
        let text = fs::read_to_string(&path).map_err(|e| format!("读取站点失败：{e}"))?;
        let domain = if state.kind == SiteKind::PortForward && state.domain.trim().is_empty() {
            "_".to_string()
        } else {
            state.domain.trim().to_string()
        };
        if domain.is_empty() {
            return Err("域名不能为空。".into());
        }
        let listen = state.listen.trim().to_string();
        if listen.is_empty() || listen.contains([';', '{', '}', '\n']) {
            return Err("监听端口无效。".into());
        }

        let mut updated = text.clone();
        let mut changed = false;
        if domain != state.original.domain {
            updated = sites::patch_server_name(&updated, &domain)?;
            changed = true;
        }
        if listen != state.original.listen {
            updated = sites::patch_listen(&updated, &listen)?;
            changed = true;
        }
        if state.has_root {
            let root = state.root.trim();
            if root.is_empty() {
                return Err("网站目录不能为空。".into());
            }
            if root != state.original.root {
                updated = sites::patch_root(&updated, root)?;
                changed = true;
            }
            if state.spa != state.original.spa {
                updated = sites::patch_try_files(&updated, state.spa)?;
                changed = true;
            }
        }
        if state.has_proxy {
            let target = state.proxy.trim();
            if target.is_empty() {
                return Err("代理目标不能为空。".into());
            }
            if target != state.original.proxy || state.websocket != state.original.websocket {
                updated = sites::patch_proxy(&updated, target, state.websocket)?;
                changed = true;
            }
        }

        if !changed || updated == text {
            return Err("没有检测到修改。".into());
        }
        let backup = write_with_backup(
            &path,
            &updated,
            &self.config.config_dir.join(".nserver/backups"),
        )?;
        let name = state.site_name.clone();
        self.last_test = None;
        self.refresh_all(true);
        self.site_selected = self
            .filtered_sites()
            .iter()
            .position(|site| site.name == name)
            .unwrap_or(0);
        self.toast(
            ToastKind::Success,
            format!(
                "已更新 {name}，备份：{}；按 t 校验",
                backup.file_name().unwrap_or_default().to_string_lossy()
            ),
        );
        Ok(())
    }

    fn edit_selected_site(&mut self) {
        if let Some(site) = self.selected_site() {
            let path = site.available.clone();
            self.open_editor(path);
        }
    }

    fn edit_selected_config(&mut self) {
        if let Some(path) = self.selected_config_file().cloned() {
            self.open_editor(path);
        }
    }

    // --------------------------------------------------------------- editor

    fn open_editor(&mut self, path: PathBuf) {
        if self.read_only {
            self.toast(ToastKind::Warning, "只读模式：不能编辑文件");
            return;
        }
        let path = fs::canonicalize(&path).unwrap_or(path);
        let root = fs::canonicalize(&self.config.config_dir)
            .unwrap_or_else(|_| self.config.config_dir.clone());
        if !path.starts_with(&root) {
            self.toast(ToastKind::Danger, "只能编辑 Nginx 配置目录内的文件");
            return;
        }
        self.open_editor_internal(path, false);
    }

    fn open_file_editor(&mut self, path: PathBuf) {
        if self.read_only {
            self.toast(ToastKind::Warning, "只读模式：不能编辑文件");
            return;
        }
        self.open_editor_internal(path, true);
    }

    fn open_editor_internal(&mut self, path: PathBuf, allow_outside: bool) {
        let path = fs::canonicalize(&path).unwrap_or(path);
        let size = fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
        if size > 1024 * 1024 {
            self.toast(ToastKind::Danger, "文件超过 1 MB，未打开编辑器");
            return;
        }
        let text = match fs::read_to_string(&path) {
            Ok(text) => text,
            Err(err) => {
                self.toast(ToastKind::Danger, format!("无法读取文件：{err}"));
                return;
            }
        };
        let had_trailing_newline = text.ends_with('\n');
        let mut lines: Vec<String> = text.lines().map(ToString::to_string).collect();
        if lines.is_empty() {
            lines.push(String::new());
        }
        let mut textarea = TextArea::new(lines);
        textarea.set_line_number_style(
            Style::default()
                .fg(self.theme.faint)
                .bg(self.theme.panel_bg),
        );
        textarea.set_cursor_line_style(
            Style::default()
                .bg(self.theme.selection_bg)
                .fg(self.theme.fg),
        );
        textarea.set_cursor_style(
            Style::default()
                .bg(self.theme.accent)
                .fg(self.theme.bg)
                .add_modifier(Modifier::BOLD),
        );
        textarea.set_selection_style(
            Style::default()
                .bg(self.theme.selection_bg)
                .fg(self.theme.selection_fg),
        );
        textarea.set_style(Style::default().fg(self.theme.fg).bg(self.theme.panel_bg));
        self.editor = Some(EditorState {
            path,
            textarea,
            dirty: false,
            changed_on_disk: false,
            error: None,
            had_trailing_newline,
            allow_outside,
        });
    }

    fn handle_editor_key(&mut self, key: KeyEvent) {
        let Some(mut editor) = self.editor.take() else {
            return;
        };
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let before = editor.textarea.lines().join("\n");
        match key.code {
            KeyCode::Esc => {
                if editor.dirty {
                    self.editor = Some(editor);
                    self.overlay = Overlay::Confirm(ConfirmState {
                        title: "放弃未保存的修改？".into(),
                        message: vec!["按 Esc 继续编辑，按 Enter 放弃。".into()],
                        action: ConfirmAction::DiscardEditor,
                        danger: true,
                    });
                    return;
                }
                self.close_editor(editor);
                return;
            }
            KeyCode::Char('s') if ctrl => {
                self.save_editor(&mut editor);
            }
            KeyCode::Char('z') if ctrl => {
                editor.textarea.undo();
            }
            KeyCode::Char('y') if ctrl => {
                editor.textarea.redo();
            }
            KeyCode::Tab => {
                editor.textarea.insert_str("    ");
            }
            _ => {
                editor.textarea.input(key);
            }
        }
        let after = editor.textarea.lines().join("\n");
        if before != after {
            editor.dirty = true;
        }
        self.editor = Some(editor);
    }

    fn save_editor(&mut self, editor: &mut EditorState) {
        let mut text = editor.textarea.lines().join("\n");
        if editor.had_trailing_newline && !text.ends_with('\n') {
            text.push('\n');
        }
        let backup_dir = if editor.allow_outside {
            home_dir().join(".nserver/backups")
        } else {
            self.config.config_dir.join(".nserver/backups")
        };
        match write_with_backup(&editor.path, &text, &backup_dir) {
            Ok(backup) => {
                editor.dirty = false;
                editor.changed_on_disk = true;
                editor.error = None;
                self.last_test = None;
                self.toast(
                    ToastKind::Success,
                    format!("已保存；备份在 {}", backup.display()),
                );
            }
            Err(err) => {
                editor.error = Some(err.clone());
                self.toast(ToastKind::Danger, err);
            }
        }
    }

    fn close_editor(&mut self, editor: EditorState) {
        let changed = editor.changed_on_disk;
        self.editor = None;
        if changed {
            self.refresh_all(true);
            self.toast(ToastKind::Info, "已返回 · 按 t 校验配置，按 r 热重载");
        }
    }

    // ----------------------------------------------------------------- certs

    fn open_cert_details(&mut self) {
        let Some(cert) = self.selected_cert() else {
            return;
        };
        let mut lines = vec![
            format!("域名    {}", cert.display_name()),
            format!("状态    {} · {}", cert.status().label(), cert.days_label()),
            format!("到期    {}", cert.expires_label()),
            format!("颁发者  {}", cert.issuer.as_deref().unwrap_or("—")),
            format!("主题    {}", cert.subject.as_deref().unwrap_or("—")),
            format!("文件    {}", cert.path.display()),
        ];
        if !cert.sans.is_empty() {
            lines.push(format!("SAN     {}", cert.sans.join(", ")));
        }
        if let Some(err) = &cert.error {
            lines.push(String::new());
            lines.push(format!("错误：{err}"));
        }
        self.overlay = Overlay::Output(OutputState {
            title: "证书详情".into(),
            success: cert.error.is_none(),
            lines,
            scroll: 0,
        });
    }

    // ------------------------------------------------------------- upstreams

    fn probe_selected_backend(&mut self) {
        let Some(upstream) = self.selected_upstream().cloned() else {
            return;
        };
        let Some(backend) = upstream.backends.get(self.backend_selected).cloned() else {
            return;
        };
        if backend.down {
            self.toast(ToastKind::Warning, "该后端标记为 down，不进行探测");
            return;
        }
        let key = Self::backend_key(&upstream, &backend);
        let result = probe_address(&backend.address);
        match &result {
            Ok(latency) => self.toast(
                ToastKind::Success,
                format!("{} · {} ms", backend.address, latency),
            ),
            Err(err) => self.toast(ToastKind::Danger, format!("{} · {err}", backend.address)),
        }
        let probe = match result {
            Ok(latency_ms) => BackendProbe {
                success: true,
                latency_ms,
                message: "TCP 连接成功".into(),
                at: Instant::now(),
            },
            Err(message) => BackendProbe {
                success: false,
                latency_ms: 0,
                message,
                at: Instant::now(),
            },
        };
        self.probes.insert(key, probe);
    }

    fn probe_all_backends(&mut self) {
        let mut targets: Vec<(String, String)> = Vec::new();
        for upstream in &self.config.upstreams {
            for backend in &upstream.backends {
                if backend.down {
                    continue;
                }
                targets.push((
                    Self::backend_key(upstream, backend),
                    backend.address.clone(),
                ));
            }
        }
        if targets.is_empty() {
            self.toast(ToastKind::Info, "没有可探测的后端");
            return;
        }
        self.toast(
            ToastKind::Info,
            format!("正在并行探测 {} 个后端…", targets.len()),
        );
        let results = probe_many(targets);
        let mut online = 0;
        let total = results.len();
        for (key, result) in results {
            if result.success {
                online += 1;
            }
            self.probes.insert(key, result);
        }
        self.toast(
            if online == total {
                ToastKind::Success
            } else if online == 0 {
                ToastKind::Danger
            } else {
                ToastKind::Warning
            },
            format!("探测完成：{online}/{total} 在线"),
        );
    }
}

fn home_dir() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/root"))
}

fn probe_many(targets: Vec<(String, String)>) -> Vec<(String, BackendProbe)> {
    std::thread::scope(|scope| {
        let handles: Vec<_> = targets
            .into_iter()
            .map(|(key, address)| {
                scope.spawn(move || {
                    let probe = match probe_address(&address) {
                        Ok(latency_ms) => BackendProbe {
                            success: true,
                            latency_ms,
                            message: "TCP 连接成功".into(),
                            at: Instant::now(),
                        },
                        Err(message) => BackendProbe {
                            success: false,
                            latency_ms: 0,
                            message,
                            at: Instant::now(),
                        },
                    };
                    (key, probe)
                })
            })
            .collect();
        handles
            .into_iter()
            .filter_map(|handle| handle.join().ok())
            .collect()
    })
}

fn probe_address(address: &str) -> Result<u128, String> {
    let lower = address.to_ascii_lowercase();
    if lower.starts_with("unix:") {
        return Err("Unix socket 不支持 TCP 探测".into());
    }
    let address = address
        .strip_prefix("http://")
        .or_else(|| address.strip_prefix("https://"))
        .unwrap_or(address);
    let (host, port) = split_host_port(address)?;
    let addrs: Vec<_> = (host.as_str(), port)
        .to_socket_addrs()
        .map_err(|e| format!("解析失败：{e}"))?
        .take(3)
        .collect();
    if addrs.is_empty() {
        return Err("没有解析到地址".into());
    }
    let mut last_error = String::from("连接失败");
    for addr in addrs {
        let started = Instant::now();
        match TcpStream::connect_timeout(&addr, Duration::from_millis(900)) {
            Ok(_) => return Ok(started.elapsed().as_millis()),
            Err(err) => last_error = format!("{err}"),
        }
    }
    Err(last_error)
}

fn split_host_port(address: &str) -> Result<(String, u16), String> {
    let address = address.trim();
    if address.is_empty() {
        return Err("地址为空".into());
    }
    if let Some(rest) = address.strip_prefix('[') {
        let end = rest
            .find(']')
            .ok_or_else(|| "IPv6 地址缺少 ]".to_string())?;
        let host = rest[..end].to_string();
        let port = rest[end + 1..]
            .strip_prefix(':')
            .and_then(|p| p.parse::<u16>().ok())
            .unwrap_or(80);
        return Ok((host, port));
    }
    if let Some((host, port)) = address.rsplit_once(':')
        && !host.is_empty()
        && !host.contains(':')
        && !port.is_empty()
        && port.chars().all(|c| c.is_ascii_digit())
    {
        return Ok((
            host.to_string(),
            port.parse::<u16>().map_err(|_| "端口无效".to_string())?,
        ));
    }
    Ok((address.to_string(), 80))
}

fn write_with_backup(path: &Path, text: &str, backup_dir: &Path) -> Result<PathBuf, String> {
    let meta = fs::metadata(path).map_err(|e| format!("读取文件信息失败：{e}"))?;
    fs::create_dir_all(backup_dir).map_err(|e| format!("创建备份目录失败：{e}"))?;
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let base_name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "config".into());
    let mut backup = backup_dir.join(format!("{stamp}-{base_name}"));
    let mut counter = 1;
    while backup.exists() {
        backup = backup_dir.join(format!("{stamp}-{counter}-{base_name}"));
        counter += 1;
    }
    fs::copy(path, &backup).map_err(|e| format!("无法创建备份：{e}"))?;

    let parent = path.parent().ok_or_else(|| "文件没有父目录".to_string())?;
    let temp = parent.join(format!(
        ".{}.nserver-{}.tmp",
        path.file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "config".into()),
        std::process::id()
    ));
    let result = (|| -> Result<(), String> {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&temp)
            .map_err(|e| format!("创建临时文件失败：{e}"))?;
        file.write_all(text.as_bytes())
            .map_err(|e| format!("写入失败：{e}"))?;
        file.sync_all().map_err(|e| format!("同步失败：{e}"))?;
        file.set_permissions(fs::Permissions::from_mode(meta.permissions().mode()))
            .map_err(|e| format!("设置权限失败：{e}"))?;
        fs::rename(&temp, path).map_err(|e| format!("替换文件失败：{e}"))?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result.map(|_| backup)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::symlink;

    use crate::cli::Cli;
    use crate::config::BackendInfo;
    use crate::config::UpstreamInfo;

    #[test]
    fn backend_key_is_stable() {
        let upstream = UpstreamInfo {
            name: "app".into(),
            ..Default::default()
        };
        let backend = BackendInfo {
            address: "127.0.0.1:3000".into(),
            ..Default::default()
        };
        assert_eq!(App::backend_key(&upstream, &backend), "app|127.0.0.1:3000");
    }

    #[test]
    fn splits_ipv4_ipv6_and_default_port() {
        assert_eq!(
            split_host_port("127.0.0.1:3000").unwrap(),
            ("127.0.0.1".into(), 3000)
        );
        assert_eq!(split_host_port("[::1]:8080").unwrap(), ("::1".into(), 8080));
        assert_eq!(split_host_port("backend").unwrap(), ("backend".into(), 80));
    }

    #[test]
    fn new_site_wizard_switches_site_types() {
        let mut form = NewSiteState::new();
        assert_eq!(form.spec.kind, SiteKind::Static);
        assert!(form.visible_fields().contains(&NewSiteField::Root));
        form.cycle_kind(1);
        assert_eq!(form.spec.kind, SiteKind::Proxy);
        assert_eq!(form.spec.proxy, "http://127.0.0.1:3000");
        assert!(form.visible_fields().contains(&NewSiteField::Proxy));
        assert!(!form.visible_fields().contains(&NewSiteField::Root));
        form.cycle_kind(1);
        assert_eq!(form.spec.kind, SiteKind::PortForward);
        assert_eq!(form.spec.server_name, "_");
        assert_eq!(form.spec.file_name, "forward-8080");
        assert_eq!(form.spec.listen, "8080");
    }

    #[test]
    fn quick_settings_updates_root_with_backup() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        fs::create_dir_all(root.join("sites-available")).unwrap();
        fs::create_dir_all(root.join("sites-enabled")).unwrap();
        let site = root.join("sites-available/demo.conf");
        fs::write(
            &site,
            "server {\n    listen 8080;\n    server_name demo.test;\n    root /tmp/old;\n    location / {\n        try_files $uri $uri/ =404;\n    }\n}\n",
        )
        .unwrap();
        symlink(
            "../sites-available/demo.conf",
            root.join("sites-enabled/demo.conf"),
        )
        .unwrap();
        fs::write(
            root.join("nginx.conf"),
            "events {}\nhttp { include sites-enabled/*.conf; }\n",
        )
        .unwrap();

        let mut app = App::new(Cli {
            config: root.join("nginx.conf"),
            ..Default::default()
        });
        app.tab = Tab::Sites;
        app.site_selected = 0;
        app.open_site_settings(Some(SettingsField::Root));
        let previous = std::mem::replace(&mut app.overlay, Overlay::None);
        let Overlay::SiteSettings(mut state) = previous else {
            panic!("settings overlay did not open");
        };
        state.root = "/srv/new-root".into();
        app.try_save_site_settings(&mut state).unwrap();
        let updated = fs::read_to_string(&site).unwrap();
        assert!(updated.contains("root /srv/new-root;"));
        assert!(
            fs::read_dir(root.join(".nserver/backups"))
                .unwrap()
                .next()
                .is_some()
        );
    }

    #[test]
    fn writes_config_with_backup() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("nginx.conf");
        fs::write(&path, "old\n").unwrap();
        let backup =
            write_with_backup(&path, "new\n", &tmp.path().join(".nserver/backups")).unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "new\n");
        assert_eq!(fs::read_to_string(backup).unwrap(), "old\n");
    }
}
