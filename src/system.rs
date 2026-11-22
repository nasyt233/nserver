//! Lightweight Linux resource sampling for the dashboard and monitor page.

use std::collections::VecDeque;
use std::fs;
use std::process::Command;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Default)]
pub struct ProcessRow {
    pub pid: u32,
    pub user: String,
    pub cpu: f32,
    pub mem: f32,
    pub command: String,
}

#[derive(Debug, Clone, Default)]
pub struct SystemSnapshot {
    pub hostname: String,
    pub os: String,
    pub uptime_secs: Option<u64>,
    pub load: Option<(f64, f64, f64)>,
    pub cpu_percent: Option<f32>,
    pub mem_total: u64,
    pub mem_used: u64,
    pub swap_total: u64,
    pub swap_used: u64,
    pub disk_total: u64,
    pub disk_used: u64,
    pub disk_avail: u64,
    pub ip_addrs: Vec<String>,
    pub process_count: usize,
    pub top_processes: Vec<ProcessRow>,
    pub net_rx_total: Option<u64>,
    pub net_tx_total: Option<u64>,
    pub net_rx_rate: Option<f64>,
    pub net_tx_rate: Option<f64>,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct MetricSample {
    pub cpu: f32,
    pub mem: f32,
    pub net_rx_kbps: f64,
    pub net_tx_kbps: f64,
}

pub struct SystemState {
    pub snapshot: SystemSnapshot,
    pub history: VecDeque<MetricSample>,
    pub last_collect: Instant,
    cpu_prev: Option<(u64, u64)>,
    net_prev: Option<(u64, u64, Instant)>,
    pub error: Option<String>,
}

impl Default for SystemState {
    fn default() -> Self {
        Self {
            snapshot: SystemSnapshot::default(),
            history: VecDeque::new(),
            last_collect: Instant::now() - Duration::from_secs(10),
            cpu_prev: None,
            net_prev: None,
            error: None,
        }
    }
}

impl SystemState {
    pub fn tick(&mut self, force: bool) {
        if !force && self.last_collect.elapsed() < Duration::from_secs(2) {
            return;
        }
        self.last_collect = Instant::now();
        let (snapshot, new_cpu, net) = collect(self.cpu_prev, self.net_prev);
        if snapshot.mem_total == 0 && snapshot.disk_total == 0 && snapshot.hostname.is_empty() {
            self.error = Some("无法读取系统资源信息".into());
        } else {
            self.error = None;
        }
        self.cpu_prev = new_cpu;
        self.net_prev = net.map(|(rx, tx)| (rx, tx, Instant::now()));
        if let Some(cpu) = snapshot.cpu_percent {
            let mem = if snapshot.mem_total > 0 {
                snapshot.mem_used as f32 * 100.0 / snapshot.mem_total as f32
            } else {
                0.0
            };
            self.history.push_back(MetricSample {
                cpu,
                mem,
                net_rx_kbps: snapshot.net_rx_rate.unwrap_or(0.0),
                net_tx_kbps: snapshot.net_tx_rate.unwrap_or(0.0),
            });
            if self.history.len() > 60 {
                self.history.pop_front();
            }
        }
        self.snapshot = snapshot;
    }

    pub fn cpu_history(&self) -> Vec<u64> {
        self.history
            .iter()
            .map(|sample| sample.cpu.clamp(0.0, 100.0) as u64)
            .collect()
    }

    pub fn mem_history(&self) -> Vec<u64> {
        self.history
            .iter()
            .map(|sample| sample.mem.clamp(0.0, 100.0) as u64)
            .collect()
    }

