# QuantDesktop

桌面级 A 股行情监控工具，基于 Tauri 2 + Vue 3 + Rust 构建。

当前版本：**v1.2.9**

## 功能

- **实时行情** — 自选股批量刷新，交易时段自适应轮询（探测→正常→空闲三态），可切换腾讯证券 / 新浪财经数据源
- **大盘指数** — 上证指数、深证成指、创业板指、科创 50、科创 100、中证 500、科创综指实时展示
- **个股详情** — 点击自选行展开详情面板，包含分时趋势图 / K线图（日/周/月）、五档盘口、基本面概要（开/高/低/量/额/换手率）
- **K 线图表** — 日 K / 周 K / 月 K，成交量副图含 MA5/MA10/MA20 均量线，蜡烛影线颜色与实体一致，涨跌颜色遵循 A 股习惯（与前收对比）
- **指数详情** — 点击指数卡片展开指数详情面板，含分时图、K线图、成交量/成交额概要
- **自动刷新** — 详情面板打开时，五档盘口 3 秒刷新，分时图 5 秒刷新，日 K 30 秒刷新，周/月 K 60 秒刷新
- **浮动行情条** — 桌面置顶迷你行情条，2 只股票 3 秒自动轮播，鼠标悬停暂停，点击恢复主窗口
- **系统托盘** — 关闭窗口最小化至托盘，左键单击切换显示/隐藏，右键菜单操作
- **自选管理** — 添加、删除、排序（置顶 / 上移 / 下移 / 右键菜单），搜索支持跨数据源回退
- **列排序** — 自选表格支持按涨跌幅、价格、成交量、代码等列排序
- **深色 / 浅色主题** — 一键切换，CSS 变量驱动，行情条同步响应
- **窗口记忆** — 主窗口位置/大小自动保存，重启恢复，跨显示器边界保护
- **离线缓存** — 行情数据写入 SQLite，重启即时恢复上一次报价
- **自适应轮询** — 交易时段进入时快速探测（3 次 ×2s），确认开市后正常轮询（2s），连续 10 次无价格变化自动降频（30s），节假日智能休眠
- **自动更新** — 启动时自动检测新版本，交易时段智能抑制弹窗，CHANGELOG 展示，一键下载安装
- **开机自启** — 状态栏开关，一键启用/禁用

## 技术栈

| 层 | 技术 |
|---|------|
| 桌面框架 | Tauri 2 |
| 前端 | Vue 3 + TypeScript + Pinia + Naive UI |
| 图表 | KLineChart v10 |
| 构建 | Vite + vue-tsc |
| 后端 | Rust (tokio, reqwest, rusqlite, chrono, serde, async-trait) |
| 数据源 | 腾讯证券 (默认)、新浪财经 (备用) — GBK 解码 |
| 持久化 | SQLite (rusqlite bundled) |
| 自动更新 | Tauri updater plugin |

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
| Windows | NSIS `.exe` / `.msi` |
| macOS | `.dmg` + `.app` |
| Linux | `.deb` + `.AppImage` |

