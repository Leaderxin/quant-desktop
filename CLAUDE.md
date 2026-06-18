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

# Alternative: cross-platform build with proxy auto-detect (see scripts/build.mjs)
node scripts/build.mjs

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
| Main UI | `main` | `index.html` → `src/main.ts` | 1100×680, starts hidden, hides to tray instead of closing, position/size persisted to SQLite |
| Ticker bar | `ticker` | `ticker.html` → `src/ticker.ts` | 230×38, always-on-top, decorationless, positioned bottom-right, skip-taskbar |

Vite is configured with two Rollup inputs (`index.html` + `ticker.html`) in [vite.config.ts](vite.config.ts). Each entry mounts a separate Vue app with its own Pinia instance. Both share the same stores and composables via import.

### Rust backend (`src-tauri/src/`)

**`lib.rs`** — Application setup. Initializes SQLite database, registers data source adapters (Sina first as default, then Tencent), restores quote cache from DB, spawns the background polling `Scheduler` (with dynamic interval based on trading session), builds the system tray menu (left-click toggle, right-click menu with show/toggle-ticker/quit), and registers all Tauri IPC commands. The main window's `CloseRequested` event is intercepted to hide instead of quit. Window position/size is saved to SQLite and restored on next launch with monitor-boundary validation.

**`domain/mod.rs`** — Shared data types serialized across the IPC boundary to TypeScript types in [src/types/index.ts](src/types/index.ts):
- `Market` enum (CN/HK/US)
- `Quote` — real-time quote with price, change, open/high/low, volume, turnover, turnover_rate
- `IndexQuote` — index-level quote
- `Depth` — 5-level bid/ask depth (bids: `Vec<Level>`, asks: `Vec<Level>`)
- `Level` — single depth level (price + volume)
- `MinuteData` — intraday minute bar (time, price, open, high, low, volume, avg_price)
- `StockBrief` — minimal stock identifier for search results

**`db/mod.rs`** — SQLite database (via `rusqlite` with bundled SQLite). Three tables: `watchlist`, `settings` (key-value), `quote_cache`. Database is stored at `{app_data_dir}/quant-desktop.db`. Auto-creates tables and default settings on first open. `init_defaults()` forces `active_datasource` to `sina` on every startup as a safety reset.

