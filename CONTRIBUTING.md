# Contributing to Settings TUI

Thank you for your interest in contributing to `settings-tui`! This document provides guidelines, architectural context, and development workflows to make your contribution seamless.

---

## 🛠️ Development Setup

### System Prerequisites
`settings-tui` connects directly to Linux subsystems via native D-Bus libraries and C bindings.

* **Debian / Ubuntu / Pop!_OS**:
  ```bash
  sudo apt update
  sudo apt install -y build-essential libdbus-1-dev pkg-config
  ```
* **Arch Linux / Manjaro**:
  ```bash
  sudo pacman -S base-devel dbus
  ```
* **Fedora / RHEL**:
  ```bash
  sudo dnf install -y dbus-devel pkg-config gcc
  ```
* **Rust Toolchain**: Rust 1.75+ is required.
  ```bash
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
  ```

### Building the Project
```bash
git clone https://github.com/ziuus/settings-tui.git
cd settings-tui

# Build in debug mode
cargo build

# Run directly
cargo run -- -s Network

# Run platform diagnostics
cargo run -- --diagnose
```

---

## 🏗️ Architecture & Codebase Layout

```
settings-tui/
├── src/
│   ├── app/                # Main event loop, keybinding dispatch, transaction macro
│   │   ├── mod.rs          # App state, focus management, modal handling, unit tests
│   │   ├── events.rs       # Terminal event handling & tick timers
│   │   ├── listeners.rs    # D-Bus and external background event listeners
│   │   └── transaction.rs  # Safe state mutation execution engine
│   ├── backends/           # Subsystem adapters
│   │   ├── network/        # NetworkManager D-Bus and nmcli client
│   │   ├── audio/          # PipeWire / WirePlumber integration
│   │   ├── bluetooth/      # BlueZ D-Bus integration
│   │   ├── power/          # UPower & power-profiles-daemon
│   │   ├── display/        # Hyprland IPC & sysfs brightness
│   │   ├── appearance/     # XDG Desktop Portal / gsettings
│   │   ├── applications/   # XDG .desktop & Flatpak desktop entry parsers
│   │   ├── services/       # systemd unit controllers
│   │   └── system/         # sysinfo, hostname1, timedate1
│   ├── config/             # Atomic config file serialization & defaults
│   ├── dbus/               # zbus proxy trait definitions
│   ├── platform/           # Subsystem capability detection & badge matrix
│   ├── security/           # Atomic file writes (0o600), safe systemctl sanitization
│   └── ui/                 # Ratatui rendering engine
│       ├── mod.rs          # Main frame layout, modals, search drawer
│       ├── widgets.rs      # Reusable UI controls (toggles, sliders, setting rows)
│       └── pages/          # Individual category renderers (network, sound, etc.)
├── assets/                 # Screenshots & visual assets
├── bin/                    # Node.js launcher wrapper for npm/npx
└── tests/                  # Unit and integration tests
```

---

## 🛡️ Core Invariants & Rules

When contributing code, adhere to these fundamental principles:

### 1. The Verification Contract
Every mutable setting **must** execute transactionally:
```
READ canonical state
  → MUTATE via native API
  → WAIT / POLL (bounded)
  → READ CANONICAL STATE
  → VERIFY postcondition
  → UPDATE UI & Notify User
```
Use the `execute_transaction!` macro in `src/app/mod.rs` to ensure any failure or verification timeout is cleanly surfaced to the user without crashing.

### 2. Never Fake Support
Linux environments vary widely. If a subsystem or feature is unsupported (e.g. Wayland fractional scaling under X11, or power profiles without `power-profiles-daemon`), report it accurately in `src/platform/mod.rs` via `CapabilityStatus` (`Mutable`, `ReadOnly`, `RequiresPermission`, `Partial`, `Unsupported`, `Unavailable`).

### 3. Safe Subprocess Execution
Avoid invoking arbitrary shell pipelines (`sh -c "..."`). Always use safe argument vectors:
```rust
std::process::Command::new("nmcli")
    .args(["device", "wifi", "connect", &ssid, "password", &password])
```

### 4. Zero Panics
Never call `.unwrap()` or `.expect()` in runtime UI rendering or backend polling paths. Return `Result<T>` and handle fallback states gracefully.

---

## 🧪 Quality Gates & Testing

Before submitting a Pull Request, ensure all quality gates pass:

```bash
# 1. Format check
cargo fmt --check

# 2. Clippy linting (strictly zero warnings)
cargo clippy --all-targets --all-features -- -D warnings

# 3. Unit & Integration test suite
cargo test --all-features

# 4. Release build verification
cargo build --release
```

---

## 📝 Pull Request Guidelines

1. **Keep PRs Focused**: One logical subsystem or feature per PR.
2. **Add Unit Tests**: Any new backend parser, keybinding handler, or modal must include automated unit tests.
3. **Update Documentation**: If you add a CLI flag or keybinding, document it in `README.md`.
4. **Follow Semantic Commit Messages**:
   * `feat(subsystem): describe new feature`
   * `fix(subsystem): describe bug fix`
   * `docs(readme): describe doc change`
   * `test(subsystem): describe added tests`
