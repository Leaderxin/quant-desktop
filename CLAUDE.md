# CLAUDE.md

本文件只记「代码里看不出来、踩过会再踩」的约定、操作步骤与坑。历史与取舍过程写在代码注释或提交信息里，不在这里复述。

## Build & Run

```bash
npm ci                      # 确定性安装
npm run tauri dev           # Vite + Tauri 开发模式
npm run tauri:build         # 生产构建（updater 产物需要签名变量，见下）
node scripts/build.mjs      # 跨平台构建 + 本地代理自动探测（Clash/V2Ray 常见端口）
npx vue-tsc --noEmit        # 前端类型检查
npm test                    # vitest；npm run test:watch 监听
cargo build --manifest-path src-tauri/Cargo.toml
```

生产构建的签名变量：

```powershell
$env:TAURI_SIGNING_PRIVATE_KEY = Get-Content "$env:USERPROFILE\.tauri\quant-desktop.key"
$env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = "your-password"
```

测试约定：

- **只 mock Tauri 边界**（`@tauri-apps/api/*` 的 `invoke`/`listen`/`emit`）。Pinia 与 store 用真的 —— 被钉的行为就是 store 自己的。需要 `document` 的 spec 在文件头加 `// @vitest-environment happy-dom`，不切全局环境。
- **组件测不了的纯逻辑抽到 `src/utils/`**：`dragSort`、`paging`、`watchGroups`、`minuteBars`、`minuteAxis`、`theme`、`webLog` 都是这么来的。

覆盖范围（细节读 spec 本身）：

| Spec | 钉住的行为 |
|------|-----------|
| `stores/market.spec.ts` | 刷新调度并发：过期响应不落地、in-flight 与切时段交错 |
| `stores/watchlist.spec.ts` | 派生顺序（分组顺序来自关联表）、悬空 id、`ticker_order`、乐观 `setTickerEnabled`、**IPC 参数拼写**（Rust snake_case / JS camelCase，拼错只表现为反序列化失败） |
| `stores/settings.spec.ts` | 派生配置兜底、写入前夹取、`applyRemoteSetting` 不回写不广播；主题三态与解 pin 顺序 |
| `utils/theme.spec.ts` | 「跟随系统」解析：显式模式不看系统、无 `matchMedia` 时按浅色且不抛错、订阅/退订 |
| `utils/prefs.spec.ts` | 任何损坏/旧版设置值都给出可用默认值 |
| `utils/csv.spec.ts` | CSV 编码（UTF-8 严格优先、失败才退 GBK）、引号内逗号/换行、表头；**行号按物理行**；与 Excel 网格共用 `mapWatchlistRows` |
| `utils/format.spec.ts` | 「较昨同期」增量的符号、不足一亿按持平、缺失给空串 |
| `utils/hotkey.spec.ts` | 加速键字符串两端一致、修饰键顺序、裸键/只带 Shift 的组合不得当热键 |
| `utils/changelog.spec.ts` | 更新说明解析：版本/日期/小节、CRLF、坏输入不抛错 |
| `utils/dragSort.spec.ts` / `paging.spec.ts` | 拖拽落点（四向 × 前后）、轮播窗口（短列表不自转） |
| `utils/watchGroups.spec.ts` | 删除分组确认框的两份名单（顺序、去重、目标不存在给空） |
| `utils/minuteBars.spec.ts` / `minuteAxis.spec.ts` | 分时图交易日切分、昨收、未来 bar 的 UTC+8 上界；网格槽位/偏移/刻度与窄宽回退 |
| `composables/useTickerWindowHeight.spec.ts` | 行情条改高时下边缘不动 |
| `utils/webLog.spec.ts` | 日志桥：参数拼接、字符截断、去重窗口、窗口前缀 |

不覆盖，别把缺席当疏漏：Vue 组件渲染（无 `@vue/test-utils`）、拖拽 DOM 事件（只测落点算术）、纯视觉（配色、间距、图标）。`cargo test` 覆盖 Rust 侧（短信分类、响应解析、时段、分组归属/迁移、日志工具）；`vue-tsc` 负责类型；暂无 lint 命令。

