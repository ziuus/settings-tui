with open('src/backends/power/mod.rs', 'r') as f:
    lines = f.readlines()
for i, line in enumerate(lines):
    if '&& power_button_action == "poweroff"' in line:
        lines[i+3] = ""
        break
with open('src/backends/power/mod.rs', 'w') as f:
    f.writelines(lines)
