//! Tail-oriented log reading and lightweight parsing for the log viewer.

use std::fs::{self, File};
use std::io::{self, Read, Seek, SeekFrom};
use std::path::Path;

#[derive(Debug, Clone, Default)]
pub struct LogSnapshot {
    pub lines: Vec<String>,
    pub total_bytes: u64,
}

pub fn read_tail(path: &Path, max_bytes: u64, max_lines: usize) -> io::Result<LogSnapshot> {
    let meta = fs::metadata(path)?;
    let total = meta.len();
    let start = total.saturating_sub(max_bytes);
    let mut file = File::open(path)?;
    if start > 0 {
        file.seek(SeekFrom::Start(start))?;
    }
    let mut buf = Vec::with_capacity((total - start) as usize);
    file.read_to_end(&mut buf)?;
    let text = String::from_utf8_lossy(&buf);
    let mut lines: Vec<String> = text
        .lines()
        .map(|l| l.trim_end_matches('\r').to_string())
        .collect();
    if start > 0 && !lines.is_empty() {
        // The first line is probably a partial record.
        lines.remove(0);
    }
    if lines.len() > max_lines {
        lines.drain(0..lines.len() - max_lines);
    }
    Ok(LogSnapshot {
        lines,
        total_bytes: total,
    })
}

#[derive(Debug, Clone, Copy, Default)]
pub struct TrafficStats {
    pub total: usize,
    pub s2xx: usize,
    pub s3xx: usize,
    pub s4xx: usize,
    pub s5xx: usize,
}

impl TrafficStats {
    pub fn add(&mut self, other: &Self) {
        self.total += other.total;
        self.s2xx += other.s2xx;
        self.s3xx += other.s3xx;
        self.s4xx += other.s4xx;
        self.s5xx += other.s5xx;
    }

    pub fn from_lines(lines: &[String]) -> Self {
        let mut stats = Self::default();
        for line in lines {
            if let Some(code) = access_status(line) {
                stats.total += 1;
                match code / 100 {
                    2 => stats.s2xx += 1,
                    3 => stats.s3xx += 1,
                    4 => stats.s4xx += 1,
                    5 => stats.s5xx += 1,
                    _ => {}
                }
            }
        }
        stats
    }
}

/// Extract the HTTP status from a Common/Combined log line or a JSON log line.
pub fn access_status(line: &str) -> Option<u16> {
    if let Some(pos) = line.find("\"status\":") {
        let rest = line[pos + 9..].trim_start_matches([' ', '"']);
        let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
        if let Ok(code) = digits.parse::<u16>() {
            return Some(code);
        }
    }
    // Find the end of the quoted request, then the next whitespace-separated token.
    let mut in_quotes = false;
    let mut escaped = false;
    let mut request_end = None;
    for (idx, ch) in line.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if ch == '\\' && in_quotes {
            escaped = true;
            continue;
        }
        if ch == '"' {
            if in_quotes {
                request_end = Some(idx + 1);
                break;
            }
            in_quotes = true;
        }
    }
    let rest = &line[request_end?..];
    for token in rest.split_whitespace() {
        if token.len() == 3 && token.chars().all(|c| c.is_ascii_digit()) {
            return token.parse().ok();
        }
        if token.starts_with('"') {
            break;
        }
    }
    None
}

pub const SEVERITIES: [&str; 8] = [
    "emerg", "alert", "crit", "error", "warn", "notice", "info", "debug",
];

pub fn error_severity(line: &str) -> Option<&'static str> {
    for level in SEVERITIES {
        let needle = format!("[{level}]");
        if line.contains(&needle) {
            return Some(level);
        }
    }
    None
}

pub fn is_error_line(line: &str) -> bool {
    matches!(
        error_severity(line),
        Some("emerg" | "alert" | "crit" | "error")
    )
}

pub fn is_warning_line(line: &str) -> bool {
    matches!(error_severity(line), Some("warn"))
}

pub fn count_errors(lines: &[String]) -> (usize, usize) {
    let errors = lines.iter().filter(|l| is_error_line(l)).count();
    let warnings = lines.iter().filter(|l| is_warning_line(l)).count();
    (errors, warnings)
}

pub fn filter_lines<'a>(lines: &'a [String], query: &str) -> Vec<&'a str> {
    let needle = query.trim().to_lowercase();
    if needle.is_empty() {
        return lines.iter().map(String::as_str).collect();
    }
    lines
        .iter()
        .filter(|line| line.to_lowercase().contains(&needle))
        .map(String::as_str)
        .collect()
}

pub fn format_bytes(bytes: u64) -> String {
    if bytes >= 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / 1024.0 / 1024.0)
    } else if bytes >= 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{bytes} B")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn reads_only_the_tail() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("access.log");
        let mut file = File::create(&path).unwrap();
        for i in 0..500 {
            writeln!(file, "line {i} ........................................").unwrap();
        }
        drop(file);
        let snapshot = read_tail(&path, 4096, 20).unwrap();
        assert!(snapshot.lines.len() <= 20);
        assert_eq!(
            snapshot.lines.last().unwrap(),
            "line 499 ........................................"
        );
    }

    #[test]
    fn extracts_status_and_severity() {
        let line =
            r#"1.2.3.4 - - [21/Nov/2026:12:00:00 +0800] "GET /api HTTP/1.1" 503 12 "-" "curl/8""#;
        assert_eq!(access_status(line), Some(503));
        assert_eq!(
            access_status(r#"{"time":"x","status":404,"request":"/x"}"#),
            Some(404)
        );
        assert_eq!(
            error_severity("2026/11/21 [error] 12#0: boom"),
            Some("error")
        );
        assert!(is_error_line("2026/11/21 [crit] 12#0: boom"));
        assert!(!is_error_line("2026/11/21 [warn] 12#0: boom"));
    }

    #[test]
    fn filters_case_insensitively() {
        let lines = vec!["GET /A".to_string(), "POST /b".to_string()];
        assert_eq!(filter_lines(&lines, "get"), vec!["GET /A"]);
        assert_eq!(filter_lines(&lines, "").len(), 2);
    }
}
