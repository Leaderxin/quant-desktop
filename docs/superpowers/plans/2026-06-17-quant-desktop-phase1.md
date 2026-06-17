# QuantDesktop Phase 1 MVP 实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 搭建 QuantDesktop 核心看盘 MVP：系统托盘驻留 + 悬浮行情条 + 主界面（指数横条 + 自选列表）+ 东方财富数据源 + SQLite 持久化 + 暗色主题。

**Architecture:** Tauri 2.x 桌面框架，Rust 后端分层架构（commands → cache → datasource trait → HTTP），Vue 3 + Naive UI + Pinia 前端，SQLite 本地存储，通过 Tauri invoke/event 通信。

**Tech Stack:** Tauri 2.x, Rust (reqwest, rusqlite, async-trait, serde, tokio), Vue 3 + TypeScript, Naive UI, Pinia, KLineChart, Vite

---

## 文件结构

```
quant-desktop/
├── package.json                       # [modify] 前端依赖
├── vite.config.ts                     # [modify] Vite 配置
├── tsconfig.json                      # [modify] TS 配置
├── index.html                         # [modify] 入口 HTML
├── src/                               # Vue 3 前端
│   ├── main.ts                        # [create] Vue 入口
│   ├── App.vue                        # [create] 根组件
│   ├── ticker.html                    # [create] 行情条独立入口
│   ├── ticker.ts                      # [create] 行情条独立入口
│   ├── types/
│   │   └── index.ts                   # [create] TS 类型定义
│   ├── stores/
│   │   ├── quote.ts                   # [create] 行情 Pinia store
│   │   ├── watchlist.ts              # [create] 自选 Pinia store
│   │   └── settings.ts               # [create] 配置 Pinia store
│   ├── composables/
│   │   ├── useTauriEvent.ts           # [create] Tauri event 监听
│   │   └── useTheme.ts               # [create] 主题切换
│   ├── assets/styles/
│   │   ├── variables.css              # [create] CSS 变量
│   │   └── dark.css                   # [create] 暗色主题
│   └── components/
│       ├── layout/
│       │   ├── AppLayout.vue          # [create] 主布局
│       │   └── TopBar.vue             # [create] 顶栏
│       ├── index/
│       │   ├── IndexBar.vue           # [create] 指数横条
│       │   └── IndexCard.vue          # [create] 指数卡片
│       ├── watchlist/
│       │   ├── WatchlistTable.vue     # [create] 自选列表
│       │   └── AddStockDialog.vue     # [create] 添加自选弹窗
│       └── ticker/
│           └── TickerBar.vue          # [create] 行情条
├── src-tauri/                         # Rust 后端
│   ├── Cargo.toml                     # [modify] Rust 依赖
│   ├── tauri.conf.json                # [modify] Tauri 配置
│   ├── build.rs                       # [modify] 构建脚本
│   ├── icons/                         # [modify] 应用图标
│   └── src/
│       ├── main.rs                    # [modify] 入口
│       ├── lib.rs                     # [modify] Tauri 应用构建
│       ├── domain/
│       │   └── mod.rs                 # [create] 领域模型
│       ├── db/
│       │   └── mod.rs                 # [create] SQLite 持久化
│       ├── datasource/
│       │   ├── mod.rs                 # [create] DataSource trait + Manager
│       │   └── eastmoney.rs           # [create] 东方财富适配器
│       ├── cache/
│       │   └── mod.rs                 # [create] 缓存 + 调度器
│       └── commands/
│           ├── mod.rs                 # [create] 命令模块入口
│           ├── quote.rs               # [create] 行情命令
│           ├── watchlist.rs           # [create] 自选命令
│           └── settings.rs            # [create] 配置命令
└── docs/superpowers/specs/
    └── 2026-06-17-quant-desktop-design.md  # [exists] 设计文档
```

---

### Task 1: 项目脚手架搭建

**Files:**
- Create: 整个项目脚手架

- [ ] **Step 1: 使用 Tauri CLI 创建项目**

```bash
cd e:/GIT/github/quant-desktop
npm create tauri-app@latest . -- --template vue-ts
```

选择 Vue + TypeScript 模板。如果提示目录非空，使用 `--force` 或在临时目录创建后迁移。

- [ ] **Step 2: 安装前端依赖**

```bash
cd e:/GIT/github/quant-desktop
npm install
npm install naive-ui pinia klinecharts
npm install -D @types/node
```

- [ ] **Step 3: 验证 scaffold 可编译**

```bash
cd e:/GIT/github/quant-desktop
cd src-tauri && cargo check
```

Expected: `Finished dev [unoptimized + debuginfo]`

- [ ] **Step 4: 验证前端可构建**

```bash
cd e:/GIT/github/quant-desktop
npm run build
```

Expected: 构建成功，无报错

- [ ] **Step 5: 提交**

```bash
git add -A
git commit -m "feat: scaffold Tauri + Vue 3 + TypeScript project"
```

---

### Task 2: Rust 领域模型

**Files:**
- Create: `src-tauri/src/domain/mod.rs`
- Modify: `src-tauri/src/lib.rs` (注册模块)

- [ ] **Step 1: 创建领域模型文件**

```rust
// src-tauri/src/domain/mod.rs
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Market {
    #[serde(rename = "CN")]
    CN,
    #[serde(rename = "HK")]
    HK,
    #[serde(rename = "US")]
    US,
}

impl Market {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_uppercase().as_str() {
            "CN" => Some(Self::CN),
            "HK" => Some(Self::HK),
            "US" => Some(Self::US),
            _ => None,
        }
    }

    pub fn as_prefix(&self) -> &str {
        match self {
            Market::CN => "0",  // 深市, 1 for 沪市 handled separately
            Market::HK => "116",
            Market::US => "105",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Quote {
    pub code: String,
    pub market: String,
    pub name: String,
    pub price: f64,
    pub change: f64,
    pub change_pct: f64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub volume: u64,
    pub turnover: f64,
    pub timestamp: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexQuote {
    pub code: String,
    pub name: String,
    pub price: f64,
    pub change: f64,
    pub change_pct: f64,
    pub volume: u64,
    pub turnover: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Depth {
    pub code: String,
    pub bids: Vec<Level>,
    pub asks: Vec<Level>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Level {
    pub price: f64,
    pub volume: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MinuteData {
    pub time: String,
    pub price: f64,
    pub volume: u64,
    pub avg_price: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StockBrief {
    pub code: String,
    pub market: String,
    pub name: String,
}

/// 行情快照的批量查询结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuotesResponse {
    pub quotes: Vec<Quote>,
    pub errors: Vec<String>,  // 失败的 codes
}
```

- [ ] **Step 2: 在 lib.rs 中注册 domain 模块**

```rust
// src-tauri/src/lib.rs — 在现有内容基础上添加:
pub mod domain;
```

- [ ] **Step 3: 验证编译**

```bash
cd src-tauri && cargo check
```

Expected: `Finished dev [unoptimized + debuginfo]`

- [ ] **Step 4: 提交**

```bash
git add src-tauri/src/domain/mod.rs src-tauri/src/lib.rs
git commit -m "feat: add Rust domain models (Quote, IndexQuote, Depth, Market)"
```

---

### Task 3: SQLite 持久化层

**Files:**
- Create: `src-tauri/src/db/mod.rs`
- Modify: `src-tauri/Cargo.toml` (添加 rusqlite)
- Modify: `src-tauri/src/lib.rs` (注册模块)

- [ ] **Step 1: 添加 rusqlite 依赖**

在 `src-tauri/Cargo.toml` 的 `[dependencies]` 中添加：

```toml
rusqlite = { version = "0.31", features = ["bundled"] }
```

- [ ] **Step 2: 创建持久化模块**

