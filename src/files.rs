//! A small, safe-ish file browser for the panel.

use std::fs;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use chrono::{DateTime, Local};

#[derive(Debug, Clone)]
pub struct FileEntry {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
    pub is_symlink: bool,
    pub size: u64,
    pub mode: String,
    pub uid: u32,
    pub modified: Option<SystemTime>,
}

impl FileEntry {
    pub fn modified_label(&self) -> String {
        self.modified
            .map(|time| {
                let dt: DateTime<Local> = time.into();
                dt.format("%m-%d %H:%M").to_string()
            })
            .unwrap_or_else(|| "—".into())
    }

    pub fn size_label(&self) -> String {
        if self.is_dir {
            "—".into()
        } else {
            crate::system::format_bytes(self.size)
        }
    }
}

#[derive(Debug, Clone)]
pub struct FileBrowser {
    pub cwd: PathBuf,
    pub entries: Vec<FileEntry>,
    pub selected: usize,
    pub error: Option<String>,
    pub show_hidden: bool,
}

impl Default for FileBrowser {
    fn default() -> Self {
        Self::new(start_dir())
    }
}

impl FileBrowser {
    pub fn new(path: PathBuf) -> Self {
        let mut browser = Self {
            cwd: path,
            entries: Vec::new(),
            selected: 0,
            error: None,
            show_hidden: false,
        };
        browser.refresh();
        browser
    }

    pub fn refresh(&mut self) {
        self.error = None;
        match load_dir(&self.cwd, self.show_hidden) {
            Ok(entries) => {
                self.entries = entries;
                self.selected = self.selected.min(self.entries.len().saturating_sub(1));
            }
            Err(err) => {
                self.entries.clear();
                self.error = Some(err);
            }
        }
    }

    pub fn selected(&self) -> Option<&FileEntry> {
        self.entries.get(self.selected)
    }

    pub fn navigate(&mut self, path: &Path) {
        let target = if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.cwd.join(path)
        };
        if target.is_dir() {
            self.cwd = fs::canonicalize(&target).unwrap_or(target);
            self.selected = 0;
            self.refresh();
        } else {
            self.error = Some(format!("不是目录：{}", target.display()));
        }
    }

    pub fn go_up(&mut self) {
        if let Some(parent) = self.cwd.parent() {
            self.cwd = parent.to_path_buf();
            self.selected = 0;
            self.refresh();
        }
    }

    pub fn toggle_hidden(&mut self) {
        self.show_hidden = !self.show_hidden;
        self.refresh();
    }

    pub fn create_dir(&mut self, name: &str) -> Result<PathBuf, String> {
        validate_name(name)?;
        let path = self.cwd.join(name);
        fs::create_dir(&path).map_err(|e| format!("创建目录失败：{e}"))?;
        self.refresh();
        Ok(path)
    }

    pub fn create_file(&mut self, name: &str) -> Result<PathBuf, String> {
        validate_name(name)?;
        let path = self.cwd.join(name);
        fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|e| format!("创建文件失败：{e}"))?;
        self.refresh();
        Ok(path)
    }

    pub fn rename_selected(&mut self, new_name: &str) -> Result<PathBuf, String> {
        validate_name(new_name)?;
        let Some(entry) = self.selected().cloned() else {
            return Err("没有选中文件。".into());
        };
        let target = entry.path.parent().unwrap_or(&self.cwd).join(new_name);
        if target.exists() {
            return Err(format!("{} 已存在。", target.display()));
        }
        fs::rename(&entry.path, &target).map_err(|e| format!("重命名失败：{e}"))?;
        self.refresh();
        Ok(target)
    }

    pub fn move_selected(&mut self, delta: isize) {
        if self.entries.is_empty() {
            return;
        }
        let len = self.entries.len() as isize;
        self.selected = (self.selected as isize + delta).rem_euclid(len) as usize;
    }
}

