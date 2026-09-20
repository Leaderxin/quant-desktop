# QuantDesktop

桌面级 A 股行情监控工具，基于 Tauri 2 + Vue 3 + Rust 构建。

当前版本：**v1.5.1**

> 本文是技术文档（技术栈 / 项目结构 / 数据流 / 构建发布）。功能介绍与截图见 [README.md](README.md)。

## 功能

- **实时行情** — 自选股批量刷新，交易时段自适应轮询（探测→正常→空闲三态），可切换腾讯证券 / 新浪财经数据源，覆盖沪深京三市个股、指数与 ETF
- **大盘指数** — 上证指数、深证成指、创业板指、科创 50、科创 100、中证 500、科创综指实时展示
- **市场概览** — 两市成交额、涨跌家数、行业/概念板块涨跌排行；四块数据独立拉取（新浪成交额 + 东财涨跌家数 + 东财行业/概念板块），任一失败只降级对应字段，面板整体不消失
- **个股详情** — 点击自选行展开详情面板，包含分时趋势图 / K 线图（全周期）、五档盘口、基本面概要（开/高/低/量/额/换手率）
- **K 线图表** — 日 K / 周 K / 月 K / 1 / 5 / 15 / 30 / 60 分钟，主图支持 MA / BOLL 叠加指标一键切换，副图支持成交量 / MACD 一键切换，成交量含 MA5/MA10/MA20 均量线，tooltip 显示涨跌幅，蜡烛影线颜色与实体一致，涨跌颜色遵循 A 股习惯（与当日开盘价对比，红涨绿跌）
- **K 线左滑加载历史** — 各周期 K 线图向左滑动拉取更早数据
- **指数详情** — 点击指数卡片展开指数详情面板，含分时图、K 线图、成交量/成交额概要
- **自动刷新** — 详情面板打开时，五档盘口 3 秒刷新，分时图 5 秒刷新，日 K 30 秒刷新，周/月 K 60 秒刷新；市场概览按交易时段自适应间隔刷新
- **浮动行情条** — 桌面置顶迷你行情条，2 只股票 3 秒自动轮播，鼠标悬停暂停，点击恢复主窗口；支持拖拽到任意位置且位置持久化，窗口高度跟随内容实际高度、适配系统显示缩放，可在自选表中逐只开关是否参与播报
- **系统托盘** — 关闭窗口最小化至托盘，左键单击切换显示/隐藏，右键菜单操作
- **自选管理** — 添加、删除、排序（置顶 / 上移 / 下移 / 右键菜单），搜索支持跨数据源回退，市场标签（沪A/深A/北A/沪指/ETF…）区分同代码的指数与个股
- **列排序** — 自选表格支持按涨跌幅、价格、成交量、代码等列排序
- **深色 / 浅色主题** — 一键切换，CSS 变量驱动，行情条同步响应
- **窗口记忆** — 主窗口与行情条位置/大小自动保存，重启恢复，跨显示器边界保护
- **离线缓存** — 行情数据写入 SQLite，重启即时恢复上一次报价
- **自适应轮询** — 交易时段进入时快速探测（3 次 ×2s），确认开市后正常轮询（2s），连续 10 次无价格变化自动降频（30s），节假日智能休眠
- **自动更新** — 启动时自动检测新版本，交易时段智能抑制弹窗，CHANGELOG 展示，一键下载安装（Microsoft Store 版由商店分发更新，内置更新器禁用）
- **开机自启** — 状态栏开关，一键启用/禁用（NSIS/绿色版走注册表 Run 键，商店版走 StartupTask WinRT API）

## 技术栈