## Git 提交信息

提交信息中**不要出现 "Claude" 字眼** —— 包括 `Co-Authored-By: Claude`、`Generated with Claude Code` 之类的工具署名尾注。提交历史面向仓库读者，工具来源只是噪音；把改了什么、为什么改写清楚即可。

## Architecture

QuantDesktop 是监控 A 股的 Tauri 2 桌面应用：两个独立 webview 窗口、SQLite 后端、可切换的数据源适配器。

### 两个窗口

| 窗口 | Label | 入口 | 配置 |
|------|-------|------|------|
| 主界面 | `main` | `index.html` → `src/main.ts` | 1100×680，启动隐藏，关闭即隐藏到托盘，位置/大小存库 |
| 行情条 | `ticker` | `ticker.html` → `src/ticker.ts` | 230×38，置顶、无边框、右下角、不进任务栏 |

[vite.config.ts](vite.config.ts) 配了两个 Rollup 入口；两个窗口各自挂一个 Vue app、各自一个 Pinia 实例，store 与 composable 通过 import 共用。

### Rust 后端（`src-tauri/src/`）

**`lib.rs`** — 应用装配：开库、注册数据源（腾讯默认，新浪备用）、恢复报价缓存、起后台 `Scheduler`、建托盘（左键显示/隐藏、右键 显示 / 行情条 / — / 检查更新 / 设置 / 退出）、注册全部 IPC、初始化 updater。主窗口的 `CloseRequested` 被拦成隐藏。窗口位置/大小落库并在下次启动时按显示器边界校验后恢复。

三个必须单点实现的窗口 helper（托盘与设置页共用，各写一份必漂）：

- `set_ticker_visible(app, db, visible)`：显示时依次 show → 置顶 / 隐藏任务栏 / `WS_EX_TOOLWINDOW` / 还原位置，并持久化 `ticker_visible`。**顺序不能反**：Windows 的 `set_skip_taskbar` 走 `ITaskbarList::DeleteTab`，窗口必须已经真正显示过才生效。
- `show_main_window(app)`：`show()` 之后必须 `unminimize()` —— Windows 的 `SW_SHOW` 不会退出最小化，用户只看到任务栏图标闪一下。托盘左键还要判 `is_minimized()`：最小化窗口在 Win32 眼里仍是 `WS_VISIBLE`，只判 `is_visible()` 会把想叫回来的窗口又藏起来。托盘的「设置」在显示窗口后还要 `emit("open-settings")`（webview 常驻，隐藏时也收得到）。
- `set_app_theme(app, theme)`：参数是**模式**（`dark`/`light`/`system`），不是解析后的主题。`system`（含缺键、坏值）必须传 `None` 给 tao —— 一旦显式设过主题，tao 会忽略系统主题变化，WebView2 的 `prefers-color-scheme` 被一起钉死，「跟随系统」就失效了。启动那次必须**先于** `main.show()`（否则先闪一帧白条）；运行期挂在 store 的模式 watch 上。只作用于 `main`（ticker 无边框、也从没被钉过）。

**`domain/mod.rs`** — IPC 数据类型，对应 [src/types/index.ts](src/types/index.ts)：`Quote`、`IndexQuote`、`Depth`/`Level`、`MinuteData`、`KLineData`、`StockBrief`、`Market`（CN/HK/US）。

**`db/mod.rs`** — SQLite（rusqlite bundled），五张表：`watchlist` / `watch_groups` / `watch_group_members` / `settings` / `quote_cache`。

