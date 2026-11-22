//! Minimal vsftpd discovery and service control.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, Default)]
pub struct FtpUser {
    pub name: String,
    pub home: String,
}

#[derive(Debug, Clone, Default)]
pub struct FtpSnapshot {
    pub installed: bool,
    pub running: bool,
    pub config_path: Option<PathBuf>,
    pub settings: Vec<(String, String)>,
    pub users: Vec<FtpUser>,
    pub error: Option<String>,
}

impl FtpSnapshot {
    pub fn refresh(&mut self) {
        self.error = None;
        self.installed = find_binary().is_some();
        self.config_path = find_config();
        self.running = pgrep_vsftpd();
        self.settings.clear();
        if let Some(path) = &self.config_path {
            match fs::read_to_string(path) {
                Ok(text) => self.settings = parse_config(&text),
                Err(err) => self.error = Some(format!("读取 {} 失败：{err}", path.display())),
            }
        } else if self.installed {
            self.error = Some("已安装 vsftpd，但没有找到配置文件。".into());
        }
        self.users = system_users();
    }

    pub fn setting(&self, key: &str) -> Option<&str> {
        self.settings
            .iter()
            .find(|(name, _)| name == key)
            .map(|(_, value)| value.as_str())
    }
}

pub fn service_action(action: &str) -> Result<String, String> {
    if !matches!(action, "start" | "stop" | "restart" | "reload") {
        return Err("不支持的 FTP 操作。".into());
    }
    if Path::new("/run/systemd/system").is_dir() && Path::new("/usr/bin/systemctl").is_file() {
        return run(Command::new("/usr/bin/systemctl").arg(action).arg("vsftpd"));
    }
    if Path::new("/usr/sbin/service").is_file() {
        return run(Command::new("/usr/sbin/service").arg("vsftpd").arg(action));
    }
    Err("当前环境没有 systemd/service，无法控制 vsftpd 服务。".into())
}

fn run(command: &mut Command) -> Result<String, String> {
    let output = command
        .env("LC_ALL", "C")
        .output()
        .map_err(|e| format!("执行失败：{e}"))?;
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    if output.status.success() {
        Ok(if stdout.trim().is_empty() {
            "操作成功。".into()
        } else {
            stdout
        })
    } else {
        Err(if stderr.trim().is_empty() {
            stdout
        } else {
            stderr
        })
    }
}

fn find_binary() -> Option<PathBuf> {
    for candidate in [
        "/usr/sbin/vsftpd",
        "/usr/local/sbin/vsftpd",
        "/usr/bin/vsftpd",
    ] {
        let path = PathBuf::from(candidate);
        if path.is_file() {
            return Some(path);
        }
    }
    std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths)
            .map(|dir| dir.join("vsftpd"))
            .find(|path| path.is_file())
    })
}

fn find_config() -> Option<PathBuf> {
    [
        "/etc/vsftpd.conf",
        "/etc/vsftpd/vsftpd.conf",
        "/etc/vsftpd/vsftpd.conf.d/vsftpd.conf",
    ]
    .into_iter()
    .map(PathBuf::from)
    .find(|path| path.is_file())
}

fn pgrep_vsftpd() -> bool {
    Command::new("pgrep")
        .args(["-x", "vsftpd"])
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

pub fn parse_config(text: &str) -> Vec<(String, String)> {
    text.lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                return None;
            }
            let (key, value) = line.split_once('=')?;
            Some((key.trim().to_string(), value.trim().to_string()))
        })
        .collect()
}

fn system_users() -> Vec<FtpUser> {
    let Ok(text) = fs::read_to_string("/etc/passwd") else {
        return Vec::new();
    };
    text.lines()
        .filter_map(|line| {
            let fields: Vec<&str> = line.split(':').collect();
            if fields.len() < 7 {
                return None;
            }
            let uid = fields[2].parse::<u32>().ok()?;
            if uid < 1000 || fields[6].contains("nologin") || fields[6].contains("false") {
                return None;
            }
            Some(FtpUser {
                name: fields[0].to_string(),
                home: fields[5].to_string(),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_vsftpd_config() {
        let text = "# comment\nlisten=YES\nlisten_port=21\nwrite_enable=YES\n";
        let settings = parse_config(text);
        assert_eq!(settings.len(), 3);
        assert_eq!(settings[1].0, "listen_port");
        assert_eq!(settings[1].1, "21");
    }
}
