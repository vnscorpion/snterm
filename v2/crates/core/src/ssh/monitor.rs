//! Thu thập thông tin máy chủ (OS, uptime, RAM, đĩa, mạng, CPU) — lệnh `MON|...` giống v1
//! (`TerminalTabViewModel.StartServerMonitoring`), chỉ khác phần `df` được mã hóa base64.
use serde::Serialize;
use std::time::Instant;

pub const MONITOR_COMMAND: &str = r#"export LC_ALL=C; hn=$(uname -n 2>/dev/null || hostname); usr=$(whoami 2>/dev/null || echo root); os=''; [ -f /etc/cloudlinux-release ] && os=cloudlinux; [ -z "$os" ] && [ -f /etc/redhat-release ] && os=$(cat /etc/redhat-release 2>/dev/null); [ -z "$os" ] && os=$(grep -E '^(ID|ID_LIKE)=' /etc/os-release 2>/dev/null | cut -d= -f2- | tr -d '"' | tr '\n' ' '); [ -z "$os" ] && os=$(uname -s); up=$(awk '{print int($1)}' /proc/uptime 2>/dev/null || echo 0); mem=$(awk '/MemTotal/{t=$2} /MemAvailable/{a=$2} END{if(t>0) printf "%d %d", (t-a)/1024, t/1024}' /proc/meminfo 2>/dev/null); disks=$(df -hP 2>/dev/null | awk 'NR>1 && $1 !~ "^(tmpfs|devtmpfs|udev|overlay|shm|none|cgroup)" { if ($6 !~ "^(/dev|/run|/proc|/sys|/snap|/var/lib/docker|/boot/efi)") printf "%s: %s  ", $6, $5 }' | sed 's/  $//'); [ -z "$disks" ] && disks=$(df -h / 2>/dev/null | awk 'NR==2{print "/: " $5}'); net=$(awk '$1 !~ /lo:|Face/{rx+=$2; tx+=$10} END{printf "%d %d", rx, tx}' /proc/net/dev 2>/dev/null); cpustat=$(awk '/^cpu /{print $2+$3+$4, $2+$3+$4+$5}' /proc/stat 2>/dev/null); osp=$(grep -E '^PRETTY_NAME=' /etc/os-release 2>/dev/null | head -1 | cut -d= -f2- | tr -d '"'); [ -z "$osp" ] && [ -f /etc/cloudlinux-release ] && osp=$(cat /etc/cloudlinux-release 2>/dev/null); [ -z "$osp" ] && [ -f /etc/redhat-release ] && osp=$(cat /etc/redhat-release 2>/dev/null); [ -z "$osp" ] && [ -f /etc/issue ] && osp=$(head -1 /etc/issue 2>/dev/null | sed 's/\\.*//'); [ -z "$osp" ] && osp="$os"; krn=$(uname -r 2>/dev/null); arch=$(uname -m 2>/dev/null); dfh=$( (df -hP -x tmpfs -x devtmpfs -x overlay -x squashfs 2>/dev/null || df -hP 2>/dev/null || df -h 2>/dev/null) | base64 2>/dev/null | tr -d '\n'); echo "MON|$hn|$usr|$os|$up|$mem|$disks|$net|$cpustat|$osp|$krn|$arch|$dfh""#;

/// Mẫu thô đọc từ một lần chạy lệnh.
#[derive(Debug, Clone, Default)]
pub struct MonitorSample {
    pub hostname: String,
    pub username: String,
    pub os_name: String,
    pub uptime_sec: i64,
    pub used_mb: i64,
    pub total_mb: i64,
    pub disk_usage: String,
    pub rx_bytes: i64,
    pub tx_bytes: i64,
    pub busy_jiffies: i64,
    pub total_jiffies: i64,
    pub os_pretty: String,
    pub kernel: String,
    pub arch: String,
    pub df_output: String,
}