- 路径由 `dirs::data_dir()/quant-desktop` 决定，**不来自** Tauri `identifier`；Windows 实为 `%APPDATA%\quant-desktop`。便携模式（exe 旁有 `portable.dat`）用 `<exe_dir>/data`。Store(MSIX) 构建仍解析 `%APPDATA%`，由文件系统虚拟化重定向到包私有目录 —— 与 NSIS/便携版数据分离、卸载即删。启动日志会打印 `Data directory: ... (portable: ...)`。
- 建表与默认设置在首次打开时完成；`init_defaults()` 只在键不存在时写入，用户改动跨重启保留。
- 迁移在每次 `Database::open()` 都跑，必须保持幂等。

**自选是多分组（标签式）归属**：`watchlist` 是全局池（`UNIQUE(code, market)`），`ticker_enabled`/`ticker_order` 在池上（行情条是跨分组扁平列表）；`watch_group_members` 是 (group, stock) 多对多并带**组内** `sort_order`。1.5.x 的全局 `sort_order` 列留在表里但不再读写（删列要重建表）。

| 场景 | 行为 |
|------|------|
| 加入已在其它分组的股票 | 追加目标分组，不移动 |
| 加入已在同组的股票 | 无操作；保留原组内位置 |
| 从分组移除 | 若是它**唯一**的分组，股票一并删除（`remove_watch_from_group` 返回 `true`） |
| `set_watch_groups` 传空列表 | 拒绝；全不勾是易误触，静默删数据代价太大。彻底删除走 `remove_watch` |
| 删除分组 | 只删归属；孤儿并入兜底分组，仍在其它分组的不动；最后一个分组不可删 |
| 重命名分组 | 拒绝空名、超长、重名 |

迁移用 `groups_migrated` 标记而不是「关联表是否为空」：用户可能合法地清空所有分组的成员而池里仍有行，空表检查会在下次启动重新播种。老数据按旧全局 `sort_order` 填入组内顺序，并回填 `ticker_order`。

**`datasource/`** — `DataSource` trait：`fetch_realtime` / `fetch_indices` / `search` / `fetch_depth` / `fetch_minute_data` / `fetch_kline` / `health_check`。`DataSourceManager` 持有适配器注册表与 active 名，切换时用 `tokio::sync::Notify` 唤醒调度器立即刷新。

- `INDEX_CODES` 是 14 个宽基指数的**候选池**，每轮一次性批量拉取，前端按 `index_codes` 过滤排序 —— 这样设置里勾选后立刻显示，不用等下一轮（交易中 2s，休市最长 30s）。`INDEX_POOL_NAMES` 是设置页用的名称表；新增代码必须**同时**在腾讯与新浪上验证。
- 量/额归一：个股原始单位是手、万元，进共享管道前 `normalize_volume(×100)`、`normalize_turnover(×10000)` 转成股、元。
- `sina.rs`（备用）：GBK、`var hq_str_`。指数必须用**股票格式**接口（不带 `s_` 前缀）——紧凑格式对 `s_sz399006` 的量额是错的；上证指数成交额在所有格式下都只有 1/100，需 `saturating_mul(100)`；指数数据已是股/元，不再乘。搜索只认精确代码；深度借腾讯；分时/K 线走 `money.finance.sina.com.cn`。
- `tencent.rs`（默认）：GBK、`v_sh600519=` + `~` 分隔；分时 `ifzq.gtimg.cn`、K 线 `web.ifzq.gtimg.cn`、深度取内嵌字段（位置 9–28）。
- `market_clock.rs`：`MarketSession`（PreOpen/MorningTrade/LunchBreak/AfternoonTrade/Closed，含周末）。`recommended_interval()`：交易 2s、盘前 5s、午休 10s、休市 30s；概览另用 `overview_interval()`。

**`hotkey.rs`** — 全局老板键（按下只隐藏主窗口，不动行情条、不改设置）。注册放在 Rust 侧，因为要响应时应用没有任何焦点。