```rust
// src-tauri/src/db/mod.rs
use rusqlite::{Connection, Result as SqliteResult, params};
use std::path::PathBuf;
use std::sync::Mutex;

pub struct Database {
    conn: Mutex<Connection>,
}

impl Database {
    /// 打开或创建数据库，自动执行迁移
    pub fn open(app_dir: PathBuf) -> SqliteResult<Self> {
        std::fs::create_dir_all(&app_dir).ok();
        let db_path = app_dir.join("quant-desktop.db");
        let conn = Connection::open(db_path)?;
        let db = Self { conn: Mutex::new(conn) };
        db.migrate()?;
        Ok(db)
    }

    fn migrate(&self) -> SqliteResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS watchlist (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                code        TEXT NOT NULL,
                market      TEXT NOT NULL DEFAULT 'CN',
                name        TEXT NOT NULL,
                sort_order  INTEGER DEFAULT 0,
                added_at    TEXT NOT NULL,
                UNIQUE(code, market)
            );
            CREATE TABLE IF NOT EXISTS settings (
                key         TEXT PRIMARY KEY,
                value       TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS quote_cache (
                code        TEXT NOT NULL,
                market      TEXT NOT NULL DEFAULT 'CN',
                data        TEXT NOT NULL,
                cached_at   TEXT NOT NULL,
                PRIMARY KEY (code, market)
            );"
        )?;
        Ok(())
    }

    // ── Watchlist CRUD ──

    pub fn get_watchlist(&self) -> SqliteResult<Vec<WatchItem>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, code, market, name, sort_order, added_at
             FROM watchlist ORDER BY sort_order ASC, id ASC"
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(WatchItem {
                id: row.get(0)?,
                code: row.get(1)?,
                market: row.get(2)?,
                name: row.get(3)?,
                sort_order: row.get(4)?,
                added_at: row.get(5)?,
            })
        })?;
        rows.collect()
    }

    pub fn add_watch(&self, code: &str, market: &str, name: &str) -> SqliteResult<()> {
        let conn = self.conn.lock().unwrap();
        let now = chrono_now();
        conn.execute(
            "INSERT OR IGNORE INTO watchlist (code, market, name, added_at)
             VALUES (?1, ?2, ?3, ?4)",
            params![code, market, name, now],
        )?;
        Ok(())
    }

    pub fn remove_watch(&self, code: &str, market: &str) -> SqliteResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM watchlist WHERE code = ?1 AND market = ?2",
            params![code, market],
        )?;
        Ok(())
    }

    pub fn reorder_watch(&self, ids: &[i64]) -> SqliteResult<()> {
        let conn = self.conn.lock().unwrap();
        for (i, id) in ids.iter().enumerate() {
            conn.execute(
                "UPDATE watchlist SET sort_order = ?1 WHERE id = ?2",
                params![i as i32, id],
            )?;
        }
        Ok(())
    }

    pub fn get_watch_codes(&self) -> SqliteResult<Vec<(String, String)>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT code, market FROM watchlist ORDER BY sort_order ASC, id ASC"
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        rows.collect()
    }

    // ── Settings CRUD ──

    pub fn get_setting(&self, key: &str) -> SqliteResult<Option<String>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT value FROM settings WHERE key = ?1")?;
        let mut rows = stmt.query_map(params![key], |row| row.get::<_, String>(0))?;
        match rows.next() {
            Some(Ok(v)) => Ok(Some(v)),
            _ => Ok(None),
        }
    }

    pub fn set_setting(&self, key: &str, value: &str) -> SqliteResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO settings (key, value) VALUES (?1, ?2)",
            params![key, value],
        )?;
        Ok(())
    }

    pub fn get_all_settings(&self) -> SqliteResult<Vec<(String, String)>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT key, value FROM settings")?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        rows.collect()
    }

    // ── Quote Cache ──

    pub fn cache_quotes(&self, quotes: &[crate::domain::Quote]) -> SqliteResult<()> {
        let conn = self.conn.lock().unwrap();
        let now = chrono_now();
        for q in quotes {
            let data = serde_json::to_string(q).unwrap_or_default();
            conn.execute(
                "INSERT OR REPLACE INTO quote_cache (code, market, data, cached_at)
                 VALUES (?1, ?2, ?3, ?4)",
                params![q.code, q.market, data, now],
            )?;
        }
        Ok(())
    }

    pub fn get_cached_quotes(&self) -> SqliteResult<Vec<crate::domain::Quote>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT data FROM quote_cache"
        )?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        rows.filter_map(|r| r.ok())
            .filter_map(|data| serde_json::from_str(&data).ok())
            .collect::<Vec<crate::domain::Quote>>();
        Ok(rows
            .filter_map(|r| r.ok())
            .filter_map(|data| serde_json::from_str::<crate::domain::Quote>(&data).ok())
            .collect())
    }
}

#[derive(Debug, Clone)]
pub struct WatchItem {
    pub id: i64,
    pub code: String,
    pub market: String,
    pub name: String,
    pub sort_order: i32,
    pub added_at: String,
}

fn chrono_now() -> String {
    // 用标准库替代 chrono，避免引入额外依赖
    use std::time::SystemTime;
    let ts = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    // 简单 ISO8601 格式
    let secs = ts % 86400;
    let days = ts / 86400;
    // 从 Unix epoch 推算日期（简化处理，精确度足够）
    format!("{:?}", SystemTime::now())
}
```

等等，用 `chrono` 更简单。改为在 Cargo.toml 添加 `chrono`。

- [ ] **Step 2 (修订): 使用 chrono 简化时间处理**

在 `src-tauri/Cargo.toml` `[dependencies]` 中添加：
```toml
chrono = { version = "0.4", features = ["serde"] }
```

然后将 `chrono_now()` 替换为：
```rust
fn chrono_now() -> String {
    chrono::Local::now().format("%Y-%m-%dT%H:%M:%S").to_string()
}
```

- [ ] **Step 3: 在 lib.rs 注册 db 模块**

```rust
// src-tauri/src/lib.rs
pub mod domain;
pub mod db;
```

- [ ] **Step 4: 验证编译**

```bash
cd src-tauri && cargo check
```

Expected: 编译成功

- [ ] **Step 5: 提交**

```bash
git add src-tauri/Cargo.toml src-tauri/src/db/ src-tauri/src/lib.rs
git commit -m "feat: add SQLite persistence layer (watchlist, settings, quote_cache)"
```

---

### Task 4: DataSource Trait + 数据源管理器

**Files:**
- Create: `src-tauri/src/datasource/mod.rs`
- Modify: `src-tauri/Cargo.toml` (添加 async-trait, reqwest)
- Modify: `src-tauri/src/lib.rs` (注册模块)

- [ ] **Step 1: 添加依赖**

在 `src-tauri/Cargo.toml` `[dependencies]` 中添加：
```toml
async-trait = "0.1"
reqwest = { version = "0.12", features = ["json"] }
```

- [ ] **Step 2: 创建 DataSource trait 和管理器**

```rust
// src-tauri/src/datasource/mod.rs
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use crate::domain::*;

/// 数据源抽象 trait —— 所有行情适配器实现此接口
#[async_trait]
pub trait DataSource: Send + Sync {
    /// 数据源唯一标识
    fn name(&self) -> &str;

    /// 显示名称
    fn display_name(&self) -> &str;

    /// 获取实时行情（批量）
    async fn fetch_realtime(
        &self,
        codes: &[String],
        market: &str,
    ) -> Result<Vec<Quote>, String>;

    /// 获取主要指数
    async fn fetch_indices(&self) -> Result<Vec<IndexQuote>, String>;

    /// 搜索股票（模糊匹配代码或名称）
    async fn search(
        &self,
        keyword: &str,
        market: &str,
    ) -> Result<Vec<StockBrief>, String>;

    /// 健康检查
    async fn health_check(&self) -> Result<bool, String>;
}

/// 数据源管理器 —— 注册、切换、统一调度
pub struct DataSourceManager {
    sources: HashMap<String, Box<dyn DataSource>>,
    active: RwLock<String>,
}

impl DataSourceManager {
    pub fn new() -> Self {
        Self {
            sources: HashMap::new(),
            active: RwLock::new(String::new()),
        }
    }

    pub fn register(&mut self, source: Box<dyn DataSource>) {
        let name = source.name().to_string();
        // 第一次注册的源自动设为 active
        if self.sources.is_empty() {
            *self.active.write().unwrap() = name.clone();
        }
        self.sources.insert(name, source);
    }

    pub fn set_active(&self, name: &str) -> Result<(), String> {
        if self.sources.contains_key(name) {
            *self.active.write().unwrap() = name.to_string();
            Ok(())
        } else {
            Err(format!("数据源 '{}' 未注册", name))
        }
    }

    pub fn active_name(&self) -> String {
        self.active.read().unwrap().clone()
    }

    pub fn active_source(&self) -> &dyn DataSource {
        let name = self.active.read().unwrap();
        // SAFETY: active 总是被设置为已注册的源名
        self.sources.get(&*name).unwrap().as_ref()
    }

    pub fn list_sources(&self) -> Vec<(&str, &str)> {
        self.sources
            .iter()
            .map(|(k, v)| (k.as_str(), v.display_name()))
            .collect()
    }
}
```

- [ ] **Step 3: 在 lib.rs 注册模块**

```rust
// src-tauri/src/lib.rs
pub mod domain;
pub mod db;
pub mod datasource;
```

