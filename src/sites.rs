//! Debian-style site discovery and safe enable/disable/create operations.

use std::fs;
use std::io::Write;
use std::os::unix::fs::{OpenOptionsExt, symlink};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use chrono::{Local, Utc};

use crate::config::{NginxConfig, ServerInfo, canonical, file_label, looks_like_config};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SiteKind {
    Static,
    Proxy,
    PortForward,
    Redirect,
    Other,
}

impl SiteKind {
    pub const CREATABLE: [SiteKind; 3] = [SiteKind::Static, SiteKind::Proxy, SiteKind::PortForward];

    pub fn label(self) -> &'static str {
        match self {
            Self::Static => "静态网站",
            Self::Proxy => "反向代理",
            Self::PortForward => "端口转发",
            Self::Redirect => "跳转",
            Self::Other => "其他",
        }
    }

    pub fn short(self) -> &'static str {
        match self {
            Self::Static => "静态",
            Self::Proxy => "代理",
            Self::PortForward => "转发",
            Self::Redirect => "跳转",
            Self::Other => "其他",
        }
    }

    pub fn next(self, delta: isize) -> Self {
        let current = Self::CREATABLE
            .iter()
            .position(|kind| *kind == self)
            .unwrap_or(0) as isize;
        let len = Self::CREATABLE.len() as isize;
        Self::CREATABLE[(current + delta).rem_euclid(len) as usize]
    }
}

#[derive(Debug, Clone)]
pub struct Site {
    pub name: String,
    pub available: PathBuf,
    pub enabled: bool,
    pub can_toggle: bool,
    pub servers: Vec<ServerInfo>,
    pub modified: Option<SystemTime>,
    pub size: u64,
}

impl Site {
    pub fn server_names(&self) -> String {
        let mut names: Vec<&str> = self
            .servers
            .iter()
            .flat_map(|s| s.names.iter().map(String::as_str))
            .collect();
        names.sort_unstable();
        names.dedup();
        if names.is_empty() {
            "—".to_string()
        } else {
            names.join("  ")
        }
    }

    pub fn listen_ports(&self) -> String {
        let mut ports: Vec<String> = self
            .servers
            .iter()
            .flat_map(|s| {
                if s.listens.is_empty() {
                    vec!["80".to_string()]
                } else {
                    s.listens.clone()
                }
            })
            .map(|l| {
                l.split_whitespace()
                    .next()
                    .unwrap_or(&l)
                    .trim_start_matches("0.0.0.0:")
                    .to_string()
            })
            .collect();
        ports.sort_unstable();
        ports.dedup();
        ports.join(" ")
    }

    pub fn tls(&self) -> bool {
        self.servers.iter().any(|s| s.tls)
    }

    pub fn kind(&self) -> SiteKind {
        let has_proxy = self.servers.iter().any(|s| !s.proxy_targets.is_empty());
        let has_root = self.servers.iter().any(|s| s.root.is_some());
        let only_default_name = self
            .servers
            .iter()
            .all(|server| server.names.is_empty() || server.names.iter().all(|name| name == "_"));
        if has_proxy && !has_root && only_default_name {
            SiteKind::PortForward
        } else if has_root {
            // A static site may also proxy a sub-path (for example /api).
            SiteKind::Static
        } else if has_proxy {
            SiteKind::Proxy
        } else if self.servers.iter().any(|s| s.internal_redirect.is_some()) {
            SiteKind::Redirect
        } else {
            SiteKind::Other
        }
    }

    pub fn kind_label(&self) -> &'static str {
        let has_root = self.servers.iter().any(|server| server.root.is_some());
        let has_proxy = self
            .servers
            .iter()
            .any(|server| !server.proxy_targets.is_empty());
        if has_root && has_proxy {
            "静态+代理"
        } else {
            self.kind().short()
        }
    }

    pub fn modified_label(&self) -> String {
        self.modified
            .map(|t| {
                let dt: chrono::DateTime<Local> = t.into();
                dt.format("%Y-%m-%d %H:%M").to_string()
            })
            .unwrap_or_else(|| "—".to_string())
    }
}

#[derive(Debug, Clone, Default)]
pub struct SiteInventory {
    pub sites: Vec<Site>,
    pub available_dir: Option<PathBuf>,
    pub enabled_dir: Option<PathBuf>,
}

