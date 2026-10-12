# clirm

**clirm** is a Rust CLI + TUI for Windows that mirrors [Bulk Crap Uninstaller](https://www.bcuninstaller.com/): multi-source app detection, filtering, bulk/quiet uninstall, leftover cleanup, startup management, and headless automation.

## Phase 2 (current)

- **Registry** scanner (HKLM/HKCU + WOW6432Node)
- **Microsoft Store** apps via `Get-AppxPackage`
- **Portable** app detection in configured paths
- **Steam** games via `libraryfolders.vdf` / `appmanifest_*.acf`
- **Chocolatey** packages via `choco list -lo`
- Multi-select (`Space`), select all (`a`), batch uninstall (`X`)
- Filter presets (`f`): All, Registry, Store, Orphans, Large
- Regex search toggle (`R`)
- Protection rules (`p`) persisted to `%APPDATA%\clirm\protected.toml`
- Export visible list to CSV + JSON (`e`)
- Single uninstall with confirmation (`x` → `y`/`n`)

## Requirements

- Windows 10/11
- Rust 1.75+
- PowerShell (Store app scan)
- Optional: Chocolatey, Steam

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
| `Space` | Toggle selection |
| `a` | Select/deselect all visible |
| `x` | Uninstall selected (confirm) |
| `X` | Batch uninstall selected (confirm) |
| `f` | Cycle filter preset |
| `R` | Toggle regex search |
| `e` | Export visible list (CSV + JSON) |
| `p` | Toggle protection on selected app |
| `y` / `n` | Confirm/cancel uninstall dialogs |
| `r` | Refresh application list |
| `q` / `Esc` | Quit |

## Configuration

`%APPDATA%\clirm\config.toml`:

```toml
show_system_components = false
default_sort = "Name"
scan_store = true
scan_portable = true
scan_steam = true
scan_chocolatey = true
portable_scan_paths = ["C:\\Program Files", "C:\\Program Files (x86)"]
```

## License

MIT