- `apply(app, accelerator)` 是唯一注册入口，空串 = 不设；**先 `unregister_all()` 再注册**，否则旧键会继续响应且设置页上看不到。
- `set_boss_key` 先注册成功再落库；注册失败与落库失败都回滚旧键（前者会先注销全部，不回滚就把原来能用的键也弄丢；后者会让系统与库不一致）。
- 启动时 `register_from_db` 读库注册，失败只记 warning，不拦启动。
- 加速键词表是 global-hotkey 的 `Code` 名，与 `KeyboardEvent.code` 基本一致，前端直接取 `e.code`；两端各写一份由 [src/utils/hotkey.ts](src/utils/hotkey.ts) 与 spec 对齐。插件钉 `~2.3`（2.4 起要求 tauri ^2.12）。

**`cache/mod.rs`** — `QuoteCache`：内存 `HashMap` + SQLite 双写，启动时 `restore_from_db()`。`Scheduler` 按会话跑自适应状态机：

| 状态 | 触发 | 间隔 |
|------|------|------|
| Probing（3×2s） | 进入早/午盘 | 2s |
| Normal | 探测到价格变化 | `market_clock` 基准（2s） |
| Idle | 连续 10 次无变化 / 节假日 | 30s |

调度器按市场分组批量拉报价、更新缓存，发 `quotes-updated` / `indices-updated` / `market-session-changed`；数据源切换有单独的 wakeup 监听。抓取失败回退缓存；Closed 时段优先用缓存。

**`commands/`** — IPC 处理函数，全部在 `lib.rs` 注册：

- `quote.rs` — `get_quotes`/`get_indices` 读缓存；`get_depth`/`get_intraday`/`get_kline` 走活跃源，分时与 K 线在活跃源失败/数据不足时回退备用源。
- `watchlist.rs` — `get_watchlist` 一次返回 `WatchlistSnapshot { items, groups, default_group_id }`，`groups[].watch_ids` 带有序成员，切分组是纯本地过滤。池/归属/组内排序/分组/行情条范围各组命令见文件；组内排序一律按 `(group_id, watch_id)` 定位。写操作后发 `watchlist-changed`。
- `settings.rs` — 键值读写、老板键、切数据源（**先落库再切**，否则重启会回退）、数据源与指数池列表、便携/商店标志。
- `autostart.rs` — 非商店版走 registry Run key；Store 版走 StartupTask WinRT。
- `window.rs` — 三个窗口 helper 的 IPC 包装。
- `import.rs` — 自选导入：一次 IPC 完成匹配 + 落库 + 逐行结果（`added`/`exists`/`failed`），只发一次 `watchlist-changed`。模板是 xlsx（表头要底色、代码列设为文本，`000001` 不被 Excel 吃掉前导零）；`.xlsx/.xls` 由后端拆网格，CSV 由前端解析，**两者共用 `mapWatchlistRows`**。匹配只认完全相等：代码优先、名称兜底；代码命中多只（沪深同号）时用名称再筛；不足 6 位的纯数字按左补零**但必须由名称证实**，否则报「不足 6 位」，且搜索关键字用名称。`exists` 按**导入前**快照判断。逐行搜索限 6 并发，单次上限 500 行。
- `market.rs` — 市场概览。`top_n` 后端再夹一次（IPC 参数不能假定已校验）。行业榜先**合并同名板块**再截取（东财 `m:90+t:2` 混了申万各级，「白酒Ⅱ/Ⅲ」成分相同）；概念榜过滤 = 代码表（风格/指数/资金/业绩各期）+ 名字规则（业绩预告、持仓、风格），后者不可省 —— 东财每季新开板块。`turnover_delta` 必须**同源同时刻**（今日与昨日曲线都取腾讯分时），混用两家口径的误差与真实增减同量级；拿不到给 `null`（面板隐藏），不给 0。
- `updater.rs` — 更新检查/安装；交易时段抑制提示；Store 版运行时早退（命令仍注册）。
- `log.rs` — `log_event`，前端日志落盘。

### Frontend（`src/`）

