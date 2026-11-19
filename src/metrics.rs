use std::{
    collections::{HashMap, VecDeque},
    fs,
    net::{Ipv4Addr, Ipv6Addr, UdpSocket},
    path::Path,
    process::Command,
    time::Instant,
};

use sysinfo::{Disks, Networks, ProcessesToUpdate, System};

#[derive(Clone, Debug)]
pub struct ProcessRow {
    pub pid: u32,
    pub name: String,
    pub cpu: f32,
    pub memory: u64,
    pub status: String,
}

#[derive(Clone, Debug)]
pub struct SocketRow {
    pub protocol: String,
    pub local: String,
    pub remote: String,
    pub state: String,
    pub inode: u64,
    pub pid: Option<u32>,
    pub process: String,
}

#[derive(Clone, Debug, Default)]
pub struct Battery {
    pub percent: Option<u8>,
    pub state: String,
}

#[derive(Clone, Debug, Default)]
pub struct Gpu {
    pub usage: Option<f32>,
    pub used_mib: Option<u64>,
    pub total_mib: Option<u64>,
    pub name: String,
}

pub struct Metrics {
    system: System,
    networks: Networks,
    disks: Disks,
    last_sample: Instant,
    last_sent: u64,
    last_recv: u64,
    pub cpu: f32,
    pub cpu_history: VecDeque<u64>,
    pub net_history: VecDeque<u64>,
    pub mem_total: u64,
    pub mem_used: u64,
    pub mem_available: u64,
    pub swap_total: u64,
    pub swap_used: u64,
    pub disk_total: u64,
    pub disk_used: u64,
    pub disk_free: u64,
    pub disk_read: u64,
    pub disk_write: u64,
    pub net_sent: u64,
    pub net_recv: u64,
    pub net_up_per_sec: f64,
    pub net_down_per_sec: f64,
    pub uptime: u64,
    pub local_ip: String,
    pub battery: Battery,
    pub gpu: Gpu,
    pub processes: Vec<ProcessRow>,
    pub sockets: Vec<SocketRow>,
    pub tick: u64,
}

impl Metrics {
    pub fn new() -> Self {
        let system = System::new_all();
        let networks = Networks::new_with_refreshed_list();
        let disks = Disks::new_with_refreshed_list();
        let (sent, recv) = network_totals(&networks);
        let mut this = Self {
            system,
            networks,
            disks,
            last_sample: Instant::now(),
            last_sent: sent,
            last_recv: recv,
            cpu: 0.0,
            cpu_history: VecDeque::with_capacity(90),
            net_history: VecDeque::with_capacity(90),
            mem_total: 0,
            mem_used: 0,
            mem_available: 0,
            swap_total: 0,
            swap_used: 0,
            disk_total: 0,
            disk_used: 0,
            disk_free: 0,
            disk_read: 0,
            disk_write: 0,
            net_sent: 0,
            net_recv: 0,
            net_up_per_sec: 0.0,
            net_down_per_sec: 0.0,
            uptime: 0,
            local_ip: local_address().unwrap_or_else(|| "unavailable".to_owned()),
            battery: Battery::default(),
            gpu: Gpu::default(),
            processes: Vec::new(),
            sockets: Vec::new(),
            tick: 0,
        };
        this.refresh();
        this
    }

    pub fn refresh(&mut self) {
        self.tick += 1;
        self.system.refresh_cpu_usage();
        self.system.refresh_memory();
        self.cpu = self.system.global_cpu_usage().clamp(0.0, 100.0);
        push_history(&mut self.cpu_history, self.cpu.round() as u64);
        self.mem_total = self.system.total_memory();
        self.mem_used = self.system.used_memory();
        self.mem_available = self.system.available_memory();
        self.swap_total = self.system.total_swap();
        self.swap_used = self.system.used_swap();
        self.uptime = System::uptime();

        self.networks.refresh(true);
        let (sent, recv) = network_totals(&self.networks);
        let elapsed = self.last_sample.elapsed().as_secs_f64();
        if elapsed > 0.1 {
            self.net_up_per_sec = sent.saturating_sub(self.last_sent) as f64 / elapsed;
            self.net_down_per_sec = recv.saturating_sub(self.last_recv) as f64 / elapsed;
        }
        self.net_sent = sent;
        self.net_recv = recv;
        self.last_sent = sent;
        self.last_recv = recv;
        self.last_sample = Instant::now();
        push_history(&mut self.net_history, (self.net_down_per_sec / 1024.0) as u64);

        // Processes are sampled less often than CPU/network to keep redraws cheap.
        if self.tick == 1 || self.tick % 2 == 0 {
            self.system.refresh_processes(ProcessesToUpdate::All, true);
            self.processes = self.system.processes().iter().map(|(pid, process)| {
                ProcessRow {
                    pid: pid.as_u32(),
                    name: process.name().to_string_lossy().into_owned(),
                    cpu: process.cpu_usage(),
                    memory: process.memory(),
                    status: format!("{:?}", process.status()),
                }
            }).collect();
            self.processes.sort_by(|a, b| b.cpu.total_cmp(&a.cpu));
        }

        if self.tick == 1 || self.tick % 5 == 0 {
            self.disks.refresh(true);
            if let Some(root) = self.disks.iter().find(|d| d.mount_point() == Path::new("/")) {
                self.disk_total = root.total_space();
                self.disk_free = root.available_space();
                self.disk_used = self.disk_total.saturating_sub(self.disk_free);
            }
            (self.disk_read, self.disk_write) = disk_io();
            self.battery = read_battery();
            self.gpu = read_gpu();
            self.sockets = read_sockets();
        }
    }