- [ ] **Step 4: 验证编译**

```bash
cd src-tauri && cargo check
```

Expected: 编译成功

- [ ] **Step 5: 提交**

```bash
git add src-tauri/Cargo.toml src-tauri/src/datasource/ src-tauri/src/lib.rs
git commit -m "feat: add DataSource trait and DataSourceManager"
```

---

### Task 5: 东方财富数据源适配器

**Files:**
- Create: `src-tauri/src/datasource/eastmoney.rs`
- Modify: `src-tauri/src/datasource/mod.rs` (注册适配器)

- [ ] **Step 1: 创建东方财富适配器**

```rust
// src-tauri/src/datasource/eastmoney.rs
use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;
use crate::domain::*;
use super::DataSource;

const EASTMONEY_URL: &str = "https://push2.eastmoney.com/api/qt/ulist.npz";
const SEARCH_URL: &str = "https://searchapi.eastmoney.com/api/suggest/get";
const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36";

pub struct EastmoneyAdapter {
    client: Client,
}

impl EastmoneyAdapter {
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .user_agent(USER_AGENT)
                .build()
                .unwrap_or_default(),
        }
    }

    /// 将股票代码转换为东方财富 secid 格式
    /// A 股: sh600519 → 1.600519, sz000001 → 0.000001
    fn code_to_secid(code: &str, market: &str) -> String {
        if market == "CN" {
            let prefix = if code.starts_with("6") || code.starts_with("5") || code.starts_with("9") {
                "1" // 沪市
            } else {
                "0" // 深市/北交所
            };
            format!("{}.{}", prefix, code)
        } else {
            format!("{}.{}", code, code) // 港股/美股 fallback
        }
    }
}

#[async_trait]
impl DataSource for EastmoneyAdapter {
    fn name(&self) -> &str {
        "eastmoney"
    }

    fn display_name(&self) -> &str {
        "东方财富"
    }

    async fn fetch_realtime(
        &self,
        codes: &[String],
        market: &str,
    ) -> Result<Vec<Quote>, String> {
        let secids: Vec<String> = codes
            .iter()
            .map(|c| Self::code_to_secid(c, market))
            .collect();
        let secids_str = secids.join(",");

        let params = [
            ("fltt", "2"),
            ("fields", "f2,f3,f4,f12,f14,f15,f16,f17,f18"),
            ("secids", &secids_str),
        ];

        #[derive(Deserialize)]
        struct RawResponse {
            data: Option<RawData>,
        }
        #[derive(Deserialize)]
        struct RawData {
            diff: Option<Vec<RawQuote>>,
        }
        #[derive(Deserialize)]
        struct RawQuote {
            #[serde(rename = "f2")]  price: Option<f64>,
            #[serde(rename = "f3")]  change_pct: Option<f64>,
            #[serde(rename = "f4")]  change: Option<f64>,
            #[serde(rename = "f12")] code: Option<String>,
            #[serde(rename = "f14")] name: Option<String>,
            #[serde(rename = "f15")] high: Option<f64>,
            #[serde(rename = "f16")] low: Option<f64>,
            #[serde(rename = "f17")] open: Option<f64>,
            #[serde(rename = "f18")] volume: Option<u64>,
        }

        let resp = self.client
            .get(EASTMONEY_URL)
            .query(&params)
            .send()
            .await
            .map_err(|e| format!("请求失败: {}", e))?;

        let body: RawResponse = resp
            .json()
            .await
            .map_err(|e| format!("解析响应失败: {}", e))?;

        let quotes = body
            .data
            .and_then(|d| d.diff)
            .unwrap_or_default()
            .into_iter()
            .filter_map(|r| {
                Some(Quote {
                    code: r.code?,
                    market: market.to_string(),
                    name: r.name.unwrap_or_default(),
                    price: r.price.unwrap_or(0.0),
                    change: r.change.unwrap_or(0.0),
                    change_pct: r.change_pct.unwrap_or(0.0),
                    open: r.open.unwrap_or(0.0),
                    high: r.high.unwrap_or(0.0),
                    low: r.low.unwrap_or(0.0),
                    volume: r.volume.unwrap_or(0),
                    turnover: 0.0,
                    timestamp: chrono::Utc::now().timestamp(),
                })
            })
            .collect();

        Ok(quotes)
    }

    async fn fetch_indices(&self) -> Result<Vec<IndexQuote>, String> {
        // 上证 000001, 深证 399001, 创业板 399006, 科创50 000688
        let index_secids = "1.000001,0.399001,0.399006,1.000688";
        let params = [
            ("fltt", "2"),
            ("fields", "f2,f3,f4,f12,f14"),
            ("secids", index_secids),
        ];

        #[derive(Deserialize)]
        struct RawResponse {
            data: Option<RawData>,
        }
        #[derive(Deserialize)]
        struct RawData {
            diff: Option<Vec<RawIndex>>,
        }
        #[derive(Deserialize)]
        struct RawIndex {
            #[serde(rename = "f2")]  price: Option<f64>,
            #[serde(rename = "f3")]  change_pct: Option<f64>,
            #[serde(rename = "f4")]  change: Option<f64>,
            #[serde(rename = "f12")] code: Option<String>,
            #[serde(rename = "f14")] name: Option<String>,
        }

        let resp = self.client
            .get(EASTMONEY_URL)
            .query(&params)
            .send()
            .await
            .map_err(|e| format!("请求失败: {}", e))?;

        let body: RawResponse = resp.json().await.map_err(|e| format!("解析失败: {}", e))?;

        let indices = body
            .data
            .and_then(|d| d.diff)
            .unwrap_or_default()
            .into_iter()
            .filter_map(|r| {
                Some(IndexQuote {
                    code: r.code?,
                    name: r.name.unwrap_or_default(),
                    price: r.price.unwrap_or(0.0),
                    change: r.change.unwrap_or(0.0),
                    change_pct: r.change_pct.unwrap_or(0.0),
                    volume: 0,
                    turnover: 0.0,
                })
            })
            .collect();

        Ok(indices)
    }

    async fn search(
        &self,
        keyword: &str,
        _market: &str,
    ) -> Result<Vec<StockBrief>, String> {
        #[derive(Deserialize)]
        struct RawResponse {
            #[serde(rename = "QuotationCodeTable")]
            data: Option<Vec<RawStock>>,
        }
        #[derive(Deserialize)]
        struct RawStock {
            #[serde(rename = "Code")] code: Option<String>,
            #[serde(rename = "Name")] name: Option<String>,
            #[serde(rename = "Market")] market_raw: Option<String>,
        }

        let resp = self.client
            .get(SEARCH_URL)
            .query(&[("input", keyword), ("type", "14"), ("token", "D43BF722C8E33BDC906FB84A85F326E1"), ("count", "20")])
            .send()
            .await
            .map_err(|e| format!("请求失败: {}", e))?;

        let body: RawResponse = resp.json().await.map_err(|e| format!("解析失败: {}", e))?;

        let results = body
            .data
            .unwrap_or_default()
            .into_iter()
            .filter(|s| {
                // 仅保留 A 股（沪 1 / 深 0）
                s.market_raw.as_deref() == Some("0")
                    || s.market_raw.as_deref() == Some("1")
            })
            .map(|s| StockBrief {
                code: s.code.unwrap_or_default(),
                market: "CN".to_string(),
                name: s.name.unwrap_or_default(),
            })
            .collect();

        Ok(results)
    }

    async fn health_check(&self) -> Result<bool, String> {
        let codes = vec!["000001".to_string()];
        self.fetch_realtime(&codes, "CN").await.map(|q| !q.is_empty())
    }
}
```

- [ ] **Step 2: 在 datasource/mod.rs 中 re-export**

在 `src-tauri/src/datasource/mod.rs` 末尾添加：
```rust
pub mod eastmoney;
```

- [ ] **Step 3: 验证编译**

```bash
cd src-tauri && cargo check
```

Expected: 编译成功

- [ ] **Step 4: 提交**

```bash
git add src-tauri/src/datasource/
git commit -m "feat: add Eastmoney data source adapter"
```

---

### Task 6: 缓存层与轮询调度器

**Files:**
- Create: `src-tauri/src/cache/mod.rs`
- Modify: `src-tauri/src/lib.rs` (注册模块)

- [ ] **Step 1: 创建缓存和调度器模块**

