# Orbit TUI

Terminal based system monitor in Go. Built using bubble tea.

## What it does

- live cpu/ram/disk usage with progress bars 
- process table
- open tcp ports + process which owns them
- gpu stats via nvidia-smi
- battery status
- public ip
- catpuccin themes because why not

## install

**from release (recommended)**

download the latest binary from [releases](https://github.com/rickyparmar-raw/orbit-tui/releases), then:

```bash
chmod +x orbit
./orbit
```

from source

```bash
git clone https://github.com/rickyparmar-raw/orbit-tui.git
cd orbit-tui
go build -o orbit .
./orbit
```

## keybinds

tab/arrows - switch tabs
up/down - move around
enter - kill process 
shift+left/right - change theme
? - help
q - quit 

## notes 

- linux only 
- gpu stuff needs nvidia-smi
- battery reads from /sys/class/power_supply/