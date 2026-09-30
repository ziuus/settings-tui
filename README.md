# Settings TUI

A production-grade universal Linux Settings Center for the terminal.

## Overview

`settings-tui` is a keyboard-driven, native settings application that brings the functionality of tools like GNOME Settings or KDE System Settings straight to your terminal. It communicates directly with native Linux subsystems via D-Bus, systemd, NetworkManager, BlueZ, WirePlumber/PipeWire, and compositor protocols, without requiring heavy desktop environments.

It runs natively on standalone Wayland compositors (Hyprland, Sway), X11 window managers (i3, bspwm), or headless remote systems over SSH.

## Capabilities & Subsystems

| Subsystem | Backend | Capabilities |
| --- | --- | --- |
| **Network** | NetworkManager (D-Bus / `nmcli`) | Wi-Fi device scanning, signal strength monitoring, network connection / disconnection |
| **Bluetooth** | BlueZ (D-Bus `org.bluez`) | Adapter power toggling, paired device discovery, connect / disconnect, device removal / forget |
| **Sound** | PipeWire & WirePlumber (`wpctl`) | Sink, source, and stream volume adjustment, mute toggling, default output device switching |
| **Power** | UPower (D-Bus) & logind | Battery status, charge state, performance / balanced / power-saver profile cycling |
| **Display** | Hyprland IPC (`hyprctl`) | Multi-monitor detection, resolution and refresh rate switching with scale preservation |
| **Appearance** | GNOME Desktop Interface (`gsettings`) | Dark mode and light mode color-scheme toggle, font and theme reporting |
| **Applications** | XDG Desktop Entry Spec | User and system `.desktop` application discovery, category filtering, background launching |
| **Services** | systemd (D-Bus `systemd1`) | System service unit enumeration, active/substate inspection, unit start and stop |
| **System** | sysinfo & logind (`login1`) | Kernel, OS distro, uptime, memory utilization, authorized power transitions |

*All capabilities are dynamically discovered at runtime with honest, granular capability semantics.*

## Installation

### From Source
```bash
cargo build --release
sudo make install
```

### Via Cargo
```bash
cargo install --path .
```

### Arch Linux (PKGBUILD)
```bash
cd extra && makepkg -si
```

## CLI Usage

```
settings-tui [OPTIONS]

Options:
  -c, --config <FILE>    Custom path to configuration file [default: ~/.config/settings-tui/config.json]
  -s, --section <NAME>   Jump directly to category (e.g. Network, Bluetooth, Sound, Power, Display, Services, System)
  -q, --search <QUERY>   Launch directly in search mode with initial query
  -d, --diagnose         Output the Linux compatibility & capability matrix and exit
  -h, --help             Print help information
  -V, --version          Print version information
```

### Capability Matrix Diagnostic
Run `settings-tui --diagnose` to inspect the authoritative capability status on your host system:
```bash
settings-tui --diagnose
```

## Keyboard Navigation

| Key | Context | Action |
| --- | --- | --- |
| `↑` / `k` | Any | Move up |
| `↓` / `j` | Any | Move down |
| `←` / `h` / `Esc` | Content | Return focus to category sidebar |
| `→` / `l` / `Enter` | Sidebar | Move focus into active category settings |
| `Enter` | Content | Activate setting / toggle / confirm power action |
| `<` / `>` or `h` / `l` | Content | Adjust slider value (volume) or cycle mode / profile |
| `Backspace` / `Del` | Content | Forget device (e.g. remove paired Bluetooth peripheral) |
| `/` | Any | Open global settings search |
| `Esc` | Search / Modal | Cancel search or dismiss modal |
| `q` | Top-level | Clean exit (terminal restored) |

## Security & Reliability Architecture

1. **Transactional Mutations**: Every mutable setting executes with optimistic or verified transaction semantics (`execute_transaction!`). The engine waits for canonical OS state changes, checks read-back values, and reports exact OS error reasons if an action fails.
2. **Crash-Safe Atomic Configuration**: Configuration persistence writes to a unique temporary file with strict `0o600` permissions, flushes buffers with `fsync`, creates `.bak` backups of previous state, preserves dotfile symlinks, and performs atomic renames.
3. **Privilege & Input Validation**: Service names are sanitized to prevent shell injection or traversal. Power state transitions (`poweroff`, `reboot`, `suspend`, `hibernate`) are whitelisted, pre-checked against logind policies, and require explicit confirmation dialogs.
4. **Viewport & Bounds Protection**: All navigation selections are dynamically clamped on every render loop, eliminating out-of-bounds panics when asynchronous background events shrink lists. Terminal viewports < 45×10 display responsive layout warnings.

## License

Licensed under either the MIT or Apache-2.0 license at your option.
