use super::{
    apply_grayscale, apply_night_light, is_grayscale_active, is_night_light_active,
    read_system_brightness, write_system_brightness, DisplayBackend, Monitor,
};
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use std::process::Command;

pub struct XrandrBackend;

impl XrandrBackend {
    pub fn new() -> Result<Self> {
        let output = Command::new("xrandr").arg("--version").output();
        match output {
            Ok(o) if o.status.success() => Ok(Self),
            _ => Err(anyhow!("xrandr binary not found or not executable")),
        }
    }

    pub fn parse_xrandr_query(output_str: &str) -> Vec<Monitor> {
        let mut monitors = Vec::new();
        let mut cur_mon: Option<Monitor> = None;

        for line in output_str.lines() {
            if line.starts_with("Screen ") {
                continue;
            }

            if !line.starts_with(' ') && !line.starts_with('\t') {
                // Potential output line: e.g. "eDP-1 connected primary 1920x1080+0+0 ..."
                // or "HDMI-1 disconnected ..."
                if let Some(mon) = cur_mon.take() {
                    monitors.push(mon);
                }

                let tokens: Vec<&str> = line.split_whitespace().collect();
                if tokens.len() >= 2 && tokens[1] == "connected" {
                    let name = tokens[0].to_string();
                    let primary = tokens.get(2).map(|&t| t == "primary").unwrap_or(false);

                    let geom_idx = if primary { 3 } else { 2 };
                    let mut width = 1920;
                    let mut height = 1080;

                    if let Some(geom) = tokens.get(geom_idx) {
                        if geom.contains('x') && geom.contains('+') {
                            let res_part = geom.split('+').next().unwrap_or("");
                            let dims: Vec<&str> = res_part.split('x').collect();
                            if dims.len() == 2 {
                                width = dims[0].parse().unwrap_or(1920);
                                height = dims[1].parse().unwrap_or(1080);
                            }
                        }
                    }

                    let id = monitors.len() as i64;
                    cur_mon = Some(Monitor {
                        id,
                        name: name.clone(),
                        description: name,
                        width,
                        height,
                        refresh_rate: 60.0,
                        scale: 1.0,
                        active: true,
                        primary,
                        supported_modes: Vec::new(),
                        mirror_of: None,
                    });
                }
            } else if let Some(ref mut mon) = cur_mon {
                // Mode line: e.g. "   1920x1080     59.96*+  50.00"
                let tokens: Vec<&str> = line.split_whitespace().collect();
                if let Some(res_str) = tokens.first() {
                    if res_str.contains('x') {
                        let is_current_res = tokens.iter().any(|t| t.contains('*'));
                        for rate_token in &tokens[1..] {
                            let clean_rate = rate_token.trim_matches(|c| c == '*' || c == '+');
                            if let Ok(hz) = clean_rate.parse::<f64>() {
                                mon.supported_modes.push(format!("{}@{:.2}", res_str, hz));
                                if rate_token.contains('*') {
                                    mon.refresh_rate = hz;
                                    let dims: Vec<&str> = res_str.split('x').collect();
                                    if dims.len() == 2 {
                                        mon.width = dims[0].parse().unwrap_or(mon.width);
                                        mon.height = dims[1].parse().unwrap_or(mon.height);
                                    }
                                }
                            }
                        }
                        if is_current_res && mon.refresh_rate == 60.0 {
                            if let Some(first_rate) = tokens.get(1) {
                                let clean = first_rate.trim_matches(|c| c == '*' || c == '+');
                                if let Ok(hz) = clean.parse::<f64>() {
                                    mon.refresh_rate = hz;
                                }
                            }
                        }
                    }
                }
            }
        }

        if let Some(mon) = cur_mon {
            monitors.push(mon);
        }

        if !monitors.is_empty() && !monitors.iter().any(|m| m.primary) {
            monitors[0].primary = true;
        }

        monitors
    }
}

#[async_trait]
impl DisplayBackend for XrandrBackend {
    async fn get_monitors(&self) -> Result<Vec<Monitor>> {
        let out = Command::new("xrandr").arg("--query").output()?;
        let text = String::from_utf8_lossy(&out.stdout);
        let monitors = Self::parse_xrandr_query(&text);
        Ok(monitors)
    }

