# Global Linux Backend Architecture Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Decouple `settings-tui` from single-distro and single-compositor (Hyprland) assumptions, adding first-class support for generic Wayland (`wlr-randr`, Sway/River/Labwc), X11 (`xrandr`), Linux DRM sysfs, and multi-vendor hardware thresholds across all standard Linux distributions.

**Architecture:** Implement modular display and input backend factories (`create_display_backend` and `create_input_backend`) that autodetect environment capabilities in hierarchical order (Hyprland -> wlr-randr -> xrandr -> DRM sysfs for display; Hyprland -> GNOME/gsettings -> Sway -> Generic for input). Generalize power/idle timeouts and multi-vendor battery threshold detection.

**Tech Stack:** Rust 2021, Tokio, async-trait, serde_json, zbus, Linux sysfs (`/sys/class/drm`, `/sys/class/power_supply`), `wlr-randr`, `xrandr`, `gsettings`, `systemd-logind`.

## Global Constraints

- No hardcoded distro paths or single-compositor lock-in (Hyprland is one backend among equals).
- Zero mock or dummy data: every backend must probe real Linux APIs, CLI interfaces, or kernel sysfs.
- All 85+ existing unit tests must continue passing, plus new unit tests for WLR, Xrandr, and DRM parsing.
- Strict code hygiene: zero clippy warnings (`cargo clippy --all-targets -- -D warnings`), formatted with `cargo fmt --check`.

---

### Task 1: WlrRandr Display Backend (`wlr-randr`)

**Files:**
- Create: `src/backends/display/wlr.rs`
- Modify: `src/backends/display/mod.rs`
- Test: `tests/wlr_display_tests.rs` or inline unit tests in `src/backends/display/wlr.rs`

**Interfaces:**
- Consumes: `DisplayBackend`, `Monitor` from `src/backends/mod.rs`
- Produces: `WlrRandrBackend` struct implementing `DisplayBackend`

- [ ] **Step 1: Write unit tests for `wlr-randr` JSON and text output parsing**
- [ ] **Step 2: Implement `WlrRandrBackend`**
  - Parse `wlr-randr --json` into `Vec<Monitor>`
  - Implement `set_resolution`, `set_scale`, `set_monitor_mode`
  - Delegate brightness, night light, and grayscale to shared universal methods
- [ ] **Step 3: Run tests and verify they pass**
- [ ] **Step 4: Commit**

---

### Task 2: Xrandr Display Backend (`xrandr`)

**Files:**
- Create: `src/backends/display/xrandr.rs`
- Modify: `src/backends/display/mod.rs`
- Test: `src/backends/display/xrandr.rs` (inline unit tests)

**Interfaces:**
- Consumes: `DisplayBackend`, `Monitor` from `src/backends/mod.rs`
- Produces: `XrandrBackend` struct implementing `DisplayBackend`

- [ ] **Step 1: Write unit tests for `xrandr --query` text parsing**
- [ ] **Step 2: Implement `XrandrBackend`**
  - Parse connected screens, current modes, refresh rates, available resolutions
  - Implement `set_resolution(name, w, h, refresh)` via `xrandr --output <name> --mode <w>x<h> --rate <refresh>`
  - Implement `set_scale(name, scale)` via `xrandr --output <name> --scale <scale>x<scale>`
  - Implement `set_monitor_mode(name, mode, mirror)` via `xrandr`
- [ ] **Step 3: Run tests and verify they pass**
- [ ] **Step 4: Commit**

---

### Task 3: DRM Sysfs Universal Hardware Display Backend (`/sys/class/drm`)

**Files:**
- Create: `src/backends/display/drm.rs`
- Modify: `src/backends/display/mod.rs`
- Test: `src/backends/display/drm.rs` (inline unit tests)

**Interfaces:**
- Consumes: `DisplayBackend`, `Monitor` from `src/backends/mod.rs`
- Produces: `DrmSysfsBackend` struct implementing `DisplayBackend`

- [ ] **Step 1: Write unit tests for DRM sysfs path parsing and mode reading**
- [ ] **Step 2: Implement `DrmSysfsBackend`**
  - Read `/sys/class/drm/card*-<connector>/status` to detect connected displays
  - Read `/sys/class/drm/card*-<connector>/modes` to discover native and supported resolutions
  - Parse connector names (`eDP-1`, `HDMI-A-1`, `DP-1`)
- [ ] **Step 3: Run tests and verify they pass**
- [ ] **Step 4: Commit**

---

### Task 4: Unified Display Factory (`create_display_backend`) & Capability Integration

**Files:**
- Modify: `src/backends/display/mod.rs`
- Modify: `src/platform/mod.rs`
- Modify: `src/app/mod.rs`
- Modify: `src/lib.rs`

**Interfaces:**
- Consumes: `HyprlandBackend`, `WlrRandrBackend`, `XrandrBackend`, `DrmSysfsBackend`
- Produces: `create_display_backend() -> Box<dyn DisplayBackend>`

- [ ] **Step 1: Wire display backend factory with priority detection**
  1. Hyprland (if running and `hyprctl` available)
  2. `WlrRandrBackend` (if Wayland and `wlr-randr` available)
  3. `XrandrBackend` (if `xrandr` available and `DISPLAY` or X11 session active)
  4. `DrmSysfsBackend` (universal Linux hardware fallback)
- [ ] **Step 2: Update `PlatformCapabilities::detect` to accurately report the detected display backend instead of hardcoded Hyprland check**
- [ ] **Step 3: Update `src/app/mod.rs` to use `create_display_backend()`**
- [ ] **Step 4: Update `src/lib.rs` diagnose output**
- [ ] **Step 5: Run tests and clippy**
- [ ] **Step 6: Commit**

---

### Task 5: Sway & Generic Linux Input Backends

**Files:**
- Modify: `src/backends/input/mod.rs`

**Interfaces:**
- Consumes: `InputBackend`, `InputSettings`
- Produces: `SwayInputBackend`, `GenericInputBackend`

- [ ] **Step 1: Write unit tests for Sway input JSON parsing**
- [ ] **Step 2: Implement `SwayInputBackend` using `swaymsg`**
- [ ] **Step 3: Implement `GenericInputBackend` using `/proc/bus/input/devices` and sysfs**
- [ ] **Step 4: Update `create_input_backend()` to detect Sway and Generic backends**
- [ ] **Step 5: Run tests and clippy**
- [ ] **Step 6: Commit**

---

### Task 6: Multi-Vendor Battery Thresholds & Universal Power/Idle Management

**Files:**
- Modify: `src/backends/power/mod.rs`

**Interfaces:**
- Consumes: `PowerBackend`, `PowerInfo`

- [ ] **Step 1: Write unit tests for multi-vendor battery threshold paths**
- [ ] **Step 2: Generalize battery threshold detection in `get_info()` and `set_charge_limit()`**
  - Standard Linux 5.4+ `charge_control_end_threshold` for all `type == "Battery"` supplies
  - Lenovo IdeaPad `conservation_mode`
  - Huawei `charge_thresholds`
  - Samsung `battery_life_extender`
- [ ] **Step 3: Guard `hypridle` actions behind `is_hyprland_running()` and support generic `systemd-logind` / GNOME / X11 idle mechanisms**
- [ ] **Step 4: Run tests and clippy**
- [ ] **Step 5: Commit**

---

### Task 7: Full Verification & Formatting

- [ ] **Step 1: Run `cargo fmt --check`**
- [ ] **Step 2: Run `cargo clippy --all-targets -- -D warnings`**
- [ ] **Step 3: Run `cargo test`**
- [ ] **Step 4: Test `--diagnose` flag to verify capabilities output**
