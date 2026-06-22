# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build & Run

```bash
# Install dependencies (use ci for deterministic installs)
npm ci

# Run in development mode (starts Vite dev server, then Tauri)
npm run tauri dev

# Build for production (cross-platform: .exe on Windows, .dmg on macOS, .deb/.AppImage on Linux)
# Requires signing env vars for updater artifacts:
#   $env:TAURI_SIGNING_PRIVATE_KEY = Get-Content "$env:USERPROFILE\.tauri\quant-desktop.key"
#   $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = "your-password"
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

**`lib.rs`** — Application setup. Initializes SQLite database, registers data source adapters (Tencent first as default, then Sina as fallback), restores quote cache from DB, spawns the background polling `Scheduler` (with adaptive polling: probe → normal → idle for holiday detection), builds the system tray menu (left-click toggle, right-click menu with show/toggle-ticker/quit), registers all Tauri IPC commands, and sets up the auto-updater. The main window's `CloseRequested` event is intercepted to hide instead of quit. Window position/size is saved to SQLite and restored on next launch with monitor-boundary validation.

**`domain/mod.rs`** — Shared data types serialized across the IPC boundary to TypeScript types in [src/types/index.ts](src/types/index.ts):
- `Market` enum (CN/HK/US)
- `Quote` — real-time quote with price, change, open/high/low, volume, turnover, turnover_rate
- `IndexQuote` — index-level quote
- `Depth` — 5-level bid/ask depth (bids: `Vec<Level>`, asks: `Vec<Level>`)
- `Level` — single depth level (price + volume)
- `MinuteData` — intraday minute bar (time, price, open, high, low, volume, avg_price)
- `KLineData` — daily/weekly/monthly K-line bar (date, open, high, low, close, volume, turnover)
- `StockBrief` — minimal stock identifier for search results

**`db/mod.rs`** — SQLite database (via `rusqlite` with bundled SQLite). Three tables: `watchlist`, `settings` (key-value), `quote_cache`. Database is stored at `{app_data_dir}/quant-desktop.db`. Auto-creates tables and default settings on first open. `init_defaults()` inserts default settings only on first run (when key does not exist); user preferences persist across restarts.

**`datasource/mod.rs`** — Pluggable data source architecture. The `DataSource` trait defines `fetch_realtime()`, `fetch_indices()`, `search()`, `fetch_depth()`, `fetch_minute_data()`, `fetch_kline()`, `health_check()`. `DataSourceManager` holds a registry of adapters and an `active` name, supporting runtime switching. A `tokio::sync::Notify` wakeup mechanism triggers immediate refresh on data source switch.

Volume/turnover normalization: adapters return raw data in 手 (hands) / 万元 for stocks, and data is normalized to 股 (shares) / 元 via `normalize_volume(×100)` and `normalize_turnover(×10000)`. The frontend `formatVolume()` converts back to human-readable units for display.

- `sina.rs` — Sina Finance (新浪财经) adapter, the **backup** data source. GBK-encoded responses, `var hq_str_xxx="..."` format. Handles code-to-exchange mapping (sh/sz prefix). **Index fetching uses stock-format API** (codes without `s_` prefix, 30+ fields per line) because the compact index-only format (`s_` prefix, 6 fields) returns incorrect volume/turnover for 创业板指 (s_sz399006). **Shanghai (`sh`) index volume is consistently 1/100 of the correct value** in all Sina formats — corrected with `saturating_mul(100)` before entering the shared pipeline. Stock-format data arrives in 股/元 pre-normalized, so `normalize_volume`/`normalize_turnover` are NOT applied for indices. Search only supports exact 6-digit code lookup. Depth data fetched via Tencent API fallback (Sina's native depth endpoint is dead). Minute/K-line data from `money.finance.sina.com.cn`. Covers 7 major indices.

- `tencent.rs` — Tencent Securities (腾讯证券) adapter, the **default** data source. GBK-encoded responses, `v_sh600519="..."` format with `~` separators. Full implementation: realtime quotes, 7 indices, exact-code search, minute data via `ifzq.gtimg.cn`, K-line (daily/weekly/monthly) via `web.ifzq.gtimg.cn`, and depth from embedded bid/ask fields (positions 9-28). Volume converted from 手 to 股 (×100), turnover from 万元 to 元 (×10000).

- `market_clock.rs` — Trading session detection (China Standard Time / UTC+8). `MarketSession` enum: PreOpen/MorningTrade/LunchBreak/AfternoonTrade/Closed with weekend detection. `recommended_interval()`: 2s trading, 5s pre-open, 10s lunch, 30s closed. Scheduler uses this as the base interval, then applies adaptive polling on top.

**`cache/mod.rs`** — `QuoteCache` provides in-memory `HashMap` storage with SQLite dual-write persistence. `restore_from_db()` on startup for instant quote display.

**`Scheduler`** spawns a `tokio` background task with an **adaptive polling state machine**:

| State | Trigger | Interval |
|-------|---------|----------|
| **Probing** (3×2s) | Entering morning/afternoon trading | 2s |
| **Normal** | Price change detected during probe | market_clock base (2s) |
| **Idle** | 10 consecutive unchanged polls, or holidays | 30s fixed |

The scheduler groups watchlist codes by market, fetches batch quotes, updates the cache, and emits three Tauri events: `quotes-updated`, `indices-updated`, and `market-session-changed`. A separate wakeup listener task triggers immediate refresh on data source switch. On fetch failure, falls back to cached data. In Closed session, serves entirely from cache.

**`commands/`** — Tauri IPC command handlers. Each file exposes `#[tauri::command]` functions registered in `lib.rs`:
- `quote.rs` — `get_quotes`, `get_indices` (read from cache), `get_depth`, `get_intraday`, `get_kline` (async via active data source)
- `watchlist.rs` — CRUD + reorder/move operations (`get_watchlist`, `add_watch`, `remove_watch`, `reorder_watch`, `move_watch_top`, `move_watch_up`, `move_watch_down`), `search_stocks` (with cross-source fallback: if active source returns empty, tries alternate source)
- `settings.rs` — `get_settings`, `set_setting`, `switch_datasource`, `list_datasources`
- `window.rs` — `show_main_window` (restore from tray)
- `updater.rs` — `check_update`, `install_update` (auto-update with trading-session-aware prompt suppression)