pub fn load_dir(path: &Path, show_hidden: bool) -> Result<Vec<FileEntry>, String> {
    let entries = fs::read_dir(path).map_err(|e| format!("读取 {} 失败：{e}", path.display()))?;
    let mut out = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if !show_hidden && name.starts_with('.') {
            continue;
        }
        let path = entry.path();
        let Ok(meta) = fs::symlink_metadata(&path) else {
            continue;
        };
        let is_symlink = meta.file_type().is_symlink();
        let is_dir = if is_symlink {
            fs::metadata(&path).map(|m| m.is_dir()).unwrap_or(false)
        } else {
            meta.is_dir()
        };
        out.push(FileEntry {
            name,
            path,
            is_dir,
            is_symlink,
            size: meta.len(),
            mode: format_mode(meta.permissions().mode()),
            uid: meta.uid(),
            modified: meta.modified().ok(),
        });
    }
    out.sort_by(|a, b| {
        b.is_dir
            .cmp(&a.is_dir)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    Ok(out)
}

pub fn preview(path: &Path) -> Result<(Vec<String>, bool), String> {
    let meta = fs::metadata(path).map_err(|e| format!("读取文件信息失败：{e}"))?;
    if meta.is_dir() {
        return Err("这是一个目录。".into());
    }
    if meta.len() > 512 * 1024 {
        return Err("文件超过 512 KB，不在预览中打开。".into());
    }
    let bytes = fs::read(path).map_err(|e| format!("读取文件失败：{e}"))?;
    match String::from_utf8(bytes) {
        Ok(text) => Ok((
            text.lines().take(400).map(ToString::to_string).collect(),
            false,
        )),
        Err(_) => Ok((vec!["（二进制文件，无法以文本预览）".into()], true)),
    }
}

pub fn delete_path_to_trash(path: &Path) -> Result<PathBuf, String> {
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| "item".into());
    let trash = trash_dir();
    fs::create_dir_all(&trash).map_err(|e| format!("创建回收站失败：{e}"))?;
    let stamp = Local::now().format("%Y%m%d-%H%M%S");
    let target = trash.join(format!("{stamp}-{name}"));
    fs::rename(path, &target).map_err(|e| format!("移入回收站失败：{e}"))?;
    Ok(target)
}

fn format_mode(mode: u32) -> String {
    let mut out = String::new();
    for shift in [6, 3, 0] {
        let bits = (mode >> shift) & 0o7;
        out.push(if bits & 0o4 != 0 { 'r' } else { '-' });
        out.push(if bits & 0o2 != 0 { 'w' } else { '-' });
        out.push(if bits & 0o1 != 0 { 'x' } else { '-' });
    }
    out
}

fn validate_name(name: &str) -> Result<(), String> {
    let name = name.trim();
    if name.is_empty()
        || name == "."
        || name == ".."
        || name.contains('/')
        || name.contains('\\')
        || name.contains('\n')
    {
        return Err("名称不能为空、不能包含路径分隔符，也不能是 . 或 ..".into());
    }
    Ok(())
}

fn start_dir() -> PathBuf {
    for path in ["/var/www", "/srv", "/root"] {
        let path = PathBuf::from(path);
        if path.is_dir() {
            return path;
        }
    }
    PathBuf::from("/")
}

pub fn trash_dir() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/root"))
        .join(".nserver/trash")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_and_creates_entries() {
        let tmp = tempfile::tempdir().unwrap();
        fs::create_dir(tmp.path().join("sub")).unwrap();
        fs::write(tmp.path().join("a.txt"), "hello").unwrap();
        let mut browser = FileBrowser::new(tmp.path().to_path_buf());
        assert_eq!(browser.entries.len(), 2);
        assert!(browser.entries[0].is_dir);
        browser.create_dir("new").unwrap();
        assert!(tmp.path().join("new").is_dir());
    }

    #[test]
    fn mode_formatter() {
        assert_eq!(format_mode(0o644), "rw-r--r--");
        assert_eq!(format_mode(0o755), "rwxr-xr-x");
    }

    #[test]
    fn validates_names() {
        assert!(validate_name("../x").is_err());
        assert!(validate_name("ok.txt").is_ok());
    }
}