| 层 | 技术 |
|---|------|
| 桌面框架 | Tauri 2 |
| 前端 | Vue 3 + TypeScript + Pinia + Naive UI |
| 图表 | KLineChart v10 |
| 构建 | Vite + vue-tsc |
| 后端 | Rust (tokio, reqwest, rusqlite, chrono, serde, async-trait) |
| 数据源 | 腾讯证券 (默认)、新浪财经 (备用) — GBK 解码；市场概览聚合新浪 + 东方财富公开接口 |
| 持久化 | SQLite (rusqlite bundled) |
| 自动更新 | Tauri updater plugin |
| 商店分发 | MSIX (makeappx，`store` cargo feature) |

## 开发

### 前置要求

- [Node.js](https://nodejs.org/) >= 18
- [Rust](https://www.rust-lang.org/tools/install) 工具链
- Windows 平台需安装 [Microsoft Visual C++ Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/)

### 开始

```bash
# 安装依赖
npm install

# 启动开发模式（同时启动 Vite 和 Tauri）
npm run tauri dev

# 类型检查
npx vue-tsc --noEmit

# 仅编译 Rust 后端
cargo build --manifest-path src-tauri/Cargo.toml
```

### 构建

```powershell
# Windows PowerShell: 设置签名环境变量（更新功能需要）
$env:TAURI_SIGNING_PRIVATE_KEY = Get-Content "$env:USERPROFILE\.tauri\quant-desktop.key"
$env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = "your-password"
npm run tauri:build

# 或使用带代理自动检测的构建脚本
node scripts/build.mjs
```

> 签名密钥由 `npx tauri signer generate` 生成。CI 构建设置 `TAURI_SIGNING_PRIVATE_KEY` 和 `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` 两个 Secrets 即可。

| 平台 | 产物 |
|------|------|
| Windows | NSIS `.exe` / `.msi` / 便携版 `.zip` |
| macOS | `.dmg` + `.app` |
| Linux | `.deb` / `.rpm` / `.AppImage` |

> 产物输出在 `src-tauri/target/release/bundle/`。

### Microsoft Store（MSIX）构建

商店版通过手动触发的 [store-release.yml](.github/workflows/store-release.yml) 工作流构建：

```bash
tauri build --features store --config tauri.microsoftstore.conf.json
```

- `store` cargo feature 会**禁用内置更新器**（更新由商店分发）并将 Windows 开机自启切换为 StartupTask WinRT API
- 构建产物经 `msiexec /a` + `makeappx` 重打包为未签名 MSIX（商店上架时由 Store 签名），清单见 [store/AppxManifest.xml](store/AppxManifest.xml)
- 商店版数据经 MSIX 文件系统虚拟化隔离在包私有目录，与 NSIS/绿色版数据互不影响

### 绿色版（Portable Zip）

绿色版是一个免安装的 `.zip` 包，解压后直接运行 `quant-desktop.exe` 即可使用，**所有数据（数据库、日志）存储在 exe 同级 `data/` 目录下**，不写入系统 `%APPDATA%`，适合 U 盘携带或多版本并存。

#### 工作原理

程序启动时检测 exe 同级目录是否存在 `portable.dat` 空文件：

| `portable.dat` | 数据目录 | 更新方式 |
|---|---|---|
| 存在 | `<exe 目录>/data/` | 手动下载新 zip 覆盖 |
| 不存在 | `%APPDATA%/quant-desktop/` | 内置自动更新 |

绿色版会自动隐藏"检查更新"按钮（状态栏）和托盘菜单中的更新选项，启动时也不会自动检测更新。

#### 本地打包

```powershell
# 1. 先执行完整构建
npm run tauri build

# 2. 手动创建绿色版 zip
$src = "src-tauri\target\release"
$staging = "portable\quant-desktop"
mkdir $staging -Force > $null
Copy-Item "$src\quant-desktop.exe" -Destination "$staging\"
New-Item -ItemType File -Path "$staging\portable.dat" > $null
Compress-Archive -Path "$staging\*" -DestinationPath "$src\bundle\quant-desktop_1.5.1_x64-portable.zip"
```

> 产物：`src-tauri\target\release\bundle\quant-desktop_<version>_x64-portable.zip`

#### CI 自动打包

CI（`.github/workflows/release.yml`）在 Windows 构建后自动执行上述打包步骤，并上传到 GitHub Release。推 tag 即可触发，无需手动操作。

#### 绿色版 vs 安装版

| | 安装版（`.exe` / `.msi`） | 绿色版（`.zip`） |
|---|---|---|
| 安装方式 | 运行安装向导 | 解压即用 |
| 数据位置 | `%APPDATA%/quant-desktop/` | `<exe 目录>/data/` |
| 自动更新 | ✅ 后台静默更新 | ❌ 手动下载替换 |
| 开机自启 | ✅ 支持 | ⚠️ 支持，但移动目录后失效 |
| 注册表 | 写入卸载信息 | 无残留 |
| 适用场景 | 日常固定使用 | U 盘携带、多版本测试 |

## 项目结构

```
quant-desktop/
├── src/                          # Vue 前端
│   ├── main.ts                   # 主窗口入口
│   ├── ticker.ts                 # 行情条入口（独立 Vue 应用）
│   ├── App.vue                   # 根组件（Naive UI 主题配置）
│   ├── types/index.ts            # 前端类型定义（与 Rust domain 对应）
│   ├── stores/                   # Pinia 状态管理
│   │   ├── quote.ts              # 行情数据（事件驱动，Map 索引）
│   │   ├── market.ts             # 市场概览（成交额/涨跌家数/板块排行，按时段自适应刷新）
│   │   ├── watchlist.ts          # 自选股 CRUD
│   │   ├── settings.ts           # 设置（主题、数据源切换、开机自启）
│   │   └── updater.ts            # 更新状态（检测/下载/安装）
│   ├── composables/
│   │   ├── useChart.ts           # K 线图表逻辑（加载、自动刷新、周期管理）
│   │   ├── useChartCore.ts       # 图表核心（KLineChart 初始化、A 股配色、红涨绿跌）
│   │   ├── useMinuteChart.ts     # 分时图逻辑（均价线、增量刷新）
│   │   ├── minutePeriod.ts       # 分钟 K 线周期定义（1/5/15/30/60）
│   │   ├── useTickerWindowHeight.ts # 行情条窗口高度自适应
│   │   └── useUpdateCheck.ts     # 更新检测（启动检测 + 交易时段门控）
│   ├── components/
│   │   ├── layout/
│   │   │   ├── AppLayout.vue     # 主布局
│   │   │   ├── TopBar.vue        # 顶栏（标语、数据源切换）
│   │   │   └── StatusBar.vue     # 底栏（版本号、检查更新、主题、开机自启、联系）
│   │   ├── index/
│   │   │   ├── IndexBar.vue      # 指数容器
│   │   │   └── IndexCard.vue     # 单条指数（点击展开详情）
│   │   ├── market/
│   │   │   └── MarketOverviewPanel.vue # 市场概览（涨跌分布、板块排行，可折叠）
│   │   ├── watchlist/
│   │   │   ├── WatchlistTable.vue # 自选股表格（排序、右键菜单、行点击展开、行情条播报开关）
│   │   │   ├── AddStockDialog.vue # 搜索添加弹窗（300ms 防抖）
│   │   │   └── MarketTag.vue     # 市场标签（沪A/深A/北A/沪指/ETF…）
│   │   ├── detail/
│   │   │   ├── StockDetail.vue   # 个股详情容器
│   │   │   ├── IndexDetail.vue   # 指数详情容器
│   │   │   ├── ChartSwitcher.vue # 图表周期切换（分时/日K/周K/月K + 分钟K）
│   │   │   ├── MainOverlaySwitcher.vue  # 主图叠加指标切换（MA/BOLL）
│   │   │   ├── SubIndicatorSwitcher.vue # 副图指标切换（成交量/MACD）
│   │   │   ├── MinuteChart.vue   # 分时趋势图（5s 自动刷新）
│   │   │   ├── KLineChart.vue    # K线图（日30s/周月60s 自动刷新，左滑加载历史）
│   │   │   ├── DepthPanel.vue    # 五档盘口（3s 自动刷新）
│   │   │   └── StockSummary.vue  # 基本面概要
│   │   ├── updater/
│   │   │   └── UpdateDialog.vue  # 更新对话框（版本对比 + CHANGELOG + 进度）
│   │   └── ticker/
│   │       └── TickerBar.vue     # 浮动行情条（轮播、拖拽、悬停暂停）
│   └── assets/
│       ├── styles/
│       │   ├── variables.css     # 设计系统变量（颜色、间距、阴影）
│       │   └── dark.css          # 滚动条样式
│       └── chart.css             # 图表容器通用样式
├── src-tauri/                    # Rust 后端
│   ├── Cargo.toml
│   ├── tauri.conf.json           # Tauri 配置（双窗口、打包、更新）
│   ├── icons/                    # 应用图标
│   └── src/
│       ├── main.rs               # 入口
│       ├── lib.rs                # 应用初始化、命令注册、托盘、调度器、更新
│       ├── domain/mod.rs         # 数据结构（Quote, IndexQuote, Depth, Level, MinuteData, KLineData, Market, MarketOverview）
│       ├── db/mod.rs             # SQLite 数据库（自选、设置、缓存三表）
│       ├── datasource/
│       │   ├── mod.rs            # DataSource trait + DataSourceManager + 量额归一化
│       │   ├── sina.rs           # 新浪财经适配器（指数用个股格式 API，沪指量 ×100）
│       │   ├── tencent.rs        # 腾讯证券适配器（默认源，北交所自动回退新浪取图表）
│       │   ├── market.rs         # 市场概览聚合（新浪成交额 + 东财涨跌家数/板块，字段级降级）
│       │   ├── search.rs         # 跨源搜索与代码-市场识别
│       │   ├── headers.rs        # HTTP 请求头（UA 等）
│       │   └── market_clock.rs   # 交易时段判断 + 自适应轮询状态机
│       ├── cache/mod.rs          # 内存缓存 + 后台轮询调度器（探测/正常/空闲三态）
│       └── commands/             # Tauri IPC 命令
│           ├── quote.rs          # 行情查询 + 盘口 + 分时数据 + K线
│           ├── market.rs         # 市场概览聚合 + 概览轮询间隔
│           ├── watchlist.rs      # 自选股 CRUD + 搜索（跨源回退）
│           ├── settings.rs       # 设置读写 + 数据源切换
│           ├── autostart.rs      # 开机自启（注册表 Run 键 / 商店版 StartupTask）
│           ├── window.rs         # 窗口控制
│           └── updater.rs        # 更新检查 + 下载安装 + 交易时段判断
├── scripts/
│   ├── build.mjs                 # 跨平台构建脚本（代理自动检测）
│   ├── extract-changelog.mjs     # CI 提取 CHANGELOG 指定版本条目
│   └── make-latest-json.mjs      # CI 生成更新清单 latest.json
├── .github/workflows/
│   ├── ci.yml                    # PR/push CI（vue-tsc + vitest + cargo check/test）
│   ├── release.yml               # tag 触发的三平台自动构建发布
│   └── store-release.yml         # 手动触发的 Microsoft Store MSIX 构建
├── index.html                    # 主窗口 HTML
├── ticker.html                   # 行情条 HTML（独立入口）
├── vite.config.ts                # Vite 配置（双入口构建）
└── tsconfig.json                 # TypeScript 严格模式配置
```

## 数据流

```
外部 API (腾讯/新浪)
  → Scheduler (tokio 后台轮询，自适应频率)
    → QuoteCache (内存 HashMap + SQLite 双写)
      → app_handle.emit("quotes-updated" / "indices-updated" / "market-session-changed")
        → Pinia Stores (Tauri 事件监听)
          → Vue 响应式组件更新（主界面 + 行情条）
```

按需请求：

| 请求 | IPC 命令 | 目标组件 | 自动刷新 |
|------|----------|----------|----------|
| 五档盘口 | `get_depth` | DepthPanel | 3 秒 |
| 分时图 | `get_intraday` | MinuteChart | 5 秒 |
| 日 K 线 | `get_kline(period="daily")` | KLineChart | 30 秒 |
| 周/月 K 线 | `get_kline(period="weekly"/"monthly")` | KLineChart | 60 秒 |
| 市场概览 | `get_market_overview` | MarketOverviewPanel | 按时段自适应（`get_overview_interval` 提供种子间隔） |

> 市场概览的四个数据块（成交额 / 涨跌家数 / 行业板块 / 概念板块）在后端以 `tokio::join!` 并行拉取，任一失败仅降级对应字段。

## 窗口架构

| 窗口 | 标签 | 入口 | 配置 |
|------|------|------|------|
| 主界面 | `main` | `index.html` → `src/main.ts` | 1100×680，启动隐藏，关闭最小化至托盘，位置/大小持久化 |
| 行情条 | `ticker` | `ticker.html` → `src/ticker.ts` | 宽 230、高度自适应内容，无边框置顶，初始定位桌面右下角，可拖拽且位置持久化（`ticker_x`/`ticker_y`），隐藏任务栏图标 |

## 数据源

通过 TopBar 下拉菜单可切换数据源，默认使用腾讯证券：

| 数据源 | 标识符 | 编码 | 说明 |
|--------|--------|------|------|
| 腾讯证券 | `tencent` | GBK | 默认源，覆盖沪深京 A 股，盘口数据嵌入行情字段（位置 9-28），量 ×100 转股 |
| 新浪财经 | `sina` | GBK | 备用源，指数用个股格式 API（更可靠）替代紧凑指数格式，沪指成交量自动 ×100 修正，盘口通过腾讯接口回退获取，支持周/月 K |

特性：

- **搜索跨源回退** — 当前源搜不到时自动尝试备用源；北交所图表数据在腾讯源缺失时自动回退新浪
- **市场概览独立聚合** — 不随双源切换，固定使用新浪（成交额）+ 东方财富（涨跌家数、行业/概念板块）公开接口
- **自适应轮询** — 探测 → 正常 → 空闲三态，开盘快速确认，节假日自动休眠
- **动态频率** — 交易 2s / 盘前 5s / 午休 10s / 闭市 30s

## 开发阶段

| 阶段 | 状态 | 内容 |
|------|------|------|
| Phase 1 (MVP) | ✅ 完成 | 项目脚手架、新浪适配器、托盘、行情条、自选 CRUD、指数看板、暗色主题 |
| Phase 2 (体验) | ✅ 完成 | 个股详情（分时图+盘口+概要）、腾讯适配器、列排序、窗口记忆、交易时段感知轮询 |
| Phase 3 (增强) | ✅ 完成 | K 线图（日/周/月）+ 成交量副图、指数详情面板、图表自动刷新、自适应轮询（探测/空闲）、自动更新、开机自启、五档盘口自动刷新 |
| Phase 4 (扩展) | ✅ 进行中 | MACD 技术指标（v1.4.0）、K线主图 MA/BOLL 叠加指标切换（v1.4.1）、行情条显隐持久化（v1.4.2）、市场标签（v1.4.3）、北交所支持（v1.4.5）、行情条逐只播报开关（v1.4.7）、市场概览面板 + Microsoft Store 上架（v1.5.0）；价格预警、自选导入/导出、港股/美股规划中 |

## 交流群

欢迎加入微信用户交流群，进行问题反馈或建议交流：

<img src="public/qrcode.png" width="200" alt="微信用户交流群二维码" />

## IDE 推荐

- [VS Code](https://code.visualstudio.com/) + [Vue - Official](https://marketplace.visualstudio.com/items?itemName=Vue.volar) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)
