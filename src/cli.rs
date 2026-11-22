//! Tiny dependency-free command line parser.

use std::path::{Path, PathBuf};

use crate::theme::ThemeName;

#[derive(Debug, Clone)]
pub struct Cli {
    pub config: PathBuf,
    pub config_explicit: bool,
    pub read_only_explicit: bool,
    pub bin: Option<PathBuf>,
    pub read_only: bool,
    pub theme: Option<ThemeName>,
    pub show_help: bool,
    pub show_version: bool,
}

impl Default for Cli {
    fn default() -> Self {
        Self {
            config: PathBuf::from("/etc/nginx/nginx.conf"),
            config_explicit: false,
            read_only_explicit: false,
            bin: None,
            read_only: false,
            theme: None,
            show_help: false,
            show_version: false,
        }
    }
}

pub fn parse<I, S>(args: I) -> Result<Cli, String>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let mut cli = Cli::default();
    let mut args = args.into_iter().map(Into::into).peekable();
    let _program = args.next();

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => cli.show_help = true,
            "-V" | "--version" => cli.show_version = true,
            "--read-only" | "--readonly" | "-r" => {
                cli.read_only = true;
                cli.read_only_explicit = true;
            }
            "-c" | "--config" => {
                let value = args
                    .next()
                    .ok_or_else(|| "缺少 --config 的路径。".to_string())?;
                cli.config = PathBuf::from(value);
                cli.config_explicit = true;
            }
            "-b" | "--bin" | "--nginx-bin" => {
                let value = args
                    .next()
                    .ok_or_else(|| "缺少 --bin 的路径。".to_string())?;
                cli.bin = Some(PathBuf::from(value));
            }
            "--theme" | "-t" => {
                let value = args
                    .next()
                    .ok_or_else(|| "缺少 --theme 的名称。".to_string())?;
                cli.theme = Some(ThemeName::parse(&value).ok_or_else(|| {
                    format!("未知主题：{value}（可选 midnight/forest/ember/mono）")
                })?);
            }
            _ if arg.starts_with("--config=") => {
                cli.config = PathBuf::from(arg.trim_start_matches("--config="));
                cli.config_explicit = true;
            }
            _ if arg.starts_with("--bin=") => {
                cli.bin = Some(PathBuf::from(arg.trim_start_matches("--bin=")));
            }
            _ if arg.starts_with("--theme=") => {
                let value = arg.trim_start_matches("--theme=");
                cli.theme = Some(ThemeName::parse(value).ok_or_else(|| {
                    format!("未知主题：{value}（可选 midnight/forest/ember/mono）")
                })?);
            }
            _ if arg.starts_with('-') => return Err(format!("未知参数：{arg}")),
            _ => {
                cli.config = PathBuf::from(arg);
                cli.config_explicit = true;
            }
        }
    }

    let preferences = crate::user_config::UserConfig::load();
    if !cli.config_explicit {
        if let Some(path) =
            std::env::var_os("NSERVER_CONFIG").or_else(|| std::env::var_os("NGINX_CONFIG"))
        {
            cli.config = PathBuf::from(path);
        } else if let Some(path) = preferences.nginx_config {
            cli.config = path;
        } else if let Some(path) = detect_config() {
            cli.config = path;
        }
    }
    if !cli.read_only_explicit {
        cli.read_only = preferences.read_only.unwrap_or(false);
    }
    if cli.theme.is_none() {
        cli.theme = preferences.theme.as_deref().and_then(ThemeName::parse);
    }
    Ok(cli)
}

pub fn detect_config() -> Option<PathBuf> {
    let candidates = [
        "/etc/nginx/nginx.conf",
        "/usr/local/nginx/conf/nginx.conf",
        "/opt/homebrew/etc/nginx/nginx.conf",
        "/opt/nginx/conf/nginx.conf",
    ];
    candidates
        .into_iter()
        .map(PathBuf::from)
        .find(|p| p.is_file())
        .or_else(|| {
            let local = Path::new("nginx.conf");
            local.is_file().then(|| local.to_path_buf())
        })
}

pub fn help_text() -> &'static str {
    "nserver — 终端服务器管理面板（Nginx / Docker / 文件 / FTP / SSL / 监控）

用法:
  nserver [选项] [nginx 配置文件]

选项:
  -c, --config <路径>   nginx 主配置（默认自动探测 /etc/nginx/nginx.conf）
  -b, --bin <路径>      nginx 可执行文件（默认自动探测）
      --read-only       只读模式：禁用编辑、启停和写入操作
      --theme <名称>    midnight | forest | ember | mono
  -h, --help            显示帮助
  -V, --version         显示版本

环境变量:
  NSERVER_CONFIG       默认 Nginx 主配置路径（兼容 NGINX_CONFIG）
  NGINX_BIN            默认 nginx 可执行文件
  OPENSSL_BIN          默认 openssl 可执行文件

页面:
  1 首页   2 站点   3 文件   4 Docker   5 监控   6 日志
  7 SSL    8 FTP    9 设置   配置/上游用 Tab 切换

快捷键:
  ? 帮助   q 退出   Tab 页面   Ctrl+B 侧栏   T 主题
  t 校验配置   r 热重载   s 启停   R 重启   Ctrl+R 刷新
  站点页: n 新建网站   m 网站设置   w 改目录   p 改代理   Space 启停
"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_config_and_flags() {
        let cli = parse([
            "nserver",
            "--config",
            "/tmp/nginx.conf",
            "--read-only",
            "--theme",
            "forest",
        ])
        .unwrap();
        assert_eq!(cli.config, PathBuf::from("/tmp/nginx.conf"));
        assert!(cli.read_only);
        assert_eq!(cli.theme, Some(ThemeName::Forest));
    }

    #[test]
    fn positional_config_and_equals_form() {
        let cli = parse(["nserver", "--theme=ember", "/srv/nginx.conf"]).unwrap();
        assert_eq!(cli.config, PathBuf::from("/srv/nginx.conf"));
        assert_eq!(cli.theme, Some(ThemeName::Ember));
    }

    #[test]
    fn explicit_cli_config_is_preserved() {
        let cli = parse(["nserver", "--config", "/tmp/custom-nginx.conf"]).unwrap();
        assert_eq!(cli.config, PathBuf::from("/tmp/custom-nginx.conf"));
        assert!(cli.config_explicit);
    }

    #[test]
    fn rejects_unknown_theme() {
        assert!(parse(["nserver", "--theme", "neon"]).is_err());
    }
}