**Stores（Pinia）**
- `quote.ts` — 监听行情事件，`Map<"market:code", Quote>` 做 O(1) 查找；`indices` 存整个候选池，`IndexBar` 按设置过滤排序。
- `watchlist.ts` — 只存快照，其余全部本地派生（`activeGroup`/`visibleItems`/`tickerItems`）。写操作一律「一次 IPC + 重拉快照」，不做乐观更新 —— 归属、顺序、孤儿处理都在后端，猜会漂；唯一例外是 `setTickerEnabled` 先翻本地再同步。
- `settings.ts` — 键值表 + 全部派生为 `computed`（单一口径）。**主题是三态**：库里的 `themeMode` ∈ `system`/`dark`/`light`，窗口用的 `theme` 只会是 `dark`/`light`。`theme` 变 → 立即落 `<html data-theme>`（`immediate`，首帧前就位）；`themeMode` 变 → 先把**模式**交原生层，**再**重读 `matchMedia`（顺序反了读到的是被钉的值）；系统主题变化只在 `system` 下采纳。`setSetting` 广播 `settings-changed`，ticker 用 `applyRemoteSetting` 就地更新且**不回写不重播**。
- `updater.ts` — 更新状态机，监听后端事件。

**`utils/prefs.ts`** — 设置值域与解析。所有值都是字符串，结构化值存 JSON；每个 parser 都回退到可用默认值而不抛错。`ALL_COLUMNS` 是列的唯一清单，`parseColumns` 会补回被坏配置弄丢的 `code`/`name`。

**`utils/webLog.ts`** — 前端日志桥（见 Logging）。

**组件地图** — `AppLayout` 之下两块：设置页（`components/settings/`，覆盖式整页，`KeepAlive` 保留分区状态，`v-if` 卸载看盘界面以停轮询并在返回时重建自选表）；看盘界面 `TopBar` / `IndexBar` / `MarketOverviewPanel` / `WatchlistTable`（含 `GroupTabs`、添加/删除/导入弹框、`GroupPicker`）/ `StockDetail`（分时/K 线/深度/摘要）/ `StatusBar`。不要在这里维护完整层级树 —— 代码即地图。

- 设置页共用原语：`SettingsRow`（标签 + 常驻说明，不靠 placeholder）、`SegmentedControl`（与 `ChartSwitcher`、概览方向同一套视觉）、`DragSortList`（拖拽 + `Alt+↑/↓` + 行内按钮）。开关用 `NSwitch size="small"`，下拉用 `NSelect size="small"`（原生 `<select>` 的展开列表不跟主题）。共用样式在 [settings.css](src/assets/styles/settings.css)，以 `.settings-page` 为祖先。
- 分组删除确认（[DeleteGroupDialog.vue](src/components/watchlist/DeleteGroupDialog.vue)）：两份名单逐只列名称 + 代码；孤儿并入**展示顺序里第一个别的分组**（不能取 `defaultGroupId`，被删的恰是默认分组时那正是它自己；与后端 fallback 规则一致）；最后一个分组时菜单项禁用。
- 自选删除确认（[DeleteStockDialog.vue](src/components/watchlist/DeleteStockDialog.vue)）：列出会从哪些分组消失。只剩一个分组时「从本组移除」与「删除自选」等价，菜单里只留后者。两个确认框共用 [ConfirmDialog.vue](src/components/common/ConfirmDialog.vue) 外壳（默认焦点在「取消」）。
- 行情条（`TickerBar`）：跨分组按 `ticker_order` 轮播，每屏 `ticker_items_per_page` 条、3 秒翻页，悬停暂停，点击恢复主窗口；只播 `ticker_enabled`；`watchlist-changed` 触发重拉。透明背景由 `document.body` 的 `ticker-transparent` class 控制，底板/圆角/阴影**一起**去掉（只改 background 会留脏边），窗口在 `tauri.conf.json` 里开了 `transparent: true`。
- Composables：`useTauriEvent`（生命周期包装）、`useTheme`、`useChart`（分时/K 线共用）、`useUpdateCheck`（交易时段抑制）。
- Styles：`variables.css`（设计令牌；涨跌配色的 `[data-color-scheme="us"]` 必须放在两个主题块之后，浅色变体用复合选择器提高特异性；派生 `-bg`/`-bar` 必须跟着主色换）、`dark.css`（滚动条）、`chart.css`、`settings.css`、`buttons.css`（弹框按钮 `.btn`/`.btn-ghost`/`.btn-danger`，28px，确认框与导入框共用）。