```rust
// src-tauri/src/cache/mod.rs
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use crate::domain::{Quote, IndexQuote};

/// 行情缓存（内存 + SQLite 双写）
pub struct QuoteCache {
    quotes: Mutex<HashMap<String, CachedQuote>>,
    indices: Mutex<Vec<IndexQuote>>,
    db: Arc<crate::db::Database>,
}

struct CachedQuote {
    data: Quote,
    cached_at: Instant,
}

impl QuoteCache {
    pub fn new(db: Arc<crate::db::Database>) -> Self {
        Self {
            quotes: Mutex::new(HashMap::new()),
            indices: Mutex::new(Vec::new()),
            db,
        }
    }

    /// 从 SQLite 恢复缓存（启动时调用）
    pub fn restore_from_db(&self) {
        if let Ok(cached) = self.db.get_cached_quotes() {
            let mut quotes = self.quotes.lock().unwrap();
            for q in cached {
                let key = format!("{}:{}", q.market, q.code);
                quotes.insert(key, CachedQuote {
                    data: q,
                    cached_at: Instant::now(),
                });
            }
        }
    }

    /// 更新缓存（行情）
    pub fn update_quotes(&self, quotes: &[Quote]) {
        let mut cache = self.quotes.lock().unwrap();
        let now = Instant::now();
        for q in quotes {
            let key = format!("{}:{}", q.market, q.code);
            cache.insert(key, CachedQuote {
                data: q.clone(),
                cached_at: now,
            });
        }
        // 异步写 SQLite（best-effort）
        let _ = self.db.cache_quotes(quotes);
    }

    /// 获取所有已缓存的行情
    pub fn get_all_quotes(&self) -> Vec<Quote> {
        let cache = self.quotes.lock().unwrap();
        cache.values().map(|c| c.data.clone()).collect()
    }

    /// 获取指定 code 的行情
    pub fn get_quote(&self, market: &str, code: &str) -> Option<Quote> {
        let cache = self.quotes.lock().unwrap();
        let key = format!("{}:{}", market, code);
        cache.get(&key).map(|c| c.data.clone())
    }

    pub fn update_indices(&self, indices: Vec<IndexQuote>) {
        *self.indices.lock().unwrap() = indices;
    }

    pub fn get_indices(&self) -> Vec<IndexQuote> {
        self.indices.lock().unwrap().clone()
    }
}

/// 定时轮询调度器
pub struct Scheduler;

impl Scheduler {
    /// 启动全局轮询 loop（在后台 tokio task 中运行）
    pub fn spawn(
        data_manager: Arc<crate::datasource::DataSourceManager>,
        cache: Arc<QuoteCache>,
        db: Arc<crate::db::Database>,
        app_handle: tauri::AppHandle,
        interval_secs: u64,
    ) {
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(interval_secs));

            loop {
                interval.tick().await;

                // 1. 获取自选 codes
                let codes = match db.get_watch_codes() {
                    Ok(c) if !c.is_empty() => c,
                    _ => {
                        // 无自选时仅刷新指数
                        Self::fetch_and_emit_indices(
                            &data_manager, &cache, &app_handle
                        ).await;
                        continue;
                    }
                };

                // 2. 按 market 分组
                let mut cn_codes: Vec<String> = Vec::new();
                for (code, market) in &codes {
                    if market == "CN" {
                        cn_codes.push(code.clone());
                    }
                }

                // 3. 批量获取行情
                if !cn_codes.is_empty() {
                    let source = data_manager.active_source();
                    match source.fetch_realtime(&cn_codes, "CN").await {
                        Ok(quotes) => {
                            cache.update_quotes(&quotes);
                            let _ = app_handle.emit("quotes-updated", &quotes);
                        }
                        Err(_e) => {
                            // 降级：发送缓存数据
                            let cached = cache.get_all_quotes();
                            let _ = app_handle.emit("quotes-updated", &cached);
                        }
                    }
                }

                // 4. 指数轮询（每轮也刷新指数）
                Self::fetch_and_emit_indices(&data_manager, &cache, &app_handle).await;
            }
        });
    }

    async fn fetch_and_emit_indices(
        manager: &crate::datasource::DataSourceManager,
        cache: &QuoteCache,
        app_handle: &tauri::AppHandle,
    ) {
        let source = manager.active_source();
        if let Ok(indices) = source.fetch_indices().await {
            cache.update_indices(indices.clone());
            let _ = app_handle.emit("indices-updated", &indices);
        }
    }
}
```

- [ ] **Step 2: 在 lib.rs 注册模块**

```rust
// src-tauri/src/lib.rs
pub mod domain;
pub mod db;
pub mod datasource;
pub mod cache;
```

- [ ] **Step 3: 验证编译**

```bash
cd src-tauri && cargo check
```

Expected: 编译成功

- [ ] **Step 4: 提交**

```bash
git add src-tauri/src/cache/ src-tauri/src/lib.rs
git commit -m "feat: add quote cache and polling scheduler"
```

---

### Task 7: Tauri Commands

**Files:**
- Create: `src-tauri/src/commands/mod.rs`
- Create: `src-tauri/src/commands/quote.rs`
- Create: `src-tauri/src/commands/watchlist.rs`
- Create: `src-tauri/src/commands/settings.rs`
- Modify: `src-tauri/src/lib.rs` (注册命令模块)

- [ ] **Step 1: 创建 commands/mod.rs**

```rust
// src-tauri/src/commands/mod.rs
pub mod quote;
pub mod watchlist;
pub mod settings;
```

- [ ] **Step 2: 创建行情命令**

```rust
// src-tauri/src/commands/quote.rs
use tauri::State;
use std::sync::Arc;
use crate::cache::QuoteCache;
use crate::datasource::DataSourceManager;
use crate::domain::{Quote, IndexQuote};

#[tauri::command]
pub fn get_quotes(cache: State<'_, Arc<QuoteCache>>) -> Vec<Quote> {
    cache.get_all_quotes()
}

#[tauri::command]
pub fn get_indices(cache: State<'_, Arc<QuoteCache>>) -> Vec<IndexQuote> {
    cache.get_indices()
}
```

- [ ] **Step 3: 创建自选命令**

```rust
// src-tauri/src/commands/watchlist.rs
use tauri::State;
use std::sync::Arc;
use crate::db::{Database, WatchItem};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct WatchRow {
    pub id: i64,
    pub code: String,
    pub market: String,
    pub name: String,
    pub sort_order: i32,
    pub added_at: String,
}

#[tauri::command]
pub fn get_watchlist(db: State<'_, Arc<Database>>) -> Result<Vec<WatchItem>, String> {
    db.get_watchlist().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn add_watch(
    db: State<'_, Arc<Database>>,
    code: String,
    market: String,
    name: String,
) -> Result<(), String> {
    db.add_watch(&code, &market, &name)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn remove_watch(
    db: State<'_, Arc<Database>>,
    code: String,
    market: String,
) -> Result<(), String> {
    db.remove_watch(&code, &market)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn reorder_watch(
    db: State<'_, Arc<Database>>,
    ids: Vec<i64>,
) -> Result<(), String> {
    db.reorder_watch(&ids).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn search_stocks(
    manager: State<'_, Arc<DataSourceManager>>,
    keyword: String,
) -> Result<Vec<crate::domain::StockBrief>, String> {
    let source = manager.active_source();
    source.search(&keyword, "CN").await
}
```

- [ ] **Step 4: 创建配置命令**

```rust
// src-tauri/src/commands/settings.rs
use tauri::State;
use std::collections::HashMap;
use std::sync::Arc;
use crate::db::Database;

#[tauri::command]
pub fn get_settings(db: State<'_, Arc<Database>>) -> Result<HashMap<String, String>, String> {
    let pairs = db.get_all_settings().map_err(|e| e.to_string())?;
    Ok(pairs.into_iter().collect())
}

#[tauri::command]
pub fn set_setting(
    db: State<'_, Arc<Database>>,
    key: String,
    value: String,
) -> Result<(), String> {
    db.set_setting(&key, &value).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn switch_datasource(
    manager: State<'_, Arc<DataSourceManager>>,
    db: State<'_, Arc<Database>>,
    name: String,
) -> Result<(), String> {
    manager.set_active(&name)?;
    db.set_setting("active_datasource", &name)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_datasources(manager: State<'_, Arc<DataSourceManager>>) -> Vec<(String, String)> {
    manager.list_sources()
        .into_iter()
        .map(|(id, name)| (id.to_string(), name.to_string()))
        .collect()
}
```

- [ ] **Step 5: 修改 lib.rs — 注册所有 commands 并注入 State**

