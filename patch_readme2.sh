sed -i '/---/!b;//!d;/^## ⚡ Why settings-tui?/i \
## 🪶 Hyper-Lightweight Performance\
\
`settings-tui` is engineered specifically for minimalists, tiling window manager users, and performance purists who refuse to install heavy GUI toolkits just to change a setting.\
\
* **Near 0% Resource Footprint**: Idles at essentially 0% CPU and consumes single-digit Megabytes of RAM.\
* **Zero Background Daemons**: `settings-tui` only runs when you open it. It does not spawn background processes or memory-leaking tracking daemons.\
* **<9 MB Binary**: Compiles down to a tiny, statically linked executable (when stripped) with zero dynamic dependencies on heavy frameworks like GTK, Qt, or Electron.\
* **Instant Sub-15ms Boot**: Launches instantly and immediately populates real-time system data using asynchronous Linux APIs (D-Bus, sysfs).\
\
---\
' README.md
