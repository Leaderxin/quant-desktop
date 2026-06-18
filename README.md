# QuantDesktop

桌面级 A 股行情监控工具，基于 Tauri 2 + Vue 3 + Rust 构建。

当前版本：**v0.3.0**

## 功能

- **实时行情** — 自选股批量刷新，交易时段智能轮询（2s 盘中 / 5s 盘前 / 10s 午休 / 30s 闭市），可切换新浪财经 / 腾讯证券数据源
- **大盘指数** — 上证指数、深证成指、创业板指、科创 50、科创综指、中证 500、上证 380 实时展示
- **个股详情** — 点击自选行展开详情面板，包含分时趋势图（KLineChart）、五档盘口、基本面概要（开/高/低/量/额/换手率）
- **浮动行情条** — 桌面置顶迷你行情条，2 只股票 3 秒自动轮播，鼠标悬停暂停，点击恢复主窗口
- **系统托盘** — 关闭窗口最小化至托盘，左键单击切换显示/隐藏，右键菜单操作
- **自选管理** — 添加、删除、排序（置顶 / 上移 / 下移 / 右键菜单），搜索支持跨数据源回退
- **列排序** — 自选表格支持按涨跌幅、价格、成交量、代码等列排序
- **深色 / 浅色主题** — 一键切换，CSS 变量驱动，行情条同步响应
- **窗口记忆** — 主窗口位置/大小自动保存，重启恢复，跨显示器边界保护
- **离线缓存** — 行情数据写入 SQLite，重启即时恢复上一次报价

## 技术栈

| 层 | 技术 |
|---|------|
| 桌面框架 | Tauri 2 |
| 前端 | Vue 3 + TypeScript + Pinia + Naive UI |
| 图表 | KLineChart v10 |
| 构建 | Vite + vue-tsc |
| 后端 | Rust (tokio, reqwest, rusqlite, chrono, serde, async-trait) |
| 数据源 | 新浪财经 (GBK 解码)、腾讯证券 (GBK 解码) |
| 持久化 | SQLite (rusqlite bundled) |

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

```bash
# 跨平台打包（自动适配系统）
npm run tauri:build

# 或使用带代理自动检测的构建脚本
node scripts/build.mjs
```

| 平台 | 产物 |
|------|------|
| Windows | NSIS `.exe` / `.msi` |
| macOS | `.dmg` + `.app` |
| Linux | `.deb` + `.AppImage` |

