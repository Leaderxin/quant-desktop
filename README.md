# QuantDesktop

桌面级 A 股行情监控工具，基于 Tauri 2 + Vue 3 + Rust 构建。

## 功能

- **实时行情** — 自选股批量刷新，默认 3 秒轮询，可切换新浪财经 / 东方财富数据源
- **大盘指数** — 上证指数、深证成指、创业板指、科创 50 实时展示
- **浮动行情条** — 桌面置顶迷你行情条，鼠标悬停暂停轮播，点击恢复主窗口
- **系统托盘** — 关闭窗口最小化至托盘，左键单击切换显示/隐藏
- **自选管理** — 添加、删除、排序（置顶 / 上移 / 下移 / 右键菜单）
- **深色 / 浅色主题** — 一键切换
- **离线缓存** — 行情数据写入 SQLite，重启即时恢复上一次报价

## 技术栈

| 层 | 技术 |
|---|------|
| 桌面框架 | Tauri 2 |
| 前端 | Vue 3 + TypeScript + Pinia + Naive UI |
| 构建 | Vite + vue-tsc |
| 后端 | Rust (tokio, reqwest, rusqlite, chrono, serde) |
| 数据源 | 新浪财经 (GBK 解码)、东方财富 (JSON API) |

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
```

| 平台 | 产物 |
|------|------|
| Windows | NSIS `.exe` |
| macOS | `.dmg` + `.app` |
| Linux | `.deb` + `.AppImage` |

> 产物输出在 `src-tauri/target/release/bundle/`。

## 项目结构

```
quant-desktop/
├── src/                          # Vue 前端
│   ├── main.ts                   # 主窗口入口
│   ├── ticker.ts                 # 行情条入口（独立 Vue 应用）
│   ├── App.vue                   # 根组件（主题配置）
│   ├── types/index.ts            # 前端类型定义（与 Rust domain 对应）
│   ├── stores/                   # Pinia 状态管理
│   │   ├── quote.ts              # 行情数据（事件驱动）
│   │   ├── watchlist.ts          # 自选股 CRUD
│   │   └── settings.ts           # 设置（主题、数据源切换）
│   ├── composables/
│   │   ├── useTauriEvent.ts      # Tauri 事件监听封装
│   │   └── useTheme.ts           # 主题工具
│   ├── components/
│   │   ├── layout/
│   │   │   ├── AppLayout.vue     # 主布局
│   │   │   └── TopBar.vue        # 顶栏（品牌、主题切换）
│   │   ├── index/
│   │   │   ├── IndexBar.vue      # 指数容器
│   │   │   └── IndexCard.vue     # 单条指数
│   │   ├── watchlist/
│   │   │   ├── WatchlistTable.vue # 自选股表格
│   │   │   └── AddStockDialog.vue # 搜索添加弹窗
│   │   └── ticker/
│   │       └── TickerBar.vue     # 浮动行情条
│   └── assets/styles/            # CSS 变量与主题
├── src-tauri/                    # Rust 后端
│   ├── Cargo.toml
│   ├── tauri.conf.json           # Tauri 配置（窗口、打包）
│   ├── icons/                    # 应用图标
│   └── src/
│       ├── main.rs               # 入口
│       ├── lib.rs                # 应用初始化、命令注册、托盘、调度器
│       ├── domain/mod.rs         # 数据结构（Quote, IndexQuote, Market…）
│       ├── db/mod.rs             # SQLite 数据库（自选、设置、缓存表）
│       ├── datasource/
│       │   ├── mod.rs            # DataSource trait + DataSourceManager
│       │   ├── sina.rs           # 新浪财经适配器
│       │   └── eastmoney.rs      # 东方财富适配器
│       ├── cache/mod.rs          # 内存缓存 + 后台轮询调度器
│       └── commands/             # Tauri IPC 命令
│           ├── quote.rs          # 行情查询
│           ├── watchlist.rs      # 自选股操作
│           ├── settings.rs       # 设置读写
│           └── window.rs         # 窗口控制
├── index.html                    # 主窗口 HTML
├── ticker.html                   # 行情条 HTML（独立入口）
├── vite.config.ts                # Vite 配置（双入口构建）
└── tsconfig.json                 # TypeScript 配置
```

## 数据流

```
外部 API (新浪/东方财富)
  → Scheduler (tokio 定时轮询)
    → QuoteCache (内存 + SQLite 双写)
      → app_handle.emit("quotes-updated")
        → useQuoteStore (Tauri 事件监听)
          → Vue 响应式组件更新
```

## 窗口架构

| 窗口 | 标签 | 入口 | 配置 |
|------|------|------|------|
| 主界面 | `main` | `index.html` → `src/main.ts` | 1100×680，启动时隐藏，关闭时最小化至托盘 |
| 行情条 | `ticker` | `ticker.html` → `src/ticker.ts` | 250×38，无边框置顶，定位桌面右下角 |

## 数据源

通过设置页面可切换数据源，默认使用新浪财经：

| 数据源 | 标识符 | 说明 |
|--------|--------|------|
| 新浪财经 | `sina` | GBK 编码接口，覆盖沪深京 A 股，搜索仅支持精确代码匹配 |
| 东方财富 | `eastmoney` | JSON API，搜索支持模糊匹配 |

## IDE 推荐

- [VS Code](https://code.visualstudio.com/) + [Vue - Official](https://marketplace.visualstudio.com/items?itemName=Vue.volar) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)