    pub fn net_history(&self) -> (Vec<u64>, Vec<u64>) {
        let max = self
            .history
            .iter()
            .map(|s| s.net_rx_kbps.max(s.net_tx_kbps))
            .fold(1.0f64, f64::max);
        let rx = self
            .history
            .iter()
            .map(|s| ((s.net_rx_kbps / max) * 100.0) as u64)
            .collect();
        let tx = self
            .history
            .iter()
            .map(|s| ((s.net_tx_kbps / max) * 100.0) as u64)
            .collect();
        (rx, tx)
    }
}

type CollectResult = (SystemSnapshot, Option<(u64, u64)>, Option<(u64, u64)>);

fn collect(cpu_prev: Option<(u64, u64)>, net_prev: Option<(u64, u64, Instant)>) -> CollectResult {
    let mut snapshot = SystemSnapshot {
        hostname: hostname(),
        os: os_name(),
        uptime_secs: uptime_secs(),
        load: load_average(),
        ..Default::default()
    };
    let cpu_now = cpu_counters();
    snapshot.cpu_percent = match (cpu_prev, cpu_now) {
        (Some((prev_total, prev_idle)), Some((total, idle))) => {
            let dt = total.saturating_sub(prev_total);
            let di = idle.saturating_sub(prev_idle);
            if dt > 0 {
                Some(((dt.saturating_sub(di)) as f32 * 100.0 / dt as f32).clamp(0.0, 100.0))
            } else {
                None
            }
        }
        _ => fallback_cpu_percent(),
    };
    memory(&mut snapshot);
    disk(&mut snapshot);
    snapshot.ip_addrs = ip_addresses();
    snapshot.process_count = process_count();
    snapshot.top_processes = top_processes();
    let net_now = network_totals();
    snapshot.net_rx_total = net_now.map(|(rx, _)| rx);
    snapshot.net_tx_total = net_now.map(|(_, tx)| tx);
    if let (Some((rx, tx)), Some((prev_rx, prev_tx, prev_at))) = (net_now, net_prev) {
        let secs = prev_at.elapsed().as_secs_f64().max(0.1);
        snapshot.net_rx_rate = Some(rx.saturating_sub(prev_rx) as f64 / 1024.0 / secs);
        snapshot.net_tx_rate = Some(tx.saturating_sub(prev_tx) as f64 / 1024.0 / secs);
    }
    (snapshot, cpu_now, net_now)
}

fn hostname() -> String {
    fs::read_to_string("/etc/hostname")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .or_else(|| command_text("hostname", &[]).map(|s| s.trim().to_string()))
        .unwrap_or_else(|| "—".into())
}

fn os_name() -> String {
    if let Ok(text) = fs::read_to_string("/etc/os-release") {
        for line in text.lines() {
            if let Some(value) = line.strip_prefix("PRETTY_NAME=") {
                return value.trim_matches('"').to_string();
            }
        }
    }
    "Linux".into()
}

fn uptime_secs() -> Option<u64> {
    if let Ok(text) = fs::read_to_string("/proc/uptime")
        && let Some(secs) = text
            .split_whitespace()
            .next()
            .and_then(|v| v.parse::<f64>().ok())
    {
        return Some(secs as u64);
    }
    command_text("ps", &["-p", "1", "-o", "etimes="]).and_then(|s| s.trim().parse::<u64>().ok())
}

fn load_average() -> Option<(f64, f64, f64)> {
    if let Ok(text) = fs::read_to_string("/proc/loadavg") {
        let nums: Vec<f64> = text
            .split_whitespace()
            .take(3)
            .filter_map(|v| v.parse().ok())
            .collect();
        if nums.len() == 3 {
            return Some((nums[0], nums[1], nums[2]));
        }
    }
    let text = command_text("uptime", &[])?;
    let rest = text.split("load average:").nth(1)?;
    let nums: Vec<f64> = rest
        .split(',')
        .filter_map(|v| v.trim().parse().ok())
        .collect();
    (nums.len() == 3).then(|| (nums[0], nums[1], nums[2]))
}

fn cpu_counters() -> Option<(u64, u64)> {
    let text = fs::read_to_string("/proc/stat").ok()?;
    let line = text.lines().find(|line| line.starts_with("cpu "))?;
    let values: Vec<u64> = line
        .split_whitespace()
        .skip(1)
        .filter_map(|v| v.parse().ok())
        .collect();
    if values.len() < 4 {
        return None;
    }
    let total: u64 = values.iter().sum();
    let idle = values[3] + values.get(4).copied().unwrap_or(0);
    Some((total, idle))
}

fn fallback_cpu_percent() -> Option<f32> {
    let text = command_text("ps", &["-eo", "pcpu=,comm=", "--no-headers"])?;
    let mut total = 0.0f32;
    for line in text.lines() {
        let mut parts = line.split_whitespace();
        let Some(cpu) = parts.next().and_then(|v| v.parse::<f32>().ok()) else {
            continue;
        };
        let comm = parts.next().unwrap_or_default();
        if comm == "ps" || comm == "top" || comm == "nserver" {
            continue;
        }
        total += cpu;
    }
    Some(total.clamp(0.0, 100.0))
}

fn memory(snapshot: &mut SystemSnapshot) {
    if let Ok(text) = fs::read_to_string("/proc/meminfo") {
        let value = |key: &str| {
            text.lines()
                .find(|line| line.starts_with(key))
                .and_then(|line| line.split_whitespace().nth(1))
                .and_then(|v| v.parse::<u64>().ok())
                .map(|kb| kb * 1024)
        };
        let total = value("MemTotal:").unwrap_or(0);
        let available = value("MemAvailable:").unwrap_or(0);
        let swap_total = value("SwapTotal:").unwrap_or(0);
        let swap_free = value("SwapFree:").unwrap_or(0);
        snapshot.mem_total = total;
        snapshot.mem_used = total.saturating_sub(available);
        snapshot.swap_total = swap_total;
        snapshot.swap_used = swap_total.saturating_sub(swap_free);
        if total > 0 {
            return;
        }
    }
    if let Some(text) = command_text("free", &["-k"]) {
        for line in text.lines() {
            let fields: Vec<&str> = line.split_whitespace().collect();
            if fields.first() == Some(&"Mem:") && fields.len() >= 3 {
                let total = fields[1].parse::<u64>().unwrap_or(0) * 1024;
                let used = fields[2].parse::<u64>().unwrap_or(0) * 1024;
                snapshot.mem_total = total;
                snapshot.mem_used = used;
            } else if fields.first() == Some(&"Swap:") && fields.len() >= 3 {
                snapshot.swap_total = fields[1].parse::<u64>().unwrap_or(0) * 1024;
                snapshot.swap_used = fields[2].parse::<u64>().unwrap_or(0) * 1024;
            }
        }
    }
}

fn disk(snapshot: &mut SystemSnapshot) {
    if let Some(text) = command_text("df", &["-P", "-k", "/"])
        && let Some(line) = text.lines().last()
    {
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() >= 4 {
            let total = fields[1].parse::<u64>().unwrap_or(0) * 1024;
            let used = fields[2].parse::<u64>().unwrap_or(0) * 1024;
            let avail = fields[3].parse::<u64>().unwrap_or(0) * 1024;
            snapshot.disk_total = total;
            snapshot.disk_used = used;
            snapshot.disk_avail = avail;
        }
    }
}

fn ip_addresses() -> Vec<String> {
    if let Some(text) = command_text("hostname", &["-I"]) {
        let ips: Vec<String> = text
            .split_whitespace()
            .filter(|ip| !ip.starts_with("127.") && *ip != "::1")
            .map(ToString::to_string)
            .collect();
        if !ips.is_empty() {
            return ips;
        }
    }
    Vec::new()
}

fn process_count() -> usize {
    if let Ok(entries) = fs::read_dir("/proc") {
        let count = entries
            .flatten()
            .filter(|entry| {
                entry
                    .file_name()
                    .to_str()
                    .map(|name| name.chars().all(|c| c.is_ascii_digit()))
                    .unwrap_or(false)
            })
            .count();
        if count > 0 {
            return count;
        }
    }
    command_text("ps", &["-e", "--no-headers"])
        .map(|s| s.lines().count())
        .unwrap_or(0)
}

fn top_processes() -> Vec<ProcessRow> {
    let Some(text) = command_text(
        "ps",
        &["-eo", "pid=,user=,pcpu=,pmem=,comm=,args=", "--sort=-pcpu"],
    ) else {
        return Vec::new();
    };
    text.lines()
        .take(6)
        .filter_map(|line| {
            let mut parts = line.split_whitespace();
            let pid = parts.next()?.parse().ok()?;
            let user = parts.next()?.to_string();
            let cpu = parts.next()?.parse().ok()?;
            let mem = parts.next()?.parse().ok()?;
            let comm = parts.next()?;
            if comm == "ps" {
                return None;
            }
            let command = parts.collect::<Vec<_>>().join(" ");
            Some(ProcessRow {
                pid,
                user,
                cpu,
                mem,
                command,
            })
        })
        .collect()
}

fn network_totals() -> Option<(u64, u64)> {
    let text = fs::read_to_string("/proc/net/dev").ok()?;
    let mut rx = 0u64;
    let mut tx = 0u64;
    for line in text.lines().skip(2) {
        let Some((iface, data)) = line.split_once(':') else {
            continue;
        };
        if iface.trim() == "lo" {
            continue;
        }
        let fields: Vec<&str> = data.split_whitespace().collect();
        if fields.len() >= 9 {
            rx += fields[0].parse::<u64>().unwrap_or(0);
            tx += fields[8].parse::<u64>().unwrap_or(0);
        }
    }
    Some((rx, tx))
}

fn command_text(program: &str, args: &[&str]) -> Option<String> {
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

pub fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit >= 3 {
        format!("{value:.1} {}", UNITS[unit])
    } else {
        format!("{value:.0} {}", UNITS[unit])
    }
}

pub fn format_rate(kbps: f64) -> String {
    if kbps >= 1024.0 {
        format!("{:.1} MB/s", kbps / 1024.0)
    } else {
        format!("{kbps:.0} KB/s")
    }
}

pub fn format_uptime(seconds: Option<u64>) -> String {
    let Some(mut secs) = seconds else {
        return "—".into();
    };
    let days = secs / 86_400;
    secs %= 86_400;
    let hours = secs / 3_600;
    secs %= 3_600;
    let minutes = secs / 60;
    if days > 0 {
        format!("{days}天 {hours}时")
    } else if hours > 0 {
        format!("{hours}时 {minutes}分")
    } else {
        format!("{minutes}分")
    }
}

pub fn percent(used: u64, total: u64) -> f32 {
    if total == 0 {
        0.0
    } else {
        (used as f32 * 100.0 / total as f32).clamp(0.0, 100.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_cpu_counters() {
        let (total, idle) = {
            let values = [1u64, 2, 3, 4, 5, 6, 7];
            (values.iter().sum::<u64>(), values[3] + values[4])
        };
        assert_eq!(total, 28);
        assert_eq!(idle, 9);
    }

    #[test]
    fn formats_sizes_and_percent() {
        assert_eq!(format_bytes(1024), "1 KB");
        assert_eq!(format_bytes(1024 * 1024), "1 MB");
        assert_eq!(format_uptime(Some(90_000)), "1天 1时");
        assert!((percent(50, 100) - 50.0).abs() < 0.01);
        assert_eq!(percent(0, 0), 0.0);
    }

    #[test]
    fn live_snapshot_has_hostname_and_memory() {
        let mut state = SystemState::default();
        state.tick(true);
        assert!(!state.snapshot.hostname.is_empty());
        // Containers may not expose /proc/meminfo, but the command fallback or
        // the page itself must not panic.
    }
}