```rust
// src-tauri/src/lib.rs
pub mod domain;
pub mod db;
pub mod datasource;
pub mod cache;
pub mod commands;

use std::sync::Arc;
use db::Database;
use datasource::DataSourceManager;
use cache::QuoteCache;

pub struct AppState {
    pub db: Arc<Database>,
    pub datasource_manager: Arc<DataSourceManager>,
    pub cache: Arc<QuoteCache>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            // 初始化数据库
            let app_dir = app.path().app_data_dir().expect("无法获取 app data dir");
            let db = Arc::new(Database::open(app_dir).expect("无法打开数据库"));

            // 初始化数据源管理器
            let mut ds_manager = DataSourceManager::new();
            ds_manager.register(Box::new(
                crate::datasource::eastmoney::EastmoneyAdapter::new()
            ));

            // 恢复上次使用的数据源
            if let Ok(Some(active)) = db.get_setting("active_datasource") {
                let _ = ds_manager.set_active(&active);
            }

            let ds_manager = Arc::new(ds_manager);

            // 初始化缓存，从 SQLite 恢复
            let cache = Arc::new(QuoteCache::new(db.clone()));
            cache.restore_from_db();

            // 管理 State
            app.manage(db.clone());
            app.manage(ds_manager.clone());
            app.manage(cache.clone());

            // 启动后台轮询
            let interval = db.get_setting("refresh_interval")
                .ok()
                .flatten()
                .and_then(|v| v.parse().ok())
                .unwrap_or(3);
            crate::cache::Scheduler::spawn(
                ds_manager,
                cache,
                db,
                app.handle().clone(),
                interval,
            );

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::quote::get_quotes,
            commands::quote::get_indices,
            commands::watchlist::get_watchlist,
            commands::watchlist::add_watch,
            commands::watchlist::remove_watch,
            commands::watchlist::reorder_watch,
            commands::watchlist::search_stocks,
            commands::settings::get_settings,
            commands::settings::set_setting,
            commands::settings::switch_datasource,
            commands::settings::list_datasources,
        ])
        .run(tauri::generate_context!())
        .expect("启动应用失败");
}
```

- [ ] **Step 6: 修改 main.rs — 使用 lib::run()**

```rust
// src-tauri/src/main.rs
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    quant_desktop_lib::run();
}
```

注意：`quant_desktop_lib` 是 Cargo.toml 中 `[lib]` 的 name。需要在 `Cargo.toml` 确认 lib name：

```toml
[lib]
name = "quant_desktop_lib"
crate-type = ["lib", "cdylib", "staticlib"]
```

- [ ] **Step 7: 验证编译**

```bash
cd src-tauri && cargo check
```

Expected: 编译成功

- [ ] **Step 8: 提交**

```bash
git add src-tauri/src/commands/ src-tauri/src/lib.rs src-tauri/src/main.rs
git commit -m "feat: add Tauri commands (quote, watchlist, settings)"
```

---

### Task 8: 系统托盘 + 窗口配置

**Files:**
- Modify: `src-tauri/src/lib.rs` (添加托盘逻辑)
- Modify: `src-tauri/tauri.conf.json` (配置多窗口)
- Modify: `src-tauri/Cargo.toml` (添加 tray 相关依赖)
- Create: `src-tauri/icons/` (托盘图标)

- [ ] **Step 1: 添加托盘图标**

需要一个 PNG 图标文件。在 `src-tauri/icons/` 中放置 `tray-icon.png`（32x32 PNG）。可使用任意临时图标。

```bash
# 如果没有图标，用 cargo 默认图标临时替代
cp src-tauri/icons/icon.png src-tauri/icons/tray-icon.png 2>/dev/null || echo "需要手动准备图标"
```

- [ ] **Step 2: 修改 tauri.conf.json — 配置多窗口**

```json
{
  "$schema": "https://raw.githubusercontent.com/tauri-apps/tauri/dev/crates/tauri-cli/schema.json",
  "productName": "QuantDesktop",
  "version": "0.1.0",
  "identifier": "com.quant-desktop.app",
  "build": {
    "beforeBuildCommand": "npm run build",
    "beforeDevCommand": "npm run dev",
    "devUrl": "http://localhost:1420",
    "frontendDist": "../dist"
  },
  "app": {
    "windows": [
      {
        "label": "main",
        "title": "QuantDesktop",
        "width": 1000,
        "height": 680,
        "visible": false,
        "center": true,
        "decorations": true,
        "resizable": true
      },
      {
        "label": "ticker",
        "title": "",
        "url": "ticker.html",
        "width": 1920,
        "height": 32,
        "decorations": false,
        "alwaysOnBottom": true,
        "skipTaskbar": true,
        "resizable": false,
        "focus": false,
        "visible": true
      }
    ],
    "trayIcon": {
      "iconPath": "icons/tray-icon.png",
      "iconAsTemplate": true,
      "tooltip": "QuantDesktop"
    },
    "security": {
      "csp": null
    }
  }
}
```

注意：Tauri 2.x 的 tray 通常在 Rust 代码中设置，而非 tauri.conf.json。如果 tauri.conf.json 不支持 trayIcon，则仅在 Rust 代码中实现。

- [ ] **Step 3: 在 lib.rs 中添加托盘逻辑**

在 `src-tauri/src/lib.rs` 的 `run()` 函数中，在 `.setup()` 之后添加：

```rust
use tauri::{
    menu::{MenuBuilder, MenuItemBuilder},
    tray::{TrayIconBuilder, MouseButton, MouseButtonState, TrayIconEvent},
    Manager,
};

// 在 .setup() callback 内部添加托盘:
let handle = app.handle().clone();

// 托盘菜单
let show_item = MenuItemBuilder::with_id("show", "显示主界面").build(app)?;
let hide_ticker_item = MenuItemBuilder::with_id("toggle_ticker", "显示/隐藏行情条").build(app)?;
let separator = MenuItemBuilder::with_id("sep", "").build(app)?; // 实际用 native 分隔线
let quit_item = MenuItemBuilder::with_id("quit", "退出").build(app)?;
let menu = MenuBuilder::new(app)
    .item(&show_item)
    .item(&hide_ticker_item)
    .separator()
    .item(&quit_item)
    .build()?;

let _tray = TrayIconBuilder::new()
    .icon(app.default_window_icon().unwrap().clone())
    .tooltip("QuantDesktop")
    .menu(&menu)
    .on_menu_event(move |app, event| {
        match event.id().as_ref() {
            "show" => {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
            "toggle_ticker" => {
                if let Some(window) = app.get_webview_window("ticker") {
                    if window.is_visible().unwrap_or(false) {
                        let _ = window.hide();
                    } else {
                        let _ = window.show();
                    }
                }
            }
            "quit" => {
                app.exit(0);
            }
            _ => {}
        }
    })
    .on_tray_icon_event(|tray, event| {
        if let TrayIconEvent::Click {
            button: MouseButton::Left,
            button_state: MouseButtonState::Up,
            ..
        } = event {
            let app = tray.app_handle();
            if let Some(window) = app.get_webview_window("main") {
                if window.is_visible().unwrap_or(false) {
                    let _ = window.hide();
                } else {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
        }
    })
    .build(app)?;
```

- [ ] **Step 4: 确保 Cargo.toml 有 tray 相关 feature**

在 `src-tauri/Cargo.toml` 的 `[dependencies]` 中确认 tauri 有 tray 特性：

```toml
tauri = { version = "2", features = ["tray-icon"] }
```

- [ ] **Step 5: 验证编译**

```bash
cd src-tauri && cargo check
```

Expected: 编译成功

- [ ] **Step 6: 提交**

```bash
git add src-tauri/src/lib.rs src-tauri/tauri.conf.json src-tauri/Cargo.toml
git commit -m "feat: add system tray with right-click menu and multi-window config"
```

---

### Task 9: Vue 前端基础 — 类型定义 + Pinia Stores + Event 监听

**Files:**
- Create: `src/types/index.ts`
- Create: `src/stores/quote.ts`
- Create: `src/stores/watchlist.ts`
- Create: `src/stores/settings.ts`
- Create: `src/composables/useTauriEvent.ts`
- Create: `src/composables/useTheme.ts`
- Create: `src/assets/styles/variables.css`
- Create: `src/assets/styles/dark.css`

- [ ] **Step 1: 创建 TypeScript 类型定义**

```typescript
// src/types/index.ts
export interface Quote {
  code: string;
  market: string;
  name: string;
  price: number;
  change: number;
  change_pct: number;
  open: number;
  high: number;
  low: number;
  volume: number;
  turnover: number;
  timestamp: number;
}

export interface IndexQuote {
  code: string;
  name: string;
  price: number;
  change: number;
  change_pct: number;
  volume: number;
  turnover: number;
}

export interface StockBrief {
  code: string;
  market: string;
  name: string;
}

export interface WatchItem {
  id: number;
  code: string;
  market: string;
  name: string;
  sort_order: number;
  added_at: string;
}
```

