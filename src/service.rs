//! Nginx process inspection and privileged service operations.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Manager {
    Systemd,
    Direct,
}

impl Manager {
    pub fn label(self) -> &'static str {
        match self {
            Self::Systemd => "systemd",
            Self::Direct => "信号",
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct ProcessInfo {
    pub master_pid: Option<u32>,
    pub workers: usize,
    pub rss_kb: u64,
    pub uptime_secs: Option<u64>,
}

impl ProcessInfo {
    pub fn running(&self) -> bool {
        self.master_pid.is_some()
    }
}

#[derive(Debug, Clone, Default)]
pub struct CommandResult {
    pub label: String,
    pub command: String,
    pub success: bool,
    pub code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub elapsed_ms: u128,
}

impl CommandResult {
    pub fn error(label: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            command: String::new(),
            success: false,
            code: None,
            stdout: String::new(),
            stderr: message.into(),
            elapsed_ms: 0,
        }
    }

    pub fn output(&self) -> String {
        let mut text = String::new();
        if !self.stdout.trim().is_empty() {
            text.push_str(self.stdout.trim_end());
        }
        if !self.stderr.trim().is_empty() {
            if !text.is_empty() {
                text.push('\n');
            }
            text.push_str(self.stderr.trim_end());
        }
        if text.is_empty() {
            text.push_str(if self.success {
                "命令执行成功。"
            } else {
                "命令没有输出。"
            });
        }
        text
    }

    pub fn summary(&self) -> String {
        if self.success {
            format!("{}成功", self.label)
        } else {
            match self.code {
                Some(code) => format!("{}失败（退出码 {code}）", self.label),
                None => format!("{}失败", self.label),
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Test,
    Reload,
    Start,
    Stop,
    Restart,
}

impl Action {
    pub fn label(self) -> &'static str {
        match self {
            Self::Test => "配置校验",
            Self::Reload => "热重载",
            Self::Start => "启动",
            Self::Stop => "停止",
            Self::Restart => "重启",
        }
    }
}

pub fn is_root() -> bool {
    // SAFETY: geteuid has no preconditions and cannot fail.
    unsafe { libc::geteuid() == 0 }
}

pub fn detect_manager() -> Manager {
    if Path::new("/run/systemd/system").is_dir() && systemctl_path().is_some() {
        Manager::Systemd
    } else {
        Manager::Direct
    }
}

fn systemctl_path() -> Option<PathBuf> {
    ["/usr/bin/systemctl", "/bin/systemctl"]
        .into_iter()
        .map(PathBuf::from)
        .find(|p| p.is_file())
}

pub fn detect_binary(explicit: Option<&Path>) -> Option<PathBuf> {
    if let Some(path) = explicit {
        return path.is_file().then(|| path.to_path_buf());
    }
    if let Some(path) = std::env::var_os("NGINX_BIN") {
        let path = PathBuf::from(path);
        if path.is_file() {
            return Some(path);
        }
    }
    let candidates = [
        "/usr/sbin/nginx",
        "/usr/local/sbin/nginx",
        "/usr/local/nginx/sbin/nginx",
        "/opt/homebrew/bin/nginx",
        "/opt/nginx/sbin/nginx",
        "/sbin/nginx",
    ];
    for candidate in candidates {
        let path = PathBuf::from(candidate);
        if path.is_file() {
            return Some(path);
        }
    }
    find_in_path("nginx")
}

fn find_in_path(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(name))
        .find(|candidate| candidate.is_file())
}

pub fn version(bin: &Path) -> Option<String> {
    let output = Command::new(bin).arg("-v").output().ok()?;
    let text = if output.stderr.is_empty() {
        String::from_utf8_lossy(&output.stdout).to_string()
    } else {
        String::from_utf8_lossy(&output.stderr).to_string()
    };
    parse_version_output(&text)
}

fn parse_version_output(text: &str) -> Option<String> {
    let line = text.lines().next()?.trim();
    let version = line
        .strip_prefix("nginx version:")
        .unwrap_or(line)
        .trim()
        .strip_prefix("nginx/")
        .unwrap_or_else(|| line.strip_prefix("nginx version:").unwrap_or(line).trim())
        .trim();
    Some(
        version
            .split_whitespace()
            .next()
            .unwrap_or(version)
            .to_string(),
    )
}

pub fn inspect(pid_file: &Path) -> ProcessInfo {
    let mut procs = scan_processes();
    if procs.is_empty() {
        return ProcessInfo::default();
    }

    let from_pid_file = fs::read_to_string(pid_file)
        .ok()
        .and_then(|s| s.trim().parse::<u32>().ok())
        .filter(|pid| procs.iter().any(|p| p.pid == *pid));

    let master_pid = from_pid_file.or_else(|| {
        let nginx_pids: Vec<u32> = procs.iter().map(|p| p.pid).collect();
        procs
            .iter()
            .filter(|p| !nginx_pids.contains(&p.ppid))
            .map(|p| p.pid)
            .min()
            .or_else(|| procs.iter().map(|p| p.pid).min())
    });

    let Some(master_pid) = master_pid else {
        return ProcessInfo::default();
    };
    let mut worker_count = procs
        .iter()
        .filter(|p| p.ppid == master_pid && p.pid != master_pid)
        .count();
    if worker_count == 0 {
        worker_count = procs.iter().filter(|p| p.pid != master_pid).count();
    }
    let rss_kb = procs
        .iter()
        .filter(|p| p.pid == master_pid || p.ppid == master_pid)
        .map(|p| p.rss_kb)
        .sum::<u64>()
        .max(
            procs
                .iter()
                .find(|p| p.pid == master_pid)
                .map(|p| p.rss_kb)
                .unwrap_or(0),
        );

    let uptime_secs = procs
        .iter()
        .find(|p| p.pid == master_pid)
        .and_then(|p| p.start_ticks)
        .and_then(|ticks| {
            let uptime = fs::read_to_string("/proc/uptime")
                .ok()?
                .split_whitespace()
                .next()?
                .parse::<f64>()
                .ok()?;
            let started = ticks as f64 / 100.0;
            (uptime >= started).then_some((uptime - started) as u64)
        })
        .or_else(|| ps_elapsed(master_pid));

    // Keep the field used; the process list is deliberately not exposed.
    procs.clear();

    ProcessInfo {
        master_pid: Some(master_pid),
        workers: worker_count,
        rss_kb,
        uptime_secs,
    }
}

fn ps_elapsed(pid: u32) -> Option<u64> {
    let output = Command::new("ps")
        .env("LC_ALL", "C")
        .args(["-o", "etimes=", "-p", &pid.to_string()])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse::<u64>()
        .ok()
}

#[derive(Debug)]
struct ProcEntry {
    pid: u32,
    ppid: u32,
    rss_kb: u64,
    start_ticks: Option<u64>,
}

fn scan_processes() -> Vec<ProcEntry> {
    let Ok(entries) = fs::read_dir("/proc") else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in entries.flatten() {
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        let Ok(pid) = name.parse::<u32>() else {
            continue;
        };
        let comm = match fs::read_to_string(entry.path().join("comm")) {
            Ok(c) => c,
            Err(_) => continue,
        };
        if comm.trim() != "nginx" {
            continue;
        }
        let status = fs::read_to_string(entry.path().join("status")).unwrap_or_default();
        let mut ppid = 0;
        let mut rss_kb = 0;
        for line in status.lines() {
            if let Some(rest) = line.strip_prefix("PPid:") {
                ppid = rest.trim().parse().unwrap_or(0);
            } else if let Some(rest) = line.strip_prefix("VmRSS:") {
                rss_kb = rest
                    .split_whitespace()
                    .next()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(0);
            }
        }
        let start_ticks = fs::read_to_string(entry.path().join("stat"))
            .ok()
            .and_then(|stat| {
                let end = stat.rfind(')')?;
                let rest = stat.get(end + 1..)?.split_whitespace().collect::<Vec<_>>();
                // Field 22 overall; after comm the vector starts at field 3.
                rest.get(19)?.parse::<u64>().ok()
            });
        out.push(ProcEntry {
            pid,
            ppid,
            rss_kb,
            start_ticks,
        });
    }
    out
}

pub fn perform(
    action: Action,
    bin: Option<&Path>,
    config: &Path,
    pid_file: &Path,
    manager: Manager,
) -> Vec<CommandResult> {
    match action {
        Action::Test => vec![test_config(bin, config)],
        Action::Reload => vec![reload(bin, config, manager)],
        Action::Start => vec![start(bin, config, manager)],
        Action::Stop => vec![stop(bin, config, manager)],
        Action::Restart => restart(bin, config, pid_file, manager),
    }
}

pub fn test_config(bin: Option<&Path>, config: &Path) -> CommandResult {
    let Some(bin) = bin else {
        return CommandResult::error("配置校验", "没有找到 nginx 可执行文件，请用 --bin 指定。");
    };
    let mut cmd = Command::new(bin);
    cmd.arg("-t").arg("-c").arg(config);
    run(cmd, "配置校验")
}

fn reload(bin: Option<&Path>, config: &Path, manager: Manager) -> CommandResult {
    if can_use_systemd(manager, config) {
        return systemctl("reload", "热重载");
    }
    let Some(bin) = bin else {
        return CommandResult::error("热重载", "没有找到 nginx 可执行文件。");
    };
    let mut cmd = Command::new(bin);
    cmd.arg("-c").arg(config).arg("-s").arg("reload");
    run(cmd, "热重载")
}

fn start(bin: Option<&Path>, config: &Path, manager: Manager) -> CommandResult {
    if can_use_systemd(manager, config) {
        return systemctl("start", "启动");
    }
    let Some(bin) = bin else {
        return CommandResult::error("启动", "没有找到 nginx 可执行文件。");
    };
    let mut cmd = Command::new(bin);
    cmd.arg("-c").arg(config);
    run(cmd, "启动")
}

fn stop(bin: Option<&Path>, config: &Path, manager: Manager) -> CommandResult {
    if can_use_systemd(manager, config) {
        return systemctl("stop", "停止");
    }
    let Some(bin) = bin else {
        return CommandResult::error("停止", "没有找到 nginx 可执行文件。");
    };
    let mut cmd = Command::new(bin);
    cmd.arg("-c").arg(config).arg("-s").arg("quit");
    run(cmd, "停止")
}

fn restart(
    bin: Option<&Path>,
    config: &Path,
    pid_file: &Path,
    manager: Manager,
) -> Vec<CommandResult> {
    if can_use_systemd(manager, config) {
        return vec![systemctl("restart", "重启")];
    }
    let mut results = vec![stop(bin, config, manager)];
    let stopped = wait_for_exit(pid_file, Duration::from_secs(4));
    if !stopped {
        results.push(CommandResult::error(
            "重启",
            "旧进程没有在 4 秒内退出，已取消启动，避免端口冲突。",
        ));
        return results;
    }
    results.push(start(bin, config, manager));
    results
}

fn wait_for_exit(pid_file: &Path, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if !inspect(pid_file).running() {
            return true;
        }
        thread::sleep(Duration::from_millis(120));
    }
    !inspect(pid_file).running()
}

fn can_use_systemd(manager: Manager, config: &Path) -> bool {
    manager == Manager::Systemd
        && fs::canonicalize(config)
            .map(|p| p == Path::new("/etc/nginx/nginx.conf"))
            .unwrap_or(false)
}

fn systemctl(verb: &str, label: &str) -> CommandResult {
    let Some(systemctl) = systemctl_path() else {
        return CommandResult::error(label, "没有找到 systemctl。");
    };
    let mut cmd = Command::new(systemctl);
    cmd.arg(verb).arg("nginx");
    run(cmd, label)
}

fn run(mut cmd: Command, label: &str) -> CommandResult {
    let display = format_command(&cmd);
    let started = Instant::now();
    cmd.env("LC_ALL", "C");
    let result = cmd.output();
    let elapsed_ms = started.elapsed().as_millis();
    match result {
        Ok(output) => {
            let mut stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let mut stderr = String::from_utf8_lossy(&output.stderr).to_string();
            truncate_output(&mut stdout);
            truncate_output(&mut stderr);
            CommandResult {
                label: label.to_string(),
                command: display,
                success: output.status.success(),
                code: output.status.code(),
                stdout,
                stderr,
                elapsed_ms,
            }
        }
        Err(err) => CommandResult {
            label: label.to_string(),
            command: display,
            success: false,
            code: None,
            stdout: String::new(),
            stderr: format!("无法执行命令：{err}"),
            elapsed_ms,
        },
    }
}

fn truncate_output(text: &mut String) {
    const LIMIT: usize = 64 * 1024;
    if text.len() > LIMIT {
        text.truncate(LIMIT);
        text.push_str("\n…（输出过长，已截断）");
    }
}

fn format_command(cmd: &Command) -> String {
    let mut parts = vec![cmd.get_program().to_string_lossy().to_string()];
    for arg in cmd.get_args() {
        let arg = arg.to_string_lossy();
        if arg.contains(' ') {
            parts.push(format!("{arg:?}"));
        } else {
            parts.push(arg.to_string());
        }
    }
    parts.join(" ")
}

pub fn format_uptime(seconds: Option<u64>) -> String {
    let Some(mut secs) = seconds else {
        return "—".to_string();
    };
    let days = secs / 86_400;
    secs %= 86_400;
    let hours = secs / 3_600;
    secs %= 3_600;
    let minutes = secs / 60;
    secs %= 60;
    if days > 0 {
        format!("{days}天 {hours}时")
    } else if hours > 0 {
        format!("{hours}时 {minutes}分")
    } else {
        format!("{minutes}分 {secs}秒")
    }
}

pub fn format_memory(kb: u64) -> String {
    if kb >= 1024 * 1024 {
        format!("{:.1} GB", kb as f64 / 1024.0 / 1024.0)
    } else if kb >= 1024 {
        format!("{:.1} MB", kb as f64 / 1024.0)
    } else {
        format!("{kb} KB")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_uptime_and_memory() {
        assert_eq!(format_uptime(Some(3_900)), "1时 5分");
        assert_eq!(format_uptime(Some(73)), "1分 13秒");
        assert_eq!(format_uptime(Some(90_000)), "1天 1时");
        assert_eq!(format_memory(2048), "2.0 MB");
    }

    #[test]
    fn parses_nginx_version_banner() {
        assert_eq!(
            parse_version_output("nginx version: nginx/1.24.0 (Ubuntu)\n").as_deref(),
            Some("1.24.0")
        );
        assert_eq!(
            parse_version_output("nginx version: openresty/1.25.3.1\n").as_deref(),
            Some("openresty/1.25.3.1")
        );
    }

    #[test]
    fn reports_command_errors() {
        let result = test_config(None, Path::new("/tmp/nginx.conf"));
        assert!(!result.success);
        assert!(result.output().contains("nginx"));
    }
}
