# Orbit

A compact Linux system-monitoring TUI built with **Rust + Ratatui**.

The dashboard follows the reference image: blue status header, six keyboard-accessible tabs, CPU/Memory/Disk panels, Network and Alerts panels, and Git/Todo summaries. All percentages and counts come from the local machine.

## Install

Requires stable Rust/Cargo and a Linux terminal.

    git clone https://github.com/0xQuerencia/orbit-tui.git
    cd orbit-tui
    cargo run --release

Or install the command:

    cargo install --path .
    orbit

The original Go/Bubble Tea prototype remains in the repository for comparison. The new Ratatui application is under src/ and is built by Cargo.

## Pages

- **Dashboard:** CPU history, GPU where supported, battery, RAM/swap, root filesystem usage, disk I/O, network totals/throughput, listening ports, alerts, Git status and todos.
- **Processes:** CPU-sorted process table, pinning and filtering.
- **Ports:** Linux TCP listeners and accessible owning PID/process.
- **Connections:** Linux TCP and UDP sockets and statuses.
- **Git:** branch, working-tree changes and recent commits from the current directory.
- **Todo:** persisted in XDG_STATE_HOME/orbit/todos.tsv (fallback ~/.local/state/orbit/todos.tsv).

## Controls

| Key | Action |
| --- | --- |
| 1–6 / Tab / arrows | Switch tabs |
| Up/Down / j/k | Select a row |
| / | Search processes, ports and connections |
| p | Pin/unpin a process |
| Enter | Request SIGTERM in Processes/Ports (requires confirmation); toggle todo |
| a | Add a todo from Dashboard/Todo |
| x / d | Complete/delete selected todo |
| w | Cycle Orbit/Catppuccin colors |
| r | Refresh |
| ? | Help |
| q / Ctrl+C | Quit |

## Notes

- Linux only. NVIDIA GPU metrics require nvidia-smi. Unsupported GPU/battery data is shown as unavailable.
- /proc and /sys provide Linux socket, battery and disk stats; sysinfo provides CPU, memory and processes. PID ownership depends on permissions.
- No external public-IP requests, telemetry, or background network uploads.
- Process termination requires confirmation and sends SIGTERM, not SIGKILL.
- Dashboard compresses on narrow terminals and tells you when the terminal is too small.
- Git status applies to the directory where Orbit was launched.

## Development

    cargo test --all-targets
    cargo build --release