> 产物输出在 `src-tauri/target/release/bundle/`。

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
│   │   └── settings.ts           # 设置（主题、数据源切换）
│   ├── composables/
│   │   ├── useTauriEvent.ts      # Tauri 事件监听封装（自动清理）
│   │   └── useTheme.ts           # 主题工具
│   ├── components/
│   │   ├── layout/
│   │   │   ├── AppLayout.vue     # 主布局
│   │   │   └── TopBar.vue        # 顶栏（品牌、数据源、主题切换）
│   │   ├── index/
│   │   │   ├── IndexBar.vue      # 指数容器
│   │   │   └── IndexCard.vue     # 单条指数
│   │   ├── watchlist/
│   │   │   ├── WatchlistTable.vue # 自选股表格（排序、右键菜单、行点击展开）
│   │   │   └── AddStockDialog.vue # 搜索添加弹窗（300ms 防抖）
│   │   ├── detail/
│   │   │   ├── StockDetail.vue   # 个股详情容器
│   │   │   ├── MinuteChart.vue   # 分时趋势图（KLineChart）
│   │   │   ├── DepthPanel.vue    # 五档盘口
│   │   │   └── StockSummary.vue  # 基本面概要
│   │   └── ticker/
│   │       └── TickerBar.vue     # 浮动行情条
│   └── assets/styles/
│       ├── variables.css          # 设计系统变量（颜色、间距、阴影）
│       └── dark.css               # 滚动条样式
├── src-tauri/                    # Rust 后端
│   ├── Cargo.toml
│   ├── tauri.conf.json           # Tauri 配置（双窗口、打包）
│   ├── icons/                    # 应用图标（16 个文件）
│   └── src/
│       ├── main.rs               # 入口
│       ├── lib.rs                # 应用初始化、命令注册、托盘、调度器
│       ├── domain/mod.rs         # 数据结构（Quote, IndexQuote, Depth, Level, MinuteData, Market）
│       ├── db/mod.rs             # SQLite 数据库（自选、设置、缓存三表）
│       ├── datasource/
│       │   ├── mod.rs            # DataSource trait + DataSourceManager
│       │   ├── sina.rs           # 新浪财经适配器（默认）
│       │   ├── tencent.rs        # 腾讯证券适配器（备用）
│       │   └── market_clock.rs   # 交易时段判断（动态轮询频率）
│       ├── cache/mod.rs          # 内存缓存 + 后台轮询调度器（动态间隔）
│       └── commands/             # Tauri IPC 命令
│           ├── quote.rs          # 行情查询 + 盘口 + 分时数据
│           ├── watchlist.rs      # 自选股 CRUD + 搜索（跨源回退）
│           ├── settings.rs       # 设置读写 + 数据源切换
│           └── window.rs         # 窗口控制
├── scripts/
│   └── build.mjs                 # 跨平台构建脚本（代理自动检测）
├── .github/workflows/
│   └── release.yml               # CI/CD 自动构建发布
├── index.html                    # 主窗口 HTML
├── ticker.html                   # 行情条 HTML（独立入口）
├── vite.config.ts                # Vite 配置（双入口构建）
└── tsconfig.json                 # TypeScript 严格模式配置
```

## 数据流

```
外部 API (新浪/腾讯)
  → Scheduler (tokio 定时轮询，market_clock 动态频率)
    → QuoteCache (内存 HashMap + SQLite 双写)
      → app_handle.emit("quotes-updated" / "indices-updated" / "market-session-changed")
        → Pinia Stores (Tauri 事件监听)
          → Vue 响应式组件更新（主界面 + 行情条）
```

按需请求（盘口、分时图）通过 `invoke("get_depth")` / `invoke("get_intraday")` 直接走数据源适配器返回前端详情面板。

## 窗口架构

| 窗口 | 标签 | 入口 | 配置 |
|------|------|------|------|
| 主界面 | `main` | `index.html` → `src/main.ts` | 1100×680，启动隐藏，关闭最小化至托盘，位置/大小持久化 |
| 行情条 | `ticker` | `ticker.html` → `src/ticker.ts` | 230×38，无边框置顶，定位桌面右下角，隐藏任务栏图标 |

## 数据源

通过 TopBar 下拉菜单可切换数据源，默认使用新浪财经：

| 数据源 | 标识符 | 编码 | 说明 |
|--------|--------|------|------|
| 新浪财经 | `sina` | GBK | 默认源，覆盖沪深京 A 股，盘口通过腾讯接口回退获取 |
| 腾讯证券 | `tencent` | GBK | 备用源，盘口数据嵌入行情字段（位置 9-28），量乘以 100 转股 |

特性：
- **搜索跨源回退** — 当前源搜不到时自动尝试备用源
- **动态轮询频率** — 交易时段 2s，盘前 5s，午休 10s，闭市 30s，周末检测

## 开发阶段

| 阶段 | 状态 | 内容 |
|------|------|------|
| Phase 1 (MVP) | ✅ 完成 | 项目脚手架、新浪适配器、托盘、行情条、自选 CRUD、指数看板、暗色主题 |
| Phase 2 (体验) | ✅ 完成 | 个股详情（分时图+盘口+概要）、腾讯适配器、列排序、窗口记忆、交易时段感知轮询 |
| Phase 3 (增强) | 📋 计划中 | K 线图+技术指标、价格预警、自选导入导出、开机自启、打包优化 |
| Phase 4 (扩展) | 🔮 远期 | 港股/美股、专业数据源（Wind/Tushare）、自动更新、跨平台适配 |

## IDE 推荐

- [VS Code](https://code.visualstudio.com/) + [Vue - Official](https://marketplace.visualstudio.com/items?itemName=Vue.volar) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)
