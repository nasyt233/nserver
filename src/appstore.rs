//! Curated application catalog and native package install/remove actions.

use std::process::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageManager {
    Apt,
    Dnf,
    Unsupported,
}

impl PackageManager {
    pub fn label(self) -> &'static str {
        match self {
            Self::Apt => "APT",
            Self::Dnf => "DNF",
            Self::Unsupported => "不支持",
        }
    }
}

#[derive(Debug, Clone)]
pub struct StoreApp {
    pub name: &'static str,
    pub package: &'static str,
    pub command: Option<&'static str>,
    pub category: &'static str,
    pub description: &'static str,
    pub installed: bool,
}

#[derive(Debug, Clone)]
pub struct StoreState {
    pub apps: Vec<StoreApp>,
    pub selected: usize,
    pub manager: PackageManager,
    pub last_refresh: std::time::Instant,
}

impl Default for StoreState {
    fn default() -> Self {
        Self {
            apps: catalog(),
            selected: 0,
            manager: detect_manager(),
            last_refresh: std::time::Instant::now(),
        }
    }
}

impl StoreState {
    pub fn refresh(&mut self) {
        self.manager = detect_manager();
        for app in &mut self.apps {
            app.installed = app
                .command
                .map(command_exists)
                .unwrap_or_else(|| package_installed(self.manager, app.package));
        }
        self.selected = self.selected.min(self.apps.len().saturating_sub(1));
        self.last_refresh = std::time::Instant::now();
    }

    pub fn selected_app(&self) -> Option<&StoreApp> {
        self.apps.get(self.selected)
    }
}

fn detect_manager() -> PackageManager {
    if std::path::Path::new("/etc/debian_version").is_file() && command_exists("apt-get") {
        PackageManager::Apt
    } else if std::path::Path::new("/etc/redhat-release").is_file() && command_exists("dnf") {
        PackageManager::Dnf
    } else {
        PackageManager::Unsupported
    }
}

fn command_exists(name: &str) -> bool {
    ["/usr/bin", "/usr/sbin", "/bin", "/sbin", "/usr/local/bin"]
        .iter()
        .map(|dir| std::path::Path::new(dir).join(name))
        .any(|path| path.is_file())
        || std::env::var_os("PATH")
            .map(|paths| {
                std::env::split_paths(&paths)
                    .map(|dir| dir.join(name))
                    .any(|path| path.is_file())
            })
            .unwrap_or(false)
}

fn package_installed(manager: PackageManager, package: &str) -> bool {
    match manager {
        PackageManager::Apt => Command::new("dpkg-query")
            .args(["-W", "-f=${Status}", package])
            .output()
            .map(|out| {
                out.status.success()
                    && String::from_utf8_lossy(&out.stdout).contains("install ok installed")
            })
            .unwrap_or(false),
        PackageManager::Dnf => Command::new("rpm")
            .args(["-q", package])
            .status()
            .map(|status| status.success())
            .unwrap_or(false),
        PackageManager::Unsupported => false,
    }
}

pub fn apply(manager: PackageManager, package: &str, install: bool) -> Result<String, String> {
    if !is_safe_package(package) {
        return Err("包名格式无效。".into());
    }
    match manager {
        PackageManager::Apt => {
            let mut output = String::new();
            if install {
                output.push_str(&run("apt-get", &["update", "-qq"], "更新软件源")?);
                output.push_str(&run(
                    "apt-get",
                    &["install", "-y", "--no-install-recommends", package],
                    "安装",
                )?);
            } else {
                output.push_str(&run("apt-get", &["remove", "-y", package], "卸载")?);
            }
            Ok(output)
        }
        PackageManager::Dnf => {
            let normalized = dnf_package(package);
            let args: Vec<&str> = if install {
                vec!["install", "-y", normalized]
            } else {
                vec!["remove", "-y", normalized]
            };
            run("dnf", &args, if install { "安装" } else { "卸载" })
        }
        PackageManager::Unsupported => {
            Err("当前系统没有检测到支持的包管理器（APT 或 DNF）。".into())
        }
    }
}

fn dnf_package(package: &str) -> &str {
    match package {
        "apache2" => "httpd",
        "openssh-server" => "openssh-server",
        "mariadb-server" => "mariadb-server",
        "php-fpm" => "php-fpm",
        "nfs-common" => "nfs-utils",
        "mongodb-clients" => "mongodb-mongosh",
        other => other,
    }
}

fn run(program: &str, args: &[&str], label: &str) -> Result<String, String> {
    let output = Command::new(program)
        .env("LC_ALL", "C")
        .env("DEBIAN_FRONTEND", "noninteractive")
        .args(args)
        .output()
        .map_err(|err| format!("{label}失败：{err}"))?;
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    if output.status.success() {
        let text = if stdout.trim().is_empty() && stderr.trim().is_empty() {
            format!("{label}完成。")
        } else {
            format!("{stdout}{stderr}")
        };
        Ok(text)
    } else {
        Err(format!(
            "{label}失败（退出码 {:?}）：\n{}",
            output.status.code(),
            if stderr.trim().is_empty() {
                stdout
            } else {
                stderr
            }
        ))
    }
}

fn is_safe_package(package: &str) -> bool {
    !package.is_empty()
        && !package.starts_with('-')
        && package
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | '+'))
}