    pub fn memory_percent(&self) -> f64 {
        percent(self.mem_used, self.mem_total)
    }

    pub fn swap_percent(&self) -> f64 {
        percent(self.swap_used, self.swap_total)
    }

    pub fn disk_percent(&self) -> f64 {
        percent(self.disk_used, self.disk_total)
    }

    pub fn listening_ports(&self) -> usize {
        self.sockets.iter().filter(|s| s.state == "LISTEN").count()
    }

    pub fn alerts(&self) -> Vec<String> {
        let mut alerts = Vec::new();
        if self.disk_total > 0 && self.disk_percent() >= 90.0 {
            alerts.push(format!("Disk at {:.0}%, {} free", self.disk_percent(), format_bytes(self.disk_free)));
        }
        if self.swap_total > 0 && self.swap_percent() >= 75.0 {
            alerts.push(format!("Swap at {:.0}%, memory is tight", self.swap_percent()));
        }
        if self.mem_total > 0 && self.memory_percent() >= 90.0 {
            alerts.push(format!("Memory at {:.0}%", self.memory_percent()));
        }
        if self.cpu >= 90.0 {
            alerts.push(format!("CPU at {:.0}%", self.cpu));
        }
        alerts
    }
}

pub fn percent(used: u64, total: u64) -> f64 {
    if total == 0 { 0.0 } else { (used as f64 / total as f64 * 100.0).clamp(0.0, 100.0) }
}

pub fn format_bytes(bytes: u64) -> String {
    let mut value = bytes as f64;
    for suffix in ["B", "KiB", "MiB", "GiB", "TiB", "PiB"] {
        if value < 1024.0 || suffix == "PiB" {
            return if suffix == "B" { format!("{bytes} B") } else { format!("{value:.1} {suffix}") };
        }
        value /= 1024.0;
    }
    unreachable!()
}

fn push_history(history: &mut VecDeque<u64>, value: u64) {
    if history.len() == 90 { history.pop_front(); }
    history.push_back(value);
}

fn network_totals(networks: &Networks) -> (u64, u64) {
    networks.iter().filter(|(name, _)| name.as_str() != "lo").fold((0, 0), |(sent, recv), (_, iface)| {
        (sent.saturating_add(iface.total_transmitted()), recv.saturating_add(iface.total_received()))
    })
}

fn local_address() -> Option<String> {
    let sock = UdpSocket::bind("0.0.0.0:0").ok()?;
    // UDP connect only chooses an interface; it sends no packet.
    sock.connect("192.0.2.1:80").ok()?;
    Some(sock.local_addr().ok()?.ip().to_string())
}

fn disk_io() -> (u64, u64) {
    let Ok(raw) = fs::read_to_string("/proc/diskstats") else { return (0, 0) };
    let mut read = 0u64;
    let mut write = 0u64;
    for line in raw.lines() {
        let fields: Vec<_> = line.split_whitespace().collect();
        if fields.len() < 10 { continue; }
        let device = fields[2];
        // Count whole physical disks, not their partitions or virtual mirrors.
        if !Path::new("/sys/block").join(device).exists() ||
            device.starts_with("loop") || device.starts_with("ram") ||
            device.starts_with("dm-") || device.starts_with("zram") { continue; }
        let sectors_read = fields[5].parse::<u64>().unwrap_or(0);
        let sectors_written = fields[9].parse::<u64>().unwrap_or(0);
        read = read.saturating_add(sectors_read.saturating_mul(512));
        write = write.saturating_add(sectors_written.saturating_mul(512));
    }
    (read, write)
}

fn read_battery() -> Battery {
    let Ok(dir) = fs::read_dir("/sys/class/power_supply") else { return Battery::default() };
    for entry in dir.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if !name.starts_with("BAT") { continue; }
        let path = entry.path();
        let percent = fs::read_to_string(path.join("capacity")).ok()
            .and_then(|n| n.trim().parse::<u8>().ok());
        let state = fs::read_to_string(path.join("status"))
            .map(|n| n.trim().to_owned()).unwrap_or_default();
        return Battery { percent, state };
    }
    Battery::default()
}