pub fn parse_sample(raw: &str) -> Option<MonitorSample> {
    let line = raw.lines().find(|l| l.starts_with("MON|"))?;
    let parts: Vec<&str> = line.splitn(13, '|').collect();
    if parts.len() < 9 {
        return None;
    }
    let num = |s: &str| s.trim().parse::<i64>().unwrap_or(0);
    let pair = |s: &str| {
        let mut it = s.trim().split(' ');
        (num(it.next().unwrap_or("0")), num(it.next().unwrap_or("0")))
    };
    let (used_mb, total_mb) = pair(parts[5]);
    let (rx, tx) = pair(parts[7]);
    let (busy, total) = pair(parts[8]);
    let raw_df = parts.get(12).map(|s| s.trim()).unwrap_or("");
    let df_output = if raw_df.is_empty() {
        String::new()
    } else {
        use base64::Engine;
        match base64::engine::general_purpose::STANDARD.decode(raw_df) {
            Ok(b) => String::from_utf8_lossy(&b).trim_end().to_string(),
            Err(_) => raw_df.replace('\u{FFFD}', "\n").replace('¶', "\n").replace('\r', "").trim_end().to_string(),
        }
    };
    Some(MonitorSample {
        hostname: parts[1].trim().into(),
        username: parts[2].trim().into(),
        os_name: parts[3].trim().into(),
        uptime_sec: num(parts[4]),
        used_mb,
        total_mb,
        disk_usage: parts[6].trim().into(),
        rx_bytes: rx,
        tx_bytes: tx,
        busy_jiffies: busy,
        total_jiffies: total,
        os_pretty: parts.get(9).map(|s| s.trim()).unwrap_or("").into(),
        kernel: parts.get(10).map(|s| s.trim()).unwrap_or("").into(),
        arch: parts.get(11).map(|s| s.trim()).unwrap_or("").into(),
        df_output,
    })
}

/// Nhóm icon hệ điều hành (giống `DetectOsGroup` v1).
pub fn detect_os_group(os_name: &str, os_pretty: &str) -> &'static str {
    let t = format!("{os_name} {os_pretty}").to_lowercase();
    let any = |keys: &[&str]| keys.iter().any(|k| t.contains(k));
    if any(&["cloudlinux", "cloud", "alma", "rocky", "centos", "redhat", "rhel", "fedora", "oracle", "amzn"]) {
        "redhat"
    } else if any(&["ubuntu", "mint", "pop"]) {
        "ubuntu"
    } else if any(&["debian", "kali", "raspbian"]) {
        "debian"
    } else if any(&["alpine"]) {
        "alpine"
    } else if any(&["arch", "manjaro", "endeavour"]) {
        "arch"
    } else if any(&["suse", "opensuse"]) {
        "suse"
    } else {
        "linux"
    }
}

/// Thông tin đã tính toán, sẵn sàng hiển thị trên thanh monitor.
#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MonitorInfo {
    pub os_group: String,
    pub hostname: String,
    pub cpu_percent: i32,
    pub ram_percent: i32,
    pub ram_text: String,
    pub upload_text: String,
    pub download_text: String,
    pub uptime_text: String,
    pub username: String,
    pub disk_text: String,
    pub os_pretty: String,
    pub kernel: String,
    pub arch: String,
    pub df_output: String,
}

/// Giữ mẫu trước để tính CPU % và tốc độ mạng giữa hai lần đo.
#[derive(Default)]
pub struct MonitorTracker {
    last_busy: i64,
    last_total: i64,
    last_rx: i64,
    last_tx: i64,
    last_time: Option<Instant>,
}

