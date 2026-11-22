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
}

impl Default for WpctlBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl WpctlBackend {
    async fn get_pw_dump(&self) -> Result<Value> {
        let output = Command::new("pw-dump").output()?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let val: Value = serde_json::from_str(&stdout)?;
        Ok(val)
    }

    pub fn parse_all_audio_state(
        dump: &Value,
    ) -> (Vec<AudioDevice>, Vec<AudioDevice>, Vec<AudioStream>) {
        let mut sinks = Vec::new();
        let mut sources = Vec::new();
        let mut streams = Vec::new();

        let arr = match dump.as_array() {
            Some(a) => a,
            None => return (sinks, sources, streams),
        };

        // First find defaults from Metadata and build node description map
        let mut default_sink_name = String::new();
        let mut default_source_name = String::new();
        let mut node_descriptions = std::collections::HashMap::new();

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
            } else if item["type"] == "PipeWire:Interface:Node" {
                if let Some(nid) = item["id"].as_u64() {
                    let desc = item["info"]["props"]["node.description"]
                        .as_str()
                        .or_else(|| item["info"]["props"]["node.name"].as_str())
                        .unwrap_or("Unknown Device");
                    node_descriptions.insert(nid as u32, desc.to_string());
                }
            }
        }

        // Now parse Nodes (devices and streams)
        for item in arr {
            if item["type"] == "PipeWire:Interface:Node" {
                let props = &item["info"]["props"];
                let media_class = props["media.class"].as_str().unwrap_or("");

                let is_sink = media_class == "Audio/Sink";
                let is_source = media_class == "Audio/Source";

                if is_sink || is_source {
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

                    let codec = props["api.bluez5.codec"]
                        .as_str()
                        .or_else(|| props["bluetooth.codec"].as_str())
                        .map(|c| {
                            match c.to_lowercase().as_str() {
                                "ldac" => "LDAC",
                                "aptx_hd" | "aptx-hd" => "aptX-HD",
                                "aptx" => "aptX",
                                "aptx_ll" | "aptx-ll" => "aptX-LL",
                                "aac" => "AAC",
                                "sbc_xq" | "sbc-xq" => "SBC-XQ",
                                "sbc" => "SBC",
                                "opus" => "Opus",
                                "lc3" => "LC3",
                                other => other,
                            }
                            .to_string()
                        });

                    let is_default = if is_sink {
                        name == default_sink_name
                    } else {
                        name == default_source_name
                    };

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

                    let dev = AudioDevice {
                        id,
                        name,
                        description,
                        is_default,
                        volume,
                        muted,
                        is_sink,
                        form_factor,
                        codec,
                    };

                    if is_sink {
                        sinks.push(dev);
                    } else {
                        sources.push(dev);
                    }
                }

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

                    let device_id = props["target.node"]
                        .as_u64()
                        .or_else(|| props["node.target"].as_u64())
                        .or_else(|| {
                            props["target.node"]
                                .as_str()
                                .and_then(|s| s.parse::<u64>().ok())
                        })
                        .map(|v| v as u32);

                    let device_name =
                        device_id.and_then(|did| node_descriptions.get(&did).cloned());

                    streams.push(AudioStream {
                        id,
                        application_name: app_name,
                        volume,
                        muted,
                        is_sink_input,
                        device_id,
                        device_name,
                    });
                }
            }
        }

        sinks.sort_by(|a, b| {
            b.is_default
                .cmp(&a.is_default)
                .then(a.description.cmp(&b.description))
        });
        sources.sort_by(|a, b| {
            b.is_default
                .cmp(&a.is_default)
                .then(a.description.cmp(&b.description))
        });
        streams.sort_by(|a, b| a.application_name.cmp(&b.application_name));

        (sinks, sources, streams)
    }

    pub fn parse_devices(dump: &Value, is_sink_filter: bool) -> Vec<AudioDevice> {
        let (sinks, sources, _) = Self::parse_all_audio_state(dump);
        if is_sink_filter {
            sinks
        } else {
            sources
        }
    }

    pub fn parse_streams(dump: &Value) -> Vec<AudioStream> {
        let (_, _, streams) = Self::parse_all_audio_state(dump);
        streams
    }
}

#[async_trait]
impl AudioBackend for WpctlBackend {
    async fn get_all_audio_state(
        &self,
    ) -> Result<(Vec<AudioDevice>, Vec<AudioDevice>, Vec<AudioStream>)> {
        let dump = self.get_pw_dump().await?;
        Ok(Self::parse_all_audio_state(&dump))
    }

    async fn get_sinks(&self) -> Result<Vec<AudioDevice>> {
        let (sinks, _, _) = self.get_all_audio_state().await?;
        Ok(sinks)
    }

    async fn get_sources(&self) -> Result<Vec<AudioDevice>> {
        let (_, sources, _) = self.get_all_audio_state().await?;
        Ok(sources)
    }

