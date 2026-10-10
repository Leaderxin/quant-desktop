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

# Frontend unit tests (vitest)
npm test

# Build Rust backend only
cargo build --manifest-path src-tauri/Cargo.toml
```

Frontend tests run with `vitest` (`npm test`, or `npm run test:watch`). Two conventions worth keeping:

- **Only the Tauri boundary is mocked** — `invoke`/`listen`/`emit` from `@tauri-apps/api/*`. Pinia and the stores under test are real, because the behaviour being pinned *is* the store's own. Specs that need `document` opt in per file with `// @vitest-environment happy-dom` rather than switching the global environment.
- **Pure logic that a component can't be tested on gets extracted into `src/utils/`.** That is why `dragSort.ts`, `paging.ts`, `watchGroups.ts`, `minuteBars.ts`, and `minuteAxis.ts` exist: the drop-index arithmetic, the carousel windowing, the delete-dialog counts, and the 分时图's session split / axis geometry all live inside components otherwise, where no test can reach them.

Coverage by area:

| File | What it pins |
|------|--------------|
| [src/stores/market.spec.ts](src/stores/market.spec.ts) | Refresh-scheduler concurrency: stale responses not landing, in-flight + session-switch interleaving |
| [src/stores/watchlist.spec.ts](src/stores/watchlist.spec.ts) | Derived ordering (group order comes from the join table, not the pool), dangling ids dropped, `ticker_order` sorting, optimistic `setTickerEnabled`, and **IPC argument spelling** (`groupId`/`watchId`/`groupIds` — Rust is snake_case, JS must be camelCase, and a typo surfaces only as a Tauri deserialization error) |
| [src/stores/settings.spec.ts](src/stores/settings.spec.ts) | Derived-config fallbacks, clamping before write, and the cross-window contract that `applyRemoteSetting` never writes back or re-broadcasts |
| [src/utils/prefs.spec.ts](src/utils/prefs.spec.ts) | Every corrupt/legacy settings value yields something usable instead of throwing |
| [src/utils/hotkey.spec.ts](src/utils/hotkey.spec.ts) | 老板键加速键字符串的两端约定：录制端产出的写法必须是 Rust 解析端认得的（词表两边各写一份，错了不报错，只表现为「录进去了、按了没反应」）、修饰键顺序固定、以及不在打字路径上的组合才许当热键 —— 裸键与只带 Shift 的组合都会把字符从全系统抢走（KeyH 是小写 h、Shift+KeyH 是大写 H、Shift+Digit1 是「!」） |
| [src/utils/changelog.spec.ts](src/utils/changelog.spec.ts) | 「关于」页更新说明的解析契约：版本头/日期/小节条目、CRLF 行尾、坏输入不抛错 |
| [src/utils/dragSort.spec.ts](src/utils/dragSort.spec.ts) | Drop-index arithmetic in all four directions × source-before/after-target |
| [src/utils/paging.spec.ts](src/utils/paging.spec.ts) | Carousel windowing, including the "short list must not self-shuffle" edge |
| [src/utils/watchGroups.spec.ts](src/utils/watchGroups.spec.ts) | The orphan/kept counts quoted in the delete-group confirmation |
| [src/utils/minuteBars.spec.ts](src/utils/minuteBars.spec.ts) | 分时图的交易日切分（适配器的滚动窗口盘中横跨两个交易日，只有最后一个可以画，日期取自数据而非本机时钟）、图上所画的昨收（上一交易日的收盘，同样来自那个窗口）、防止跨日刷新把昨天的 bar 接着往后追加的判定，以及"未来 bar"过滤所比较的 UTC+8 墙钟上界 |
| [src/utils/minuteAxis.spec.ts](src/utils/minuteAxis.spec.ts) | 分时图网格算术（每个 bar 间隔的槽位数、让曲线左对齐的右侧偏移、刻度坐标、低于该宽度时整张网格必须交还给 klinecharts）以及关于昨收对称的区间与其百分比标签 |
| [src/composables/useTickerWindowHeight.spec.ts](src/composables/useTickerWindowHeight.spec.ts) | Ticker window resizes keep the bottom edge anchored |

Deliberately **not** covered, so nobody mistakes absence for an oversight: Vue component rendering (would need `@vue/test-utils`, not currently a dependency), drag-and-drop DOM events (only the index math is tested), and anything visual — the token palette, the tab-bar spacing, icon silhouettes. Those are eyeball-only. `cargo test` covers the Rust side (blacklist classification, response parsing, session intervals, and group ownership/migration semantics in `db::tests`); `vue-tsc` with the strict tsconfig enforces type correctness on both app and test code. There is no lint command configured yet.

## Git 提交信息

提交信息中**不要出现 "Claude" 字眼** —— 包括 `Co-Authored-By: Claude`、"Generated with Claude Code" 之类的工具署名尾注。提交历史面向仓库读者，工具来源只是噪音；把改了什么、为什么改写清楚即可。

## Architecture

QuantDesktop is a **Tauri 2 desktop app** for monitoring Chinese A-share stock markets. It has two separate webview windows driven by two Vite/Vue entry points, a SQLite-backed Rust backend, and a pluggable data source layer for fetching market data.

### Two-window layout

| Window | Label | Entry | Config |
|--------|-------|-------|--------|
| Main UI | `main` | `index.html` → `src/main.ts` | 1100×680, starts hidden, hides to tray instead of closing, position/size persisted to SQLite |
| Ticker bar | `ticker` | `ticker.html` → `src/ticker.ts` | 230×38, always-on-top, decorationless, positioned bottom-right, skip-taskbar |

Vite is configured with two Rollup inputs (`index.html` + `ticker.html`) in [vite.config.ts](vite.config.ts). Each entry mounts a separate Vue app with its own Pinia instance. Both share the same stores and composables via import.

### Rust backend (`src-tauri/src/`)

**`lib.rs`** — Application setup. Initializes SQLite database, registers data source adapters (Tencent first as default, then Sina as fallback), restores quote cache from DB, spawns the background polling `Scheduler` (with adaptive polling: probe → normal → idle for holiday detection), builds the system tray menu (left-click toggle, right-click menu with show/settings/toggle-ticker/quit), registers all Tauri IPC commands, and sets up the auto-updater. The main window's `CloseRequested` event is intercepted to hide instead of quit. Window position/size is saved to SQLite and restored on next launch with monitor-boundary validation.

Also home to `set_ticker_visible(app, db, visible)` — the single implementation of "show/hide the ticker window", shared by the tray menu and the settings page's `set_ticker_visible` command. Both callers need the same four follow-ups on show (always-on-top, skip-taskbar, `WS_EX_TOOLWINDOW`, position restore) and the same persistence; having two copies guarantees one of them drifts, and the symptoms (window lands in the taskbar, or off-screen) are invisible when editing the other. Order matters: Windows' `set_skip_taskbar` goes through `ITaskbarList::DeleteTab`, which only takes effect once the window has actually been shown.

Also home to `set_app_theme(app, theme)` — the single implementation of "把应用主题同步到窗口标题栏", shared by startup and the settings page's `set_window_theme` command.

Also home to `show_main_window(app)` — the single implementation of "把主窗口显示出来并聚焦", shared by the three entries that need it: the ticker bar's click, the tray menu's 「显示主界面」and 「设置」, and the tray icon's left-click. The step that matters is `unminimize()`: `show()` 在 Windows 上走 `SW_SHOW`，对最小化的窗口只是「在当前大小和位置显示」，窗口照旧缩在任务栏里 —— 用户看到任务栏图标闪一下、界面不出来。漏掉这一处的症状是「点了没反应」，而另外两处照旧正常，改哪一处都看不出来。托盘左键还要额外判一次 `is_minimized()`：最小化的窗口在 Win32 眼里**仍然是 visible**（`WS_VISIBLE` 还在），只判 `is_visible()` 会落进「隐藏」分支，把用户想叫回来的窗口反倒藏起来。

托盘菜单的「设置」不只是一次 `show_main_window` —— 它随后 `emit("open-settings")`，由主窗口 `AppLayout` 的监听把 `showSettings` 置真。主窗口的 webview 是常驻的（隐藏不等于卸载），所以窗口处于隐藏或最小化状态时这个事件也收得到。

**标题栏不受 `<html data-theme>` 影响**：标题栏是操作系统画的，CSS 变量够不到它 —— 不显式告诉窗口的话，暗色主题下顶上那一条会照旧是白的。tao 收到 `set_theme` 后会调 `DwmSetWindowAttribute(DWMWA_USE_IMMERSIVE_DARK_MODE)`（Windows 10 1809+ 生效；macOS 走 NSWindow 的 appearance），所以 `set_app_theme` 这一次调用就是全部。两个调用点各有非它不可的理由：启动那次必须**先于 `main.show()`**（窗口是先 show 出来、webview 才加载完的，晚一步就会先闪一帧白条）；运行期那次挂在前端的 `applyTheme` 上而不是只挂 `toggleTheme`，因为主窗口启动恢复和行情条收到 `theme-changed` 走的都是它 —— 行情条调它只是白跑一次 IPC（命令只作用于 `main`），换来的是没有任何一条改主题的路径会漏掉标题栏。`set_app_theme` 只作用于 `main`：ticker 是 `decorations: false`，没有标题栏可画。

**`domain/mod.rs`** — Shared data types serialized across the IPC boundary to TypeScript types in [src/types/index.ts](src/types/index.ts):
- `Market` enum (CN/HK/US)
- `Quote` — real-time quote with price, change, open/high/low, volume, turnover, turnover_rate
- `IndexQuote` — index-level quote
- `Depth` — 5-level bid/ask depth (bids: `Vec<Level>`, asks: `Vec<Level>`)
- `Level` — single depth level (price + volume)
- `MinuteData` — intraday minute bar (time, price, open, high, low, volume, avg_price)
- `KLineData` — daily/weekly/monthly K-line bar (date, open, high, low, close, volume, turnover)
- `StockBrief` — minimal stock identifier for search results

**`db/mod.rs`** — SQLite database (via `rusqlite` with bundled SQLite). Five tables: `watchlist`, `watch_groups`, `watch_group_members`, `settings` (key-value), `quote_cache`. The database lives at `dirs::data_dir().join("quant-desktop")/quant-desktop.db` — note this is **not** derived from the Tauri `identifier` in `tauri.conf.json`; on Windows the real path is `%APPDATA%\quant-desktop\quant-desktop.db`. In portable mode (a `portable.dat` next to the executable) it is `<exe_dir>/data/quant-desktop.db` instead. Microsoft Store (MSIX) builds add no branch here — the app still resolves `%APPDATA%\quant-desktop`, but MSIX file-system virtualization transparently redirects those writes to the package-private `%LOCALAPPDATA%\Packages\<package family name>\LocalCache\Roaming\quant-desktop\`. Store data is therefore separate from NSIS/portable data and is deleted on uninstall; the `unvirtualizedResources` capability that would have shared one directory with the other builds was removed (see [store/AppxManifest.xml](store/AppxManifest.xml)). The startup log prints the resolved path as `Data directory: <path> (portable: ...)`. Auto-creates tables and default settings on first open. `init_defaults()` inserts default settings only on first run (when key does not exist); user preferences persist across restarts. Schema migrations run inside `Database::open()` on **every** launch, so they must stay idempotent.

**Watchlist data model — multi-group (tag-like) ownership:**

- `watchlist` is a **global stock pool**: a stock is inserted once (`UNIQUE(code, market)`) and carries `ticker_enabled` + `ticker_order` (the ticker carousel is a flat, cross-group list, so its ordering lives here, not on the join table).
- `watch_groups` — group id/name/display order.
- `watch_group_members` — the (group, stock) many-to-many join, carrying the **per-group** `sort_order`. A stock has a different position in each group, which is exactly why ordering cannot stay on `watchlist` (the 1.5.x global `sort_order` column is left in place but no longer read or written; dropping it would require a table rebuild).

Semantics worth preserving when touching this code:

| Case | Behaviour |
|------|-----------|
| Adding a stock already in another group | Adds to the target group too — no move |
| Adding a stock already in the same group | No-op; keeps its existing in-group position |
| Removing from a group | If it was the stock's **only** group, the stock is deleted too (`remove_watch_from_group` returns `true`) — an orphan row would be invisible in the UI |
| `set_watch_groups` with an empty list | Rejected; unchecking every box is an easy misclick and silently deleting data is too expensive. Full deletion goes through `remove_watch` |
| Deleting a group | Deletes only the memberships. Stocks that become orphaned move into the default group; stocks still in other groups are untouched. The last group cannot be deleted |
| Renaming a group | Rejects empty/over-long names and duplicates |

Migration is guarded by a `groups_migrated` settings key rather than "is the join table empty" — a user may legitimately remove every stock from every group while the pool still has rows, and an empty-table check would re-seed them on the next launch. Legacy rows get their in-group order from the old global `sort_order`, and `ticker_order` is backfilled from it so existing users' carousel order is unchanged.

**`datasource/mod.rs`** — Pluggable data source architecture. The `DataSource` trait defines `fetch_realtime()`, `fetch_indices()`, `search()`, `fetch_depth()`, `fetch_minute_data()`, `fetch_kline()`, `health_check()`. `DataSourceManager` holds a registry of adapters and an `active` name, supporting runtime switching. A `tokio::sync::Notify` wakeup mechanism triggers immediate refresh on data source switch.

`INDEX_CODES` is the **candidate pool** of 14 broad-based indices that both adapters fetch in one batched request each poll; the frontend filters and orders this by the user's `index_codes` setting. Fetching the whole pool rather than just the selection means toggling an index in settings shows it on the bar *immediately* instead of waiting for the next poll (2s during trading, up to 30s when closed) — the extra cost is a few lines of response text. `INDEX_POOL_NAMES` maps those codes to display names for the settings page (names come from the quote response, so the settings page cannot derive them for unselected indices). `index_pool_names_match_codes` pins the two lists together, and any new code must be verified against **both** Tencent and Sina — the two use different index formats, so one working says nothing about the other.

Volume/turnover normalization: adapters return raw data in 手 (hands) / 万元 for stocks, and data is normalized to 股 (shares) / 元 via `normalize_volume(×100)` and `normalize_turnover(×10000)`. The frontend `formatVolume()` converts back to human-readable units for display.

- `sina.rs` — Sina Finance (新浪财经) adapter, the **backup** data source. GBK-encoded responses, `var hq_str_xxx="..."` format. Handles code-to-exchange mapping (sh/sz prefix). **Index fetching uses stock-format API** (codes without `s_` prefix, 30+ fields per line) because the compact index-only format (`s_` prefix, 6 fields) returns incorrect volume/turnover for 创业板指 (s_sz399006). **Shanghai (`sh`) index volume is consistently 1/100 of the correct value** in all Sina formats — corrected with `saturating_mul(100)` before entering the shared pipeline. Stock-format data arrives in 股/元 pre-normalized, so `normalize_volume`/`normalize_turnover` are NOT applied for indices. Search only supports exact 6-digit code lookup. Depth data fetched via Tencent API fallback (Sina's native depth endpoint is dead). Minute/K-line data from `money.finance.sina.com.cn`. Covers 7 major indices.

- `tencent.rs` — Tencent Securities (腾讯证券) adapter, the **default** data source. GBK-encoded responses, `v_sh600519="..."` format with `~` separators. Full implementation: realtime quotes, 7 indices, exact-code search, minute data via `ifzq.gtimg.cn`, K-line (daily/weekly/monthly) via `web.ifzq.gtimg.cn`, and depth from embedded bid/ask fields (positions 9-28). Volume converted from 手 to 股 (×100), turnover from 万元 to 元 (×10000).

- `market_clock.rs` — Trading session detection (China Standard Time / UTC+8). `MarketSession` enum: PreOpen/MorningTrade/LunchBreak/AfternoonTrade/Closed with weekend detection. `recommended_interval()`: 2s trading, 5s pre-open, 10s lunch, 30s closed. Scheduler uses this as the base interval, then applies adaptive polling on top.

**`hotkey.rs`** — 全局快捷键。目前只有一个老板键：按下即隐藏主窗口。注册放在 Rust 侧而不是用配套的 JS 插件 —— 它要在主窗口已经隐藏、整个应用没有焦点的时候仍然响应，那正是 webview 里的 JS 拿不到执行时机的情形。

`apply(app, accelerator)` 是唯一的注册入口（空串 = 不设），**先 `unregister_all()` 再注册**：改键时只加不减的话，旧键会继续响应而设置页上已经看不到它了 —— 一个用户既关不掉也查不到的全局热键。`commands::settings::set_boss_key` 是「先注册、成功了再落库」，两条失败路径都**回滚到旧键**：注册失败时 —— `apply` 会先注销全部，新键注册不上时旧键已经没了，不回滚的话用户试着换一个已被占用的键，代价是连原来能用的那个也一起丢掉；落库失败时（DB 锁 / IO）—— OS 里已经在响应新键，不回滚的话「系统响应的键」与「库/界面显示的键」会分家到下次重启。启动那次由 `register_from_db` 在 setup 末尾读库注册，注册失败只记 warning，不拦启动（组合键被别的程序占着是用户环境问题，设置页里改一次就会重试并把原因回显）。

老板键**只隐藏主窗口**，不碰行情条，也不改任何设置 —— 这是一次临时躲避，不是把配置改掉。恢复走托盘图标或点一下行情条（即上面的 `show_main_window`）。做成来回切的键，在「人真的站在身后」的那一刻就有被按亮的风险。

加速键字符串的词表是 global-hotkey 那套 `Code` 名（`KeyH` / `Digit1` / `F5` / `ArrowUp` …），与浏览器 `KeyboardEvent.code` 几乎逐字相同，所以前端录制时直接取 `e.code`、不做映射表；词表两边各写一份，由 [src/utils/hotkey.ts](src/utils/hotkey.ts) 与其 spec 钉住对齐 —— 对不上的症状是「录进去了、按了没反应」，而设置页上显示得好好的。`Cargo.toml` 里插件钉在 `~2.3`：2.4.0 起要求 tauri ^2.12，会让 cargo 顺势顶掉 tauri / tao / wry。

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
- `watchlist.rs` — the watchlist pool, group membership, and ticker range:
  - Snapshot: `get_watchlist` returns a `WatchlistSnapshot { items, groups, default_group_id }` in one call — `groups[].watch_ids` carries the ordered member ids, so switching groups in the tab bar is pure local filtering with no intermediate state.
  - Pool: `add_watch` (upsert + join group), `remove_watch` (delete everywhere), `search_stocks` (cross-source fallback)
  - Membership: `remove_watch_from_group`, `set_watch_groups` (full-overwrite, for the context menu's checkable submenu)
  - Per-group ordering: `move_group_member_top|up|down`, `reorder_group_members` — all keyed by `(group_id, watch_id)`, since the same stock sits at different positions in different groups
  - Groups: `add_watch_group`, `rename_watch_group`, `delete_watch_group`, `reorder_watch_groups`
  - Ticker range: `set_watch_ticker_enabled`, `set_ticker_enabled_bulk`, `reorder_ticker`
- `settings.rs` — `get_settings`, `set_setting`, `set_boss_key` (writes the setting *and* re-registers the global shortcut; empty string = unset), `switch_datasource`, `list_datasources`, `list_index_pool`, `get_portable_mode`, `is_store_build`
- `autostart.rs` — `get_autostart`, `set_autostart` (OS-level autostart; registry Run key via tauri-plugin-autostart, except Windows store builds which use the packaged-app StartupTask WinRT API)
- `window.rs` — `show_main_window` (ticker-bar click; delegates to `crate::show_main_window`), `set_ticker_visible` (settings-page toggle; delegates to `crate::set_ticker_visible`), `set_window_theme` (delegates to `crate::set_app_theme` — see the 标题栏 note below)
- `market.rs` — `get_market_overview(direction, top_n)`, `get_overview_interval`. `top_n` comes from the settings page and is re-clamped backend-side via `market::clamp_top_n` — IPC arguments cannot be assumed to have been validated by the UI.
- `updater.rs` — `check_update`, `install_update` (auto-update with trading-session-aware prompt suppression; store builds return early at runtime — the commands stay registered so the frontend gets a clean response)

### Frontend (`src/`)

**Stores (Pinia)** — Four stores mirroring the backend state:
- `quote.ts` — Listens to `quotes-updated` and `indices-updated` Tauri events. Quotes stored in a `Map<"market:code", Quote>` for O(1) lookup. `indices` holds the whole 14-index candidate pool; `IndexBar` filters/orders it by `settings.indexCodes`.
- `watchlist.ts` — Holds the `WatchlistSnapshot` and derives everything else locally: `activeGroup` (the selected tab), `visibleItems` (active group's members in in-group order), `tickerItems` (cross-group, sorted by `ticker_order`). Mutations are one IPC followed by a snapshot refetch — no optimistic updates, because group membership, in-group order, and orphan handling all live in the backend and guessing them locally drifts. The single exception is `setTickerEnabled`, which flips the switch locally first so the toggle feels instant.
- `settings.ts` — Key-value settings map plus a **derived** config layer (`indexCodes`, `sectorTopN`, `tickerTransparent`, `watchlistColumns`, `colorScheme`, …). Every config value is a `computed` over the string map rather than its own `ref`, so the map is the single source of truth and there is no window where local state and the DB disagree. Manages theme (`<html data-theme>` **and** the OS title bar, via `set_window_theme`), colour scheme (`<html data-color-scheme>`), data source switching, auto-launch, and the ticker window toggle. `toggleTheme` delegates to `applyTheme` instead of setting `data-theme` itself — the title-bar sync hangs off `applyTheme`, so a second copy of "flip the theme" would be a copy that forgets it. `setSetting` broadcasts `settings-changed` for the ticker window; the ticker applies the payload via `applyRemoteSetting` (no refetch — that would cost 5 extra IPC calls per toggle).
- `updater.ts` — Update state (checking/available/downloading/installing). Watches backend update events.

**`utils/prefs.ts`** — Settings value domains and parsers. Settings are strings in SQLite, so structured values are JSON; every parser (`parseColumns`, `parseDefaultSort`, `parseJsonArray`, `parseBool`, `parseCount`) falls back to a usable default instead of throwing. A single corrupted key must not blank the UI. `ALL_COLUMNS` is the canonical column list (key + label + `required`); `parseColumns` honours the user's order but re-inserts `code`/`name` if a bad config dropped them.

**Component hierarchy (main window)**:
```
App.vue → NConfigProvider + NMessageProvider + NDialogProvider
  └─ AppLayout.vue
       ├─ SettingsPage.vue   (v-if 切换：设置页打开时看盘界面整体卸载；顶部横向分区 Tab + 居中限宽 1000px 的内容列，参考 CC Switch；Tab 条与内容列共用 max(24px,(100%-1000px)/2) 外边距保证右缘对齐)
       │    ├─ IndexSection.vue      (候选池勾选 + 拖拽排序)
       │    ├─ MarketSection.vue     (显示开关 + 榜单条数)
       │    ├─ WatchlistSection.vue  (列显示/顺序、默认排序、涨跌配色)
       │    ├─ TickerSection.vue     (显示、透明背景、每屏条数、轮播范围)
       │    ├─ GeneralSection.vue    (主题、开机自启、数据源、老板键)
       │    └─ AboutSection.vue      (关于：版本、检查更新、更新说明、GitHub Star / 商店好评引导)
       └─ 看盘界面
            ├─ TopBar.vue (slogan, data source dropdown)
            ├─ IndexBar.vue → IndexCard.vue × N (按设置筛选/排序)
            ├─ MarketOverviewPanel.vue (受 marketOverviewVisible 控制)
            ├─ WatchlistTable.vue (NDataTable: 动态列、默认排序、右键菜单)
            │    ├─ GroupTabs.vue (分组标签栏：切换/重命名/右键菜单/新建)
            │    ├─ AddStockDialog.vue (搜索 + 加入当前分组)
            │    └─ StockDetail.vue (expanded row detail panel)
            │         ├─ ChartSwitcher.vue (toggle: 分时/日K/周K/月K)
            │         ├─ MinuteChart.vue (intraday chart, auto-refresh 5s)
            │         ├─ KLineChart.vue (daily/weekly/monthly K-line, auto-refresh 30s/60s)
            │         ├─ DepthPanel.vue (5-level bid/ask, auto-refresh 3s)
            │         └─ StockSummary.vue (open/high/low/volume/turnover/turnover_rate)
            └─ StatusBar.vue (版本、检查更新、设置入口、主题、开机自启、联系)
```

**设置页（`src/components/settings/`）** — 覆盖式整页，入口在状态栏的齿轮按钮。用 `KeepAlive` 切换分区，保住各分区的本地 UI 状态（正在输入的自定义条数、轮播范围的分组筛选）。`AppLayout` 用 `v-if` 而非 `v-show` 承载它，两个后果都是要的：设置期间看盘界面的轮询全部停掉；返回时自选表重建，`defaultSortOrder` 这类只在挂载时生效的初值会按新设置重新应用（否则改完默认排序要重启应用才看得到）。

共用原语：`SettingsRow.vue`（标签 + 常驻说明 + 控件；说明一律常驻，不靠 placeholder/tooltip）、`SegmentedControl.vue`（与 `ChartSwitcher`、市场概览方向切换同一套视觉）、`components/common/DragSortList.vue`（拖拽 + `Alt+↑/↓` + 每行 ↑/↓ 按钮，拖拽不是唯一路径）。开关一律用 naive-ui 的 `NSwitch size="small"`（状态栏与设置分区共用；主题色经 `App.vue` 的 themeOverrides 跟随应用强调色）；下拉一律 `NSelect size="small"`，不用原生 `<select>` —— 原生 select 展开后的选项列表是操作系统画的，不跟应用主题（暗色下弹一块白底系统列表），合上的框再怎么用 CSS 画也补不上这一半。卡片/表头/列表行等共用样式在 [src/assets/styles/settings.css](src/assets/styles/settings.css)，以 `.settings-page` 为祖先选择器 —— 6 个分区各自 scoped 的话那几十行会复制六份。

**分组标签栏（[GroupTabs.vue](src/components/watchlist/GroupTabs.vue)）** — 点击切换、双击就地重命名、右键菜单（重命名 / 删除分组 / 上移 / 下移）、＋新建。删除确认框在本地算出影响面（快照里已有每组的有序成员 id）：`orphans` 是「只属于这一个分组」的股票，会并入默认分组；其余不受影响；并写明「自选本身不会被删除」。最后一个分组时菜单项禁用。

**Ticker bar** ([TickerBar.vue](src/components/ticker/TickerBar.vue)) — Standalone mini Vue app. Cycles through `watchlist.tickerItems` (cross-group, sorted by `ticker_order`) taking `ticker_items_per_page` at a time with 3-second auto-scroll. Pauses on hover. Clicking restores the main window. Reloads the watchlist on the `watchlist-changed` event, which is why every ticker-range mutation emits it. 只播报 `ticker_enabled` 为开的自选；该开关与轮播顺序统一在设置页的「行情条 → 轮播范围」维护（原先自选表里的逐行开关列已移除）。

设置同步走 `settings-changed` 事件（主窗口 `setSetting` 时广播），ticker 用 `applyRemoteSetting` 就地更新 —— 它读不到主窗口的 store，而重拉整份设置要 5 次 IPC，为一个开关付这个代价不值得。**透明背景**由 `document.body` 的 `ticker-transparent` class 控制（底板画在 [ticker.html](ticker.html) 的 body 上，不在组件里）：底板、圆角、阴影必须**一起**去掉，只把 background 改成 transparent 会留下一圈阴影脏边；窗口本身在 `tauri.conf.json` 里开了 `transparent: true`，否则 webview 会先画一层不透明底，CSS 怎么改都透不过去。

**Composables**:
- `useTauriEvent.ts` — Vue lifecycle wrapper for `listen()` (auto-cleanup on unmount)
- `useTheme.ts` — Standalone theme state (used by ticker)
- `useChart.ts` — Shared chart composable (init, data loading, auto-refresh with period-dependent intervals, theme-aware styling for both minute and K-line charts)
- `useUpdateCheck.ts` — Startup update check with trading-session gating (suppresses prompts during active trading)

**Styles**:
- `variables.css` — Design system tokens: 4 surface levels, border system, text palette, semantic up/down colors (red=up, green=down per A-share convention), monospace font for numbers (tabular-nums), 4px-base spacing scale, radius tokens, shadow tokens, dark + light theme overrides. Also holds the **涨跌配色方案覆盖块** (`[data-color-scheme="us"]`) — placed *after* both theme blocks because it has the same specificity (0,1,0) as `[data-theme="light"]` and wins on source order; the light variant needs the compound `[data-theme="light"][data-color-scheme="us"]` (0,2,0) to beat the light-theme definitions. The derived `-bg`/`-bar` tokens must flip with the primaries, or heat bars and text colours disagree.
- `dark.css` — Scrollbar theming
- `chart.css` — Shared chart container styles (overlay, error, status text)
- `settings.css` — Settings-page shared styles (cards, rows, list rows, checkbox, inputs), scoped under `.settings-page` so the five section components don't each carry a copy

### 官网（`website/`）

对外落地页，发布在 <https://leaderxin.github.io/quant-desktop/>，由 [pages.yml](.github/workflows/pages.yml) 部署到 GitHub Pages。**它与 Tauri 应用没有任何代码关系** —— 不在 `vite.config.ts` 的两个 Rollup 入口里，不参与 `npm run build`，也不依赖任何 npm 包。三个文件（`index.html` / `styles.css` / `app.js`）就是全部，改完刷新即见。

三处刻意的设计：

- **素材不复制。** `public/screenshots/` 是唯一来源（README 也在用），`website/assets/` 由 [scripts/site-assets.mjs](scripts/site-assets.mjs) 复制生成，并加进 `.gitignore` —— 与 `dist/` 同级，都是产物。之所以要走脚本而不是直接放两份图：两份副本必然漂移，README 换了截图而官网上还是旧的。脚本在 CI 与本地是**同一条命令**（`npm run site:assets`），所以本地过了 CI 就会过。它顺带做一件 CI 才有意义的事：扫描 `index.html` 里每个本地 `src`/`href` 并断言文件存在 —— 截图名写错在浏览器里只是一张破图，评审时看不出来。
- **配色与 `variables.css` 同源，且是仓库里唯一一处有意重复。** 站点没有构建步骤，就拿不到 `variables.css`；把调色板搬一份进 `website/styles.css` 是唯一的办法。所以**改那边的 surface / text / accent token 时，这里要跟着改** —— 这是「同一个值写两遍」的例外，其余地方一律不允许。
- **主题契约与应用一致**：`<html data-theme>`，`<head>` 里那段内联脚本在首帧前定好主题（否则深色访问者会先看到一帧白闪），`app.js` 只做增强 —— 禁用 JS 时页面依然完整可读，`.reveal` 的隐藏也由内联脚本加的 `.js` 类门控，不是无条件写死。`app.js` 里没有网络请求。

注意 **`ticker-dark.png` / `ticker-light.png` 只有 344×59**，必须按原尺寸展示（`.ticker-runway` 就是为此存在的）—— 其余截图是 2000px 级，可自由缩放。

Pages 的**首次启用需要在仓库 Settings → Pages 里把 Source 改成「GitHub Actions」**，工作流里的 `configure-pages: enablement: true` 通常能自动开，但不算数。工作流的 `paths` 过滤器里带着 `public/screenshots/**`：换一张截图同样应该重新发布，否则站点上还是旧图。

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
- **Minute chart**: `invoke("get_intraday")` → adapter → `useMinuteChart` composable. Loads once on open, then auto-refreshes every **5s**. Both adapters return a *rolling window* of the last N minute bars (Tencent `,,242`, Sina `datalen=240&scale=5`), which intraday spans the previous trading day too — so `MinuteData.time` carries `YYYY-MM-DD HH:mm` and [src/utils/minuteBars.ts](src/utils/minuteBars.ts) keeps only the last session present. Dropping the date there (or stamping bars with the client's `today`) makes the chart draw yesterday's afternoon as if it were today's. The refresh publishes it via `subscribeBar`, which klinecharts only ever *appends* to — so when the mapped session differs from the one already loaded (the panel left open across 09:30, or overnight) the refresh re-installs the whole loader instead; and each bar is compared against a UTC+8 "now" (`beijingNow`) rather than `Date.now()`, since the timestamps carry the *source's* wall clock and a client west of UTC+8 would otherwise judge the entire session to be in the future and freeze the chart. The composable registers its own `onUnmounted` for the 5s poll and the resize observer — `useChartCore`'s only disposes the chart instance, so they would otherwise leak one per mount.
- **Minute chart y-axis** carries both scales — prices on the right (where they already were) and 涨跌幅 on the left — over one range symmetric about 昨收, so the two columns read row by row and 0.00% sits dead centre (`convertToPixel(prevClose) === paneHeight / 2`, verified). The percent column is a second `createYAxis` on `candle_pane` that relabels itself through `displayValueToText`; the range comes from a single `overrideYAxis({ paneId })` so both axes get it, and it sets `gap` too — klinecharts' default is asymmetric (20% top / 10% bottom) and would tilt a symmetric range. The half-span is the day's largest `close` deviation from 昨收 (a 分时图 draws a close line, not candles) floored at 1% so the opening minute isn't magnified. The label hook is the only way to reach the hover readout: each `YAxisWidget` draws its own `CrosshairHorizontalLabelView`, and that text goes through the axis's `displayValueToText` — a `createTicks` override relabels the axis but leaves hover showing the price. 昨收 is read from the window itself — the close of the bar right before the drawn session (`sessionPrevClose`), i.e. the previous session's close — so the baseline always describes the day that is actually drawn; the `price - change` prop is only the fallback for a window with no prior session (a first-day listing). With no 昨收 the axis *and* the baseline are removed, not merely left uninstalled: the range callback and the label hook read 昨收 live, so keeping them would draw a range centred on 0 with blank labels.
- **Minute chart x-axis** is pinned to one trading session (09:30–15:00) — the curve grows left-to-right from the open and the rest of the day stays blank. klinecharts lays out by bar count and right-aligns short series (29 bars landed at x=335–615 of a 700px plot, i.e. a blank left half), so [src/utils/minuteAxis.ts](src/utils/minuteAxis.ts) computes the bar space and right offset itself, slot = one bar (240 slots for Tencent's 1-min feed, 48 for Sina's 5-min), and [useMinuteChart.ts](src/composables/useMinuteChart.ts) re-applies it whenever the data or the container size changes (a resize otherwise drifts back to a left blank). Pan and zoom are off (`setScrollEnabled(false)` / `setZoomEnabled(false)`) — a pannable 分时图 would only fight the re-layout. The axis labels are a fixed `createTicks` list (09:30 / 10:30 / 11:30-13:00 / 14:00 / 15:00) because klinecharts extrapolates tick timestamps linearly across the blank region, which reads "12:10" during the morning — the lunch break isn't linear in wall-clock. Only the axis is corrected; the crosshair still extrapolates in the blank region. Both the labels and the computed offset give way to klinecharts when a full session no longer fits the pane (`sessionFits`): below that width the bar space is under `barSpaceLimit.min`, `setBarSpace` is silently dropped, and the five labels would be clamped onto the right edge on top of each other.
- **K-line (daily)**: `invoke("get_kline", {period: "daily"})` → adapter → `useChart`. Loads once, auto-refreshes every **30s** (last candle updates intraday).
- **K-line (weekly/monthly)**: Same path, auto-refreshes every **60s**.

User mutations (add/remove/reorder watchlist) go through `invoke()` → Rust commands → SQLite, then the frontend re-fetches the watchlist. The scheduler picks up changes on the next poll cycle. Every watchlist-affecting command emits `watchlist-changed`, which is what keeps the ticker window's copy of the list in sync.

DataSource switching triggers a `Notify` wakeup → Scheduler immediately refreshes with the new adapter.

Cross-window settings sync: `setSetting` writes the DB and broadcasts `settings-changed` with `{key, value}`. The ticker window applies it directly via `applyRemoteSetting` and must **not** re-broadcast — doing so would ping-pong between the two windows. (`theme-changed` and `datasource-changed` remain separate because they have additional per-window side effects.)

### K-line chart styling

- **Candle colors**: `compareRule: 'previous_close'` — A-share convention (red=close>昨收, green=close<昨收, gray=close=昨收)
- **Wick colors**: Explicitly set `upWickColor`/`downWickColor`/`noChangeWickColor` to match body/border colors, preventing klinecharts defaults from mismatching
- **Volume indicator**: Custom `VOL` indicator with MA5/MA10/MA20 lines and colored volume bars at the bottom

### Key dependencies

- **Rust**: `tauri` v2 (with tray-icon feature), `rusqlite` (bundled), `reqwest` (rustls-tls), `tokio` (full), `chrono`, `serde`/`serde_json`, `encoding_rs` (GBK decoding), `async-trait`, `log` + `simplelog` (file+stderr logging), `tauri-plugin-global-shortcut` (老板键，钉 `~2.3`，见 `hotkey.rs`)
- **Frontend**: `vue` 3, `pinia`, `naive-ui`, `@tauri-apps/api`, `@tauri-apps/plugin-opener`, `@tauri-apps/plugin-updater`, `vite`, `vue-tsc`, `vitest` (dev, store unit tests), `klinecharts` (v10 beta), `@lucide/vue` (统一图标库 —— 全应用的功能图标一律从这里按需导入、`:size` 控制尺寸，不再手绘内联 SVG；例外是品牌图标用官方原版，如状态栏的 GitHub mark，以及非图标性质的绘制如二维码占位图。旧包名 `lucide-vue-next` 已停在 1.0.0 并被上游标记弃用，图标名与 props 不变，换的只是包名)

### Default settings (auto-inserted on first run)

Written by `db::init_defaults()`; the frontend mirrors them in [src/stores/settings.ts](src/stores/settings.ts) and [src/utils/prefs.ts](src/utils/prefs.ts). Defaults are only inserted when the key is absent, so user changes survive restarts. Complex values are stored as JSON strings.

| Key | Default | Description |
|-----|---------|-------------|
| `active_datasource` | `tencent` | Active market data provider |
| `theme` | `light` | UI theme (dark/light) |
| `ticker_visible` | `1` | Ticker bar visibility (persisted on tray toggle **and** settings-page toggle) |
| `auto_launch` | `false` | OS-level autostart |
| `index_codes` | the original 7 | JSON array; order = left-to-right order on the index bar |
| `market_overview_visible` | `1` | `0` skips the overview entirely (no turnover/breadth/sector requests) |
| `sector_top_n` | `5` | Sector ranking rows shown. The settings page offers only `SECTOR_TOP_N_PRESETS` (5/10); the backend still clamps via `market::clamp_top_n` (1–50) |
| `ticker_transparent` | `0` | Ticker window background |
| `ticker_items_per_page` | `2` | Quotes shown per carousel page (1–`TICKER_ITEMS_MAX`, i.e. 10; the control shows presets up to `TICKER_ITEMS_PRESET_MAX` = 4 plus a 自定义 input). The upper bound doubles as the ticker window's max height |
| `watchlist_columns` | all 8 | JSON array of column keys in display order (never contains `ticker_enabled`) |
| `watchlist_default_sort` | `""` | JSON `{key, order}`; empty means "no sort" (watchlist order) |
| `color_scheme` | `cn` | `cn` = red-up/green-down (A-share), `us` = the reverse |
| `boss_key` | `""` | Global accelerator that hides the main window. Empty = not set — the app registers no global hotkey unless the user picks one |

The list lives in `Database::DEFAULT_SETTINGS` and every key name comes from the `db::keys` module — bare string literals are what this indirection exists to prevent: a typo in a key doesn't fail to compile, it makes `get_setting` return `None` and the caller silently take its fallback, so the only symptom is "that setting never takes effect". Keys are also read/written from several places (`window_x` and `ticker_x` each appear at three call sites), so a rename has to be a single edit.

`groups_migrated` is deliberately **not** in that table: the one-shot watch-group migration is guarded by its *absence*, so giving it a default would make the migration never run.

`sector_top_n` 的**界面**取值域（5/10）比**库**里的取值域（1–50）窄 —— 差额是「自定义」档移除留下的。老版本可能存过 8、20 这类值，而分段控件对它们一个都匹配不上，会显示成「全都不选中」，看着像坏了。所以 [MarketSection.vue](src/components/settings/MarketSection.vue) 挂载时把它们归到最近的档并写回（`nearestTopNPreset`，等距取小），且必须等 `loaded` 为真 —— 之前 `sectorTopN` 是兜底值，那时写回等于拿兜底值覆盖库里真正的设置。后端区间**没有**跟着收窄：`clamp_top_n` 是 IPC 边界上的兜底，而且 `concept_prefetch_size` 的上限推导与它的测试都建立在 1–50 上。行情条那一侧是反过来的 —— 预设（1–4）之外的值就是「自定义」，必须原样读回（见 [settings.spec.ts](src/stores/settings.spec.ts) 里 `9` 不被夹到 4 的那条）。

Four tests pin the contract between this table and the frontend: `default_settings_keys_are_pinned` (the key strings are the wire format — renaming one merely stops the frontend from reading it), `default_settings_have_no_duplicate_keys`, `migration_marker_is_not_a_default`, and the two wire-format checks `boolean_defaults_use_the_0_1_wire_format` / `numeric_defaults_parse` (the frontend's `parseBool` accepts only `"0"`/`"1"`, and `parseInt` silently yields NaN otherwise).

Two deliberate non-settings: the market-overview **refresh interval** is not configurable — there is no base polling interval at all, since `market_clock` sets the pace per session and the adaptive state machine adjusts from there (the settings page used to show a read-only row for it; both that row and the vestigial `refresh_interval` key are gone). **Ticker window width** stays fixed at 230 logical px (its height is measured from content, see `useTickerWindowHeight`).

### Window position persistence

Main window position/size is saved to SQLite `settings` table on move/resize/close. Keys: `window_x`, `window_y`, `window_width`, `window_height`. On next launch, restored with monitor-boundary validation (clamped to visible area if monitor config changed). Ticker window is always positioned at bottom-right via `available_monitors()` calculation.

## Development phases

| Phase | Status | Scope |
|-------|--------|-------|
| Phase 1 (MVP) | ✅ Complete | Scaffold, Sina adapter, tray, ticker, watchlist CRUD, index bar, dark theme |
| Phase 2 (Experience) | ✅ Complete | Detail panel (minute chart + depth + summary), Tencent adapter, column sorting, window position memory, market_clock dynamic polling |
| Phase 3 (Quality) | ✅ Complete | Code review fixes (36 items): logging, error handling, spawn_blocking, CSS tokens, accessibility, CSP, encoding_rs migration, design system, dead code cleanup |
| Phase 4 (Enhancement) | ✅ Partial | K-line chart (daily/weekly/monthly) ✅, chart auto-refresh ✅, adaptive polling (probe/idle) ✅, depth auto-refresh ✅, index detail panel ✅, auto-update ✅, 设置页（指数区/市场概览/自选列表/行情条/通用/关于）✅, 自选分组（多归属）✅, 行情条轮播范围与透明背景 ✅, price alerts 📋, import/export (JSON/CSV) 📋, auto-start ✅, packaging polish 📋 |
| Phase 5 (Extension) | 🔮 Future | HK/US market support, professional data sources (Wind/Tushare), macOS/Linux adaptation |

## CI/CD

- [release.yml](.github/workflows/release.yml) — triggered on `v*` tags or manual dispatch. Matrix build for Windows (MSVC), macOS (universal), Linux (gnu). Uploads `.exe`/`.msi`/`.dmg`/`.deb`/`.AppImage` artifacts.
- [ci.yml](.github/workflows/ci.yml) — push/PR CI: `vue-tsc` + vitest (ubuntu), `cargo check` in both feature universes (default and `--features store`) + `cargo test` (windows — the store universe's Windows-only code only compiles there).
- [pages.yml](.github/workflows/pages.yml) — 官网发布：跑 `node scripts/site-assets.mjs` 组装并校验 `website/`，push 到 master 时部署到 GitHub Pages；PR 上只校验不部署（`configure-pages` 可能尝试开启 Pages，而来自 fork 的 PR 拿不到那个权限）。
- [store-release.yml](.github/workflows/store-release.yml) — manually dispatched Microsoft Store (MSIX) build: `tauri build --features store --config tauri.microsoftstore.conf.json`, then repacks the intermediate MSI into an unsigned MSIX (Store-signed on ingestion) via `msiexec /a` + `makeappx` with [store/AppxManifest.xml](store/AppxManifest.xml). The `store` cargo feature disables the built-in updater (Store distributes updates) and switches Windows autostart to the StartupTask API.

`scripts/build.mjs` provides a cross-platform build wrapper with automatic proxy detection (Clash/V2Ray on common ports 7890/10809/1080/8118/8080/1087/4780).