- [ ] **Step 2: 创建行情 Pinia Store**

```typescript
// src/stores/quote.ts
import { defineStore } from 'pinia';
import { ref } from 'vue';
import type { Quote, IndexQuote } from '@/types';
import { useTauriEvent } from '@/composables/useTauriEvent';

export const useQuoteStore = defineStore('quote', () => {
  const quotes = ref<Map<string, Quote>>(new Map());
  const indices = ref<IndexQuote[]>([]);
  const lastUpdate = ref<number>(0);

  // 监听 Tauri 推送的行情更新
  useTauriEvent<Quote[]>('quotes-updated', (data) => {
    const map = new Map<string, Quote>();
    for (const q of data) {
      map.set(`${q.market}:${q.code}`, q);
    }
    quotes.value = map;
    lastUpdate.value = Date.now();
  });

  useTauriEvent<IndexQuote[]>('indices-updated', (data) => {
    indices.value = data;
  });

  function getQuote(code: string, market = 'CN'): Quote | undefined {
    return quotes.value.get(`${market}:${code}`);
  }

  return { quotes, indices, lastUpdate, getQuote };
});
```

- [ ] **Step 3: 创建自选 Pinia Store**

```typescript
// src/stores/watchlist.ts
import { defineStore } from 'pinia';
import { ref } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import type { WatchItem } from '@/types';

export const useWatchlistStore = defineStore('watchlist', () => {
  const items = ref<WatchItem[]>([]);
  const loading = ref(false);

  async function fetchWatchlist() {
    loading.value = true;
    try {
      items.value = await invoke<WatchItem[]>('get_watchlist');
    } finally {
      loading.value = false;
    }
  }

  async function addStock(code: string, market: string, name: string) {
    await invoke('add_watch', { code, market, name });
    await fetchWatchlist();
  }

  async function removeStock(code: string, market: string) {
    await invoke('remove_watch', { code, market });
    await fetchWatchlist();
  }

  async function reorder(ids: number[]) {
    await invoke('reorder_watch', { ids });
    await fetchWatchlist();
  }

  return { items, loading, fetchWatchlist, addStock, removeStock, reorder };
});
```

- [ ] **Step 4: 创建配置 Pinia Store**

```typescript
// src/stores/settings.ts
import { defineStore } from 'pinia';
import { ref } from 'vue';
import { invoke } from '@tauri-apps/api/core';

export const useSettingsStore = defineStore('settings', () => {
  const settings = ref<Record<string, string>>({});
  const datasources = ref<[string, string][]>([]);
  const activeDatasource = ref('eastmoney');

  async function fetchSettings() {
    settings.value = await invoke<Record<string, string>>('get_settings');
    activeDatasource.value = settings.value['active_datasource'] || 'eastmoney';
    datasources.value = await invoke<[string, string][]>('list_datasources');
  }

  async function setSetting(key: string, value: string) {
    await invoke('set_setting', { key, value });
    settings.value[key] = value;
  }

  async function switchDatasource(name: string) {
    await invoke('switch_datasource', { name });
    activeDatasource.value = name;
    settings.value['active_datasource'] = name;
  }

  const theme = ref<'dark' | 'light'>('dark');

  function toggleTheme() {
    theme.value = theme.value === 'dark' ? 'light' : 'dark';
    document.documentElement.setAttribute('data-theme', theme.value);
    setSetting('theme', theme.value);
  }

  return { settings, datasources, activeDatasource, theme, fetchSettings, setSetting, switchDatasource, toggleTheme };
});
```

- [ ] **Step 5: 创建 Event 监听 composable**

```typescript
// src/composables/useTauriEvent.ts
import { onMounted, onUnmounted } from 'vue';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';

export function useTauriEvent<T>(event: string, handler: (data: T) => void) {
  let unlisten: UnlistenFn | null = null;

  onMounted(async () => {
    unlisten = await listen<T>(event, (e) => {
      handler(e.payload);
    });
  });

  onUnmounted(() => {
    unlisten?.();
  });
}
```

- [ ] **Step 6: 创建主题 composable**

```typescript
// src/composables/useTheme.ts
import { ref, watch } from 'vue';

const currentTheme = ref<'dark' | 'light'>('dark');

export function useTheme() {
  function applyTheme(theme: 'dark' | 'light') {
    currentTheme.value = theme;
    document.documentElement.setAttribute('data-theme', theme);
  }

  function toggle() {
    applyTheme(currentTheme.value === 'dark' ? 'light' : 'dark');
  }

  return { currentTheme, applyTheme, toggle };
}
```

- [ ] **Step 7: 创建暗色主题 CSS**

```css
/* src/assets/styles/variables.css */
:root {
  --color-up: #ef5350;
  --color-down: #66bb6a;
  --color-bg: #1e1e2e;
  --color-card-bg: #252536;
  --color-text-primary: #e0e0e0;
  --color-text-secondary: #888888;
  --color-border: #333344;
  --color-header-bg: #1a1a2e;
  --font-size-sm: 12px;
  --font-size-base: 14px;
  --font-size-lg: 16px;
}

[data-theme="light"] {
  --color-bg: #ffffff;
  --color-card-bg: #fafafa;
  --color-text-primary: #1a1a1a;
  --color-text-secondary: #666666;
  --color-border: #e8e8e8;
  --color-header-bg: #f0f0f0;
}

/* src/assets/styles/dark.css */
body {
  background-color: var(--color-bg);
  color: var(--color-text-primary);
  font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', 'PingFang SC',
    'Microsoft YaHei', sans-serif;
  margin: 0;
  padding: 0;
  font-size: var(--font-size-base);
}

* {
  box-sizing: border-box;
}
```

- [ ] **Step 8: 验证构建**

```bash
npm run build
```

Expected: 构建成功

- [ ] **Step 9: 提交**

```bash
git add src/types/ src/stores/ src/composables/ src/assets/styles/
git commit -m "feat: add Vue types, Pinia stores, event composables, and dark theme CSS"
```

---

### Task 10: Vue 前端 — App.vue 入口 + 主界面组件

**Files:**
- Create: `src/components/layout/AppLayout.vue`
- Create: `src/components/layout/TopBar.vue`
- Create: `src/components/index/IndexBar.vue`
- Create: `src/components/index/IndexCard.vue`
- Create: `src/components/watchlist/WatchlistTable.vue`
- Create: `src/components/watchlist/AddStockDialog.vue`
- Modify: `src/App.vue`

- [ ] **Step 1: 创建 App.vue 入口**

```vue
<!-- src/App.vue -->
<script setup lang="ts">
import { onMounted } from 'vue';
import { NConfigProvider, darkTheme, lightTheme } from 'naive-ui';
import { useSettingsStore } from '@/stores/settings';
import { useWatchlistStore } from '@/stores/watchlist';
import AppLayout from '@/components/layout/AppLayout.vue';

const settings = useSettingsStore();
const watchlist = useWatchlistStore();

onMounted(async () => {
  await settings.fetchSettings();
  await watchlist.fetchWatchlist();
});
</script>

<template>
  <n-config-provider :theme="settings.theme === 'dark' ? darkTheme : lightTheme">
    <AppLayout />
  </n-config-provider>
</template>
```

- [ ] **Step 2: 创建主布局 AppLayout.vue**

```vue
<!-- src/components/layout/AppLayout.vue -->
<script setup lang="ts">
import TopBar from './TopBar.vue';
import IndexBar from '@/components/index/IndexBar.vue';
import WatchlistTable from '@/components/watchlist/WatchlistTable.vue';
</script>

<template>
  <div class="app-layout">
    <TopBar />
    <IndexBar />
    <WatchlistTable />
  </div>
</template>

<style scoped>
.app-layout {
  display: flex;
  flex-direction: column;
  height: 100vh;
  background-color: var(--color-bg);
}
</style>
```

- [ ] **Step 3: 创建 TopBar.vue**