### Frontend (`src/`)

**Stores (Pinia)** — Four stores mirroring the backend state:
- `quote.ts` — Listens to `quotes-updated` and `indices-updated` Tauri events. Quotes stored in a `Map<"market:code", Quote>` for O(1) lookup.
- `watchlist.ts` — Calls Tauri `invoke()` commands for CRUD. Re-fetches full list after mutations.
- `settings.ts` — Key-value settings map. Manages theme toggle (dark/light on `<html data-theme>`), data source switching, and auto-launch toggle. Emits `theme-changed` and `datasource-changed` events for cross-window sync.
- `updater.ts` — Update state (checking/available/downloading/installing). Watches backend update events.

**Component hierarchy (main window)**:
```
App.vue → NConfigProvider + NMessageProvider (theme overrides, accent=blue)
  └─ AppLayout.vue
       ├─ TopBar.vue (slogan, data source dropdown)
       ├─ IndexBar.vue → IndexCard.vue × N (market indices from quote store)
       ├─ WatchlistTable.vue (NDataTable: sortable columns, right-click context menu, row-click expands detail)
       │    ├─ AddStockDialog.vue (search with 300ms debounce + add modal)
       │    └─ StockDetail.vue (expanded row detail panel)
       │         ├─ ChartSwitcher.vue (toggle: 分时/日K/周K/月K)
       │         ├─ MinuteChart.vue (intraday chart, auto-refresh 5s)
       │         ├─ KLineChart.vue (daily/weekly/monthly K-line, auto-refresh 30s/60s)
       │         ├─ DepthPanel.vue (5-level bid/ask, auto-refresh 3s)
       │         └─ StockSummary.vue (open/high/low/volume/turnover/turnover_rate)
       └─ StatusBar.vue (version, check update, theme toggle, auto-launch toggle, contact)
```

**Ticker bar** ([TickerBar.vue](src/components/ticker/TickerBar.vue)) — Standalone mini Vue app that polls watchlist + settings, listens to quote events, and cycles through stocks two at a time with 3-second auto-scroll. Pauses on hover. Clicking restores the main window. Polls settings every 1s to sync theme changes.

**Composables**:
- `useTauriEvent.ts` — Vue lifecycle wrapper for `listen()` (auto-cleanup on unmount)
- `useTheme.ts` — Standalone theme state (used by ticker)
- `useChart.ts` — Shared chart composable (init, data loading, auto-refresh with period-dependent intervals, theme-aware styling for both minute and K-line charts)
- `useUpdateCheck.ts` — Startup update check with trading-session gating (suppresses prompts during active trading)

**Styles**:
- `variables.css` — Design system tokens: 4 surface levels, border system, text palette, semantic up/down colors (red=up, green=down per A-share convention), monospace font for numbers (tabular-nums), 4px-base spacing scale, radius tokens, shadow tokens, dark + light theme overrides
- `dark.css` — Scrollbar theming
- `chart.css` — Shared chart container styles (overlay, error, status text)