    async fn set_resolution(
        &self,
        name: &str,
        width: i32,
        height: i32,
        refresh: f64,
    ) -> Result<()> {
        let mode_arg = format!("{}x{}", width, height);
        let rate_arg = format!("{:.2}", refresh);
        let out = Command::new("xrandr")
            .arg("--output")
            .arg(name)
            .arg("--mode")
            .arg(&mode_arg)
            .arg("--rate")
            .arg(&rate_arg)
            .output()?;

        if out.status.success() {
            Ok(())
        } else {
            let err = String::from_utf8_lossy(&out.stderr);
            Err(anyhow!("xrandr failed to set resolution: {}", err.trim()))
        }
    }

    async fn set_scale(&self, name: &str, scale: f64) -> Result<()> {
        let scale_arg = format!("{:.2}x{:.2}", scale, scale);
        let out = Command::new("xrandr")
            .arg("--output")
            .arg(name)
            .arg("--scale")
            .arg(&scale_arg)
            .output()?;

        if out.status.success() {
            Ok(())
        } else {
            let err = String::from_utf8_lossy(&out.stderr);
            Err(anyhow!("xrandr failed to set scale: {}", err.trim()))
        }
    }

    async fn set_monitor_mode(
        &self,
        name: &str,
        mode: &str,
        mirror_target: Option<&str>,
    ) -> Result<()> {
        let mut cmd = Command::new("xrandr");
        cmd.arg("--output").arg(name);

        match mode {
            "disable" | "off" => {
                cmd.arg("--off");
            }
            "mirror" => {
                if let Some(target) = mirror_target {
                    cmd.arg("--same-as").arg(target).arg("--auto");
                } else {
                    return Err(anyhow!("No mirror target provided"));
                }
            }
            _ => {
                cmd.arg("--auto");
            }
        }

        let out = cmd.output()?;
        if out.status.success() {
            Ok(())
        } else {
            let err = String::from_utf8_lossy(&out.stderr);
            Err(anyhow!(
                "xrandr failed to apply monitor mode: {}",
                err.trim()
            ))
        }
    }

    async fn get_brightness(&self) -> Result<Option<u32>> {
        Ok(read_system_brightness())
    }

    async fn set_brightness(&self, percent: u32) -> Result<()> {
        write_system_brightness(percent)
    }

    async fn is_night_light_enabled(&self) -> Result<bool> {
        is_night_light_active()
    }

    async fn set_night_light_enabled(&self, enabled: bool, temperature: u32) -> Result<()> {
        apply_night_light(enabled, temperature)
    }

    async fn is_grayscale_enabled(&self) -> Result<Option<bool>> {
        is_grayscale_active()
    }

    async fn set_grayscale_enabled(&self, enabled: bool) -> Result<()> {
        apply_grayscale(enabled)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_xrandr_query() {
        let raw = r#"Screen 0: minimum 16 x 16, current 1920 x 1080, maximum 32767 x 32767
eDP-1 connected primary 1920x1080+0+0 (normal left inverted right x axis y axis) 340mm x 190mm
   1920x1080     59.96*+  48.00  
   1440x1080     59.99  
   1280x720      59.86  
HDMI-1 disconnected (normal left inverted right x axis y axis)
DP-1 connected 2560x1440+1920+0 (normal left inverted right x axis y axis) 600mm x 340mm
   2560x1440     144.00*+ 120.00   60.00  
"#;

        let monitors = XrandrBackend::parse_xrandr_query(raw);
        assert_eq!(monitors.len(), 2);

        // eDP-1
        assert_eq!(monitors[0].name, "eDP-1");
        assert!(monitors[0].primary);
        assert_eq!(monitors[0].width, 1920);
        assert_eq!(monitors[0].height, 1080);
        assert!((monitors[0].refresh_rate - 59.96).abs() < 0.01);
        assert!(monitors[0].active);
        assert_eq!(monitors[0].supported_modes.len(), 4);

        // DP-1
        assert_eq!(monitors[1].name, "DP-1");
        assert!(!monitors[1].primary);
        assert_eq!(monitors[1].width, 2560);
        assert_eq!(monitors[1].height, 1440);
        assert!((monitors[1].refresh_rate - 144.0).abs() < 0.01);
    }
}
