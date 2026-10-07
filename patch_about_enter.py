import sys

with open('src/app/mod.rs', 'r') as f:
    lines = f.readlines()

new_lines = []
for idx, line in enumerate(lines):
    new_lines.append(line)
    if 'else if cat == "System" {' in line:
        insert = """                        } else if cat == "About & Support" {
                            if is_enter {
                                let url = if self.selected_item == 0 {
                                    "https://github.com/ziuus/settings-tui"
                                } else {
                                    "https://github.com/sponsors/ziuus"
                                };
                                let _ = std::process::Command::new("xdg-open")
                                    .arg(url)
                                    .spawn();
                            }
"""
        new_lines.insert(-1, insert)

with open('src/app/mod.rs', 'w') as f:
    f.writelines(new_lines)
