# AGENTS.md — Agentic Workflow & Architecture Reference

This file documents architectural constraints, state patterns, and verification standards for autonomous AI coding agents working on `settings-tui`.

---

## 1. Core Mission & Identity

`settings-tui` is a universal Linux Settings Center TUI written in Rust. It interacts directly with native Linux subsystems (D-Bus, PipeWire, BlueZ, NetworkManager, UPower, systemd, sysfs, Wayland/Hyprland IPC).

**Golden Rule**: Never fake support, never mock production backends with dummy data, and never assume an asynchronous Linux state mutation succeeded without verifying the actual read-back state.

---

## 2. The Verification Contract (Transactions)

All mutable settings must follow the transaction lifecycle:
```
READ CANONICAL STATE
  → VALIDATE INPUT
  → EXECUTE MUTATION via native API
  → BOUNDED RETRY / PROPAGATION WAIT (with timeout)
  → READ CANONICAL STATE
  → VERIFY POSTCONDITION
  → UPDATE TUI & NOTIFY USER
```

Use the `execute_transaction!` macro located in [`src/app/mod.rs`](src/app/mod.rs):
```rust
let (mutation_ok, verified, final_state, mutation_error) = execute_transaction!(
    backend.mutate_setting(&target),
    backend.get_current_state(),
    |state: &State| state.is_applied(),
    15, 100 // 15 attempts, 100ms intervals
);
```

---

## 3. Capability Honesty

Linux environments differ wildly (systemd vs openrc, Wayland vs X11, PipeWire vs ALSA, NetworkManager vs iwd).

* Always detect subsystem availability in [`src/platform/mod.rs`](src/platform/mod.rs).
* Use the canonical `CapabilityStatus` enum:
  * `Mutable`: Full read and write support.
  * `ReadOnly(reason)`: Information is visible, but adjustments cannot be made.
  * `RequiresPermission(reason)`: Mutation requires Polkit or root elevation.
  * `Partial(reason)`: Core features work, but edge features are unmanaged.
  * `Unsupported(reason)`: Feature is not supported by current compositor or daemon.
  * `Unavailable(reason)`: Daemon or service is not running or installed.
* UI badges: `[~]` (Partial), `[*]` (Requires Elevation), `[R]` (Read-only), `[x]` (Unsupported), `[!]` (Unavailable).

---

## 4. UI Layout & State Machine Invariants

1. **Focus States**:
   * `Focus::Sidebar`: Navigation moves through category list.
   * `Focus::Content`: Navigation operates within the active page rows.
2. **Modal Interception**:
   * Modals (`confirm_action`, `password_modal`, `is_searching`) must intercept keystrokes at the top of `App::handle_key` before any regular navigation occurs.
   * Modals must render on top using `ratatui::widgets::Clear` to avoid bleed-through.
3. **Small Terminal & Bounds Protection**:
   * Never assume terminal dimensions. Use `.clamp()` and `.saturating_sub()` on all item indices and slice offsets to prevent panics when resizing.
4. **Zero Startup Lag**:
   * Background subsystem polling must run on frame 0 to populate initial UI state immediately.

---

## 5. Native Linux APIs Reference

* **Network**:
  * D-Bus: `org.freedesktop.NetworkManager` (`/org/freedesktop/NetworkManager`)
  * Wi-Fi APs: `org.freedesktop.NetworkManager.AccessPoint`
  * CLI: `nmcli -t -f ...` (used safely with argument vectors)
  * DNS: Parse `/etc/resolv.conf` for active nameservers.
* **Power**:
  * D-Bus: `org.freedesktop.UPower` (`/org/freedesktop/UPower`)
  * Devices: `org.freedesktop.UPower.Device` (`/org/freedesktop/UPower/devices/battery_BAT0`)
  * Profiles: `net.hadess.PowerProfiles` (`/net/hadess/PowerProfiles`)
* **Audio**:
  * WirePlumber / PipeWire: CLI `wpctl status`, `wpctl get-volume`, `wpctl set-volume`, `wpctl set-mute`, `wpctl set-default`.
* **Bluetooth**:
  * D-Bus: `org.bluez` (`/org/bluez/hci0`)
  * Devices: `org.bluez.Device1`
* **Display**:
  * Hyprland IPC: `hyprctl -j monitors` and `hyprctl keyword monitor ...`
  * Backlight: `brightnessctl -m` and `/sys/class/backlight/`
* **System & Services**:
  * D-Bus: `org.freedesktop.systemd1` (`/org/freedesktop/systemd1`)
  * Time: `org.freedesktop.timedate1` (`/org/freedesktop/timedate1`)
  * Hostname: `org.freedesktop.hostname1` (`/org/freedesktop/hostname1`)

---

## 6. Pre-Commit Quality Checklist

Every agent task must verify all 4 quality gates before declaring completion:

```bash
# 1. Formatting
cargo fmt --check

# 2. Clippy Linter (zero warnings permitted)
cargo clippy --all-targets --all-features -- -D warnings

# 3. Unit & Integration Tests (all must pass)
cargo test --all-features

# 4. Release Build
cargo build --release
```
