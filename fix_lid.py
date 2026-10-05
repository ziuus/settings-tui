with open('src/backends/power/mod.rs', 'r') as f:
    lines = f.readlines()
for i, line in enumerate(lines):
    if 'else if l.starts_with("#HandleLidSwitch=")' in line:
        lines[i] = lines[i].replace('{', '&& lid_action == "suspend" {')
        lines[i+1] = "" # remove inner if
        lines[i+4] = "" # remove inner }
        break
with open('src/backends/power/mod.rs', 'w') as f:
    f.writelines(lines)