### Data flow

```
DataSource API (Sina/Tencent)
  → Scheduler (tokio background poll, adaptive interval)
    → QuoteCache (in-memory HashMap + SQLite dual-write)
      → app_handle.emit("quotes-updated" / "indices-updated" / "market-session-changed")
        → Pinia Stores (Tauri event listener)
          → Vue reactive components (main window + ticker bar)
```

On-demand requests:
- **Depth**: `invoke("get_depth")` → active DataSource adapter → returned to `DepthPanel`. Auto-refreshes every **3s** while detail panel is open.
- **Minute chart**: `invoke("get_intraday")` → adapter → `useChart` composable. Loads once on open, then auto-refreshes every **5s**.
- **K-line (daily)**: `invoke("get_kline", {period: "daily"})` → adapter → `useChart`. Loads once, auto-refreshes every **30s** (last candle updates intraday).
- **K-line (weekly/monthly)**: Same path, auto-refreshes every **60s**.

User mutations (add/remove/reorder watchlist) go through `invoke()` → Rust commands → SQLite, then the frontend re-fetches the watchlist. The scheduler picks up changes on the next poll cycle.

DataSource switching triggers a `Notify` wakeup → Scheduler immediately refreshes with the new adapter.

### K-line chart styling

- **Candle colors**: `compareRule: 'previous_close'` — A-share convention (red=close>昨收, green=close<昨收, gray=close=昨收)
- **Wick colors**: Explicitly set `upWickColor`/`downWickColor`/`noChangeWickColor` to match body/border colors, preventing klinecharts defaults from mismatching
- **Volume indicator**: Custom `VOL` indicator with MA5/MA10/MA20 lines and colored volume bars at the bottom

### Key dependencies

- **Rust**: `tauri` v2 (with tray-icon feature), `rusqlite` (bundled), `reqwest` (rustls-tls), `tokio` (full), `chrono`, `serde`/`serde_json`, `encoding_rs` (GBK decoding), `async-trait`, `log` + `simplelog` (file+stderr logging)
- **Frontend**: `vue` 3, `pinia`, `naive-ui`, `@tauri-apps/api`, `@tauri-apps/plugin-opener`, `@tauri-apps/plugin-updater`, `vite`, `vue-tsc`, `klinecharts` (v10 beta)

### Default settings (auto-inserted on first run)

| Key | Default | Description |
|-----|---------|-------------|
| `active_datasource` | `tencent` | Active market data provider (default: Tencent, persisted per user preference) |
| `refresh_interval` | `3` | Polling interval in seconds (overridden by market_clock + adaptive polling) |
| `theme` | `dark` | UI theme (dark/light) |
| `ticker_visible` | `true` | Ticker bar visibility |

### Window position persistence

Main window position/size is saved to SQLite `settings` table on move/resize/close. Keys: `window_x`, `window_y`, `window_width`, `window_height`. On next launch, restored with monitor-boundary validation (clamped to visible area if monitor config changed). Ticker window is always positioned at bottom-right via `available_monitors()` calculation.

## Development phases

| Phase | Status | Scope |
|-------|--------|-------|
| Phase 1 (MVP) | ✅ Complete | Scaffold, Sina adapter, tray, ticker, watchlist CRUD, index bar, dark theme |
| Phase 2 (Experience) | ✅ Complete | Detail panel (minute chart + depth + summary), Tencent adapter, column sorting, window position memory, market_clock dynamic polling |
| Phase 3 (Quality) | ✅ Complete | Code review fixes (36 items): logging, error handling, spawn_blocking, CSS tokens, accessibility, CSP, encoding_rs migration, design system, dead code cleanup |
| Phase 4 (Enhancement) | ✅ Partial | K-line chart (daily/weekly/monthly) ✅, chart auto-refresh ✅, adaptive polling (probe/idle) ✅, depth auto-refresh ✅, index detail panel ✅, auto-update ✅, price alerts 📋, import/export (JSON/CSV) 📋, auto-start ✅, packaging polish 📋 |
| Phase 5 (Extension) | 🔮 Future | HK/US market support, professional data sources (Wind/Tushare), macOS/Linux adaptation |

## CI/CD

GitHub Actions workflow at [.github/workflows/release.yml](.github/workflows/release.yml) — triggered on `v*` tags or manual dispatch. Matrix build for Windows (MSVC), macOS (universal), Linux (gnu). Uploads `.exe`/`.msi`/`.dmg`/`.deb`/`.AppImage` artifacts.

`scripts/build.mjs` provides a cross-platform build wrapper with automatic proxy detection (Clash/V2Ray on common ports 7890/10809/1080/8118/8080/1087/4780).