    async fn get_streams(&self) -> Result<Vec<AudioStream>> {
        let (_, _, streams) = self.get_all_audio_state().await?;
        Ok(streams)
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

    async fn set_default_source(&self, id: u32) -> Result<()> {
        let status = Command::new("wpctl")
            .arg("set-default")
            .arg(id.to_string())
            .status()?;
        if status.success() {
            Ok(())
        } else {
            Err(anyhow!("wpctl set-default source failed"))
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

    async fn move_stream(&self, stream_id: u32, device_id: u32) -> Result<()> {
        // Try WirePlumber/PipeWire metadata target.node assignment first
        let status = Command::new("pw-metadata")
            .arg("-n")
            .arg("default")
            .arg(stream_id.to_string())
            .arg("target.node")
            .arg(device_id.to_string())
            .status();

        if let Ok(s) = status {
            if s.success() {
                return Ok(());
            }
        }

        // Fallback to pactl move-sink-input / move-source-output
        let pactl_status = Command::new("pactl")
            .arg("move-sink-input")
            .arg(stream_id.to_string())
            .arg(device_id.to_string())
            .status();

        if let Ok(ps) = pactl_status {
            if ps.success() {
                return Ok(());
            }
        }

        Err(anyhow!(
            "Failed to route audio stream {} to device {}",
            stream_id,
            device_id
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_parse_all_audio_state() {
        let dump = json!([
            {
                "type": "PipeWire:Interface:Metadata",
                "info": { "name": "default" },
                "metadata": [
                    {
                        "key": "default.audio.sink",
                        "value": { "name": "alsa_output.pci-0000_00_1f.3.analog-stereo" }
                    },
                    {
                        "key": "default.audio.source",
                        "value": { "name": "alsa_input.pci-0000_00_1f.3.analog-stereo" }
                    }
                ]
            },
            {
                "id": 42,
                "type": "PipeWire:Interface:Node",
                "info": {
                    "props": {
                        "media.class": "Audio/Sink",
                        "node.name": "alsa_output.pci-0000_00_1f.3.analog-stereo",
                        "node.description": "Built-in Audio Analog Stereo",
                        "device.form.factor": "internal"
                    },
                    "params": {
                        "Props": [
                            {
                                "volume": 0.75,
                                "mute": false
                            }
                        ]
                    }
                }
            },
            {
                "id": 43,
                "type": "PipeWire:Interface:Node",
                "info": {
                    "props": {
                        "media.class": "Audio/Source",
                        "node.name": "alsa_input.pci-0000_00_1f.3.analog-stereo",
                        "node.description": "Built-in Microphone",
                        "device.form.factor": "internal"
                    },
                    "params": {
                        "Props": [
                            {
                                "volume": 0.60,
                                "mute": true
                            }
                        ]
                    }
                }
            },
            {
                "id": 101,
                "type": "PipeWire:Interface:Node",
                "info": {
                    "props": {
                        "media.class": "Stream/Output/Audio",
                        "application.name": "Firefox",
                        "node.name": "Firefox Playback",
                        "target.node": 42
                    },
                    "params": {
                        "Props": [
                            {
                                "volume": 0.80,
                                "mute": false
                            }
                        ]
                    }
                }
            }
        ]);

        let (sinks, sources, streams) = WpctlBackend::parse_all_audio_state(&dump);
        assert_eq!(sinks.len(), 1);
        assert_eq!(sinks[0].id, 42);
        assert_eq!(sinks[0].name, "alsa_output.pci-0000_00_1f.3.analog-stereo");
        assert!(sinks[0].is_default);
        assert!(sinks[0].is_sink);
        assert!((sinks[0].volume - 0.75).abs() < 1e-4);
        assert!(!sinks[0].muted);

        assert_eq!(sources.len(), 1);
        assert_eq!(sources[0].id, 43);
        assert_eq!(sources[0].name, "alsa_input.pci-0000_00_1f.3.analog-stereo");
        assert!(sources[0].is_default);
        assert!(!sources[0].is_sink);
        assert!((sources[0].volume - 0.60).abs() < 1e-4);
        assert!(sources[0].muted);

        assert_eq!(streams.len(), 1);
        assert_eq!(streams[0].id, 101);
        assert_eq!(streams[0].application_name, "Firefox");
        assert_eq!(streams[0].device_id, Some(42));
        assert_eq!(
            streams[0].device_name.as_deref(),
            Some("Built-in Audio Analog Stereo")
        );
        assert!(streams[0].is_sink_input);
        assert!(!streams[0].muted);

        // Verify parse_devices and parse_streams wrappers match
        let dev_sinks = WpctlBackend::parse_devices(&dump, true);
        assert_eq!(dev_sinks.len(), 1);
        assert_eq!(dev_sinks[0].id, 42);

        let dev_sources = WpctlBackend::parse_devices(&dump, false);
        assert_eq!(dev_sources.len(), 1);
        assert_eq!(dev_sources[0].id, 43);

        let dev_streams = WpctlBackend::parse_streams(&dump);
        assert_eq!(dev_streams.len(), 1);
        assert_eq!(dev_streams[0].id, 101);
    }
}