impl SiteInventory {
    pub fn load(config: &NginxConfig) -> Self {
        let available_dir = existing_dir(config.config_dir.join("sites-available"));
        let enabled_dir = existing_dir(config.config_dir.join("sites-enabled"));
        let mut by_source: std::collections::HashMap<PathBuf, Vec<ServerInfo>> =
            std::collections::HashMap::new();
        for server in &config.servers {
            by_source
                .entry(canonical(&server.source))
                .or_default()
                .push(server.clone());
        }
        let mut sites = Vec::new();
        let mut seen_names = std::collections::HashSet::new();

        if let Some(available) = &available_dir
            && let Ok(entries) = fs::read_dir(available)
        {
            let mut files: Vec<PathBuf> = entries
                .flatten()
                .map(|e| e.path())
                .filter(|p| {
                    p.is_file()
                        && p.file_name()
                            .map(|n| !n.to_string_lossy().starts_with('.'))
                            .unwrap_or(false)
                })
                .collect();
            files.sort();
            for path in files {
                let name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();
                if name.is_empty() {
                    continue;
                }
                let meta = fs::metadata(&path).ok();
                let size = meta.as_ref().map(|m| m.len()).unwrap_or(0);
                if !looks_like_config(&path, size) {
                    continue;
                }
                let enabled_path = enabled_dir.as_ref().map(|d| d.join(&name));
                let link_meta = enabled_path
                    .as_ref()
                    .and_then(|p| fs::symlink_metadata(p).ok());
                let is_enabled = link_meta.is_some();
                let toggle_safe = match &link_meta {
                    None => true,
                    Some(m) => m.file_type().is_symlink(),
                };
                let servers = by_source
                    .remove(&canonical(&path))
                    .filter(|servers| !servers.is_empty())
                    .unwrap_or_else(|| crate::config::servers_in_file(&path, &config.config_dir));
                seen_names.insert(name.clone());
                sites.push(Site {
                    name,
                    available: path,
                    enabled: is_enabled,
                    can_toggle: enabled_dir.is_some() && toggle_safe,
                    servers,
                    modified: meta.and_then(|m| m.modified().ok()),
                    size,
                });
            }
        }

        // Sites living outside sites-available (nginx.conf, conf.d, a symlink
        // pointing elsewhere).  They are shown read-only.
        for (source, servers) in by_source {
            let path = &servers
                .first()
                .map(|s| s.source.clone())
                .unwrap_or(source.clone());
            let name = file_label(path, &config.config_dir);
            if seen_names.contains(&name) {
                continue;
            }
            seen_names.insert(name.clone());
            let meta = fs::metadata(path).ok();
            sites.push(Site {
                name,
                available: path.clone(),
                enabled: true,
                can_toggle: false,
                servers,
                modified: meta.as_ref().and_then(|m| m.modified().ok()),
                size: meta.map(|m| m.len()).unwrap_or(0),
            });
        }

        // Broken or copied entries in sites-enabled still deserve a row.
        if let Some(enabled) = &enabled_dir
            && let Ok(entries) = fs::read_dir(enabled)
        {
            for entry in entries.flatten() {
                let path = entry.path();
                let name = entry.file_name().to_string_lossy().to_string();
                if seen_names.contains(&name) {
                    continue;
                }
                let meta = fs::symlink_metadata(&path).ok();
                let is_symlink = meta
                    .as_ref()
                    .map(|m| m.file_type().is_symlink())
                    .unwrap_or(false);
                seen_names.insert(name.clone());
                sites.push(Site {
                    name,
                    available: if is_symlink {
                        fs::read_link(&path)
                            .ok()
                            .map(|target| {
                                if target.is_absolute() {
                                    target
                                } else {
                                    path.parent().unwrap_or(Path::new(".")).join(target)
                                }
                            })
                            .unwrap_or(path.clone())
                    } else {
                        path.clone()
                    },
                    enabled: true,
                    can_toggle: is_symlink && available_dir.is_some(),
                    servers: Vec::new(),
                    modified: meta.and_then(|m| m.modified().ok()),
                    size: 0,
                });
            }
        }

        sites.sort_by(|a, b| {
            b.enabled
                .cmp(&a.enabled)
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });

        Self {
            sites,
            available_dir,
            enabled_dir,
        }
    }

    #[allow(dead_code)]
    pub fn enabled_count(&self) -> usize {
        self.sites.iter().filter(|site| site.enabled).count()
    }