> 产物输出在 `src-tauri/target/release/bundle/`。

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
Compress-Archive -Path "$staging\*" -DestinationPath "$src\bundle\quant-desktop_1.2.8_x64-portable.zip"
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
│   │   ├── watchlist.ts          # 自选股 CRUD
│   │   ├── settings.ts           # 设置（主题、数据源切换、开机自启）
│   │   └── updater.ts            # 更新状态（检测/下载/安装）
│   ├── composables/
│   │   ├── useTauriEvent.ts      # Tauri 事件监听封装（自动清理）
│   │   ├── useTheme.ts           # 主题工具
│   │   ├── useChart.ts           # 图表通用逻辑（初始化、加载、自动刷新）
│   │   └── useUpdateCheck.ts     # 更新检测（启动检测 + 交易时段门控）
│   ├── components/
│   │   ├── layout/
│   │   │   ├── AppLayout.vue     # 主布局
│   │   │   ├── TopBar.vue        # 顶栏（标语、数据源切换）
│   │   │   └── StatusBar.vue     # 底栏（版本号、检查更新、主题、开机自启、联系）
│   │   ├── index/
│   │   │   ├── IndexBar.vue      # 指数容器
│   │   │   └── IndexCard.vue     # 单条指数（点击展开详情）
│   │   ├── watchlist/
│   │   │   ├── WatchlistTable.vue # 自选股表格（排序、右键菜单、行点击展开）
│   │   │   └── AddStockDialog.vue # 搜索添加弹窗（300ms 防抖）
│   │   ├── detail/
│   │   │   ├── StockDetail.vue   # 个股详情容器
│   │   │   ├── IndexDetail.vue   # 指数详情容器
│   │   │   ├── ChartSwitcher.vue # 图表周期切换（分时/日K/周K/月K）
│   │   │   ├── MinuteChart.vue   # 分时趋势图（5s 自动刷新）
│   │   │   ├── KLineChart.vue    # K线图（日30s/周月60s 自动刷新）
│   │   │   ├── DepthPanel.vue    # 五档盘口（3s 自动刷新）
│   │   │   └── StockSummary.vue  # 基本面概要
│   │   ├── updater/
│   │   │   └── UpdateDialog.vue  # 更新对话框（版本对比 + CHANGELOG + 进度）
│   │   └── ticker/
│   │       └── TickerBar.vue     # 浮动行情条
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
│       ├── domain/mod.rs         # 数据结构（Quote, IndexQuote, Depth, Level, MinuteData, KLineData, Market）
│       ├── db/mod.rs             # SQLite 数据库（自选、设置、缓存三表）
│       ├── datasource/
│       │   ├── mod.rs            # DataSource trait + DataSourceManager + 量额归一化
│       │   ├── sina.rs           # 新浪财经适配器（指数用个股格式 API，沪指量 ×100）
│       │   ├── tencent.rs        # 腾讯证券适配器（默认源）
│       │   └── market_clock.rs   # 交易时段判断 + 自适应轮询状态机
│       ├── cache/mod.rs          # 内存缓存 + 后台轮询调度器（探测/正常/空闲三态）
│       └── commands/             # Tauri IPC 命令
│           ├── quote.rs          # 行情查询 + 盘口 + 分时数据 + K线
│           ├── watchlist.rs      # 自选股 CRUD + 搜索（跨源回退）
│           ├── settings.rs       # 设置读写 + 数据源切换
│           ├── window.rs         # 窗口控制
│           └── updater.rs        # 更新检查 + 下载安装 + 交易时段判断
├── scripts/
│   ├── build.mjs                 # 跨平台构建脚本（代理自动检测）
│   ├── extract-changelog.mjs     # CI 提取 CHANGELOG 指定版本条目
│   └── make-latest-json.mjs      # CI 生成更新清单 latest.json
├── .github/workflows/
│   └── release.yml               # CI/CD 自动构建发布
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

## 窗口架构

| 窗口 | 标签 | 入口 | 配置 |
|------|------|------|------|
| 主界面 | `main` | `index.html` → `src/main.ts` | 1100×680，启动隐藏，关闭最小化至托盘，位置/大小持久化 |
| 行情条 | `ticker` | `ticker.html` → `src/ticker.ts` | 230×38，无边框置顶，定位桌面右下角，隐藏任务栏图标 |

## 数据源

通过 TopBar 下拉菜单可切换数据源，默认使用腾讯证券：

| 数据源 | 标识符 | 编码 | 说明 |
|--------|--------|------|------|
| 腾讯证券 | `tencent` | GBK | 默认源，覆盖沪深京 A 股，盘口数据嵌入行情字段（位置 9-28），量 ×100 转股 |
| 新浪财经 | `sina` | GBK | 备用源，指数用个股格式 API（更可靠）替代紧凑指数格式，沪指成交量自动 ×100 修正，盘口通过腾讯接口回退获取 |

特性：
- **搜索跨源回退** — 当前源搜不到时自动尝试备用源
- **自适应轮询** — 探测 → 正常 → 空闲三态，开盘快速确认，节假日自动休眠
- **动态频率** — 交易 2s / 盘前 5s / 午休 10s / 闭市 30s

## 开发阶段

| 阶段 | 状态 | 内容 |
|------|------|------|
| Phase 1 (MVP) | ✅ 完成 | 项目脚手架、新浪适配器、托盘、行情条、自选 CRUD、指数看板、暗色主题 |
| Phase 2 (体验) | ✅ 完成 | 个股详情（分时图+盘口+概要）、腾讯适配器、列排序、窗口记忆、交易时段感知轮询 |
| Phase 3 (增强) | ✅ 完成 | K 线图（日/周/月）+ 成交量副图、指数详情面板、图表自动刷新、自适应轮询（探测/空闲）、自动更新、开机自启、五档盘口自动刷新 |
| Phase 4 (扩展) | 🔮 远期 | 技术指标叠加（MA/BOLL/MACD）、价格预警、自选导入/导出、港股/美股、专业数据源（Wind/Tushare） |

## 运维群

欢迎加入微信运维群，进行产品运维、问题反馈或建议交流：

<img src="public/qrcode.jpg" width="200" alt="微信运维群二维码" />

## IDE 推荐

- [VS Code](https://code.visualstudio.com/) + [Vue - Official](https://marketplace.visualstudio.com/items?itemName=Vue.volar) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)
