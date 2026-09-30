# Settings TUI

A production-grade universal Linux Settings Center for the terminal.

## What it is

`settings-tui` is a modern, keyboard-first, native settings application that brings the functionality of tools like GNOME Settings or KDE System Settings straight to your terminal. 

## Why it exists

Many terminal tools offer monitoring (like `btop`), but full graphical configuration managers are tightly coupled to their desktop environments (GNOME, KDE). `settings-tui` aims to provide a unified configuration interface using native Linux APIs, D-Bus, and standard configuration files, capable of running perfectly on lightweight Window Managers (Hyprland, Sway), over SSH, or natively within any terminal environment.

## Supported Environments

- **Distribution**: Agnostic (tested on Arch Linux, adaptable to others)
- **Session**: Wayland & X11
- **Init**: systemd
- **Audio**: PipeWire/WirePlumber
- **Network**: NetworkManager (via D-Bus)

*Capability detection is built-in; missing dependencies gracefully degrade.*

## Installation

```bash
cargo install --path .
```
Or build from source:
```bash
git clone ...
cd settings-tui
cargo build --release
```

## Keyboard Controls

| Key | Action |
| --- | --- |
| `↑` / `k` | Navigate Up |
| `↓` / `j` | Navigate Down |
| `Enter` | Select / Toggle |
| `Esc` / `q` | Back / Quit |
| `/` | Search |
| `Tab` | Next Control |

## Architecture

`settings-tui` uses a strict layered architecture:

1. **TUI**: Pure Ratatui-based rendering. No domain logic.
2. **Settings Domain**: Transactions, capabilities, and history.
3. **Backend Interfaces**: Traits abstracting system functions (`NetworkBackend`, `SystemBackend`, etc.).
4. **Linux Adapters**: Concrete implementations (e.g., `NetworkManagerBackend`, `PipeWireBackend`).

## Backends Support (WIP)

- **Network**: Mock implementation (NetworkManager planned)
- **System**: Real backend using `sysinfo`
- **Bluetooth**: BlueZ D-Bus planned
- **Power**: UPower planned

## Limitations

- Does not perform destructive disk operations
- Requires correct privileges for system-wide configuration
- Hyprland/Wayland specific settings are currently stubbed