### Data flow 与事件

```
DataSource → Scheduler → QuoteCache（内存 + SQLite）
  → emit quotes-updated / indices-updated / market-session-changed
    → Pinia stores → Vue 组件（主窗口 + 行情条）
```

按需请求：深度 3s、分时 5s、日 K 30s、周/月 K 60s。分时/K 线的关键约束（日期来自数据而非本机时钟、以 UTC+8 墙钟过滤未来 bar、网格按会话固定、composable 自己注册卸载）见 [minuteBars.ts](src/utils/minuteBars.ts)、[minuteAxis.ts](src/utils/minuteAxis.ts)、[useMinuteChart.ts](src/composables/useMinuteChart.ts) 的注释与 spec。

自选写操作：`invoke` → Rust → SQLite → 前端重拉；每个影响自选的命令都发 `watchlist-changed`，行情条靠它同步。数据源切换走 `Notify` 立即刷新。

跨窗口设置：`setSetting` 写库并广播 `settings-changed`；ticker 直接 `applyRemoteSetting`，**不得回广播**（会来回弹）。`datasource-changed` 单独一条，因为它还有每窗口的副作用。主题不需要专线：两个窗口各自用本窗口的 `matchMedia` 解析 —— ticker 的窗口没被钉过，读到的是系统真值。

K 线：蜡烛按 `compareRule: 'previous_close'`（红涨绿跌灰平），wick 颜色显式对齐，底部是带 MA5/10/20 的 `VOL`。

### Logging

日志与数据库同目录：`%APPDATA%\quant-desktop\quant-desktop.log`（便携模式 `<exe_dir>/data/`）。

- **本机时区**；**追加写 + 2MB 轮转**（启动与运行期都检查，改名 `.log.1`）；级别由 `QUANT_LOG` 覆盖（默认 info）；panic hook 落一条 error（release 是 GUI，stderr 不可见）。
- 前端经 `log_event` 落同一份文件，前缀 `[web] [main|ticker]`；[bootstrapLog.ts](src/bootstrapLog.ts) 必须排在 App / TickerBar 的 import 之前（静态 import 先求值整张依赖图）。捕获 `console.error/warn`、`window.onerror`、`unhandledrejection`；同一句 5 秒内只转发一次，单条封顶 4000 字符。
- 高频轮询失败一律 `logging::warn_throttled`（同 key 5 分钟一条，抑制的仍在 debug）；跨窗口事件走 `events::emit_or_warn`；自选写库失败 warn、破坏性操作 info、导入每次一行汇总。

### 默认设置

由 `db::init_defaults()` 写入；前端镜像在 [settings.ts](src/stores/settings.ts) 与 [prefs.ts](src/utils/prefs.ts)。只在键不存在时写入，复杂值存 JSON。

| Key | 默认 | 说明 |
|-----|------|------|
| `active_datasource` | `tencent` | 数据源 |
| `theme` | `system` | `system` 跟随系统（Windows「默认应用模式」）/`dark`/`light` 显式。首启默认跟随系统：透明行情条的文字要与桌面深浅一致才可读 |
| `ticker_visible` | `1` | 行情条可见性（托盘与设置页都持久化） |
| `auto_launch` | `false` | 开机自启 |
| `index_codes` | 原始 7 个 | JSON 数组，顺序即指数条从左到右 |
| `market_overview_visible` | `1` | `0` 时整块概览不发请求 |
| `sector_top_n` | `5` | 榜单条数；界面只给 5/10，后端仍按 `clamp_top_n` 夹到 1–50 |
| `ticker_transparent` | `0` | 透明背景 |
| `ticker_items_per_page` | `2` | 每屏条数 1–10（预设到 4 + 自定义）；上限也就是行情条最大高度 |
| `watchlist_columns` | 全部 8 列 | JSON 数组，顺序即列顺序，不含 `ticker_enabled` |
| `watchlist_default_sort` | `""` | JSON `{key, order}`，空 = 不自排 |
| `color_scheme` | `cn` | `cn` 红涨绿跌 / `us` 反过来 |
| `boss_key` | `""` | 空 = 不注册任何全局热键 |

