//! Curated junk scanner and cleaner for safe, well-known caches.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

#[derive(Debug, Clone)]
pub struct CleanItem {
    pub id: &'static str,
    pub label: &'static str,
    pub description: &'static str,
    pub bytes: u64,
    pub selected: bool,
    pub available: bool,
}

#[derive(Debug, Clone)]
pub struct CleanerState {
    pub items: Vec<CleanItem>,
    pub selected: usize,
    pub last_scan: Instant,
    pub error: Option<String>,
}

impl Default for CleanerState {
    fn default() -> Self {
        Self {
            items: Vec::new(),
            selected: 0,
            last_scan: Instant::now() - std::time::Duration::from_secs(10),
            error: None,
        }
    }
}

impl CleanerState {
    pub fn refresh(&mut self) {
        self.error = None;
        self.items = scan();
        self.selected = self.selected.min(self.items.len().saturating_sub(1));
        self.last_scan = Instant::now();
    }

    pub fn selected(&self) -> Option<&CleanItem> {
        self.items.get(self.selected)
    }

    pub fn toggle_selected(&mut self) {
        if let Some(item) = self.items.get_mut(self.selected)
            && item.available
        {
            item.selected = !item.selected;
        }
    }

    pub fn select_all(&mut self, selected: bool) {
        for item in &mut self.items {
            if item.available {
                item.selected = selected;
            }
        }
    }

    pub fn selected_bytes(&self) -> u64 {
        self.items
            .iter()
            .filter(|item| item.selected && item.available)
            .map(|item| item.bytes)
            .sum()
    }

    pub fn selected_ids(&self) -> Vec<String> {
        self.items
            .iter()
            .filter(|item| item.selected && item.available)
            .map(|item| item.id.to_string())
            .collect()
    }
}

fn scan() -> Vec<CleanItem> {
    let mut items = Vec::new();
    let trash = crate::files::trash_dir();
    items.push(CleanItem {
        id: "nserver_trash",
        label: "NSERVER 回收站",
        description: "文件管理删除的文件都暂存在这里，清理后不可恢复。",
        bytes: dir_size(&trash),
        selected: false,
        available: trash.exists(),
    });
    let apt = PathBuf::from("/var/cache/apt/archives");
    items.push(CleanItem {
        id: "apt_cache",
        label: "APT 下载缓存",
        description: "已下载的 .deb 软件包缓存，清理不影响已安装程序。",
        bytes: dir_size(&apt),
        selected: false,
        available: apt.is_dir(),
    });
    let journal = command_output("journalctl", &["--disk-usage"])
        .and_then(|text| parse_journal_size(&text))
        .unwrap_or(0);
    items.push(CleanItem {
        id: "journal",
        label: "systemd 日志",
        description: "保留最近 7 天日志，清理旧 journal。",
        bytes: journal,
        selected: false,
        available: journal > 0,
    });
    let npm = home_dir().join(".npm/_cacache");
    items.push(CleanItem {
        id: "npm_cache",
        label: "npm 缓存",
        description: "npm 包下载缓存，可通过 npm cache clean 重建。",
        bytes: dir_size(&npm),
        selected: false,
        available: npm.is_dir(),
    });
    let pip = home_dir().join(".cache/pip");
    items.push(CleanItem {
        id: "pip_cache",
        label: "pip 缓存",
        description: "Python 包下载缓存，可通过 pip cache purge 重建。",
        bytes: dir_size(&pip),
        selected: false,
        available: pip.is_dir(),
    });
    items
}

pub fn clean(ids: &[String]) -> Result<String, String> {
    if ids.is_empty() {
        return Err("没有选中任何清理项。".into());
    }
    let mut report = String::new();
    for id in ids {
        let result = match id.as_str() {
            "nserver_trash" => clean_trash(),
            "apt_cache" => command_result("apt-get", &["clean"], "APT 缓存清理"),
            "journal" => command_result("journalctl", &["--vacuum-time=7d"], "systemd 日志清理"),
            "npm_cache" => command_result("npm", &["cache", "clean", "--force"], "npm 缓存清理"),
            "pip_cache" => command_result("pip", &["cache", "purge"], "pip 缓存清理"),
            other => Err(format!("未知清理项：{other}")),
        };
        match result {
            Ok(text) => report.push_str(&format!("✓ {id}\n{text}\n\n")),
            Err(err) => report.push_str(&format!("× {id}\n{err}\n\n")),
        }
    }
    Ok(report)
}

fn clean_trash() -> Result<String, String> {
    let trash = crate::files::trash_dir();
    if !trash.exists() {
        return Ok("回收站已经是空的。".into());
    }
    for entry in fs::read_dir(&trash).map_err(|err| format!("读取回收站失败：{err}"))? {
        let path = entry
            .map_err(|err| format!("读取回收站项失败：{err}"))?
            .path();
        if path.is_dir() {
            fs::remove_dir_all(&path)
                .map_err(|err| format!("删除 {} 失败：{err}", path.display()))?;
        } else {
            fs::remove_file(&path).map_err(|err| format!("删除 {} 失败：{err}", path.display()))?;
        }
    }
    Ok("回收站已清空。".into())
}

fn command_result(program: &str, args: &[&str], label: &str) -> Result<String, String> {
    let output = Command::new(program)
        .env("LC_ALL", "C")
        .args(args)
        .output()
        .map_err(|err| format!("{label}无法执行：{err}"))?;
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    if output.status.success() {
        Ok(if stdout.trim().is_empty() {
            format!("{label}完成。")
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

fn command_output(program: &str, args: &[&str]) -> Option<String> {
    let output = Command::new(program)
        .env("LC_ALL", "C")
        .args(args)
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).to_string())
}

fn parse_journal_size(text: &str) -> Option<u64> {
    let token = text
        .split("take up")
        .nth(1)?
        .split_whitespace()
        .next()?
        .trim_end_matches('.');
    let number: f64 = token
        .trim_end_matches(|ch: char| ch.is_ascii_alphabetic())
        .parse()
        .ok()?;
    let unit = token
        .chars()
        .last()
        .filter(|ch| ch.is_ascii_alphabetic())
        .unwrap_or('B');
    let multiplier = match unit.to_ascii_uppercase() {
        'K' => 1024u64,
        'M' => 1024 * 1024,
        'G' => 1024 * 1024 * 1024,
        'T' => 1024u64 * 1024 * 1024 * 1024,
        _ => 1,
    };
    Some((number * multiplier as f64) as u64)
}

fn dir_size(path: &Path) -> u64 {
    let Ok(meta) = fs::symlink_metadata(path) else {
        return 0;
    };
    if meta.is_file() {
        return meta.len();
    }
    if !meta.is_dir() {
        return 0;
    }
    let mut total = 0;
    let mut count = 0;
    let mut stack = vec![path.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            count += 1;
            if count > 100_000 {
                return total;
            }
            let Ok(meta) = fs::symlink_metadata(entry.path()) else {
                continue;
            };
            if meta.is_dir() {
                stack.push(entry.path());
            } else if meta.is_file() {
                total += meta.len();
            }
        }
    }
    total
}

fn home_dir() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/root"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_journal_size() {
        assert_eq!(
            parse_journal_size("Archived and active journals take up 123.4M in the file system."),
            Some(129_394_278)
        );
    }

    #[test]
    fn empty_ids_are_rejected() {
        assert!(clean(&[]).is_err());
    }
}
