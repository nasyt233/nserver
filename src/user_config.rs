//! Persistent per-user NSERVER preferences stored in ~/.nserver/config.toml.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserConfig {
    pub path: PathBuf,
    pub nginx_config: Option<PathBuf>,
    pub theme: Option<String>,
    pub read_only: Option<bool>,
}

impl UserConfig {
    pub fn load() -> Self {
        let path = config_path();
        let mut config = Self {
            path,
            nginx_config: None,
            theme: None,
            read_only: None,
        };
        let Ok(text) = fs::read_to_string(&config.path) else {
            return config;
        };
        parse_preferences(&text, &mut config);
        config
    }

    pub fn save(&self) -> Result<(), String> {
        let parent = self
            .path
            .parent()
            .ok_or_else(|| "配置文件没有父目录".to_string())?;
        fs::create_dir_all(parent).map_err(|e| format!("创建 ~/.nserver 失败：{e}"))?;
        let mut text = String::from("# NSERVER user preferences\n");
        if let Some(path) = &self.nginx_config {
            text.push_str(&format!(
                "nginx_config = {}\n",
                quote(path.to_string_lossy().as_ref())
            ));
        }
        if let Some(theme) = &self.theme {
            text.push_str(&format!("theme = {}\n", quote(theme)));
        }
        if let Some(read_only) = self.read_only {
            text.push_str(&format!("read_only = {read_only}\n"));
        }
        atomic_write(&self.path, text.as_bytes()).map_err(|e| format!("保存配置失败：{e}"))
    }
}

fn parse_preferences(text: &str, config: &mut UserConfig) {
    for line in text.lines() {
        let Some((key, raw)) = line.split_once('=') else {
            continue;
        };
        let value = raw.trim().trim_matches('"');
        match key.trim() {
            "nginx_config" if !value.is_empty() => {
                config.nginx_config = Some(PathBuf::from(value));
            }
            "theme" if !value.is_empty() => config.theme = Some(value.to_string()),
            "read_only" => config.read_only = value.parse().ok(),
            _ => {}
        }
    }
}

fn config_path() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".nserver")
        .join("config.toml")
}

fn quote(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

fn atomic_write(path: &Path, contents: &[u8]) -> std::io::Result<()> {
    let temp = path.with_extension("toml.tmp");
    {
        let mut file = fs::File::create(&temp)?;
        file.write_all(contents)?;
        file.sync_all()?;
    }
    fs::rename(temp, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_path_is_in_nserver_home() {
        let path = config_path();
        assert!(path.ends_with(".nserver/config.toml"));
    }

    #[test]
    fn quote_escapes_strings() {
        assert_eq!(quote("a\\b\"c"), "\"a\\\\b\\\"c\"");
    }

    #[test]
    fn loads_supported_preferences() {
        let mut config = UserConfig {
            path: PathBuf::from("/tmp/nserver-test/config.toml"),
            nginx_config: None,
            theme: None,
            read_only: None,
        };
        parse_preferences(
            "nginx_config = \"/etc/nginx/nginx.conf\"\ntheme = \"forest\"\nread_only = true\n",
            &mut config,
        );
        assert_eq!(
            config.nginx_config,
            Some(PathBuf::from("/etc/nginx/nginx.conf"))
        );
        assert_eq!(config.theme.as_deref(), Some("forest"));
        assert_eq!(config.read_only, Some(true));
    }
}
