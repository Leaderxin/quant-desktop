# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build & Run

```bash
# Install dependencies
npm install

# Run in development mode (starts Vite dev server, then Tauri)
npm run tauri dev

# Build for production (cross-platform: .exe on Windows, .dmg on macOS, .deb/.AppImage on Linux)
npm run tauri:build

# Type-check the frontend
npx vue-tsc --noEmit

# Build Rust backend only
cargo build --manifest-path src-tauri/Cargo.toml
```

There are no dedicated test or lint commands configured yet. `vue-tsc` with the strict tsconfig enforces type correctness; `cargo build` compiles the Rust side.

## Architecture

QuantDesktop is a **Tauri 2 desktop app** for monitoring Chinese A-share stock markets. It has two separate webview windows driven by two Vite/Vue entry points, a SQLite-backed Rust backend, and a pluggable data source layer for fetching market data.

### Two-window layout

| Window | Label | Entry | Config |
|--------|-------|-------|--------|
| Main UI | `main` | `index.html` → `src/main.ts` | 1100×680, starts hidden, hides to tray instead of closing |
| Ticker bar | `ticker` | `ticker.html` → `src/ticker.ts` | 250×38, always-on-top, decorationless, positioned bottom-right |

Vite is configured with two Rollup inputs (`index.html` + `ticker.html`) in [vite.config.ts](vite.config.ts). Each entry mounts a separate Vue app with its own Pinia instance. Both share the same stores and composables via import.

### Rust backend (`src-tauri/src/`)

**`lib.rs`** — Application setup. Initializes SQLite database, registers data source adapters (Sina first as default, then Eastmoney), restores quote cache from DB, spawns the background polling `Scheduler`, builds the system tray menu, and registers all Tauri IPC commands. The main window's `CloseRequested` event is intercepted to hide instead of quit.

**`domain/mod.rs`** — Shared data types: `Quote`, `IndexQuote`, `StockBrief`, `Market` enum (CN/HK/US). These are serialized across the IPC boundary to TypeScript types in [src/types/index.ts](src/types/index.ts).

**`db/mod.rs`** — SQLite database (via `rusqlite` with bundled SQLite). Three tables: `watchlist`, `settings` (key-value), `quote_cache`. Database is stored at `{app_data_dir}/quant-desktop.db`. Auto-creates tables and default settings on first open.

**`datasource/mod.rs`** — Pluggable data source architecture. The `DataSource` trait defines `fetch_realtime()`, `fetch_indices()`, `search()`, `health_check()`. `DataSourceManager` holds a registry of adapters and an `active` name, supporting runtime switching. First registered source becomes the default.
- `sina.rs` — Sina Finance API. GBK-encoded responses, parsed from `var hq_str_xxx="..."` format. Handles code-to-exchange mapping (sh/sz prefix). Search only supports exact 6-digit code lookup.
- `eastmoney.rs` — Eastmoney (东方财富) API. JSON responses with secid format (`1.600519` for Shanghai, `0.000001` for Shenzhen). Search uses the suggest API. Falls back to Eastmoney for stock search when active source returns empty.

**`cache/mod.rs`** — `QuoteCache` provides in-memory `HashMap` storage with SQLite dual-write persistence. `Scheduler` spawns a `tokio` background task that polls the active data source on a configurable interval (default 3s), groups watchlist codes by market, fetches batch quotes, updates the cache, and emits `quotes-updated` and `indices-updated` Tauri events to the frontend. On fetch failure, it falls back to cached data.

**`commands/`** — Tauri IPC command handlers. Each file exposes `#[tauri::command]` functions that are registered in `lib.rs`:
- `quote.rs` — `get_quotes`, `get_indices` (read from cache)
- `watchlist.rs` — CRUD + reorder/move operations on watchlist
- `settings.rs` — `get_settings`, `set_setting`, `switch_datasource`, `list_datasources`
- `window.rs` — `show_main_window` (restore from tray)

### Frontend (`src/`)

**Stores (Pinia)** — Three stores mirroring the backend state:
- `quote.ts` — Listens to `quotes-updated` and `indices-updated` Tauri events. Quotes stored in a `Map<"market:code", Quote>` for O(1) lookup.
- `watchlist.ts` — Calls Tauri `invoke()` commands for CRUD. Re-fetches full list after mutations.
- `settings.ts` — Key-value settings map. Manages theme toggle (dark/light on `<html data-theme>`) and data source switching.

**Component hierarchy (main window)**:
```
App.vue → NConfigProvider + NMessageProvider
  └─ AppLayout.vue
       ├─ TopBar.vue (brand, active datasource tag, theme toggle)
       ├─ IndexBar.vue → IndexCard.vue × N (market indices from quote store)
       └─ WatchlistTable.vue (NDataTable with context menu)
            └─ AddStockDialog.vue (search + add modal)
```

**Ticker bar** ([TickerBar.vue](src/components/ticker/TickerBar.vue)) — Standalone mini Vue app that polls watchlist + settings, listens to quote events, and cycles through stocks two at a time with 3-second auto-scroll. Pauses on hover. Clicking restores the main window. Also polls settings every 1s to sync theme changes.

**Composables**:
- `useTauriEvent.ts` — Vue lifecycle wrapper for `listen()` (auto-cleanup)
- `useTheme.ts` — Standalone theme state (used by ticker)

### Data flow

```
DataSource API (Sina/Eastmoney)
  → Scheduler (tokio background poll)
    → QuoteCache (in-memory + SQLite)
      → app_handle.emit("quotes-updated")
        → useQuoteStore (Tauri event listener)
          → Vue reactive components
```

User mutations (add/remove/reorder watchlist) go through `invoke()` → Rust commands → SQLite, then the frontend re-fetches the watchlist. The scheduler picks up changes on the next poll cycle.

### Key dependencies

- **Rust**: `tauri` v2 (with tray-icon feature), `rusqlite` (bundled), `reqwest` (rustls-tls), `tokio` (full), `chrono`, `serde`/`serde_json`, `encoding` (GBK decoding for Sina)
- **Frontend**: `vue` 3, `pinia`, `naive-ui`, `@tauri-apps/api`, `vite`, `vue-tsc`, `klinecharts`

### Default settings (auto-inserted on first run)

| Key | Default | Description |
|-----|---------|-------------|
| `active_datasource` | `sina` | Active market data provider (forced to sina on every startup) |
| `refresh_interval` | `3` | Polling interval in seconds |
| `theme` | `dark` | UI theme |
| `ticker_visible` | `true` | Ticker bar visibility |
