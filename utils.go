package main

// imports
import (
	"fmt"
	"os"
	"os/exec"
	"sort"
	"strconv"
	"strings"
	"syscall"
	"time"

	"github.com/shirou/gopsutil/v3/disk"
	"github.com/shirou/gopsutil/v3/host"

	psnet "github.com/shirou/gopsutil/v3/net"
	"github.com/shirou/gopsutil/v3/process"
)

func measureDiskSpace() (pct float64, total, used string) {
	d, err := disk.Usage("/")
	if err != nil || d == nil {
		return 0, "", ""
	}
	gb := float64(1024 * 1024 * 1024)
	return d.UsedPercent / 100,
		fmt.Sprintf("%.1fGB", float64(d.Total)/gb),
		fmt.Sprintf("%.1fGB", float64(d.Used)/gb)
}

func measureDiskIO() (readStr, writeStr string, readBytes, writeBytes uint64) {
	counters, err := disk.IOCounters()
	if err != nil || len(counters) == 0 {
		return "N/A", "N/A", 0, 0
	}
	for _, c := range counters {
		readBytes += c.ReadBytes
		writeBytes += c.WriteBytes
	}
	return formatByteSize(readBytes), formatByteSize(writeBytes), readBytes, writeBytes
}

func measureUptime() string {
	bootTime, err := host.BootTime()
	if err != nil || bootTime == 0 {
		return ""
	}
	return time.Since(time.Unix(int64(bootTime), 0)).Round(time.Minute).String()
}

func measureNetworkIO() (sent, recv string, sentBytes, recvBytes uint64) {
	counters, err := psnet.IOCounters(false)
	if err != nil || len(counters) == 0 {
		return "", "", 0, 0
	}
	return formatByteSize(counters[0].BytesSent),
		formatByteSize(counters[0].BytesRecv),
		counters[0].BytesSent,
		counters[0].BytesRecv
}

func measureBattery() (pct int, state, remaining string) {
	for _, bat := range []string{"BAT1", "BAT0"} {
		capPath := "/sys/class/power_supply/" + bat + "/capacity"
		if _, err := os.Stat(capPath); err != nil {
			continue
		}
		data, _ := os.ReadFile(capPath)
		pct, _ = strconv.Atoi(strings.TrimSpace(string(data)))

		statusPath := "/sys/class/power_supply/" + bat + "/status"
		if data, err := os.ReadFile(statusPath); err == nil {
			state = strings.TrimSpace(string(data))
		}

		pairs := []struct {
			path string
			mode string
		}{
			{"/sys/class/power_supply/" + bat + "/time_to_full_avg", "Charging"},
			{"/sys/class/power_supply/" + bat + "/time_to_empty_avg", "Discharging"},
		}
		for _, pair := range pairs {
			if state == pair.mode {
				if data, err := os.ReadFile(pair.path); err == nil {
					if mins, err := strconv.Atoi(strings.TrimSpace(string(data))); err == nil && mins > 0 {
						remaining = fmt.Sprintf("%dh %dm", mins/60, mins%60)
					}
				}
			}
		}
		break
	}
	return
}

func measureGPU() (usage float64, name, memUsed, memTotal string) {
	out, err := exec.Command("nvidia-smi",
		"--query-gpu=utilization.gpu,name,memory.used,memory.total",
		"--format=csv,noheader,nounits",
	).Output()
	if err != nil {
		return 0, "", "", ""
	}
	parts := strings.Split(strings.TrimSpace(string(out)), ", ")
	if len(parts) >= 4 {
		u, _ := strconv.ParseFloat(parts[0], 64)
		return u, parts[1], parts[2] + "MiB", parts[3] + "MiB"
	}
	return 0, "", "", ""
}

func measureCPUTemperature() float64 {
	entries, err := os.ReadDir("/sys/class/thermal")
	if err != nil {
		return 0
	}
	var maxTemp float64
	for _, entry := range entries {
		if !strings.HasPrefix(entry.Name(), "thermal_zone") {
			continue
		}
		data, err := os.ReadFile("/sys/class/thermal/" + entry.Name() + "/temp")
		if err != nil {
			continue
		}
		temp, _ := strconv.ParseFloat(strings.TrimSpace(string(data)), 64)
		temp /= 1000
		if temp > maxTemp {
			maxTemp = temp
		}
	}
	return maxTemp
}

func measureGPUTemperature() float64 {
	out, err := exec.Command("nvidia-smi",
		"--query-gpu=temperature.gpu",
		"--format=csv,noheader,nounits",
	).Output()
	if err != nil {
		return 0
	}
	temp, _ := strconv.ParseFloat(strings.TrimSpace(string(out)), 64)
	return temp
}