```vue
<!-- src/components/layout/TopBar.vue -->
<script setup lang="ts">
import { useSettingsStore } from '@/stores/settings';
import { NIcon, NButton, NTag } from 'naive-ui';
import { Sunny16Filled, WeatherMoon16Filled } from '@vicons/fluent';

const settings = useSettingsStore();
</script>

<template>
  <div class="top-bar">
    <div class="top-left">
      <span class="app-title">📈 QuantDesktop</span>
      <n-tag size="small" :bordered="false">
        {{ settings.activeDatasource }}
      </n-tag>
    </div>
    <div class="top-right">
      <n-button text @click="settings.toggleTheme()">
        <n-icon size="18">
          <WeatherMoon16Filled v-if="settings.theme === 'dark'" />
          <Sunny16Filled v-else />
        </n-icon>
      </n-button>
    </div>
  </div>
</template>

<style scoped>
.top-bar {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 8px 16px;
  background-color: var(--color-header-bg);
  border-bottom: 1px solid var(--color-border);
  flex-shrink: 0;
}
.top-left {
  display: flex;
  align-items: center;
  gap: 12px;
}
.app-title {
  font-weight: 700;
  font-size: 16px;
  color: var(--color-text-primary);
}
.top-right {
  display: flex;
  align-items: center;
  gap: 8px;
}
</style>
```

> 注：图标库 `@vicons/fluent` 需要安装：`npm install @vicons/fluent`

- [ ] **Step 4: 创建指数横条 IndexBar.vue + IndexCard.vue**

```vue
<!-- src/components/index/IndexBar.vue -->
<script setup lang="ts">
import { useQuoteStore } from '@/stores/quote';
import IndexCard from './IndexCard.vue';

const quote = useQuoteStore();
</script>

<template>
  <div class="index-bar">
    <IndexCard
      v-for="idx in quote.indices"
      :key="idx.code"
      :index="idx"
    />
    <div v-if="quote.indices.length === 0" class="index-placeholder">
      等待指数数据...
    </div>
  </div>
</template>

<style scoped>
.index-bar {
  display: flex;
  gap: 10px;
  padding: 8px 16px;
  border-bottom: 1px solid var(--color-border);
  flex-shrink: 0;
  overflow-x: auto;
}
.index-placeholder {
  color: var(--color-text-secondary);
  font-size: var(--font-size-sm);
  padding: 8px 0;
}
</style>
```

```vue
<!-- src/components/index/IndexCard.vue -->
<script setup lang="ts">
import type { IndexQuote } from '@/types';
import { computed } from 'vue';

const props = defineProps<{ index: IndexQuote }>();

const isUp = computed(() => props.index.change_pct >= 0);
</script>

<template>
  <div class="index-card">
    <span class="index-name">{{ index.name }}</span>
    <span class="index-price" :class="isUp ? 'up' : 'down'">
      {{ index.price.toFixed(2) }}
    </span>
    <span class="index-change" :class="isUp ? 'up' : 'down'">
      {{ isUp ? '+' : '' }}{{ index.change_pct.toFixed(2) }}%
    </span>
  </div>
</template>

<style scoped>
.index-card {
  display: flex;
  gap: 8px;
  align-items: center;
  background-color: var(--color-card-bg);
  border-radius: 6px;
  padding: 6px 12px;
  flex-shrink: 0;
  font-size: var(--font-size-sm);
}
.index-name {
  color: var(--color-text-secondary);
}
.index-price {
  font-weight: 700;
}
.up { color: var(--color-up); }
.down { color: var(--color-down); }
.index-change {
  font-weight: 500;
}
</style>
```

- [ ] **Step 5: 创建自选列表 WatchlistTable.vue**

```vue
<!-- src/components/watchlist/WatchlistTable.vue -->
<script setup lang="ts">
import { computed } from 'vue';
import { NButton, NDataTable, NSpace, NModal, NCard } from 'naive-ui';
import type { DataTableColumns } from 'naive-ui';
import { useWatchlistStore } from '@/stores/watchlist';
import { useQuoteStore } from '@/stores/quote';
import type { WatchItem } from '@/types';
import AddStockDialog from './AddStockDialog.vue';
import { ref } from 'vue';

const watchlist = useWatchlistStore();
const quoteStore = useQuoteStore();
const showAddDialog = ref(false);

interface WatchRowData extends WatchItem {
  price?: number;
  change?: number;
  change_pct?: number;
}

const columns: DataTableColumns<WatchRowData> = [
  { title: '代码', key: 'code', width: 80 },
  { title: '名称', key: 'name', width: 100 },
  {
    title: '最新价', key: 'price', width: 90,
    render(row) {
      const q = quoteStore.getQuote(row.code, row.market);
      return q?.price?.toFixed(2) ?? '--';
    }
  },
  {
    title: '涨跌幅', key: 'change_pct', width: 90,
    render(row) {
      const q = quoteStore.getQuote(row.code, row.market);
      if (!q) return '--';
      const v = q.change_pct;
      return h('span', { style: { color: v >= 0 ? 'var(--color-up)' : 'var(--color-down)' } },
        `${v >= 0 ? '+' : ''}${v.toFixed(2)}%`);
    }
  },
  {
    title: '涨跌额', key: 'change', width: 90,
    render(row) {
      const q = quoteStore.getQuote(row.code, row.market);
      if (!q) return '--';
      const v = q.change;
      return h('span', { style: { color: v >= 0 ? 'var(--color-up)' : 'var(--color-down)' } },
        `${v >= 0 ? '+' : ''}${v.toFixed(2)}`);
    }
  },
  {
    title: '操作', key: 'action', width: 80,
    render(row) {
      return h(NButton, {
        size: 'tiny', text: true, type: 'error',
        onClick: () => watchlist.removeStock(row.code, row.market)
      }, { default: () => '删除' });
    }
  },
];

import { h } from 'vue';
</script>

<template>
  <div class="watchlist-container">
    <div class="watchlist-header">
      <span class="section-title">自选股</span>
      <NButton size="small" type="primary" @click="showAddDialog = true">
        + 添加
      </NButton>
    </div>
    <NDataTable
      :columns="columns"
      :data="watchlist.items"
      :bordered="false"
      :single-line="false"
      size="small"
      max-height="500"
      virtual-scroll
    />
    <AddStockDialog v-model:show="showAddDialog" />
  </div>
</template>

<style scoped>
.watchlist-container {
  flex: 1;
  padding: 8px 16px;
  overflow: auto;
}
.watchlist-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 8px;
}
.section-title {
  font-weight: 600;
  font-size: var(--font-size-base);
  color: var(--color-text-primary);
}
</style>
```

- [ ] **Step 6: 创建添加自选弹窗 AddStockDialog.vue**

```vue
<!-- src/components/watchlist/AddStockDialog.vue -->
<script setup lang="ts">
import { ref, watch } from 'vue';
import { NModal, NInput, NList, NListItem, NButton, NSpace, useMessage } from 'naive-ui';
import { invoke } from '@tauri-apps/api/core';
import type { StockBrief } from '@/types';
import { useWatchlistStore } from '@/stores/watchlist';

const props = defineProps<{ show: boolean }>();
const emit = defineEmits<{ 'update:show': (v: boolean) => void }>();
const message = useMessage();
const watchlist = useWatchlistStore();

const keyword = ref('');
const results = ref<StockBrief[]>([]);
const searching = ref(false);
let debounceTimer: ReturnType<typeof setTimeout> | null = null;

watch(() => keyword.value, (val) => {
  if (debounceTimer) clearTimeout(debounceTimer);
  if (!val || val.trim().length === 0) {
    results.value = [];
    return;
  }
  debounceTimer = setTimeout(async () => {
    searching.value = true;
    try {
      results.value = await invoke<StockBrief[]>('search_stocks', { keyword: val.trim() });
    } catch {
      results.value = [];
    } finally {
      searching.value = false;
    }
  }, 300);
});

async function handleAdd(stock: StockBrief) {
  await watchlist.addStock(stock.code, stock.market, stock.name);
  message.success(`已添加 ${stock.name}`);
  keyword.value = '';
  results.value = [];
}
</script>

<template>
  <NModal :show="props.show" @update:show="emit('update:show', $event)">
    <NCard title="添加自选" style="width: 400px;" closable @close="emit('update:show', false)">
      <NSpace vertical>
        <NInput
          v-model:value="keyword"
          placeholder="输入代码或名称搜索..."
          :loading="searching"
          clearable
        />
        <NList v-if="results.length > 0" hoverable>
          <NListItem v-for="s in results" :key="s.code">
            <div class="search-result">
              <div>
                <span class="result-name">{{ s.name }}</span>
                <span class="result-code">{{ s.code }}</span>
              </div>
              <NButton size="tiny" @click="handleAdd(s)">+ 添加</NButton>
            </div>
          </NListItem>
        </NList>
      </NSpace>
    </NCard>
  </NModal>
</template>

<style scoped>
.search-result {
  display: flex;
  justify-content: space-between;
  align-items: center;
  width: 100%;
}
.result-name {
  font-weight: 500;
  margin-right: 8px;
}
.result-code {
  color: var(--color-text-secondary);
  font-size: var(--font-size-sm);
}
</style>
```