impl MonitorTracker {
    pub fn update(&mut self, s: &MonitorSample, fallback_host: &str, fallback_user: &str) -> MonitorInfo {
        let mut cpu = 0i32;
        if self.last_total > 0 && s.total_jiffies > self.last_total {
            let d_busy = s.busy_jiffies - self.last_busy;
            let d_total = s.total_jiffies - self.last_total;
            if d_total > 0 {
                cpu = ((d_busy * 100) / d_total).clamp(0, 100) as i32;
            }
        }
        self.last_busy = s.busy_jiffies;
        self.last_total = s.total_jiffies;

        let now = Instant::now();
        let elapsed = self.last_time.map(|t| now.duration_since(t).as_secs_f64()).unwrap_or(0.0);
        self.last_time = Some(now);
        let (mut up_mbps, mut down_mbps) = (0.0f64, 0.0f64);
        if self.last_tx > 0 && self.last_rx > 0 && elapsed > 0.5 {
            let d_tx = (s.tx_bytes - self.last_tx).max(0) as f64;
            let d_rx = (s.rx_bytes - self.last_rx).max(0) as f64;
            up_mbps = d_tx * 8.0 / (elapsed * 1_000_000.0);
            down_mbps = d_rx * 8.0 / (elapsed * 1_000_000.0);
        }
        self.last_tx = s.tx_bytes;
        self.last_rx = s.rx_bytes;

        let ram_text = if s.total_mb > 1024 {
            format!("{:.2} GB / {:.2} GB", s.used_mb as f64 / 1024.0, s.total_mb as f64 / 1024.0)
        } else {
            format!("{} MB / {} MB", s.used_mb, s.total_mb)
        };
        let speed = |mbps: f64| {
            if mbps < 0.1 { format!("{:.1} KB/s", mbps * 1000.0 / 8.0) } else { format!("{mbps:.2} Mb/s") }
        };
        let uptime_text = if s.uptime_sec >= 86400 {
            format!("{} days", s.uptime_sec / 86400)
        } else if s.uptime_sec >= 3600 {
            format!("{}h {}m", s.uptime_sec / 3600, (s.uptime_sec % 3600) / 60)
        } else {
            format!("{}m", s.uptime_sec / 60)
        };
        let disk_text = if s.disk_usage.trim().is_empty() {
            "/: --".to_string()
        } else if s.disk_usage.starts_with('/') {
            s.disk_usage.clone()
        } else {
            format!("/: {}", s.disk_usage)
        };
        let ram_percent = if s.total_mb > 0 {
            ((s.used_mb as f64 / s.total_mb as f64 * 100.0).round() as i32).clamp(0, 100)
        } else {
            0
        };
        MonitorInfo {
            os_group: detect_os_group(&s.os_name, &s.os_pretty).to_string(),
            hostname: if s.hostname.is_empty() { fallback_host.to_string() } else { s.hostname.clone() },
            cpu_percent: cpu,
            ram_percent,
            ram_text,
            upload_text: speed(up_mbps),
            download_text: speed(down_mbps),
            uptime_text,
            username: if s.username.is_empty() { fallback_user.to_string() } else { s.username.clone() },
            disk_text,
            os_pretty: s.os_pretty.clone(),
            kernel: s.kernel.clone(),
            arch: s.arch.clone(),
            df_output: s.df_output.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_and_tracks() {
        let raw = "MON|web-01|root|ubuntu|90000|512 2048|/: 45%|1000 2000|100 200|Ubuntu 22.04 LTS|5.15.0|x86_64|\n";
        let s = parse_sample(raw).unwrap();
        assert_eq!(s.hostname, "web-01");
        assert_eq!(s.total_mb, 2048);
        let mut t = MonitorTracker::default();
        let i = t.update(&s, "h", "u");
        assert_eq!(i.os_group, "ubuntu");
        assert_eq!(i.uptime_text, "1 days");
        assert_eq!(i.ram_text, "0.50 GB / 2.00 GB");
        assert_eq!(i.ram_percent, 25);
        assert_eq!(detect_os_group("rocky", ""), "redhat");
        assert_eq!(detect_os_group("", "Alpine Linux"), "alpine");
        assert_eq!(detect_os_group("foo", ""), "linux");
        let s2 = MonitorSample { busy_jiffies: 150, total_jiffies: 300, ..s.clone() };
        let i2 = t.update(&s2, "h", "u");
        assert_eq!(i2.cpu_percent, 50);
    }
}