**`datasource/mod.rs`** — Pluggable data source architecture. The `DataSource` trait defines `fetch_realtime()`, `fetch_indices()`, `search()`, `fetch_depth()`, `fetch_minute_data()`, `health_check()`. `DataSourceManager` holds a registry of adapters and an `active` name, supporting runtime switching. A `tokio::sync::Notify` wakeup mechanism triggers immediate refresh on data source switch.
- `sina.rs` — Sina Finance (新浪财经) adapter, the **default** data source. GBK-encoded responses, parsed from `var hq_str_xxx="..."` format. Handles code-to-exchange mapping (sh/sz prefix). Search only supports exact 6-digit code lookup. Depth data fetched via Tencent API fallback (Sina's native depth endpoint is dead). Minute data from `money.finance.sina.com.cn` 5-min K-line endpoint. Covers 7 major indices.
- `tencent.rs` — Tencent Securities (腾讯证券) adapter, the backup source. GBK-encoded responses, `v_sh600519="..."` format with `~` separators. Full implementation: realtime quotes, 7 indices, exact-code search, minute data via `ifzq.gtimg.cn`, and depth from embedded bid/ask fields (positions 9-28). Volume converted from 手 to 股 (×100).
- `market_clock.rs` — Trading session detection. `MarketSession` enum: PreOpen/MorningTrade/LunchBreak/AfternoonTrade/Closed with weekend detection. `recommended_interval()`: 2s trading, 5s pre-open, 10s lunch, 30s closed. Scheduler uses this to dynamically adjust polling frequency.

**`cache/mod.rs`** — `QuoteCache` provides in-memory `HashMap` storage with SQLite dual-write persistence. `restore_from_db()` on startup for instant quote display. `Scheduler` spawns a `tokio` background task that polls the active data source with dynamic interval (via `market_clock`), groups watchlist codes by market, fetches batch quotes, updates the cache, and emits three Tauri events: `quotes-updated`, `indices-updated`, and `market-session-changed`. A separate wakeup listener task triggers immediate refresh on data source switch. On fetch failure, falls back to cached data.

**`commands/`** — Tauri IPC command handlers. Each file exposes `#[tauri::command]` functions registered in `lib.rs`:
- `quote.rs` — `get_quotes`, `get_indices` (read from cache), `get_depth`, `get_intraday` (async via active data source)
- `watchlist.rs` — CRUD + reorder/move operations (`get_watchlist`, `add_watch`, `remove_watch`, `reorder_watch`, `move_watch_top`, `move_watch_up`, `move_watch_down`), `search_stocks` (with cross-source fallback: if active source returns empty, tries alternate source)
- `settings.rs` — `get_settings`, `set_setting`, `switch_datasource`, `list_datasources`
- `window.rs` — `show_main_window` (restore from tray)

### Frontend (`src/`)

**Stores (Pinia)** — Three stores mirroring the backend state:
- `quote.ts` — Listens to `quotes-updated` and `indices-updated` Tauri events. Quotes stored in a `Map<"market:code", Quote>` for O(1) lookup.
- `watchlist.ts` — Calls Tauri `invoke()` commands for CRUD. Re-fetches full list after mutations.
- `settings.ts` — Key-value settings map. Manages theme toggle (dark/light on `<html data-theme>`) and data source switching. Emits `theme-changed` and `datasource-changed` events for cross-window sync.

**Component hierarchy (main window)**:
```
App.vue → NConfigProvider + NMessageProvider (theme overrides, accent=blue)
  └─ AppLayout.vue
       ├─ TopBar.vue (brand SVG, data source dropdown, theme toggle)
       ├─ IndexBar.vue → IndexCard.vue × N (market indices from quote store)
       └─ WatchlistTable.vue (NDataTable: sortable columns, right-click context menu, row-click expands detail)
            ├─ AddStockDialog.vue (search with 300ms debounce + add modal)
            └─ StockDetail.vue (expanded row detail panel)
                 ├─ MinuteChart.vue (intraday chart via klinecharts lib)
                 ├─ DepthPanel.vue (5-level bid/ask display)
                 └─ StockSummary.vue (open/high/low/volume/turnover/turnover_rate)
```

**Ticker bar** ([TickerBar.vue](src/components/ticker/TickerBar.vue)) — Standalone mini Vue app that polls watchlist + settings, listens to quote events, and cycles through stocks two at a time with 3-second auto-scroll. Pauses on hover. Clicking restores the main window. Polls settings every 1s to sync theme changes.

**Composables**:
- `useTauriEvent.ts` — Vue lifecycle wrapper for `listen()` (auto-cleanup on unmount)
- `useTheme.ts` — Standalone theme state (used by ticker)

**Styles**:
- `variables.css` — Design system tokens: 4 surface levels, border system, text palette, semantic up/down colors (red=up, green=down per A-share convention), monospace font for numbers (tabular-nums), 4px-base spacing scale, radius tokens, shadow tokens, dark + light theme overrides
- `dark.css` — Scrollbar theming

### Data flow

```
DataSource API (Sina/Tencent)
  → Scheduler (tokio background poll, dynamic interval via market_clock)
    → QuoteCache (in-memory HashMap + SQLite dual-write)
      → app_handle.emit("quotes-updated" / "indices-updated" / "market-session-changed")
        → Pinia Stores (Tauri event listener)
          → Vue reactive components (main window + ticker bar)
```

On-demand requests (depth, minute data) go through `invoke("get_depth")` / `invoke("get_intraday")` → active DataSource adapter → returned directly to the frontend detail panel.

User mutations (add/remove/reorder watchlist) go through `invoke()` → Rust commands → SQLite, then the frontend re-fetches the watchlist. The scheduler picks up changes on the next poll cycle.

DataSource switching triggers a `Notify` wakeup → Scheduler immediately refreshes with the new adapter.

### Key dependencies

- **Rust**: `tauri` v2 (with tray-icon feature), `rusqlite` (bundled), `reqwest` (rustls-tls), `tokio` (full), `chrono`, `serde`/`serde_json`, `encoding` (GBK decoding), `async-trait`
- **Frontend**: `vue` 3, `pinia`, `naive-ui`, `@tauri-apps/api`, `@tauri-apps/plugin-opener`, `vite`, `vue-tsc`, `klinecharts` (v10 beta)

### Default settings (auto-inserted on first run)

| Key | Default | Description |
|-----|---------|-------------|
| `active_datasource` | `sina` | Active market data provider (forced to sina on every startup) |
| `refresh_interval` | `3` | Polling interval in seconds (overridden by market_clock dynamically) |
| `theme` | `dark` | UI theme (dark/light) |
| `ticker_visible` | `true` | Ticker bar visibility |

### Window position persistence

Main window position/size is saved to SQLite `settings` table on move/resize/close. Keys: `window_x`, `window_y`, `window_width`, `window_height`. On next launch, restored with monitor-boundary validation (clamped to visible area if monitor config changed). Ticker window is always positioned at bottom-right via `available_monitors()` calculation.

## Development phases

| Phase | Status | Scope |
|-------|--------|-------|
| Phase 1 (MVP) | ✅ Complete | Scaffold, Sina adapter, tray, ticker, watchlist CRUD, index bar, dark theme |
| Phase 2 (Experience) | ✅ Complete | Detail panel (minute chart + depth + summary), Tencent adapter, column sorting, window position memory, market_clock dynamic polling |
| Phase 3 (Enhancement) | 📋 Planned | K-line chart (daily/weekly/monthly) + technical indicators (MA/BOLL/MACD), price alerts, import/export (JSON/CSV), auto-start, packaging polish |
| Phase 4 (Extension) | 🔮 Future | HK/US market support, professional data sources (Wind/Tushare), auto-update, macOS/Linux adaptation |

## CI/CD

GitHub Actions workflow at [.github/workflows/release.yml](.github/workflows/release.yml) — triggered on `v*` tags or manual dispatch. Matrix build for Windows (MSVC), macOS (universal), Linux (gnu). Uploads `.exe`/`.msi`/`.dmg`/`.deb`/`.AppImage` artifacts.

`scripts/build.mjs` provides a cross-platform build wrapper with automatic proxy detection (Clash/V2Ray on common ports 7890/10809/1080/8118/8080/1087/4780).