- [ ] **Step 7: 修改 main.ts 注册 Pinia + Naive UI**

```typescript
// src/main.ts
import { createApp } from 'vue';
import { createPinia } from 'pinia';
import App from './App.vue';
import './assets/styles/variables.css';
import './assets/styles/dark.css';

const app = createApp(App);
app.use(createPinia());
app.mount('#app');
```

- [ ] **Step 8: 安装图标依赖 + 验证构建**

```bash
npm install @vicons/fluent
npm run build
```

Expected: 构建成功

- [ ] **Step 9: 提交**

```bash
git add src/App.vue src/main.ts src/components/ package.json
git commit -m "feat: add main UI components (TopBar, IndexBar, WatchlistTable, AddStockDialog)"
```

---

### Task 11: Vue 前端 — 行情条 TickerBar 独立窗口

**Files:**
- Create: `src/components/ticker/TickerBar.vue`
- Create: `src/ticker.ts` (行情条独立入口)
- Create: `ticker.html` (行情条 HTML)
- Modify: `vite.config.ts` (多入口构建)

- [ ] **Step 1: 创建行情条入口 HTML**

```html
<!-- ticker.html -->
<!DOCTYPE html>
<html lang="zh-CN">
<head>
  <meta charset="UTF-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1.0" />
  <title>Ticker</title>
  <style>
    body { margin: 0; padding: 0; overflow: hidden; background: transparent; }
  </style>
</head>
<body>
  <div id="app"></div>
  <script type="module" src="/src/ticker.ts"></script>
</body>
</html>
```

- [ ] **Step 2: 创建行情条 Vue 入口**

```typescript
// src/ticker.ts
import { createApp } from 'vue';
import { createPinia } from 'pinia';
import TickerBar from './components/ticker/TickerBar.vue';
import './assets/styles/variables.css';
import './assets/styles/dark.css';

const app = createApp(TickerBar);
app.use(createPinia());
app.mount('#app');
```

- [ ] **Step 3: 创建 TickerBar.vue 组件**

```vue
<!-- src/components/ticker/TickerBar.vue -->
<script setup lang="ts">
import { computed, onMounted, ref } from 'vue';
import { useQuoteStore } from '@/stores/quote';
import { useWatchlistStore } from '@/stores/watchlist';

const quoteStore = useQuoteStore();
const watchlist = useWatchlistStore();
const paused = ref(false);

onMounted(async () => {
  await watchlist.fetchWatchlist();
});

const tickerItems = computed(() => {
  return watchlist.items.map(item => {
    const q = quoteStore.getQuote(item.code, item.market);
    return {
      name: item.name,
      code: item.code,
      price: q?.price ?? null,
      changePct: q?.change_pct ?? null,
    };
  });
});

const scrollStyle = computed(() => ({
  animationPlayState: paused.value ? 'paused' : 'running',
}));
</script>

<template>
  <div
    class="ticker-bar"
    @mouseenter="paused = true"
    @mouseleave="paused = false"
  >
    <div class="ticker-track" :style="scrollStyle">
      <span
        v-for="item in tickerItems"
        :key="item.code"
        class="ticker-item"
      >
        <span class="ticker-name">{{ item.name }}</span>
        <span v-if="item.price !== null" class="ticker-price" :class="item.changePct !== null && item.changePct >= 0 ? 'up' : 'down'">
          {{ item.price.toFixed(2) }}
        </span>
        <span v-if="item.price === null" class="ticker-na">--</span>
        <span v-if="item.changePct !== null" class="ticker-change" :class="item.changePct >= 0 ? 'up' : 'down'">
          {{ item.changePct >= 0 ? '+' : '' }}{{ item.changePct.toFixed(2) }}%
        </span>
      </span>
    </div>
  </div>
</template>

<style scoped>
.ticker-bar {
  width: 100vw;
  height: 32px;
  overflow: hidden;
  background-color: #0d1117;
  border-top: 1px solid #222;
  display: flex;
  align-items: center;
  user-select: none;
  cursor: default;
  -webkit-app-region: no-drag;
}
.ticker-track {
  display: flex;
  gap: 24px;
  white-space: nowrap;
  animation: scroll-left 30s linear infinite;
  padding: 0 16px;
}
.ticker-item {
  display: flex;
  gap: 6px;
  align-items: center;
  flex-shrink: 0;
}
.ticker-name {
  color: #999;
  font-size: 12px;
}
.ticker-price {
  font-weight: 600;
  font-size: 12px;
}
.ticker-na {
  color: #666;
  font-size: 12px;
}
.ticker-change {
  font-size: 12px;
}
.up { color: #ef5350; }
.down { color: #66bb6a; }

@keyframes scroll-left {
  0% { transform: translateX(0); }
  100% { transform: translateX(-50%); }
}
</style>
```

- [ ] **Step 4: 修改 vite.config.ts 支持多入口**

```typescript
// vite.config.ts
import { defineConfig } from 'vite';
import vue from '@vitejs/plugin-vue';
import { resolve } from 'path';
import { fileURLToPath } from 'url';

const __dirname = fileURLToPath(new URL('.', import.meta.url));

export default defineConfig({
  plugins: [vue()],
  resolve: {
    alias: {
      '@': resolve(__dirname, 'src'),
    },
  },
  build: {
    rollupOptions: {
      input: {
        main: resolve(__dirname, 'index.html'),
        ticker: resolve(__dirname, 'ticker.html'),
      },
    },
  },
});
```

- [ ] **Step 5: 验证构建**

```bash
npm run build
```

Expected: 构建成功，dist 目录下有 ticker.html

- [ ] **Step 6: 提交**

```bash
git add src/components/ticker/ src/ticker.ts ticker.html vite.config.ts
git commit -m "feat: add ticker bar with scrolling animation and multi-entry build"
```

---

### Task 12: 联调集成与最终验证

**Files:**
- Modify: `src-tauri/src/lib.rs` (确保所有部分正确集成)
- Modify: `src-tauri/tauri.conf.json` (确认窗口 URL 正确)

- [ ] **Step 1: 全量编译检查**

```bash
cd src-tauri && cargo check
npm run build
```

Expected: 两者均无错误

- [ ] **Step 2: 检查 Tauri dev 模式启动**

```bash
cd e:/GIT/github/quant-desktop
npx tauri dev
```

Expected: 
- 系统托盘图标出现
- 行情条窗口出现在屏幕底部
- 点击托盘图标可打开主界面
- 指数数据正常加载显示

- [ ] **Step 3: 检查关键数据流**

在 Tauri dev 模式下验证：
- 东方财富 API 数据拉取正常（检查终端日志）
- 自选添加/删除功能正常
- 轮询推送正常（行情自动刷新）

- [ ] **Step 4: 修复集成问题后进行最终提交**

```bash
git add -A
git commit -m "feat: complete Phase 1 MVP integration"
```

---

## 附录 A: 默认配置初始化

在 Database 首次创建后，写入默认 settings：

```rust
// 在 Database::open() 后调用
pub fn init_defaults(&self) {
    let defaults = [
        ("active_datasource", "eastmoney"),
        ("refresh_interval", "3"),
        ("theme", "dark"),
        ("ticker_visible", "true"),
    ];
    for (k, v) in defaults {
        // 仅在 key 不存在时插入
        if self.get_setting(k).ok().flatten().is_none() {
            let _ = self.set_setting(k, v);
        }
    }
}
```

## 附录 B: Cargo.toml 完整依赖清单

```toml
[dependencies]
tauri = { version = "2", features = ["tray-icon"] }
tauri-plugin-shell = "2"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
rusqlite = { version = "0.31", features = ["bundled"] }
reqwest = { version = "0.12", features = ["json"] }
async-trait = "0.1"
chrono = { version = "0.4", features = ["serde"] }
tokio = { version = "1", features = ["full"] }
```

## 附录 C: package.json 完整依赖清单

```json
{
  "dependencies": {
    "@tauri-apps/api": "^2",
    "@tauri-apps/plugin-shell": "^2",
    "naive-ui": "^2",
    "pinia": "^2",
    "klinecharts": "^9",
    "vue": "^3",
    "@vicons/fluent": "^0.12"
  },
  "devDependencies": {
    "@tauri-apps/cli": "^2",
    "typescript": "^5",
    "vite": "^5",
    "@vitejs/plugin-vue": "^5",
    "@types/node": "^20"
  }
}
```
