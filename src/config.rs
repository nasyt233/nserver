//! A small, forgiving nginx configuration reader.
//!
//! It is deliberately not a complete nginx grammar.  It understands the
//! structural parts needed by the TUI: blocks, directives, quoted strings,
//! includes and comments.  `nginx -t` remains the source of truth for syntax.

use std::collections::{BTreeSet, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use glob::glob;

#[derive(Debug, Clone)]
pub struct Directive {
    pub name: String,
    pub args: Vec<String>,
    pub children: Option<Vec<Directive>>,
    pub source: PathBuf,
    pub line: usize,
}

#[derive(Debug, Clone, Default)]
pub struct ServerInfo {
    pub source: PathBuf,
    pub line: usize,
    pub names: Vec<String>,
    pub listens: Vec<String>,
    pub tls: bool,
    pub cert_path: Option<PathBuf>,
    pub key_path: Option<PathBuf>,
    pub root: Option<String>,
    pub proxy_targets: Vec<String>,
    pub locations: usize,
    pub internal_redirect: Option<String>,
}

impl ServerInfo {
    pub fn display_names(&self) -> String {
        if self.names.is_empty() {
            "未命名 server".to_string()
        } else {
            self.names.join("  ")
        }
    }

    pub fn listen_label(&self) -> String {
        if self.listens.is_empty() {
            "80 (默认)".to_string()
        } else {
            self.listens
                .iter()
                .map(|l| {
                    l.split_whitespace()
                        .next()
                        .unwrap_or(l)
                        .trim_start_matches("0.0.0.0:")
                        .to_string()
                })
                .collect::<Vec<_>>()
                .join(", ")
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct BackendInfo {
    pub address: String,
    pub down: bool,
    pub backup: bool,
    pub extra: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct UpstreamInfo {
    pub name: String,
    pub source: PathBuf,
    pub line: usize,
    pub method: String,
    pub backends: Vec<BackendInfo>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogKind {
    Access,
    Error,
    System,
}

impl LogKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Access => "访问",
            Self::Error => "错误",
            Self::System => "系统",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogSource {
    pub kind: LogKind,
    pub path: PathBuf,
    pub label: String,
}

#[derive(Debug, Clone)]
pub struct NginxConfig {
    pub main: PathBuf,
    pub config_dir: PathBuf,
    pub servers: Vec<ServerInfo>,
    pub upstreams: Vec<UpstreamInfo>,
    pub logs: Vec<LogSource>,
    pub pid_file: PathBuf,
    pub files: Vec<PathBuf>,
    pub parse_errors: Vec<String>,
}

impl Default for NginxConfig {
    fn default() -> Self {
        Self {
            main: PathBuf::from("/etc/nginx/nginx.conf"),
            config_dir: PathBuf::from("/etc/nginx"),
            servers: Vec::new(),
            upstreams: Vec::new(),
            logs: Vec::new(),
            pid_file: PathBuf::from("/run/nginx.pid"),
            files: Vec::new(),
            parse_errors: Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
struct Token {
    kind: TokenKind,
    line: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum TokenKind {
    Word(String),
    LBrace,
    RBrace,
    Semi,
}

/// Parse a config string into a directive tree.
pub fn parse_str(text: &str, source: &Path) -> (Vec<Directive>, Vec<String>) {
    let mut errors = Vec::new();
    let tokens = tokenize(text, &mut errors);
    let mut parser = Parser {
        tokens,
        pos: 0,
        source,
        errors: &mut errors,
    };
    let directives = parser.parse_scope(true);
    (directives, errors)
}

fn tokenize(text: &str, errors: &mut Vec<String>) -> Vec<Token> {
    let chars: Vec<char> = text.chars().collect();
    let mut tokens = Vec::new();
    let mut i = 0;
    let mut line = 1;
    let mut word = String::new();
    let mut word_line = 1;

    let flush = |word: &mut String, line: usize, tokens: &mut Vec<Token>| {
        if !word.is_empty() {
            tokens.push(Token {
                kind: TokenKind::Word(std::mem::take(word)),
                line,
            });
        }
    };

    while i < chars.len() {
        let c = chars[i];
        match c {
            '\n' => {
                flush(&mut word, word_line, &mut tokens);
                line += 1;
                i += 1;
            }
            c if c.is_whitespace() => {
                flush(&mut word, word_line, &mut tokens);
                i += 1;
            }
            '#' => {
                flush(&mut word, word_line, &mut tokens);
                while i < chars.len() && chars[i] != '\n' {
                    i += 1;
                }
            }
            ';' => {
                flush(&mut word, word_line, &mut tokens);
                tokens.push(Token {
                    kind: TokenKind::Semi,
                    line,
                });
                i += 1;
            }
            '{' => {
                // ${variable} belongs to a word, not to a block.
                if word.ends_with('$') {
                    word.push('{');
                    i += 1;
                    while i < chars.len() {
                        word.push(chars[i]);
                        if chars[i] == '}' {
                            i += 1;
                            break;
                        }
                        i += 1;
                    }
                } else {
                    flush(&mut word, word_line, &mut tokens);
                    tokens.push(Token {
                        kind: TokenKind::LBrace,
                        line,
                    });
                    i += 1;
                }
            }
            '}' => {
                flush(&mut word, word_line, &mut tokens);
                tokens.push(Token {
                    kind: TokenKind::RBrace,
                    line,
                });
                i += 1;
            }
            '"' | '\'' => {
                if word.is_empty() {
                    word_line = line;
                }
                let quote = c;
                i += 1;
                let mut closed = false;
                while i < chars.len() {
                    let ch = chars[i];
                    if ch == '\n' {
                        line += 1;
                    }
                    if ch == '\\' && i + 1 < chars.len() {
                        word.push(ch);
                        i += 1;
                        word.push(chars[i]);
                        i += 1;
                        continue;
                    }
                    if ch == quote {
                        closed = true;
                        i += 1;
                        break;
                    }
                    word.push(ch);
                    i += 1;
                }
                if !closed {
                    errors.push(format!(
                        "第 {word_line} 行：引号没有闭合，配置解析可能不完整"
                    ));
                }
            }
            '\\' if i + 1 < chars.len() => {
                word.push(c);
                i += 1;
                word.push(chars[i]);
                i += 1;
            }
            _ => {
                if word.is_empty() {
                    word_line = line;
                }
                word.push(c);
                i += 1;
            }
        }
    }
    flush(&mut word, word_line, &mut tokens);
    tokens
}

struct Parser<'a> {
    tokens: Vec<Token>,
    pos: usize,
    source: &'a Path,
    errors: &'a mut Vec<String>,
}

impl Parser<'_> {
    fn parse_scope(&mut self, top: bool) -> Vec<Directive> {
        let mut out = Vec::new();
        loop {
            while matches!(
                self.tokens.get(self.pos).map(|t| &t.kind),
                Some(TokenKind::Semi)
            ) {
                self.pos += 1;
            }
            let Some(token) = self.tokens.get(self.pos) else {
                return out;
            };
            if matches!(token.kind, TokenKind::RBrace) {
                if top {
                    self.errors
                        .push(format!("第 {} 行：多余的 '}}'", token.line));
                    self.pos += 1;
                    continue;
                }
                self.pos += 1;
                return out;
            }

            let mut words: Vec<(String, usize)> = Vec::new();
            loop {
                let Some(token) = self.tokens.get(self.pos) else {
                    return out;
                };
                match &token.kind {
                    TokenKind::Word(w) => {
                        words.push((w.clone(), token.line));
                        self.pos += 1;
                    }
                    TokenKind::Semi => {
                        let line = token.line;
                        self.pos += 1;
                        if let Some((name, first_line)) = words.first() {
                            out.push(Directive {
                                name: name.to_ascii_lowercase(),
                                args: words[1..].iter().map(|(w, _)| w.clone()).collect(),
                                children: None,
                                source: self.source.to_path_buf(),
                                line: *first_line,
                            });
                        } else {
                            self.errors.push(format!("第 {line} 行：多余的 ';'"));
                        }
                        break;
                    }
                    TokenKind::LBrace => {
                        let line = token.line;
                        self.pos += 1;
                        if words.is_empty() {
                            self.errors.push(format!("第 {line} 行：块缺少名字"));
                            self.skip_block();
                            break;
                        }
                        let name = words[0].0.to_ascii_lowercase();
                        let args = words[1..].iter().map(|(w, _)| w.clone()).collect();
                        let first_line = words[0].1;
                        let children = self.parse_scope(false);
                        out.push(Directive {
                            name,
                            args,
                            children: Some(children),
                            source: self.source.to_path_buf(),
                            line: first_line,
                        });
                        break;
                    }
                    TokenKind::RBrace => {
                        let line = token.line;
                        // The block ended before a directive's semicolon.
                        // Consume the brace so the caller stays in sync.
                        self.errors.push(format!("第 {line} 行：块结束前缺少 ';'"));
                        self.pos += 1;
                        if !top {
                            return out;
                        }
                        words.clear();
                    }
                }
            }
        }
    }

    fn skip_block(&mut self) {
        let mut depth = 1usize;
        while let Some(token) = self.tokens.get(self.pos) {
            match token.kind {
                TokenKind::LBrace => depth += 1,
                TokenKind::RBrace => {
                    depth -= 1;
                    if depth == 0 {
                        self.pos += 1;
                        return;
                    }
                }
                _ => {}
            }
            self.pos += 1;
        }
    }
}

/// Load the main config, expand `include` directives, and build the view model.
pub fn load(main: &Path) -> NginxConfig {
    let main = absolute(main);
    let config_dir = main
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let mut ctx = LoadContext {
        config_dir: config_dir.clone(),
        errors: Vec::new(),
        files: BTreeSet::new(),
        stack: HashSet::new(),
    };
    let directives = ctx.expand_file(&main);

    let mut servers = Vec::new();
    let mut upstreams = Vec::new();
    walk_structure(
        &directives,
        &config_dir,
        &mut servers,
        &mut upstreams,
        false,
    );

    let mut logs = Vec::new();
    collect_logs(&directives, &config_dir, &mut logs);
    if logs.is_empty() {
        add_default_logs(&config_dir, &mut logs);
    }

    let pid_file = find_pid_file(&directives, &config_dir);
    let mut files: BTreeSet<PathBuf> = ctx.files;
    for file in scan_config_files(&config_dir) {
        files.insert(file);
    }
    files.insert(main.clone());

    NginxConfig {
        main,
        config_dir,
        servers,
        upstreams,
        logs,
        pid_file,
        files: files.into_iter().collect(),
        parse_errors: ctx.errors,
    }
}

fn absolute(path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(path)
    }
}

struct LoadContext {
    config_dir: PathBuf,
    errors: Vec<String>,
    files: BTreeSet<PathBuf>,
    stack: HashSet<PathBuf>,
}

impl LoadContext {
    fn expand_file(&mut self, path: &Path) -> Vec<Directive> {
        let path = absolute(path);
        let canonical = fs::canonicalize(&path).unwrap_or_else(|_| path.clone());
        if !self.stack.insert(canonical.clone()) {
            self.errors
                .push(format!("{}：include 出现循环引用，已跳过", path.display()));
            return Vec::new();
        }
        self.files.insert(path.clone());

        let text = match fs::read_to_string(&path) {
            Ok(t) => t,
            Err(err) => {
                self.errors
                    .push(format!("{}：读取失败：{err}", path.display()));
                self.stack.remove(&canonical);
                return Vec::new();
            }
        };
        let (directives, mut errors) = parse_str(&text, &path);
        self.errors.append(&mut errors);
        let expanded = self.expand_directives(directives);
        self.stack.remove(&canonical);
        expanded
    }

    fn expand_directives(&mut self, directives: Vec<Directive>) -> Vec<Directive> {
        let mut out = Vec::new();
        for mut directive in directives {
            if directive.name == "include" && directive.children.is_none() {
                for arg in &directive.args {
                    self.include_pattern(arg, &mut out);
                }
                continue;
            }
            if let Some(children) = directive.children.take() {
                directive.children = Some(self.expand_directives(children));
            }
            out.push(directive);
        }
        out
    }

    fn include_pattern(&mut self, pattern: &str, out: &mut Vec<Directive>) {
        if pattern.starts_with("http://") || pattern.starts_with("https://") {
            return;
        }
        let raw = Path::new(pattern);
        let joined = if raw.is_absolute() {
            raw.to_path_buf()
        } else {
            self.config_dir.join(raw)
        };
        let has_glob = pattern.contains(['*', '?', '[']);
        let pattern_text = joined.to_string_lossy().to_string();
        let mut matched = Vec::new();
        match glob(&pattern_text) {
            Ok(paths) => {
                for entry in paths.flatten() {
                    if entry.is_file() {
                        matched.push(entry);
                    }
                }
            }
            Err(err) => {
                self.errors
                    .push(format!("include 模式无效：{pattern}（{err}）"));
                return;
            }
        }
        matched.sort();
        if matched.is_empty() {
            if !has_glob {
                self.errors
                    .push(format!("include 文件不存在：{}", joined.display()));
            }
            return;
        }
        for path in matched {
            let included = self.expand_file(&path);
            out.extend(included);
        }
    }
}

fn walk_structure(
    directives: &[Directive],
    config_dir: &Path,
    servers: &mut Vec<ServerInfo>,
    upstreams: &mut Vec<UpstreamInfo>,
    inside_upstream: bool,
) {
    for directive in directives {
        if directive.name == "upstream" && directive.children.is_some() && !inside_upstream {
            upstreams.push(build_upstream(directive));
            continue;
        }
        if directive.name == "server" && !inside_upstream {
            if directive.children.is_some() {
                servers.push(build_server(directive, config_dir));
            }
            continue;
        }
        if let Some(children) = &directive.children {
            walk_structure(children, config_dir, servers, upstreams, inside_upstream);
        }
    }
}

fn build_server(directive: &Directive, config_dir: &Path) -> ServerInfo {
    let mut server = ServerInfo {
        source: directive.source.clone(),
        line: directive.line,
        ..Default::default()
    };
    let mut names = Vec::new();
    let mut cert_seen = false;

    for child in directive.children.as_deref().unwrap_or_default() {
        match child.name.as_str() {
            "listen" => {
                if let Some(arg) = child.args.first() {
                    server.listens.push(child.args.join(" "));
                    if child.args.iter().any(|a| a == "ssl") {
                        server.tls = true;
                    }
                    if arg.contains("ssl") {
                        server.tls = true;
                    }
                }
            }
            "server_name" => names.extend(child.args.iter().cloned()),
            "root" => {
                if let Some(arg) = child.args.first() {
                    server.root = Some(arg.clone());
                }
            }
            "ssl_certificate" => {
                if let Some(arg) = child.args.first() {
                    server.cert_path = Some(resolve_path(arg, config_dir));
                    server.tls = true;
                    cert_seen = true;
                }
            }
            "ssl_certificate_key" => {
                if let Some(arg) = child.args.first() {
                    server.key_path = Some(resolve_path(arg, config_dir));
                }
            }
            "return" => {
                server.internal_redirect = Some(child.args.join(" "));
            }
            "location" => {
                server.locations += 1;
                collect_proxy_passes(child, &mut server.proxy_targets);
            }
            _ => {}
        }
    }

    for name in names {
        if !server.names.contains(&name) {
            server.names.push(name);
        }
    }
    if cert_seen {
        server.tls = true;
    }
    server
}

fn collect_proxy_passes(directive: &Directive, out: &mut Vec<String>) {
    if directive.name == "proxy_pass"
        && let Some(target) = directive.args.first()
        && !out.contains(target)
    {
        out.push(target.clone());
    }
    for child in directive.children.as_deref().unwrap_or_default() {
        collect_proxy_passes(child, out);
    }
}

fn build_upstream(directive: &Directive) -> UpstreamInfo {
    let mut upstream = UpstreamInfo {
        name: directive
            .args
            .first()
            .cloned()
            .unwrap_or_else(|| "未命名".into()),
        source: directive.source.clone(),
        line: directive.line,
        method: "round-robin".into(),
        backends: Vec::new(),
    };
    for child in directive.children.as_deref().unwrap_or_default() {
        match child.name.as_str() {
            "server" => {
                if let Some(address) = child.args.first() {
                    let flags = &child.args[1..];
                    upstream.backends.push(BackendInfo {
                        address: address.clone(),
                        down: flags.iter().any(|f| f == "down"),
                        backup: flags.iter().any(|f| f == "backup"),
                        extra: flags.to_vec(),
                    });
                }
            }
            "least_conn" | "ip_hash" | "hash" | "random" => {
                upstream.method = child.name.clone();
            }
            _ => {}
        }
    }
    upstream
}

fn collect_logs(directives: &[Directive], config_dir: &Path, out: &mut Vec<LogSource>) {
    for directive in directives {
        let kind = match directive.name.as_str() {
            "access_log" => Some(LogKind::Access),
            "error_log" => Some(LogKind::Error),
            _ => None,
        };
        if let Some(kind) = kind
            && let Some(arg) = directive.args.first()
        {
            let lower = arg.to_ascii_lowercase();
            if lower != "off"
                && lower != "stderr"
                && !lower.starts_with("syslog:")
                && !lower.starts_with("memory:")
            {
                let path = resolve_path(arg, config_dir);
                let source = LogSource {
                    kind,
                    label: log_label(&path),
                    path,
                };
                if !out.contains(&source) {
                    out.push(source);
                }
            }
        }
        if let Some(children) = &directive.children {
            collect_logs(children, config_dir, out);
        }
    }
}

fn log_label(path: &Path) -> String {
    path.file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| path.display().to_string())
}

fn add_default_logs(config_dir: &Path, out: &mut Vec<LogSource>) {
    let candidates = [
        (LogKind::Access, config_dir.join("logs/access.log")),
        (LogKind::Error, config_dir.join("logs/error.log")),
        (LogKind::Access, PathBuf::from("/var/log/nginx/access.log")),
        (LogKind::Error, PathBuf::from("/var/log/nginx/error.log")),
    ];
    for (kind, path) in candidates {
        if path.exists() && !out.iter().any(|l| l.path == path) {
            out.push(LogSource {
                kind,
                label: log_label(&path),
                path,
            });
        }
    }
}

fn find_pid_file(directives: &[Directive], config_dir: &Path) -> PathBuf {
    fn walk(directives: &[Directive], config_dir: &Path) -> Option<PathBuf> {
        for directive in directives {
            if directive.name == "pid"
                && let Some(arg) = directive.args.first()
            {
                return Some(resolve_path(arg, config_dir));
            }
            if let Some(children) = &directive.children
                && let Some(found) = walk(children, config_dir)
            {
                return Some(found);
            }
        }
        None
    }
    if let Some(pid) = walk(directives, config_dir) {
        return pid;
    }
    for candidate in [
        PathBuf::from("/run/nginx.pid"),
        PathBuf::from("/var/run/nginx.pid"),
        config_dir.join("logs/nginx.pid"),
    ] {
        if candidate.exists() {
            return candidate;
        }
    }
    PathBuf::from("/run/nginx.pid")
}

fn resolve_path(arg: &str, config_dir: &Path) -> PathBuf {
    let path = Path::new(arg);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        config_dir.join(path)
    }
}

fn scan_config_files(config_dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let dirs = [
        config_dir.to_path_buf(),
        config_dir.join("conf.d"),
        config_dir.join("sites-available"),
        config_dir.join("sites-enabled"),
        config_dir.join("snippets"),
        config_dir.join("modules-enabled"),
    ];
    for dir in dirs {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(meta) = entry.metadata() else {
                continue;
            };
            if meta.is_dir() {
                continue;
            }
            if looks_like_config(&path, meta.len()) {
                out.push(path);
            }
        }
    }
    out
}

pub fn looks_like_config(path: &Path, size: u64) -> bool {
    if size > 1024 * 1024 {
        return false;
    }
    let Some(name) = path.file_name().map(|s| s.to_string_lossy().to_string()) else {
        return false;
    };
    if name.starts_with('.') || name.ends_with('~') {
        return false;
    }
    let lower = name.to_ascii_lowercase();
    let denied = [
        ".key", ".crt", ".pem", ".der", ".p12", ".pfx", ".gz", ".zip", ".log", ".pid", ".swp",
        ".bak", ".tmp", ".lock",
    ];
    !denied.iter().any(|ext| lower.ends_with(ext))
}

/// Parse a single site file without expanding includes.  This is used to show
/// useful server details for a site that is currently disabled (and therefore
/// not present in the main config include tree).
pub fn servers_in_file(path: &Path, config_dir: &Path) -> Vec<ServerInfo> {
    let Ok(text) = fs::read_to_string(path) else {
        return Vec::new();
    };
    let (directives, _) = parse_str(&text, path);
    let mut servers = Vec::new();
    let mut upstreams = Vec::new();
    walk_structure(&directives, config_dir, &mut servers, &mut upstreams, false);
    servers
}

/// Return a stable, human-readable label for a config file.
pub fn file_label(path: &Path, config_dir: &Path) -> String {
    path.strip_prefix(config_dir)
        .unwrap_or(path)
        .display()
        .to_string()
}

/// Resolve the source path so symlinked sites can be matched to their target.
pub fn canonical(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;

    fn write(path: &Path, text: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        let mut file = fs::File::create(path).unwrap();
        file.write_all(text.as_bytes()).unwrap();
    }

    #[test]
    fn parses_quotes_comments_and_blocks() {
        let cfg = r#"
            # comment
            http {
                server {
                    listen 80 default_server;
                    server_name example.com "www.example.com";
                    root /var/www/$host;
                    location / {
                        proxy_pass http://app;
                    }
                    location ~ "\.php$" { return 404; }
                }
                upstream app { server 127.0.0.1:3000; }
            }
        "#;
        let (dirs, errors) = parse_str(cfg, Path::new("test.conf"));
        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(dirs[0].name, "http");
        let http = &dirs[0].children.as_ref().unwrap();
        assert_eq!(http[0].name, "server");
        let server = &http[0].children.as_ref().unwrap();
        assert_eq!(server[1].args, vec!["example.com", "www.example.com"]);
        assert_eq!(server[4].args, vec!["~", r"\.php$"]);
    }

    #[test]
    fn builds_servers_upstreams_and_logs() {
        let tmp = tempfile::tempdir().unwrap();
        let conf = tmp.path().join("nginx.conf");
        write(
            &conf,
            r#"
            pid /tmp/demo.pid;
            error_log logs/error.log;
            access_log logs/access.log;
            http {
                upstream app {
                    least_conn;
                    server 127.0.0.1:3000 weight=3;
                    server 127.0.0.1:3001 backup;
                    server 127.0.0.1:3002 down;
                }
                server {
                    listen 443 ssl;
                    server_name example.com www.example.com;
                    ssl_certificate certs/example.pem;
                    location / {
                        proxy_pass http://app;
                    }
                }
            }
            "#,
        );
        let model = load(&conf);
        assert!(model.parse_errors.is_empty(), "{:?}", model.parse_errors);
        assert_eq!(model.servers.len(), 1);
        let server = &model.servers[0];
        assert!(server.tls);
        assert_eq!(server.names.len(), 2);
        assert_eq!(server.proxy_targets, vec!["http://app"]);
        assert_eq!(
            server.cert_path.as_deref(),
            Some(tmp.path().join("certs/example.pem").as_path())
        );
        assert_eq!(model.upstreams.len(), 1);
        assert_eq!(model.upstreams[0].method, "least_conn");
        assert_eq!(model.upstreams[0].backends.len(), 3);
        assert!(model.upstreams[0].backends[1].backup);
        assert!(model.upstreams[0].backends[2].down);
        assert!(model.logs.iter().any(|l| l.kind == LogKind::Access));
        assert_eq!(model.pid_file, PathBuf::from("/tmp/demo.pid"));
    }

    #[test]
    fn expands_globs_and_is_cycle_safe() {
        let tmp = tempfile::tempdir().unwrap();
        write(
            &tmp.path().join("nginx.conf"),
            "http { include sites-enabled/*.conf; }\n",
        );
        write(
            &tmp.path().join("sites-enabled/a.conf"),
            "server { listen 8080; server_name a.test; }\n",
        );
        write(
            &tmp.path().join("sites-enabled/b.conf"),
            "server { listen 8081; server_name b.test; }\n",
        );
        let model = load(&tmp.path().join("nginx.conf"));
        assert_eq!(model.servers.len(), 2);
        assert!(model.parse_errors.is_empty(), "{:?}", model.parse_errors);
        assert!(
            model
                .files
                .iter()
                .any(|f| f.ends_with("sites-enabled/a.conf"))
        );
    }
}