键名一律从 `db::keys` 取：裸字符串拼错不会编译失败，只会让 `get_setting` 返回 `None`、调用方静默用兜底值，症状是「这个设置永远不生效」。

`groups_migrated` **故意不在默认表里**：迁移靠它不存在来触发，给默认值会让迁移永不执行。

`sector_top_n` 的界面域（5/10）比库里（1–50）窄：老版本可能存过 8/20，分段控件匹配不上会显示「全都不选中」。`MarketSection` 挂载（且 `loaded` 为真）时归到最近档并写回；后端不跟着收窄（`clamp_top_n` 是 IPC 兜底，概念榜预取上限也按 1–50 推导）。行情条相反：预设之外的值就是「自定义」，必须原样读回。

以下测试把这张表和前端钉在一起：键名不可改（就是 wire format）、无重复键、`groups_migrated` 不是默认键、布尔只认 `"0"/"1"`、数字可解析。

两个刻意不是设置的东西：市场概览的刷新间隔不可配（节奏全由 `market_clock` + 自适应状态机决定）；行情条宽度固定 230 逻辑像素（高度按内容测量，见 `useTickerWindowHeight`）。

### 窗口位置

主窗口的 `window_x`/`window_y`/`window_width`/`window_height` 在移动/缩放/关闭时写入 `settings`，下次启动按显示器边界校验后恢复。行情条固定右下角（`available_monitors()` 计算）。

### 官网（`website/`）

独立静态站，发布在 <https://leaderxin.github.io/quant-desktop/>，不在 Vite 入口里、不参与 `npm run build`、不依赖 npm 包。应用内唯一入口是状态栏的 `SITE_URL`。

改完跑 **`npm run site:assets`**（资源复制 + 引用校验 + 顶栏一致性校验 + sitemap 生成）。截图、配色、SEO、内容页与图片管线的具体约束见 [website/CLAUDE.md](website/CLAUDE.md)。

## CI/CD

- [release.yml](.github/workflows/release.yml) — `v*` tag 或手动触发；Windows MSVC / macOS universal / Linux gnu 矩阵，产出 exe/msi/dmg/deb/AppImage。
- [ci.yml](.github/workflows/ci.yml) — push/PR：`vue-tsc` + vitest；`cargo check` 跑 default 与 `--features store` 两套 + `cargo test`（Windows，商店版的 Windows-only 代码只能在这里编译）。
- [pages.yml](.github/workflows/pages.yml) — 官网发布：跑 `site-assets.mjs` 组装校验，push 到 master 部署；PR 只校验（fork 拿不到开启 Pages 的权限）。
- [store-release.yml](.github/workflows/store-release.yml) — 手动触发 MSIX：`tauri build --features store --config tauri.microsoftstore.conf.json`，再用 `msiexec /a` + `makeappx` 重打包（商店签名）。`store` feature 关掉内置 updater、把 Windows 自启换成 StartupTask。

## 依赖版本钉子

`tauri-plugin-dialog` 钉 `~2.7`、npm 侧 `@tauri-apps/plugin-dialog ~2.7.3` 且 `@tauri-apps/api ~2.11.0`（2.8 起要求 tauri ^2.12；`tauri build` 会核对两侧 major/minor，漂移直接挂）；`tauri-plugin-global-shortcut` 钉 `~2.3`（2.4 同理）。图标统一用 `@lucide/vue`（旧包 `lucide-vue-next` 已弃用）；图表用 klinecharts v10 beta。
