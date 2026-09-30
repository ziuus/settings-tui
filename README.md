# Settings TUI (`settings-tui`)

[![CI](https://github.com/ziuus/settings-tui/actions/workflows/ci.yml/badge.svg)](https://github.com/ziuus/settings-tui/actions/workflows/ci.yml)
[![npm version](https://img.shields.io/npm/v/settings-tui.svg)](https://www.npmjs.com/package/settings-tui)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE-MIT)
[![Platform: Linux](https://img.shields.io/badge/Platform-Linux-orange.svg)]()

> A production-grade universal Linux Settings Center for the terminal.

`settings-tui` brings the power and completeness of modern graphical control centers (GNOME Settings, KDE System Settings) directly to your terminal. It communicates directly with native Linux subsystems via D-Bus, systemd, NetworkManager, BlueZ, WirePlumber/PipeWire, sysfs, and compositor protocols, with zero desktop environment lock-in.

Runs natively across standalone Wayland compositors (Hyprland, Sway), X11 window managers (i3, bspwm, dwm), or headless machines over SSH.

---

## Capabilities & Subsystems

| Subsystem | Backend | Key Features |
| --- | --- | --- |
| **Network** | NetworkManager (D-Bus / `nmcli`) | Active interface card (IP, Gateway, Device), Wi-Fi radio toggle, AP scanning, connection/disconnection |
| **Bluetooth** | BlueZ (D-Bus `org.bluez`) | Adapter power toggle, paired/discovered peripheral management, connect, disconnect, forget/remove |
| **Sound** | PipeWire & WirePlumber (`wpctl`) | Output sinks, input sources, per-application streams, numerical volume sliders, mute toggle, default device selector |
| **Power** | UPower (D-Bus) & logind | Battery status, charge percentage, energy profiles (Performance, Balanced, Power Saver) |
| **Display** | Sysfs / `brightnessctl` & Hyprland | Backlight brightness slider (`0-100%`), multi-monitor resolution and refresh rate switching |
| **Appearance** | GNOME Desktop Interface (`gsettings`) | Dark mode and light mode color-scheme toggle, font name and icon theme reporting |
| **Applications** | XDG Desktop Entry Spec | User and system `.desktop` discovery, category filtering, search, and detached background launcher |
| **Services** | systemd (D-Bus `systemd1`) | System unit inspection, active/substate monitoring, verified unit start/stop with Polkit integration |
| **System** | sysinfo, `hostname1`, `timedate1`, logind | Hostname, chassis, timezone, toggleable NTP sync, disk storage partition usage bars, protected power transitions |

---

## Installation

### Via npm (Universal)
```bash
# Run instantly with zero manual installation:
npx settings-tui

# Or install globally:
npm install -g settings-tui

# Then launch anytime:
settings-tui
# or simply:
settings
```

### From Source (Cargo)
```bash
git clone https://github.com/ziuus/settings-tui.git
cd settings-tui
cargo build --release
sudo make install
```

### Arch Linux (PKGBUILD)
```bash
cd extra && makepkg -si
```

---

## CLI Usage

```
settings-tui [OPTIONS]

Options:
  -s, --section <NAME>   Jump directly to category (Network, Bluetooth, Sound, Power, Display, Appearance, Applications, Services, System)
  -q, --search <QUERY>   Launch directly in search mode with initial query
  -d, --diagnose         Output the Linux compatibility & capability matrix and exit
  -c, --config <FILE>    Custom path to configuration file [default: ~/.config/settings-tui/config.json]
  -h, --help             Print help information
  -V, --version          Print version information
```

### System Compatibility Diagnostic
Run `settings-tui --diagnose` to inspect the detected capabilities on your system:
```bash
settings-tui --diagnose
```

---

## Keyboard Navigation

| Key | Context | Action |
| --- | --- | --- |
| `↑` / `k`, `↓` / `j` | Any | Navigate items or categories |
| `←` / `h` / `Esc` | Content | Return focus to category sidebar |
| `→` / `l` / `Enter` | Sidebar | Move focus into active settings category |
| `Enter` | Content | Activate setting / toggle / set default / confirm power action |
| `Space` | Content | Toggle switches / mute audio device |
| `<` / `>` or `+` / `-` | Content | Adjust volume, brightness slider, or cycle mode |
| `Backspace` / `Del` | Content | Forget device (e.g. remove paired Bluetooth peripheral) |
| `/` | Any | Open global settings search across all categories |
| `Esc` | Search / Modal | Cancel search or dismiss modal dialog |
| `q` | Top-level | Clean exit (terminal restored) |

---

## Security & Reliability Architecture

1. **Transactional Mutations**: Every mutable operation executes with verified transaction semantics (`execute_transaction!`). The engine waits for bounded canonical OS state updates, verifies read-back values, and reports exact system error reasons if an action fails.
2. **Safe Power Actions**: Power operations (`suspend`, `hibernate`, `reboot`, `poweroff`) require explicit confirmation modals with distinct prompts; accidental `Space` keystrokes are blocked.
3. **Atomic Configuration**: Configuration files write to unique temporary files with `0o600` permissions, execute `fsync`, maintain `.bak` fallbacks, preserve symlinks, and use atomic rename semantics.
4. **Viewport Protection**: Dynamic selection clamping ensures list changes never cause out-of-bounds panics. Terminals smaller than 45×10 show responsive resizing warnings.

---

## Automated Publishing

This repository includes automated GitHub Actions workflows:
- **CI**: Formats, lints (Clippy), and runs test suites on every push and pull request.
- **npm Release**: Triggered automatically on creating a release or Git tag (`v*`), compiles optimized release binaries and publishes to npm.
  *(Requires setting the `NPM_TOKEN` secret in your GitHub repository's **Settings → Secrets and variables → Actions**).*

---

## License

Licensed under the [MIT License](LICENSE-MIT).
