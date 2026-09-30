use super::{AudioBackend, AudioDevice, AudioStream};
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde_json::Value;
use std::process::Command;

pub struct WpctlBackend;

impl WpctlBackend {
    pub fn new() -> Self {
        Self
    }

    async fn get_pw_dump(&self) -> Result<Value> {
        let output = Command::new("pw-dump").output()?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let val: Value = serde_json::from_str(&stdout)?;
        Ok(val)
    }

    fn parse_devices(dump: &Value, is_sink_filter: bool) -> Vec<AudioDevice> {
        let mut devices = Vec::new();
        let arr = match dump.as_array() {
            Some(a) => a,
            None => return devices,
        };

        // First find defaults from Metadata
        let mut default_sink_name = String::new();
        let mut default_source_name = String::new();

        for item in arr {
            if item["type"] == "PipeWire:Interface:Metadata" && item["info"]["name"] == "default" {
                if let Some(meta) = item["metadata"].as_array() {
                    for m in meta {
                        if m["key"] == "default.audio.sink" {
                            if let Some(name) = m["value"]["name"].as_str() {
                                default_sink_name = name.to_string();
                            }
                        } else if m["key"] == "default.audio.source" {
                            if let Some(name) = m["value"]["name"].as_str() {
                                default_source_name = name.to_string();
                            }
                        }
                    }
                }
            }
        }

        // Now parse Nodes
        for item in arr {
            if item["type"] == "PipeWire:Interface:Node" {
                let props = &item["info"]["props"];
                let media_class = props["media.class"].as_str().unwrap_or("");

                let is_sink = media_class == "Audio/Sink";
                let is_source = media_class == "Audio/Source";

                if (is_sink_filter && is_sink) || (!is_sink_filter && is_source) {
                    let id = item["id"].as_u64().unwrap_or(0) as u32;
                    let name = props["node.name"].as_str().unwrap_or("Unknown").to_string();
                    let description = props["node.description"]
                        .as_str()
                        .unwrap_or("Unknown")
                        .to_string();
                    let form_factor = props["device.form.factor"]
                        .as_str()
                        .unwrap_or("Unknown")
                        .to_string();

                    // Check default
                    let is_default = if is_sink_filter {
                        name == default_sink_name
                    } else {
                        name == default_source_name
                    };

                    // Check volume & mute from Props
                    let mut volume = 1.0;
                    let mut muted = false;

                    if let Some(node_props) = item["info"]["params"]["Props"].as_array() {
                        if let Some(p) = node_props.first() {
                            if let Some(v) = p["volume"].as_f64() {
                                volume = v;
                            }
                            if let Some(m) = p["mute"].as_bool() {
                                muted = m;
                            }
                        }
                    }

                    devices.push(AudioDevice {
                        id,
                        name,
                        description,
                        is_default,
                        volume,
                        muted,
                        is_sink,
                        form_factor,
                    });
                }
            }
        }

        // Sort: defaults first, then by description
        devices.sort_by(|a, b| {
            b.is_default
                .cmp(&a.is_default)
                .then(a.description.cmp(&b.description))
        });

        devices
    }

    fn parse_streams(dump: &Value) -> Vec<AudioStream> {
        let mut streams = Vec::new();
        let arr = match dump.as_array() {
            Some(a) => a,
            None => return streams,
        };

        for item in arr {
            if item["type"] == "PipeWire:Interface:Node" {
                let props = &item["info"]["props"];
                let media_class = props["media.class"].as_str().unwrap_or("");

                let is_sink_input = media_class == "Stream/Output/Audio";
                let is_source_output = media_class == "Stream/Input/Audio";

                if is_sink_input || is_source_output {
                    let id = item["id"].as_u64().unwrap_or(0) as u32;

                    let mut app_name = props["application.name"].as_str().unwrap_or("").to_string();
                    if app_name.is_empty() {
                        app_name = props["node.name"]
                            .as_str()
                            .unwrap_or("Unknown Application")
                            .to_string();
                    }

                    let mut volume = 1.0;
                    let mut muted = false;

                    if let Some(node_props) = item["info"]["params"]["Props"].as_array() {
                        if let Some(p) = node_props.first() {
                            if let Some(v) = p["volume"].as_f64() {
                                volume = v;
                            }
                            if let Some(m) = p["mute"].as_bool() {
                                muted = m;
                            }
                        }
                    }

                    streams.push(AudioStream {
                        id,
                        application_name: app_name,
                        volume,
                        muted,
                        is_sink_input,
                    });
                }
            }
        }

        streams.sort_by(|a, b| a.application_name.cmp(&b.application_name));
        streams
    }
}

#[async_trait]
impl AudioBackend for WpctlBackend {
    async fn get_sinks(&self) -> Result<Vec<AudioDevice>> {
        let dump = self.get_pw_dump().await?;
        Ok(Self::parse_devices(&dump, true))
    }

    async fn get_sources(&self) -> Result<Vec<AudioDevice>> {
        let dump = self.get_pw_dump().await?;
        Ok(Self::parse_devices(&dump, false))
    }

    async fn get_streams(&self) -> Result<Vec<AudioStream>> {
        let dump = self.get_pw_dump().await?;
        Ok(Self::parse_streams(&dump))
    }

    async fn get_volume(&self, id: u32) -> Result<(f64, bool)> {
        let output = Command::new("wpctl")
            .arg("get-volume")
            .arg(id.to_string())
            .output()?;
        let text = String::from_utf8_lossy(&output.stdout);
        let muted = text.contains("[MUTED]");
        let vol = text
            .split_whitespace()
            .nth(1)
            .and_then(|v| v.parse::<f64>().ok())
            .unwrap_or(0.0);
        Ok((vol, muted))
    }

    async fn set_default_sink(&self, id: u32) -> Result<()> {
        let status = Command::new("wpctl")
            .arg("set-default")
            .arg(id.to_string())
            .status()?;
        if status.success() {
            Ok(())
        } else {
            Err(anyhow!("wpctl set-default failed"))
        }
    }

    async fn set_volume(&self, id: u32, volume: f64) -> Result<()> {
        let vol_str = format!("{:.2}", volume);
        let status = Command::new("wpctl")
            .arg("set-volume")
            .arg(id.to_string())
            .arg(&vol_str)
            .status()?;
        if status.success() {
            Ok(())
        } else {
            Err(anyhow!("wpctl set-volume failed"))
        }
    }

    async fn set_mute(&self, id: u32, mute: bool) -> Result<()> {
        let arg = if mute { "1" } else { "0" };
        let status = Command::new("wpctl")
            .arg("set-mute")
            .arg(id.to_string())
            .arg(arg)
            .status()?;
        if status.success() {
            Ok(())
        } else {
            Err(anyhow!("wpctl set-mute failed"))
        }
    }

    async fn toggle_mute(&self, id: u32) -> Result<()> {
        let status = Command::new("wpctl")
            .arg("set-mute")
            .arg(id.to_string())
            .arg("toggle")
            .status()?;
        if status.success() {
            Ok(())
        } else {
            Err(anyhow!("wpctl set-mute toggle failed"))
        }
    }
}
