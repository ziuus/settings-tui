import re

with open('src/app/mod.rs', 'r') as f:
    app = f.read()

# We want to add category_max_items to impl App. We'll add it right before clamp_selection.
method_code = """
    pub fn category_max_items(&self, cat: &str) -> usize {
        match cat {
            "Network" => {
                if self.wifi_enabled {
                    self.networks.len() + 3 + self.vpns.len()
                } else {
                    3 + self.vpns.len()
                }
            }
            "Bluetooth" => {
                if self.bluetooth_powered {
                    self.bluetooth_devices.len() + 1
                } else {
                    1
                }
            }
            "Sound" => {
                self.audio_sinks.len()
                    + self.audio_sources.len()
                    + self.audio_streams.len()
                    + 1
            }
            "Services" => self.services.len(),
            "Display" => {
                let base = if self.display_brightness.is_some() {
                    1
                } else {
                    0
                } + if self.night_light_enabled.is_some() {
                    1
                } else {
                    0
                };
                base + self.monitors.len() * 3
            }
            "Power" => {
                let has_limit = self
                    .power_info
                    .as_ref()
                    .and_then(|info| info.charge_limit)
                    .is_some();
                1 + (if has_limit { 1 } else { 0 }) + 5
            }
            "Mouse & Touchpad" => 4,
            "Applications" => self.autostart_apps.len() + self.applications.len(),
            "Appearance" => 6,
            "System" => 5,
            _ => 0,
        }
    }
"""

if 'pub fn category_max_items' not in app:
    app = app.replace('pub fn clamp_selection(&mut self) {', method_code + '\n    pub fn clamp_selection(&mut self) {')

# Replace clamp_selection match block
clamp_match_pattern = r'let max_items = match cat\.as_str\(\) \{.*?\n            \};\n            if max_items == 0 \{'
clamp_replacement = 'let max_items = self.category_max_items(cat);\n            if max_items == 0 {'
app = re.sub(clamp_match_pattern, clamp_replacement, app, flags=re.DOTALL)

# Replace handle_key match block
handle_key_pattern = r'let max_items = match cat\.as_str\(\) \{.*?\n                        \};\n                        if max_items > 0'
handle_key_replacement = 'let max_items = self.category_max_items(cat);\n                        if max_items > 0'
app = re.sub(handle_key_pattern, handle_key_replacement, app, flags=re.DOTALL)

with open('src/app/mod.rs', 'w') as f:
    f.write(app)