fn read_gpu() -> Gpu {
    let out = Command::new("nvidia-smi")
        .args(["--query-gpu=utilization.gpu,memory.used,memory.total,name", "--format=csv,noheader,nounits"])
        .output();
    let Ok(out) = out else { return Gpu::default() };
    if !out.status.success() { return Gpu::default(); }
    let line = String::from_utf8_lossy(&out.stdout);
    let fields: Vec<_> = line.lines().next().unwrap_or("").split(',').map(str::trim).collect();
    if fields.len() < 4 { return Gpu::default(); }
    Gpu {
        usage: fields[0].parse().ok(),
        used_mib: fields[1].parse().ok(),
        total_mib: fields[2].parse().ok(),
        name: fields[3].to_owned(),
    }
}

fn read_sockets() -> Vec<SocketRow> {
    let mut sockets = Vec::new();
    for (file, protocol) in [("/proc/net/tcp", "TCP"), ("/proc/net/tcp6", "TCP6"), ("/proc/net/udp", "UDP"), ("/proc/net/udp6", "UDP6")] {
        if let Ok(raw) = fs::read_to_string(file) {
            sockets.extend(parse_proc_net(&raw, protocol));
        }
    }
    let owners = socket_owners();
    for socket in &mut sockets {
        if let Some((pid, name)) = owners.get(&socket.inode) {
            socket.pid = Some(*pid);
            socket.process = name.clone();
        }
    }
    sockets.sort_by(|a, b| a.local.cmp(&b.local));
    sockets
}

fn socket_owners() -> HashMap<u64, (u32, String)> {
    let mut owners = HashMap::new();
    let Ok(entries) = fs::read_dir("/proc") else { return owners };
    for entry in entries.flatten() {
        let Ok(pid) = entry.file_name().to_string_lossy().parse::<u32>() else { continue };
        let path = entry.path();
        let name = fs::read_to_string(path.join("comm")).unwrap_or_default().trim().to_owned();
        let Ok(fds) = fs::read_dir(path.join("fd")) else { continue };
        for fd in fds.flatten() {
            let Ok(target) = fs::read_link(fd.path()) else { continue };
            let link = target.to_string_lossy();
            let Some(inode) = link.strip_prefix("socket:[").and_then(|s| s.strip_suffix(']'))
                .and_then(|s| s.parse::<u64>().ok()) else { continue };
            owners.entry(inode).or_insert_with(|| (pid, name.clone()));
        }
    }
    owners
}

pub fn parse_proc_net(content: &str, protocol: &str) -> Vec<SocketRow> {
    content.lines().skip(1).filter_map(|line| {
        let cols: Vec<_> = line.split_whitespace().collect();
        if cols.len() < 10 { return None; }
        let local = decode_address(cols[1])?;
        let remote = decode_address(cols[2])?;
        let state = match cols[3] {
            "0A" => "LISTEN", "01" => "ESTABLISHED", "06" => "TIME_WAIT",
            "08" => "CLOSE_WAIT", "07" => if protocol.starts_with("UDP") { "UNCONN" } else { "CLOSED" },
            "02" => "SYN_SENT", "03" => "SYN_RECV", "04" => "FIN_WAIT1",
            "05" => "FIN_WAIT2", "09" => "LAST_ACK", "0B" => "CLOSING", _ => "OTHER",
        };
        Some(SocketRow {
            protocol: protocol.to_owned(),
            local,
            remote,
            state: state.to_owned(),
            inode: cols[9].parse().ok()?,
            pid: None,
            process: "-".to_owned(),
        })
    }).collect()
}

fn decode_address(raw: &str) -> Option<String> {
    let (hex, port) = raw.split_once(':')?;
    let port = u16::from_str_radix(port, 16).ok()?;
    let address = match hex.len() {
        8 => {
            let addr = u32::from_str_radix(hex, 16).ok()?;
            Ipv4Addr::from(addr.to_le_bytes()).to_string()
        }
        32 => {
            let mut octets = [0u8; 16];
            for (index, group) in hex.as_bytes().chunks_exact(8).enumerate() {
                let word = u32::from_str_radix(std::str::from_utf8(group).ok()?, 16).ok()?;
                octets[index * 4..index * 4 + 4].copy_from_slice(&word.to_le_bytes());
            }
            format!("[{}]", Ipv6Addr::from(octets))
        }
        _ => return None,
    };
    Some(format!("{address}:{port}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percentages_are_safe() {
        assert_eq!(percent(9, 0), 0.0);
        assert_eq!(percent(120, 100), 100.0);
        assert_eq!(percent(75, 100), 75.0);
    }

    #[test]
    fn parses_linux_socket() {
        let sample = "sl local_address rem_address st tx_queue rx_queue tr tm->when retrnsmt uid timeout inode\n\
         0: 0100007F:1F90 00000000:0000 0A 00000000:00000000 00:00000000 00000000 1000 0 42 1 000\n";
        let parsed = parse_proc_net(sample, "TCP");
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].local, "127.0.0.1:8080");
        assert_eq!(parsed[0].state, "LISTEN");
        assert_eq!(parsed[0].inode, 42);
    }

    #[test]
    fn formats_bytes() {
        assert_eq!(format_bytes(1024), "1.0 KiB");
        assert_eq!(format_bytes(0), "0 B");
    }
}