    pub fn toggle(&self, site: &Site, enable: bool) -> Result<String, String> {
        let available_dir = self
            .available_dir
            .as_ref()
            .ok_or_else(|| "当前 Nginx 没有 sites-available 目录。".to_string())?;
        let enabled_dir = self
            .enabled_dir
            .as_ref()
            .ok_or_else(|| "当前 Nginx 没有 sites-enabled 目录。".to_string())?;
        if !site.available.starts_with(available_dir) {
            return Err("这个站点文件不在 sites-available 中，无法安全切换。".into());
        }
        let enabled_path = enabled_dir.join(&site.name);

        if enable {
            if fs::symlink_metadata(&enabled_path).is_ok() {
                return Err(format!("{} 已经存在。", enabled_path.display()));
            }
            symlink(
                Path::new("../sites-available").join(&site.name),
                &enabled_path,
            )
            .map_err(|e| format!("创建启用链接失败：{e}"))?;
            Ok(format!("已启用 {}。按 r 热重载生效。", site.name))
        } else {
            let meta = fs::symlink_metadata(&enabled_path)
                .map_err(|_| format!("{} 当前没有启用链接。", site.name))?;
            if !meta.file_type().is_symlink() {
                return Err("sites-enabled 中是普通文件而不是链接，为避免误删已取消。".into());
            }
            let target =
                fs::read_link(&enabled_path).map_err(|e| format!("读取启用链接失败：{e}"))?;
            let resolved = if target.is_absolute() {
                target
            } else {
                enabled_dir.join(target)
            };
            if canonical(&resolved) != canonical(&site.available) {
                return Err("启用链接指向了另一个文件，已取消。".into());
            }
            fs::remove_file(&enabled_path).map_err(|e| format!("删除启用链接失败：{e}"))?;
            Ok(format!("已停用 {}。按 r 热重载生效。", site.name))
        }
    }

    pub fn delete_to_trash(&self, site: &Site) -> Result<PathBuf, String> {
        if site.enabled {
            return Err("站点仍在启用中，请先停用再删除。".into());
        }
        let available_dir = self
            .available_dir
            .as_ref()
            .ok_or_else(|| "当前 Nginx 没有 sites-available 目录。".to_string())?;
        if !site.available.starts_with(available_dir) {
            return Err("只能删除 sites-available 中的站点。".into());
        }
        let trash = available_dir.join(".trash");
        fs::create_dir_all(&trash).map_err(|e| format!("创建回收站失败：{e}"))?;
        let stamp = Utc::now().format("%Y%m%d-%H%M%S");
        let target = trash.join(format!("{stamp}-{}", site.name));
        fs::rename(&site.available, &target).map_err(|e| format!("移入回收站失败：{e}"))?;
        Ok(target)
    }
}

fn existing_dir(path: PathBuf) -> Option<PathBuf> {
    path.is_dir().then_some(path)
}

#[derive(Debug, Clone)]
pub struct NewSiteSpec {
    pub kind: SiteKind,
    pub file_name: String,
    pub server_name: String,
    pub listen: String,
    pub root: String,
    pub proxy: String,
    pub websocket: bool,
    pub spa: bool,
    pub enable: bool,
}

impl Default for NewSiteSpec {
    fn default() -> Self {
        Self {
            kind: SiteKind::Static,
            file_name: "example.com".into(),
            server_name: "example.com www.example.com".into(),
            listen: "80".into(),
            root: "/var/www/example.com/html".into(),
            proxy: String::new(),
            websocket: false,
            spa: false,
            enable: true,
        }
    }
}

pub fn create_site(
    config: &NginxConfig,
    inventory: &SiteInventory,
    spec: &NewSiteSpec,
) -> Result<PathBuf, String> {
    let available = inventory
        .available_dir
        .as_ref()
        .ok_or_else(|| "当前布局没有 sites-available；请手动在 conf.d 中创建配置。".to_string())?;
    let mut name = spec.file_name.trim().to_string();
    if name.is_empty() {
        name = default_file_name(spec);
    }
    validate_file_name(&name)?;
    let path = available.join(&name);
    if path.exists() {
        return Err(format!("{} 已存在。", path.display()));
    }

    let listen = spec.listen.trim();
    if listen.is_empty()
        || listen.contains([';', '{', '}', '\n'])
        || listen.split_whitespace().count() != 1
    {
        return Err("监听端口需要一个值，例如 80 / 127.0.0.1:8080。".into());
    }

    let mut server_name = spec.server_name.trim().to_string();
    if spec.kind == SiteKind::PortForward && server_name.is_empty() {
        server_name = "_".to_string();
    }
    if server_name.is_empty() {
        return Err("域名不能为空；端口转发可填 _。".into());
    }
    if server_name.contains([';', '{', '}']) || server_name.contains('\n') {
        return Err("域名含有非法字符。".into());
    }

    let root = spec.root.trim();
    let proxy = spec.proxy.trim();
    let proxy = if matches!(spec.kind, SiteKind::Proxy | SiteKind::PortForward) {
        if proxy.is_empty() {
            return Err("反向代理/端口转发需要目标地址，例如 127.0.0.1:3000。".into());
        }
        Some(normalize_proxy(proxy)?)
    } else {
        if root.is_empty() {
            return Err("静态网站需要网站目录。".into());
        }
        None
    };
    if !root.is_empty() {
        if root.contains(['\n', ';', '{', '}']) {
            return Err("网站目录含有非法字符。".into());
        }
        if root.split_whitespace().count() > 1 {
            return Err("网站目录中暂不支持空格；创建后可手工调整。".into());
        }
    }

    let body = render_site(&name, spec, &server_name, listen, root, proxy.as_deref());
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o644)
        .open(&path)
        .map_err(|e| format!("创建 {} 失败：{e}", path.display()))?;
    if let Err(err) = file.write_all(body.as_bytes()) {
        let _ = fs::remove_file(&path);
        return Err(format!("写入配置失败：{err}"));
    }
    drop(file);

    if spec.enable
        && let Some(enabled) = &inventory.enabled_dir
    {
        let link = enabled.join(&name);
        if let Err(err) = symlink(Path::new("../sites-available").join(&name), &link) {
            let _ = fs::remove_file(&path);
            return Err(format!("创建启用链接失败：{err}"));
        }
    }
    let _ = config;
    Ok(path)
}