func measurePublicIP() string {
	for _, service := range []string{"ifconfig.me", "icanhazip.com", "api.ipify.org"} {
		out, err := exec.Command("curl", "-s", "--max-time", "2", service).Output()
		if err == nil && len(out) > 0 {
			return strings.TrimSpace(string(out))
		}
	}
	return ""
}

func gatherOpenPorts() []PortEntry {
	connections, err := psnet.Connections("tcp")
	if err != nil {
		return nil
	}
	seen := map[uint32]bool{}
	var entries []PortEntry
	for _, c := range connections {
		if c.Status != "LISTEN" || c.Laddr.Port == 0 || seen[c.Laddr.Port] {
			continue
		}
		seen[c.Laddr.Port] = true
		procName := "-"
		if c.Pid > 0 {
			if p, err := process.NewProcess(c.Pid); err == nil {
				if n, err := p.Name(); err == nil {
					procName = n
				}
			}
		}
		entries = append(entries, PortEntry{
			Port: strconv.Itoa(int(c.Laddr.Port)),
			PID:  strconv.Itoa(int(c.Pid)),
			Name: procName,
		})
	}
	return entries
}

func gatherProcesses(limit int) []ProcessEntry {
	procs, err := process.Processes()
	if err != nil {
		return nil
	}
	var entries []ProcessEntry
	for _, p := range procs {
		name, _ := p.Name()
		if name == "" {
			name = "-"
		}
		if len(name) > 24 {
			name = name[:21] + "..."
		}
		cpuPct, _ := p.CPUPercent()
		memPct, _ := p.MemoryPercent()
		statuses, _ := p.Status()
		status := "-"
		if len(statuses) > 0 {
			status = statuses[0]
		}
		entries = append(entries, ProcessEntry{
			PID:    strconv.Itoa(int(p.Pid)),
			Name:   name,
			CPU:    fmt.Sprintf("%.1f", cpuPct),
			Memory: fmt.Sprintf("%.1f", memPct),
			Status: status,
		})
	}
	sort.Slice(entries, func(i, j int) bool {
		ci, _ := strconv.ParseFloat(entries[i].CPU, 64)
		cj, _ := strconv.ParseFloat(entries[j].CPU, 64)
		return ci > cj
	})
	if len(entries) > limit {
		entries = entries[:limit]
	}
	return entries
}

func gatherConnections() []ConnectionEntry {
	connections, err := psnet.Connections("inet")
	if err != nil {
		return nil
	}
	entries := make([]ConnectionEntry, 0, len(connections))
	for _, c := range connections {
		procName := "-"
		if c.Pid > 0 {
			if p, err := process.NewProcess(c.Pid); err == nil {
				if n, err := p.Name(); err == nil {
					procName = n
				}
			}
		}
		protocol := "TCP"
		if c.Type == syscall.SOCK_DGRAM {
			protocol = "UDP"
		}
		localAddr := fmt.Sprintf("%s:%d", c.Laddr.IP, c.Laddr.Port)
		remoteAddr := fmt.Sprintf("%s:%d", c.Raddr.IP, c.Raddr.Port)
		if c.Raddr.IP == "" {
			remoteAddr = "-"
		}
		entries = append(entries, ConnectionEntry{
			Local:    localAddr,
			Remote:   remoteAddr,
			State:    c.Status,
			Process:  procName,
			Protocol: protocol,
		})
	}
	return entries
}

func killProcess(pid int32) error {
	return syscall.Kill(int(pid), syscall.SIGTERM)
}

func formatByteSize(bytes uint64) string {
	if bytes < 1024 {
		return fmt.Sprintf("%d B", bytes)
	}
	units := []string{"KB", "MB", "GB", "TB"}
	size := float64(bytes)
	for _, unit := range units {
		size /= 1024
		if size < 1024 {
			return fmt.Sprintf("%.1f %s", size, unit)
		}
	}
	return fmt.Sprintf("%.1f PB", size)
}

func formatByteRate(bps float64) string {
	if bps < 1024 {
		return fmt.Sprintf("%.0f B/s", bps)
	}
	units := []string{"KB/s", "MB/s", "GB/s", "TB/s"}
	rate := bps
	for _, unit := range units {
		rate /= 1024
		if rate < 1024 {
			return fmt.Sprintf("%.1f %s", rate, unit)
		}
	}
	return fmt.Sprintf("%.1f PB/s", rate)
}
