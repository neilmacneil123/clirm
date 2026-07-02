# clirm: BCUninstaller-style CLI TUI

## Overview

**clirm** is a Rust CLI + TUI for Windows that mirrors Bulk Crap Uninstaller: multi-source app detection, filtering, bulk/quiet uninstall, leftover cleanup, startup management, and headless automation.

- **Stack**: Rust, ratatui, crossterm, tokio, winreg, windows crate
- **Platform**: Windows 10/11 only
- **Binary**: Single `clirm.exe` with TUI (default) and CLI subcommands

## Architecture

```
clirm/
├── src/
│   ├── main.rs              # clap: TUI default, CLI subcommands
│   ├── cli/                 # list, export, uninstall, scan
│   ├── models.rs            # ApplicationEntry, StartupEntry, JunkItem
│   ├── config.rs
│   ├── filter/              # presets, regex, bcul import
│   ├── protection.rs
│   ├── ratings.rs
│   ├── export.rs
│   ├── scanner/             # orchestrator + providers
│   ├── uninstall/           # engine, quiet, force, queue
│   ├── junk/
│   ├── startup/
│   └── tui/                 # app state, views, dialogs, handler
├── assets/ratings.json
└── README.md
```

## PR Plan

### PR 1: Phase 1 scaffold and core TUI loop

- **Description:** Initialize Cargo project with module structure, config loading, registry-based app scanner, basic ratatui TUI (list, search, detail pane, single uninstall), and admin elevation check.
- **Files/components affected:** Cargo.toml, src/main.rs, src/models.rs, src/config.rs, src/scanner/mod.rs, src/scanner/registry.rs, src/tui/mod.rs, src/tui/apps_view.rs, src/tui/details.rs, src/tui/handler.rs, src/uninstall/mod.rs, src/uninstall/engine.rs, README.md
- **Dependencies:** None

### PR 2: Phase 2 multi-source detection and bulk operations

- **Description:** Add Store, Portable, Steam, and Chocolatey providers; multi-select and batch uninstall queue; filter presets and regex search; protection rules; CSV/JSON export.
- **Files/components affected:** src/scanner/store.rs, src/scanner/portable.rs, src/scanner/steam.rs, src/scanner/chocolatey.rs, src/filter/mod.rs, src/protection.rs, src/export.rs, src/tui/dialogs.rs, src/uninstall/queue.rs
- **Dependencies:** PR 1

### PR 3: Phase 3 advanced uninstall and leftovers

- **Description:** Quiet uninstall engine with type-specific flags, basic GUI automation for loud uninstallers, junk scanner, leftovers view, and force uninstall.
- **Files/components affected:** src/uninstall/quiet.rs, src/uninstall/force.rs, src/junk/mod.rs, src/junk/scanner.rs, src/tui/leftovers_view.rs
- **Dependencies:** PR 2

### PR 4: Phase 4 full parity and headless CLI

- **Description:** Windows Features and Updates providers, startup manager view, ratings heuristics with bundled DB, headless CLI (list, export, uninstall) with BCU-compatible flags, .bcul import support.
- **Files/components affected:** src/scanner/features.rs, src/scanner/updates.rs, src/startup/mod.rs, src/ratings.rs, src/cli/mod.rs, src/filter/bcul.rs, assets/ratings.json
- **Dependencies:** PR 3