fn validate_file_name(name: &str) -> Result<(), String> {
    if name.is_empty()
        || name.starts_with('.')
        || name.contains('/')
        || name.contains('\\')
        || name.contains(char::is_whitespace)
    {
        return Err("文件名不能为空、不能包含空格或路径分隔符，也不能以点开头。".into());
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
    {
        return Err("文件名只能包含字母、数字、点、下划线和短横线。".into());
    }
    Ok(())
}

fn default_file_name(spec: &NewSiteSpec) -> String {
    if spec.kind == SiteKind::PortForward {
        let port = spec.listen.trim().replace([':', '.'], "-");
        return format!("forward-{port}");
    }
    let first = spec.server_name.split_whitespace().next().unwrap_or("site");
    let clean: String = first
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
        .collect();
    if clean.is_empty() {
        "site".to_string()
    } else {
        clean
    }
}

fn normalize_proxy(value: &str) -> Result<String, String> {
    let value = value.trim();
    if value.is_empty() {
        return Err("目标地址不能为空。".into());
    }
    if value.contains([';', '{', '}', '\n']) || value.split_whitespace().count() > 1 {
        return Err("目标地址不能包含空格或 ; { }。".into());
    }
    if value.starts_with("http://") || value.starts_with("https://") || value.starts_with("unix:") {
        return Ok(value.to_string());
    }
    if value.chars().all(|c| c.is_ascii_digit()) {
        return Ok(format!("http://127.0.0.1:{value}"));
    }
    if let Some(port) = value.strip_prefix(':')
        && port.chars().all(|c| c.is_ascii_digit())
    {
        return Ok(format!("http://127.0.0.1:{port}"));
    }
    Ok(format!("http://{value}"))
}

fn render_site(
    name: &str,
    spec: &NewSiteSpec,
    server_name: &str,
    listen: &str,
    root: &str,
    proxy: Option<&str>,
) -> String {
    let date = Local::now().format("%Y-%m-%d %H:%M");
    let id = sanitize_identifier(name);
    let mut out = String::new();
    out.push_str(&format!("# 由 nserver 创建于 {date}\n"));
    out.push_str(&format!("# site: {name}\n"));
    out.push_str("# managed-by: nserver\n\n");

    if spec.websocket && proxy.is_some() {
        out.push_str(&format!(
            "map $http_upgrade $nserver_ws_{id} {{\n    default upgrade;\n    ''      close;\n}}\n\n"
        ));
    }

    out.push_str("server {\n");
    out.push_str(&format!("    listen {listen};\n"));
    out.push_str(&format!("    server_name {server_name};\n\n"));

    match spec.kind {
        SiteKind::Proxy | SiteKind::PortForward => {
            let proxy = proxy.unwrap_or("http://127.0.0.1:3000");
            if !root.is_empty() {
                out.push_str(&format!("    # 原静态目录：{root}\n"));
            }
            out.push_str("    location / {\n");
            out.push_str(&format!("        proxy_pass {proxy};\n"));
            out.push_str("        proxy_http_version 1.1;\n");
            out.push_str("        proxy_set_header Host $host;\n");
            out.push_str("        proxy_set_header X-Real-IP $remote_addr;\n");
            out.push_str("        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;\n");
            out.push_str("        proxy_set_header X-Forwarded-Proto $scheme;\n");
            if spec.websocket {
                out.push_str("        proxy_set_header Upgrade $http_upgrade;\n");
                out.push_str(&format!(
                    "        proxy_set_header Connection $nserver_ws_{id};\n"
                ));
                out.push_str("        proxy_read_timeout 3600s;\n");
            }
            out.push_str("    }\n");
        }
        _ => {
            out.push_str(&format!("    root {root};\n"));
            out.push_str("    index index.html index.htm;\n\n");
            out.push_str("    location / {\n");
            if spec.spa {
                out.push_str("        try_files $uri $uri/ /index.html;\n");
            } else {
                out.push_str("        try_files $uri $uri/ =404;\n");
            }
            out.push_str("    }\n");
        }
    }

    out.push_str("\n    # 如需独立日志，取消下面两行注释：\n");
    out.push_str("    # access_log /var/log/nginx/");
    out.push_str(&sanitize_log_name(name));
    out.push_str(".access.log;\n");
    out.push_str("    # error_log  /var/log/nginx/");
    out.push_str(&sanitize_log_name(name));
    out.push_str(".error.log;\n");
    out.push_str("}\n");
    out
}

/// Replace the server_name in the first server block.
pub fn patch_server_name(text: &str, value: &str) -> Result<String, String> {
    validate_value(value, true)?;
    let (mut lines, trailing) = split_text(text);
    let (start, end) = first_server_bounds(&lines)?;
    if let Some(index) = directive_index(&lines, start, end, "server_name") {
        lines[index] = replace_args(&lines[index], "server_name", value, false);
    } else {
        let at = directive_index(&lines, start, end, "listen")
            .map(|index| index + 1)
            .unwrap_or(start + 1);
        lines.insert(at, format!("    server_name {value};"));
    }
    Ok(join_text(lines, trailing))
}

/// Replace the first listen address, preserving flags such as ssl/http2.
pub fn patch_listen(text: &str, value: &str) -> Result<String, String> {
    validate_value(value, false)?;
    if value.split_whitespace().count() != 1 {
        return Err("监听端口只能是一个值。".into());
    }
    let (mut lines, trailing) = split_text(text);
    let (start, end) = first_server_bounds(&lines)?;
    if let Some(index) = directive_index(&lines, start, end, "listen") {
        lines[index] = replace_args(&lines[index], "listen", value, true);
    } else {
        lines.insert(start + 1, format!("    listen {value};"));
    }
    Ok(join_text(lines, trailing))
}

/// Replace the site root in the first server block, inserting it if needed.
pub fn patch_root(text: &str, value: &str) -> Result<String, String> {
    validate_value(value, false)?;
    if value.split_whitespace().count() != 1 {
        return Err("网站目录中暂不支持空格。".into());
    }
    let (mut lines, trailing) = split_text(text);
    let (start, end) = first_server_bounds(&lines)?;
    if let Some(index) = directive_index(&lines, start, end, "root") {
        lines[index] = replace_args(&lines[index], "root", value, false);
    } else {
        let at = (start + 1..=end)
            .find(|index| {
                let code = code_part(&lines[*index]).trim_start();
                code.starts_with("location")
            })
            .unwrap_or(end);
        lines.insert(at, format!("    root {value};"));
    }
    Ok(join_text(lines, trailing))
}

/// Replace the first try_files directive (SPA fallback toggle).
pub fn patch_try_files(text: &str, spa: bool) -> Result<String, String> {
    let (mut lines, trailing) = split_text(text);
    let (start, end) = first_server_bounds(&lines)?;
    let Some(index) = directive_index(&lines, start, end, "try_files") else {
        return Err("站点里没有 try_files，无法切换 SPA 回退；请用完整编辑器修改。".into());
    };
    let value = if spa {
        "$uri $uri/ /index.html"
    } else {
        "$uri $uri/ =404"
    };
    lines[index] = replace_args(&lines[index], "try_files", value, false);
    Ok(join_text(lines, trailing))
}

/// Replace proxy_pass and optionally maintain WebSocket headers.
pub fn patch_proxy(text: &str, target: &str, websocket: bool) -> Result<String, String> {
    validate_value(target, false)?;
    if target.split_whitespace().count() != 1 {
        return Err("代理目标不能包含空格。".into());
    }
    let target = normalize_proxy(target)?;
    let (mut lines, trailing) = split_text(text);
    let (start, end) = first_server_bounds(&lines)?;
    if directive_index(&lines, start, end, "proxy_pass").is_none() {
        return Err(
            "站点里没有 proxy_pass，无法直接改为代理；请用完整编辑器或新建站点向导。".into(),
        );
    }

    // Remove old WebSocket headers before re-adding based on the new setting.
    for index in (start..=end).rev() {
        let code = code_part(&lines[index]).trim_start();
        if code.starts_with("proxy_set_header Upgrade")
            || code.starts_with("proxy_set_header Connection")
        {
            lines.remove(index);
        }
    }
    let (start, end) = first_server_bounds(&lines)?;
    let index = directive_index(&lines, start, end, "proxy_pass")
        .ok_or_else(|| "找不到 proxy_pass。".to_string())?;
    lines[index] = replace_args(&lines[index], "proxy_pass", &target, false);

    if websocket {
        let indent: String = lines[index]
            .chars()
            .take_while(|c| c.is_whitespace())
            .collect();
        let has_version = (start..=end).any(|i| {
            code_part(&lines[i])
                .trim_start()
                .starts_with("proxy_http_version")
        });
        let mut insert_at = index + 1;
        if !has_version {
            lines.insert(insert_at, format!("{indent}proxy_http_version 1.1;"));
            insert_at += 1;
        }
        lines.insert(
            insert_at,
            format!("{indent}proxy_set_header Upgrade $http_upgrade;"),
        );
        insert_at += 1;
        lines.insert(
            insert_at,
            format!("{indent}proxy_set_header Connection \"upgrade\";"),
        );
    }
    Ok(join_text(lines, trailing))
}

pub fn detect_websocket(text: &str) -> bool {
    text.lines().any(|line| {
        code_part(line)
            .trim_start()
            .starts_with("proxy_set_header Upgrade")
    })
}

pub fn detect_spa(text: &str) -> bool {
    text.lines().any(|line| {
        let code = code_part(line);
        code.contains("try_files") && code.contains("/index.html")
    })
}

/// Return (server_name, first listen, first root, first proxy_pass).
pub fn site_settings_values(text: &str) -> (String, String, String, String) {
    let (lines, _) = split_text(text);
    let Ok((start, end)) = first_server_bounds(&lines) else {
        return (String::new(), String::new(), String::new(), String::new());
    };
    let directive_value = |name: &str| {
        directive_index(&lines, start, end, name)
            .map(|index| {
                let code = code_part(&lines[index]).trim_start();
                let semi = code.find(';').unwrap_or(code.len());
                code[name.len()..semi].trim().to_string()
            })
            .unwrap_or_default()
    };
    (
        directive_value("server_name"),
        directive_value("listen"),
        directive_value("root"),
        directive_value("proxy_pass"),
    )
}

fn validate_value(value: &str, allow_space: bool) -> Result<(), String> {
    let value = value.trim();
    if value.is_empty() {
        return Err("值不能为空。".into());
    }
    if value.contains([';', '{', '}', '\n']) {
        return Err("值不能包含 ; { } 或换行。".into());
    }
    if !allow_space && value.split_whitespace().count() > 1 {
        return Err("值不能包含空格。".into());
    }
    Ok(())
}

fn split_text(text: &str) -> (Vec<String>, bool) {
    let trailing = text.ends_with('\n');
    let lines: Vec<String> = text.lines().map(ToString::to_string).collect();
    (lines, trailing)
}

fn join_text(lines: Vec<String>, trailing: bool) -> String {
    let mut out = lines.join("\n");
    if trailing {
        out.push('\n');
    }
    out
}

fn first_server_bounds(lines: &[String]) -> Result<(usize, usize), String> {
    let mut start = None;
    let mut depth = 0i32;
    for (index, line) in lines.iter().enumerate() {
        let code = code_part(line);
        if start.is_none() {
            let trimmed = code.trim_start();
            if let Some(rest) = trimmed.strip_prefix("server")
                && rest.trim_start().starts_with('{')
            {
                start = Some(index);
                depth = brace_delta(code);
                if depth == 0 {
                    return Err("server 块写在一行内，请用完整编辑器修改。".into());
                }
                if depth < 0 {
                    return Err("server 块花括号不匹配。".into());
                }
            }
        } else {
            depth += brace_delta(code);
            if depth == 0 {
                return Ok((start.unwrap_or(0), index));
            }
            if depth < 0 {
                return Err("server 块花括号不匹配。".into());
            }
        }
    }
    Err("没有找到完整的 server 块。".into())
}

fn directive_index(lines: &[String], start: usize, end: usize, name: &str) -> Option<usize> {
    (start..=end).find(|index| {
        let code = code_part(&lines[*index]).trim_start();
        code.strip_prefix(name)
            .map(|rest| rest.starts_with(char::is_whitespace) && rest.contains(';'))
            .unwrap_or(false)
    })
}

fn replace_args(line: &str, name: &str, value: &str, keep_extra: bool) -> String {
    let indent: String = line.chars().take_while(|c| c.is_whitespace()).collect();
    let code = code_part(line).trim_start();
    let semi = code.find(';').unwrap_or(code.len());
    let args = code.get(name.len()..semi).unwrap_or_default().trim();
    let extra = if keep_extra {
        args.split_whitespace()
            .skip(1)
            .collect::<Vec<_>>()
            .join(" ")
    } else {
        String::new()
    };
    let trailing = line.get(indent.len() + semi + 1..).unwrap_or_default();
    if extra.is_empty() {
        format!("{indent}{name} {value};{trailing}")
    } else {
        format!("{indent}{name} {value} {extra};{trailing}")
    }
}

fn code_part(line: &str) -> &str {
    let mut quote = None;
    let mut escaped = false;
    for (index, ch) in line.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if ch == '\\' {
            escaped = true;
            continue;
        }
        match quote {
            Some(q) if ch == q => quote = None,
            Some(_) => {}
            None if ch == '"' || ch == '\'' => quote = Some(ch),
            None if ch == '#' => return &line[..index],
            None => {}
        }
    }
    line
}

