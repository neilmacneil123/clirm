# clirm

**clirm** is a Rust CLI + TUI for Windows that mirrors [Bulk Crap Uninstaller](https://www.bcuninstaller.com/): multi-source app detection, filtering, bulk/quiet uninstall, leftover cleanup, startup management, and headless automation.

## Phase 1 (current)

- Registry-based application scanner (HKLM/HKCU + WOW6432Node)
- Ratatui TUI with scrollable app list, search filter, and detail pane
- Single-application uninstall
- Config file at `%APPDATA%\clirm\config.toml`
- Administrator elevation check

## Requirements

- Windows 10/11
- Rust 1.75+

## Build

```powershell
cargo build --release
```

## Run

```powershell
cargo run
```

Run as Administrator for uninstall operations.

## TUI keybindings

| Key | Action |
|-----|--------|
| `j` / `Down` | Move selection down |
| `k` / `Up` | Move selection up |
| `/` | Enter search/filter mode |
| `x` / `Enter` | Uninstall selected application |
| `r` | Refresh application list |
| `q` / `Esc` | Quit |

## Configuration

On first run, clirm creates `%APPDATA%\clirm\config.toml`:

```toml
show_system_components = false
default_sort = "Name"
```

## Project layout

```
src/
├── main.rs
├── models.rs
├── config.rs
├── scanner/
│   ├── mod.rs
│   └── registry.rs
├── tui/
│   ├── mod.rs
│   ├── apps_view.rs
│   ├── details.rs
│   └── handler.rs
└── uninstall/
    ├── mod.rs
    └── engine.rs
```

## License

MIT