fn catalog() -> Vec<StoreApp> {
    let defs = [
        (
            "Nginx",
            "nginx",
            Some("nginx"),
            "基础",
            "高性能 Web 服务器与反向代理",
        ),
        (
            "Docker",
            "docker.io",
            Some("docker"),
            "容器",
            "容器运行时与 Docker CLI",
        ),
        (
            "vsftpd",
            "vsftpd",
            Some("vsftpd"),
            "服务",
            "轻量 FTP 服务器",
        ),
        (
            "Certbot",
            "certbot",
            Some("certbot"),
            "安全",
            "Let's Encrypt 证书签发工具",
        ),
        ("UFW", "ufw", Some("ufw"), "安全", "简单防火墙"),
        (
            "Fail2Ban",
            "fail2ban",
            Some("fail2ban-client"),
            "安全",
            "防暴力破解与登录封禁",
        ),
        (
            "Redis",
            "redis-server",
            Some("redis-server"),
            "数据库",
            "内存键值数据库",
        ),
        (
            "MariaDB",
            "mariadb-server",
            Some("mysqld"),
            "数据库",
            "MySQL 兼容数据库",
        ),
        (
            "PostgreSQL",
            "postgresql",
            Some("psql"),
            "数据库",
            "关系型数据库",
        ),
        (
            "Node.js",
            "nodejs",
            Some("node"),
            "运行时",
            "JavaScript 运行时",
        ),
        (
            "Python 3",
            "python3",
            Some("python3"),
            "运行时",
            "Python 解释器",
        ),
        ("Rclone", "rclone", Some("rclone"), "工具", "云存储同步工具"),
        ("tmux", "tmux", Some("tmux"), "工具", "终端复用器"),
        ("htop", "htop", Some("htop"), "工具", "交互式进程查看器"),
        ("Git", "git", Some("git"), "开发", "版本控制工具"),
        ("curl", "curl", Some("curl"), "工具", "HTTP 命令行客户端"),
        ("wget", "wget", Some("wget"), "工具", "命令行下载工具"),
        ("unzip", "unzip", Some("unzip"), "工具", "ZIP 解压工具"),
        (
            "SQLite",
            "sqlite3",
            Some("sqlite3"),
            "数据库",
            "嵌入式 SQL 数据库",
        ),
        (
            "WireGuard",
            "wireguard",
            Some("wg"),
            "安全",
            "现代 VPN 工具",
        ),
        (
            "Apache HTTP Server",
            "apache2",
            Some("apache2ctl"),
            "服务",
            "成熟的 HTTP 服务器",
        ),
        (
            "Caddy",
            "caddy",
            Some("caddy"),
            "服务",
            "自动 HTTPS 的 Web 服务器",
        ),
        (
            "HAProxy",
            "haproxy",
            Some("haproxy"),
            "服务",
            "高可用 TCP/HTTP 负载均衡器",
        ),
        (
            "OpenSSH Server",
            "openssh-server",
            Some("sshd"),
            "服务",
            "远程安全登录服务",
        ),
        (
            "NFS utilities",
            "nfs-common",
            Some("mount.nfs"),
            "存储",
            "NFS 客户端与工具",
        ),
        (
            "Samba",
            "samba",
            Some("smbd"),
            "存储",
            "SMB/CIFS 文件共享服务",
        ),
        (
            "Ansible",
            "ansible",
            Some("ansible"),
            "运维",
            "自动化配置管理",
        ),
        (
            "Prometheus Node Exporter",
            "prometheus-node-exporter",
            Some("prometheus-node-exporter"),
            "监控",
            "主机指标采集器",
        ),
        ("rsync", "rsync", Some("rsync"), "工具", "增量文件同步工具"),
        ("jq", "jq", Some("jq"), "工具", "JSON 命令行处理器"),
        (
            "OpenSSL",
            "openssl",
            Some("openssl"),
            "安全",
            "TLS 与加密工具",
        ),
        (
            "Certbot Nginx",
            "python3-certbot-nginx",
            Some("certbot"),
            "安全",
            "Certbot Nginx 集成插件",
        ),
        (
            "Composer",
            "composer",
            Some("composer"),
            "开发",
            "PHP 依赖管理器",
        ),
        (
            "PHP-FPM",
            "php-fpm",
            Some("php-fpm"),
            "运行时",
            "PHP FastCGI 进程管理器",
        ),
        (
            "Memcached",
            "memcached",
            Some("memcached"),
            "数据库",
            "分布式内存缓存",
        ),
        (
            "MongoDB tools",
            "mongodb-clients",
            Some("mongosh"),
            "数据库",
            "MongoDB 命令行工具",
        ),
    ];
    defs.into_iter()
        .map(|(name, package, command, category, description)| StoreApp {
            name,
            package,
            command,
            category,
            description,
            installed: false,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_unsafe_package_names() {
        assert!(!is_safe_package("nginx;rm"));
        assert!(!is_safe_package("--help"));
        assert!(is_safe_package("docker.io"));
    }

    #[test]
    fn catalog_is_unique() {
        let apps = catalog();
        let mut names: Vec<&str> = apps.iter().map(|app| app.package).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), apps.len());
        assert!(apps.iter().any(|app| app.package == "nginx"));
    }
}