fn brace_delta(line: &str) -> i32 {
    let mut delta = 0;
    for ch in code_part(line).chars() {
        match ch {
            '{' => delta += 1,
            '}' => delta -= 1,
            _ => {}
        }
    }
    delta
}

fn sanitize_log_name(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

fn sanitize_identifier(name: &str) -> String {
    let mut id: String = name
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    if id.is_empty() {
        id.push_str("site");
    }
    id
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;

    fn write(path: &Path, body: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        let mut f = fs::File::create(path).unwrap();
        f.write_all(body.as_bytes()).unwrap();
    }

    #[test]
    fn discovers_debian_sites_and_toggles_them() {
        let tmp = tempfile::tempdir().unwrap();
        let conf = tmp.path().join("nginx.conf");
        write(&conf, "http { include sites-enabled/*; }\n");
        let site = tmp.path().join("sites-available/demo");
        write(&site, "server { listen 8080; server_name demo.test; }\n");
        fs::create_dir_all(tmp.path().join("sites-enabled")).unwrap();
        symlink(
            "../sites-available/demo",
            tmp.path().join("sites-enabled/demo"),
        )
        .unwrap();

        let model = crate::config::load(&conf);
        let inv = SiteInventory::load(&model);
        assert_eq!(inv.sites.len(), 1);
        assert!(inv.sites[0].enabled);
        assert_eq!(inv.sites[0].server_names(), "demo.test");

        inv.toggle(&inv.sites[0], false).unwrap();
        let model = crate::config::load(&conf);
        let inv = SiteInventory::load(&model);
        assert!(!inv.sites[0].enabled);
        assert_eq!(inv.enabled_count(), 0);
        // Disabled sites are parsed directly so names/ports remain visible.
        assert_eq!(inv.sites[0].server_names(), "demo.test");

        inv.toggle(&inv.sites[0], true).unwrap();
        let model = crate::config::load(&conf);
        let inv = SiteInventory::load(&model);
        assert!(inv.sites[0].enabled);
    }

    #[test]
    fn classifies_hybrid_site_as_static_plus_proxy() {
        let tmp = tempfile::tempdir().unwrap();
        let conf = tmp.path().join("nginx.conf");
        write(&conf, "http { include sites-enabled/*; }\n");
        let site = tmp.path().join("sites-available/hybrid.conf");
        write(
            &site,
            "server {\n    listen 8080;\n    server_name hybrid.test;\n    root /srv/hybrid;\n    location /api { proxy_pass http://127.0.0.1:3000; }\n}\n",
        );
        fs::create_dir_all(tmp.path().join("sites-enabled")).unwrap();
        symlink(
            "../sites-available/hybrid.conf",
            tmp.path().join("sites-enabled/hybrid.conf"),
        )
        .unwrap();
        let model = crate::config::load(&conf);
        let inv = SiteInventory::load(&model);
        assert_eq!(inv.sites[0].kind(), SiteKind::Static);
        assert_eq!(inv.sites[0].kind_label(), "静态+代理");
    }

    #[test]
    fn creates_static_and_proxy_sites() {
        let tmp = tempfile::tempdir().unwrap();
        let conf = tmp.path().join("nginx.conf");
        write(&conf, "http { include sites-enabled/*; }\n");
        fs::create_dir_all(tmp.path().join("sites-available")).unwrap();
        fs::create_dir_all(tmp.path().join("sites-enabled")).unwrap();
        let model = crate::config::load(&conf);
        let inv = SiteInventory::load(&model);

        let spec = NewSiteSpec {
            kind: SiteKind::Proxy,
            file_name: "app.test".into(),
            server_name: "app.test".into(),
            listen: "8080".into(),
            root: String::new(),
            proxy: "127.0.0.1:3000".into(),
            websocket: false,
            spa: false,
            enable: true,
        };
        let path = create_site(&model, &inv, &spec).unwrap();
        let text = fs::read_to_string(path).unwrap();
        assert!(text.contains("server_name app.test"));
        assert!(text.contains("proxy_pass http://127.0.0.1:3000"));
        assert!(tmp.path().join("sites-enabled/app.test").exists());
    }

    #[test]
    fn creates_static_spa_and_port_forward() {
        let tmp = tempfile::tempdir().unwrap();
        let conf = tmp.path().join("nginx.conf");
        write(&conf, "http { include sites-enabled/*; }\n");
        fs::create_dir_all(tmp.path().join("sites-available")).unwrap();
        fs::create_dir_all(tmp.path().join("sites-enabled")).unwrap();
        let model = crate::config::load(&conf);
        let inv = SiteInventory::load(&model);

        let static_spec = NewSiteSpec {
            kind: SiteKind::Static,
            file_name: "spa".into(),
            server_name: "spa.test".into(),
            listen: "8081".into(),
            root: "/srv/spa".into(),
            proxy: String::new(),
            websocket: false,
            spa: true,
            enable: true,
        };
        let path = create_site(&model, &inv, &static_spec).unwrap();
        let text = fs::read_to_string(path).unwrap();
        assert!(text.contains("try_files $uri $uri/ /index.html;"));

        let forward_spec = NewSiteSpec {
            kind: SiteKind::PortForward,
            file_name: "forward-9000".into(),
            server_name: "_".into(),
            listen: "9000".into(),
            root: String::new(),
            proxy: "127.0.0.1:3000".into(),
            websocket: true,
            spa: false,
            enable: true,
        };
        let path = create_site(&model, &inv, &forward_spec).unwrap();
        let text = fs::read_to_string(path).unwrap();
        assert!(text.contains("server_name _;"));
        assert!(text.contains("proxy_pass http://127.0.0.1:3000;"));
        assert!(text.contains("map $http_upgrade $nserver_ws_forward_9000"));
        assert!(text.contains("proxy_set_header Connection $nserver_ws_forward_9000;"));
    }

    #[test]
    fn patches_static_site_fields() {
        let text = concat!(
            "server {\n",
            "    listen 80 default_server;\n",
            "    server_name old.test;\n",
            "    root /var/www/old;\n",
            "\n",
            "    location / {\n",
            "        try_files $uri $uri/ =404;\n",
            "    }\n",
            "}\n",
        );
        let updated = patch_server_name(text, "new.test www.new.test").unwrap();
        let updated = patch_listen(&updated, "8080").unwrap();
        let updated = patch_root(&updated, "/srv/new").unwrap();
        let updated = patch_try_files(&updated, true).unwrap();
        assert!(updated.contains("server_name new.test www.new.test;"));
        assert!(updated.contains("listen 8080 default_server;"));
        assert!(updated.contains("root /srv/new;"));
        assert!(updated.contains("try_files $uri $uri/ /index.html;"));
    }

    #[test]
    fn patches_proxy_and_websocket() {
        let text = concat!(
            "server {\n",
            "    listen 80;\n",
            "    server_name app.test;\n",
            "    location / {\n",
            "        proxy_pass http://127.0.0.1:3000;\n",
            "    }\n",
            "}\n",
        );
        let updated = patch_proxy(text, "127.0.0.1:4000", true).unwrap();
        assert!(updated.contains("proxy_pass http://127.0.0.1:4000;"));
        assert!(updated.contains("proxy_set_header Upgrade $http_upgrade;"));
        assert!(detect_websocket(&updated));
        let removed = patch_proxy(&updated, "http://127.0.0.1:5000", false).unwrap();
        assert!(!detect_websocket(&removed));
        assert!(removed.contains("proxy_pass http://127.0.0.1:5000;"));
        let (domain, listen, root, proxy) = site_settings_values(&removed);
        assert_eq!(domain, "app.test");
        assert_eq!(listen, "80");
        assert_eq!(root, "");
        assert_eq!(proxy, "http://127.0.0.1:5000");
    }

    #[test]
    fn patch_missing_proxy_is_an_error() {
        let text = "server {\n    listen 80;\n    root /srv;\n}\n";
        assert!(patch_proxy(text, "127.0.0.1:3000", false).is_err());
    }

    #[test]
    fn rejects_unsafe_names() {
        assert!(validate_file_name("../evil").is_err());
        assert!(validate_file_name("has space").is_err());
        assert!(validate_file_name("ok-site_1.conf").is_ok());
    }
}
