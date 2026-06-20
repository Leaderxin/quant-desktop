# QuantDesktop 代码审查修复计划与技术方案

**基于**: [CODE_REVIEW_REPORT.md](CODE_REVIEW_REPORT.md) 第 8.2 节（需立即修复）和 8.3 节（主要改进建议）  
**计划日期**: 2026-06-20

---

## 目录

1. [修复优先级矩阵](#1-修复优先级矩阵)
2. [🔴 P0: 立即修复 (Bug)](#2-p0-立即修复-bug)
   - [F4: TickerBar 拖拽空 catch 块](#21-f4-tickerbar-拖拽空-catch-块)
3. [🟡 P1: 高优先级改进](#3-p1-高优先级改进)
   - [B11: move_watch_top 非原子操作 + 同类方法修复](#31-b11-move_watch_top-非原子操作--同类方法修复)
   - [F3/PERF1: Ticker 事件驱动更新替代轮询](#32-f3perf1-ticker-事件驱动更新替代轮询)
4. [🟢 P2: 中优先级改进](#4-p2-中优先级改进)
   - [D1: 引入结构化错误类型](#41-d1-引入结构化错误类型)
   - [B6: 统一数据归一化逻辑](#42-b6-统一数据归一化逻辑)
   - [D2: 共享 reqwest::Client 单例](#43-d2-共享-reqwestclient-单例)
   - [F1: 全局错误边界](#44-f1-全局错误边界)
   - [F10/PERF2: 图表实例复用](#45-f10perf2-图表实例复用)
5. [实施顺序与时间估算](#5-实施顺序与时间估算)

---

## 1. 修复优先级矩阵

| 优先级 | 编号 | 描述 | 影响范围 | 风险 | 工作量 |
|--------|------|------|----------|------|--------|
| 🔴 P0 | F4 | Ticker 拖拽静默吞错 | 用户体验/窗口管理 | 低 | 0.5h |
| 🟡 P1 | B11 | move_watch 原子性 | 数据一致性 | 中 | 2h |
| 🟡 P1 | F3/PERF1 | 事件驱动替代轮询 | 性能/网络 | 中 | 3h |
| 🟢 P2 | D1 | 结构化错误类型 | 全局 | 高 | 4h |
| 🟢 P2 | B6 | 统一数据归一化 | datasource/ | 低 | 1.5h |
| 🟢 P2 | D2 | 共享 reqwest Client | datasource/ | 低 | 1h |
| 🟢 P2 | F1 | 全局错误边界 | 前端全局 | 低 | 1h |
| 🟢 P2 | F10 | 图表实例复用 | 图表组件 | 中 | 2h |

---

## 2. 🔴 P0: 立即修复 (Bug)

### 2.1 F4: TickerBar 拖拽空 catch 块

**文件**: [src/components/ticker/TickerBar.vue:195-203](src/components/ticker/TickerBar.vue#L195-L203)

**问题描述**:  
`onMouseDown` 中获取窗口位置、缩放因子、显示器边界的 try/catch 块为空——所有错误被静默吞掉。回退值将所有边界设为 `Infinity`，导致窗口可被拖到屏幕外无法找回。

**技术方案**:

将空 catch 替换为带日志的容错回退：

```typescript
// 修改前 (行 195-203):
  } catch {
    winStartPhysicalX = 0;
    winStartPhysicalY = 0;
    scaleFactor = 1;
    clampMinX = 0;
    clampMinY = 0;
    clampMaxX = Infinity;
    clampMaxY = Infinity;
  }

// 修改后:
  } catch (e) {
    console.error('[TickerBar] 拖拽初始化失败，使用安全回退值:', e);
    winStartPhysicalX = 0;
    winStartPhysicalY = 0;
    scaleFactor = 1;
    // 使用合理的 4K 默认边界防止窗口被拖出屏幕
    clampMinX = 0;
    clampMinY = 0;
    clampMaxX = 3840;
    clampMaxY = 2160;
  }
```

**改动文件**:
- `src/components/ticker/TickerBar.vue` — 1 处修改

**验证方法**:
- 正常拖拽 ticker 窗口不受影响
- 模拟 `availableMonitors()` 返回错误时的行为（可将方法临时改为 `throw`）

---

## 3. 🟡 P1: 高优先级改进

### 3.1 B11: move_watch_top 非原子操作 + 同类方法修复

**文件**: 
- [src-tauri/src/commands/watchlist.rs:41-51](src-tauri/src/commands/watchlist.rs#L41-L51) (move_watch_top)
- [src-tauri/src/commands/watchlist.rs:54-67](src-tauri/src/commands/watchlist.rs#L54-L67) (move_watch_up)
- [src-tauri/src/commands/watchlist.rs:69-82](src-tauri/src/commands/watchlist.rs#L69-L82) (move_watch_down)
- [src-tauri/src/db/mod.rs](src-tauri/src/db/mod.rs) — Database 新增方法

**问题描述**:  
`move_watch_top` 分两步操作：先 `get_watchlist()` 获取全列表，再 `reorder_watch()` 写回。两步之间 Mutex 释放，存在 TOCTOU 竞态。如果并发操作修改了自选股列表（虽然当前场景概率低），sort_order 会基于过期数据。

同类问题还存在于 `move_watch_up` 和 `move_watch_down`，它们同样采用"读取-修改-写入"的三段模式。

**技术方案**:

在 `Database` 中新增三个原子方法，每个方法在单次锁持有期间完成全部操作：

#### 3.1.1 Database 新增方法

在 [src-tauri/src/db/mod.rs](src-tauri/src/db/mod.rs) 的 `impl Database` 块末尾（`get_cached_quotes` 之后）新增：

```rust
/// 将指定条目移至自选列表顶部（原子操作，单次加锁）
pub fn move_watch_top(&self, id: i64) -> SqliteResult<()> {
    let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
    let mut stmt = conn.prepare(
        "SELECT id FROM watchlist ORDER BY sort_order ASC, id ASC"
    )?;
    let ids: Vec<i64> = stmt.query_map([], |row| row.get(0))?
        .collect::<SqliteResult<Vec<_>>>()?;

    let mut sort_order = 0i32;
    // 目标条目 → sort_order=0
    conn.execute(
        "UPDATE watchlist SET sort_order = ?1 WHERE id = ?2",
        params![sort_order, id],
    )?;
    sort_order += 1;
    // 其余条目按原顺序依次分配 sort_order
    for other_id in &ids {
        if *other_id != id {
            conn.execute(
                "UPDATE watchlist SET sort_order = ?1 WHERE id = ?2",
                params![sort_order, other_id],
            )?;
            sort_order += 1;
        }
    }
    Ok(())
}

/// 将指定条目上移一位（原子操作，单次加锁）
pub fn move_watch_up(&self, id: i64) -> SqliteResult<()> {
    let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
    let mut stmt = conn.prepare(
        "SELECT id FROM watchlist ORDER BY sort_order ASC, id ASC"
    )?;
    let ids: Vec<i64> = stmt.query_map([], |row| row.get(0))?
        .collect::<SqliteResult<Vec<_>>>()?;

    if let Some(pos) = ids.iter().position(|&x| x == id) {
        if pos > 0 {
            // 交换 pos 和 pos-1 的 sort_order
            let prev_id = ids[pos - 1];
            conn.execute(
                "UPDATE watchlist SET sort_order = ?1 WHERE id = ?2",
                params![pos as i32, id],
            )?;
            conn.execute(
                "UPDATE watchlist SET sort_order = ?1 WHERE id = ?2",
                params![(pos - 1) as i32, prev_id],
            )?;
        }
    }
    Ok(())
}

/// 将指定条目下移一位（原子操作，单次加锁）
pub fn move_watch_down(&self, id: i64) -> SqliteResult<()> {
    let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
    let mut stmt = conn.prepare(
        "SELECT id FROM watchlist ORDER BY sort_order ASC, id ASC"
    )?;
    let ids: Vec<i64> = stmt.query_map([], |row| row.get(0))?
        .collect::<SqliteResult<Vec<_>>>()?;

    if let Some(pos) = ids.iter().position(|&x| x == id) {
        if pos + 1 < ids.len() {
            let next_id = ids[pos + 1];
            conn.execute(
                "UPDATE watchlist SET sort_order = ?1 WHERE id = ?2",
                params![(pos + 1) as i32, id],
            )?;
            conn.execute(
                "UPDATE watchlist SET sort_order = ?1 WHERE id = ?2",
                params![pos as i32, next_id],
            )?;
        }
    }
    Ok(())
}
```

#### 3.1.2 简化 commands/watchlist.rs

修改 `src-tauri/src/commands/watchlist.rs`，三个 move 方法改为直接委托给 Database：

```rust
// move_watch_top: 20行 → 3行
#[tauri::command]
pub fn move_watch_top(
    db: State<'_, Arc<Database>>,
    id: i64,
) -> Result<(), String> {
    db.move_watch_top(id).map_err(|e| e.to_string())
}

// move_watch_up: 14行 → 3行
#[tauri::command]
pub fn move_watch_up(
    db: State<'_, Arc<Database>>,
    id: i64,
) -> Result<(), String> {
    db.move_watch_up(id).map_err(|e| e.to_string())
}

// move_watch_down: 14行 → 3行
#[tauri::command]
pub fn move_watch_down(
    db: State<'_, Arc<Database>>,
    id: i64,
) -> Result<(), String> {
    db.move_watch_down(id).map_err(|e| e.to_string())
}
```

**设计理由**:
- 三个方法逻辑合并到 Database 层，单次 `Mutex::lock` 保证原子性
- 使用 `sort_order` 直接交换而非重新分配，减少 UPDATE 次数（move_up/move_down 各只需 2 条 UPDATE）
- `move_watch_top` 仍需遍历分配（因为移除一个后所有后续元素的顺序都变了）
- Tauri command 变为单纯的委托层，职责清晰

**改动文件**:
- `src-tauri/src/db/mod.rs` — 新增 3 个方法 (约70行)
- `src-tauri/src/commands/watchlist.rs` — 简化 3 个方法 (减少约40行)

**验证方法**:
- 手动测试：添加多只股票，使用右键菜单移动它们（置顶/上移/下移），验证排序正确
- 代码审查确认：单次 `conn.lock()` 持有期间完成所有 SQL 操作

---

### 3.2 F3/PERF1: Ticker 事件驱动更新替代轮询

**文件**:
- [src/components/ticker/TickerBar.vue](src/components/ticker/TickerBar.vue) — 前端
- [src-tauri/src/commands/watchlist.rs](src-tauri/src/commands/watchlist.rs) — 后端
- [src-tauri/src/lib.rs](src-tauri/src/lib.rs) — 后端（可能需要）

**问题描述**:  
TickerBar 每 3 秒通过 `setInterval` 调用 `watchlist.fetchWatchlist()` 轮询自选股变化。这产生不必要的 IPC 调用和 SQLite 查询，即使自选股列表未发生任何变化。

**技术方案**:

采用 **事件驱动** 替代轮询：

```
┌──────────────────────────────────────────────────────┐
│  修改前 (轮询)                                        │
│  TickerBar ──(每3s)──► invoke("get_watchlist") ──► DB │
│                                                       │
│  修改后 (事件驱动)                                     │
│  MainWindow 增删改 ──► emit("watchlist-changed")      │
│  TickerBar ◄── listen("watchlist-changed") ──► 更新    │
└──────────────────────────────────────────────────────┘
```

#### 3.2.1 后端: 自选股变更后发射事件

修改 `src-tauri/src/commands/watchlist.rs`，为增/删/排序操作添加 `AppHandle` 参数并发射事件：

```rust
use tauri::Emitter; // 新增 import

#[tauri::command]
pub fn add_watch(
    db: State<'_, Arc<Database>>,
    app_handle: tauri::AppHandle,  // 新增参数
    code: String,
    market: String,
    name: String,
) -> Result<(), String> {
    db.add_watch(&code, &market, &name)
        .map_err(|e| e.to_string())?;
    // 通知所有窗口自选股已变更
    let _ = app_handle.emit("watchlist-changed", ());
    Ok(())
}

#[tauri::command]
pub fn remove_watch(
    db: State<'_, Arc<Database>>,
    app_handle: tauri::AppHandle,
    code: String,
    market: String,
) -> Result<(), String> {
    db.remove_watch(&code, &market)
        .map_err(|e| e.to_string())?;
    let _ = app_handle.emit("watchlist-changed", ());
    Ok(())
}

// move_watch_top, move_watch_up, move_watch_down, reorder_watch 同样处理
#[tauri::command]
pub fn reorder_watch(
    db: State<'_, Arc<Database>>,
    app_handle: tauri::AppHandle,
    ids: Vec<i64>,
) -> Result<(), String> {
    db.reorder_watch(&ids).map_err(|e| e.to_string())?;
    let _ = app_handle.emit("watchlist-changed", ());
    Ok(())
}

#[tauri::command]
pub fn move_watch_top(
    db: State<'_, Arc<Database>>,
    app_handle: tauri::AppHandle,
    id: i64,
) -> Result<(), String> {
    db.move_watch_top(id).map_err(|e| e.to_string())?;
    let _ = app_handle.emit("watchlist-changed", ());
    Ok(())
}
// move_watch_up / move_watch_down 同理
```

> **Tauri v2 说明**: `AppHandle` 可以直接作为 `#[tauri::command]` 参数注入，框架自动从当前应用上下文获取。这是 Tauri v2 的标准模式。

#### 3.2.2 前端 Main Window: watchlist store 触发的 re-fetch 不变

Main window 的 `watchlist` store 在 `addStock`/`removeStock` 中本来就会 `re-fetch`，无需修改。Main window 不需要监听 `watchlist-changed` 事件（它自己触发的变更已经通过 re-fetch 同步了）。

#### 3.2.3 前端 TickerBar: 替换轮询为事件监听

修改 `src/components/ticker/TickerBar.vue`：

```typescript
// 删除: watchlistPollTimer 变量 (第20行)
// 删除: startWatchlistPoll 函数 (第51-55行)
// 新增: watchlist-changed 事件监听

let unlistenWatchlist: UnlistenFn | null = null;

function startWatchlistListener() {
  listen('watchlist-changed', () => {
    watchlist.fetchWatchlist().catch((e) => {
      console.error('[TickerBar] watchlist-changed refresh failed:', e);
    });
  }).then((unlisten) => {
    unlistenWatchlist = unlisten;
  }).catch((e) => {
    console.error('[TickerBar] Failed to listen watchlist-changed:', e);
  });
}
```

在 `onMounted` 中将 `startWatchlistPoll()` 替换为 `startWatchlistListener()`：

```typescript
onMounted(async () => {
  try {
    await settings.fetchSettings();
    settings.applyTheme(settings.theme);
    await watchlist.fetchWatchlist();
    await quoteStore.startListening();
    startCycle();
    startThemeListen();
    startDatasourceListen();
    startWatchlistListener();  // ← 替换 startWatchlistPoll()
  } catch (e) {
    initFailed.value = true;
    console.error('[TickerBar] init failed:', e);
  }
});
```

在 `onUnmounted` 和 `handleClick` 的重试逻辑中同步更新清理代码：

```typescript
onUnmounted(() => {
  quoteStore.stopListening();
  if (cycleTimer) clearInterval(cycleTimer);
  if (unlistenTheme) unlistenTheme();
  if (unlistenDatasource) unlistenDatasource();
  if (unlistenWatchlist) unlistenWatchlist();  // ← 替换 watchlistPollTimer
  // ...drag cleanup...
});
```

> **注意**: TickerBar 初始化时仍需要首次 `fetchWatchlist()` 获取初始列表（已在 `onMounted` 中），之后依赖事件触发增量更新。

**优势分析**:
- 消除不必要的 3 秒轮询（每 3s 一次 IPC + SQLite 查询 → 仅在变更时触发）
- Tauri 事件是进程内 IPC，零额外开销
- 实时性更好（变更即时反映，而非最多延迟 3 秒）

**改动文件**:
- `src-tauri/src/commands/watchlist.rs` — 6 个方法添加 `app_handle` + `emit`
- `src/components/ticker/TickerBar.vue` — 替换轮询为事件监听

**验证方法**:
- 主窗口添加/删除自选股，观察 ticker 是否即时更新
- ticker 窗口无自选股变更时，确认不再有每 3 秒的 IPC 调用
- 可以在 Rust 后端添加 `log::debug!` 在 emit 时输出来验证事件触发

---

## 4. 🟢 P2: 中优先级改进

### 4.1 D1: 引入结构化错误类型

**涉及文件**:
- `src-tauri/Cargo.toml` — 添加 `thiserror` 依赖
- `src-tauri/src/domain/mod.rs` — 定义 `AppError` 枚举
- `src-tauri/src/datasource/mod.rs` — DataSource trait 签名变更
- `src-tauri/src/datasource/sina.rs` — 适配器 Error 适配
- `src-tauri/src/datasource/tencent.rs` — 适配器 Error 适配
- `src-tauri/src/datasource/search.rs` — 搜索 Error 适配
- `src-tauri/src/commands/*.rs` — 命令层 Error 转换

**设计目标**:  
不改变前端接口（Tauri 序列化 `String` 不变），仅在 Rust 内部使用结构化错误，在 IPC 边界转换为 String。

**技术方案**:

#### 4.1.1 添加依赖

```toml
# src-tauri/Cargo.toml
thiserror = "2"
```

> `thiserror 2.x` 支持 edition 2021，MSRV 1.73+，项目完全兼容。

#### 4.1.2 定义错误类型

在 `src-tauri/src/domain/mod.rs` 末尾新增：

```rust
/// 统一应用错误类型
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("数据库错误: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("网络请求失败 ({source}): {message}")]
    Network {
        source: String,
        message: String,
    },

    #[error("数据源不可用: {0}")]
    DataSourceUnavailable(String),

    #[error("不支持的操作: {0}")]
    Unsupported(String),

    #[error("未找到: {0}")]
    NotFound(String),

    #[error("数据解析失败 ({source}): {message}")]
    Parse { source: String, message: String },
}

impl AppError {
    /// 创建一个网络错误
    pub fn network(source: &str, message: impl Into<String>) -> Self {
        Self::Network {
            source: source.to_string(),
            message: message.into(),
        }
    }

    /// 创建一个解析错误
    pub fn parse(source: &str, message: impl Into<String>) -> Self {
        Self::Parse {
            source: source.to_string(),
            message: message.into(),
        }
    }
}
```

#### 4.1.3 更新 DataSource trait

将 trait 签名从 `Result<T, String>` 更新为 `Result<T, AppError>`：

```rust
// datasource/mod.rs
use crate::domain::AppError;

#[async_trait]
pub trait DataSource: Send + Sync {
    fn name(&self) -> &str;
    fn display_name(&self) -> &str;

    async fn fetch_realtime(
        &self, codes: &[String], market: &str,
    ) -> Result<Vec<Quote>, AppError>;  // String → AppError

    async fn fetch_indices(&self) -> Result<Vec<IndexQuote>, AppError>;

    async fn search(
        &self, keyword: &str, market: &str,
    ) -> Result<Vec<StockBrief>, AppError>;

    // ... 其余方法同理
}
```

#### 4.1.4 更新适配器（以 Sina 为例）

```rust
// sina.rs — 错误创建替换
// 旧: Err(format!("Sina request failed: {:#}", e))
// 新: Err(AppError::network("sina", format!("请求失败: {:#}", e)))

// 旧: Err(format!("Sina minute parse failed: {} — body: {}", e, ...))
// 新: Err(AppError::parse("sina", format!("分钟数据解析失败: {}", e)))
```

#### 4.1.5 IPC 边界转换

Tauri 的 `#[tauri::command]` 要求错误实现 `impl Into<tauri::InvokeError>` 或 `serde::Serialize`。`String` 自动满足此要求。`AppError` 默认不满足。

**策略**: 在 command 函数中使用 `.map_err(|e| e.to_string())` 进行边界转换，保持前端接口不变。

```rust
// commands/watchlist.rs
#[tauri::command]
pub fn add_watch(...) -> Result<(), String> {
    db.add_watch(...).map_err(|e| e.to_string()) // rusqlite::Error → String (不变)
}
```

> 由于我们使 `AppError` 实现 `From<rusqlite::Error>`，当 database 方法返回 `SqliteResult<T>` 时，在 command 层通过 `.map_err(|e| e.to_string())` 转换。当 datasource 方法返回 `Result<T, AppError>` 时，同样通过 `.map_err(|e| e.to_string())` 转换（利用了 `thiserror` 生成的 `Display` 实现）。

**分步实施策略** (降低风险):

| 步骤 | 内容 | 风险 |
|------|------|------|
| Step 1 | 添加 `thiserror`，创建 `AppError` 类型，添加 `From<rusqlite::Error>` | 零风险（新增代码未使用） |
| Step 2 | 更新 `DataSource` trait 签名 → 更新 Sina/Tencent 适配器 | 中风险（编译失败） |
| Step 3 | 更新 `search.rs` | 低风险 |
| Step 4 | 更新 `cache/mod.rs`（少量引用） | 低风险 |
| Step 5 | 更新 `commands/` 层 | 低风险 |

**改动文件**: 约 10 个文件

**不需要改动**: 所有前端代码（IPC 接口不变）

---

### 4.2 B6: 统一数据归一化逻辑

**文件**:
- [src-tauri/src/datasource/mod.rs](src-tauri/src/datasource/mod.rs) — 新增归一化常量/函数
- [src-tauri/src/datasource/sina.rs](src-tauri/src/datasource/sina.rs)
- [src-tauri/src/datasource/tencent.rs](src-tauri/src/datasource/tencent.rs)

**问题描述**:  
成交量 `×100`（手→股）和成交额 `×10000`（万元→元）的归一化逻辑在 SinaAdapter 和 TencentAdapter 中重复出现 8+ 处：

| 位置 | 归一化 | 出现次数 |
|------|--------|----------|
| sina.rs parse_sina_index (行 130-131) | volume×100, turnover×10000 | 1 |
| sina.rs fetch_depth (行 438, 449) | volume×100 (Level) | 2 |
| tencent.rs parse_quote_line (行 68, 81) | volume×100, turnover×10000 | 2 |
| tencent.rs parse_index_line (行 123-124) | volume×100, turnover×10000 | 2 |
| tencent.rs fetch_minute_data (行 281) | volume×100 | 1 |
| tencent.rs fetch_kline (行 358) | volume×100 | 1 |
| tencent.rs fetch_depth (行 417, 430) | volume×100 (Level) | 2 |

**技术方案**:

#### 4.2.1 在 datasource/mod.rs 中定义归一化常量/函数

```rust
// datasource/mod.rs — 在已有常量下方新增

/// 成交量归一化: 手 → 股 (×100)
pub const VOLUME_HANDS_TO_SHARES: u64 = 100;

/// 成交额归一化: 万元 → 元 (×10000)
pub const TURNOVER_WAN_TO_YUAN: f64 = 10000.0;

/// 将成交量从手转换为股
#[inline]
pub fn normalize_volume(volume_hands: u64) -> u64 {
    volume_hands * VOLUME_HANDS_TO_SHARES
}

/// 将成交额从万元转换为元
#[inline]
pub fn normalize_turnover(turnover_wan: f64) -> f64 {
    turnover_wan * TURNOVER_WAN_TO_YUAN
}
```

#### 4.2.2 替换适配器中的内联归一化

```rust
// sina.rs — parse_sina_index
// 修改前:
let volume_shares = volume * 100;
let turnover_yuan = turnover * 10000.0;
// 修改后:
let volume_shares = super::normalize_volume(volume);
let turnover_yuan = super::normalize_turnover(turnover);

// tencent.rs — parse_quote_line
// 修改前:
let volume_shares = volume * 100;
// 修改后:
let volume_shares = super::normalize_volume(volume);

// tencent.rs — fetch_depth (bids/asks)
// 修改前:
bids.push(Level { price, volume: vol * 100 });
// 修改后:
bids.push(Level { price, volume: super::normalize_volume(vol) });
```

**改动文件**: 3 个文件

**验证方法**: 编译通过后，功能行为完全等价（纯重构）。

---

### 4.3 D2: 共享 reqwest::Client 单例

**文件**:
- [src-tauri/src/datasource/mod.rs](src-tauri/src/datasource/mod.rs) — 新增共享 Client
- [src-tauri/src/datasource/sina.rs](src-tauri/src/datasource/sina.rs)
- [src-tauri/src/datasource/tencent.rs](src-tauri/src/datasource/tencent.rs)

**问题描述**:  
当前系统有 3 个独立的 `reqwest::Client` 实例（SinaAdapter、TencentAdapter、search::CLIENT），每个都有自己的连接池。合并为单一共享 Client 可减少资源占用和连接数。

**技术方案**:

#### 4.3.1 在 datasource/mod.rs 中定义共享 Client

```rust
// datasource/mod.rs
use std::sync::OnceLock;
use reqwest::Client;
use std::time::Duration;

/// 全局共享 HTTP 客户端
static SHARED_CLIENT: OnceLock<Client> = OnceLock::new();

/// 获取或初始化共享的 reqwest::Client
pub fn shared_client() -> &'static Client {
    SHARED_CLIENT.get_or_init(|| {
        Client::builder()
            .timeout(Duration::from_secs(10))
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
            .pool_max_idle_per_host(10)
            .build()
            .expect("Failed to build shared reqwest Client")
    })
}
```

#### 4.3.2 更新适配器使用共享 Client

```rust
// sina.rs
impl SinaAdapter {
    pub fn new() -> Self {
        Self {
            client: super::shared_client().clone(), // clone 是浅拷贝，共享连接池
        }
    }
}

// tencent.rs — 同理
```

> **关键点**: `reqwest::Client::clone()` 是浅拷贝——底层连接池、超时等配置共享同一个实例。每个 adapter 仍然持有自己的 `Client` 句柄，但共享底层资源。

#### 4.3.3 更新 search.rs 使用同一个共享 Client

```rust
// search.rs
// 删除原有的 static CLIENT: OnceLock<Client>
// 使用 super::shared_client()
```

**改动文件**: 4 个文件  
**风险**: 低（`Client::clone()` 是 reqwest 的标准用法）

**验证方法**:
- `cargo build` 编译通过
- 应用运行正常，数据获取不受影响

---

### 4.4 F1: 全局错误边界

**文件**: [src/App.vue](src/App.vue)

**问题描述**:  
当前仅有 `onMounted` 的 try/catch 处理初始化错误。子组件的未捕获错误（如渲染错误、事件处理异常）会导致静默崩溃——用户看到空白界面，无任何提示。

**技术方案**:

在 `App.vue` 的 `<script setup>` 中添加 `onErrorCaptured` 钩子：

```typescript
// App.vue — 在现有 import 中添加 onErrorCaptured
import { onMounted, onUnmounted, ref, computed, onErrorCaptured } from 'vue';

// 在现有代码之后添加:
const appError = ref<string | null>(null);

onErrorCaptured((err, instance, info) => {
  const componentName = instance?.$.type?.__name || instance?.$.type?.name || 'Unknown';
  const msg = `[${componentName}] ${String(err).slice(0, 200)}`;
  console.error('[App] onErrorCaptured:', msg, info);
  
  // 仅展示第一个错误，避免错误风暴
  if (!appError.value) {
    appError.value = `界面错误: ${msg}`;
  }
  
  // 返回 false 阻止错误继续向上传播到浏览器控制台
  return false;
});
```

相应地，将 `appError` 传递给 `AppLayout`：

```html
<AppLayout
  :init-error="initError"
  :init-ready="initReady"
  :quote-error="quote.error"
  :app-error="appError"
  @retry="handleRetry"
/>
```

在 `AppLayout.vue` 中新增一个错误 banner（类似已有的 `initError` banner 但不可关闭）：

```html
<!-- AppLayout.vue template 中，在 initError banner 后新增 -->
<div v-if="appError" class="layout-error-banner layout-error-banner--critical">
  <span>{{ appError }}</span>
  <button class="error-dismiss" @click="appError = null">✕</button>
</div>
```

**改动文件**: 2 个文件  
**风险**: 低

---

### 4.5 F10/PERF2: 图表实例复用

**文件**:
- [src/composables/useChart.ts](src/composables/useChart.ts)
- [src/components/detail/MinuteChart.vue](src/components/detail/MinuteChart.vue)
- [src/components/detail/KLineChart.vue](src/components/detail/KLineChart.vue)

**问题描述**:  
当用户在自选股表格中切换不同股票时，`MinuteChart` 和 `KLineChart` 组件会因 `code` prop 变化而重新执行 `onMounted` → `initChart`。虽然 `initChart` 中有 `if (!chart)` 保护，但 Vue 在 `key` 不变时复用组件实例。问题出在当用户切换周期（分时→日K→周K）时，图表样式需要从 area 切换到 candle_solid，当前通过重新初始化处理。

实际分析后，主要问题是：
1. 切换股票时图表数据通过 `watch` 正确更新，图表实例本身被复用
2. 切换周期时 `period` prop 变化触发 `loadData` 重新获取数据，图表实例也是复用的

经过重新审查，`useChart.ts` 的 `initChart` 已经有 `if (!chart)` 保护，图表实例实际上是复用的。主要改进点是 **dataLoader 的 'more' 分支**（加载更多历史数据），当前返回空数组。

**修订后的技术方案**:

#### 4.5.1 实现 dataLoader 的 'more' 支持

当用户缩放/滚动图表查看历史数据时，`dataLoader.getBars` 会以 `type: 'more'` 调用。当前实现返回空数组，导致无法加载更多历史数据。

```typescript
// useChart.ts — 修改 dataLoader
const dataLoader: DataLoader = {
  getBars: (params) => {
    if (params.type === 'init') {
      params.callback(klineData.value, false);
    } else if (params.type === 'more') {
      // 当前一次性加载全部数据，无需额外加载
      // 返回空数组 + true 表示"没有更多数据"
      params.callback([], true);
    }
  },
};
```

#### 4.5.2 避免不必要的图表 re-init

`initChart` 在 chart 已存在时仍调用了 `setPeriod` 和 `applyChartStyles`——这是必要的（周期/样式可能变了），但可以避免重复调用 `setSymbol`：

```typescript
// useChart.ts — initChart 优化
async function initChart(period: PeriodType) {
  if (!options.chartRef.value) return;
  
  const isNew = !chart;
  if (isNew) {
    chart = init(options.chartRef.value, {
      locale: 'zh-CN',
      layout: { basicParams: { yAxisInside: true } },
    });
    if (!chart) {
      error.value = '图表初始化失败';
      return;
    }
    // 仅新图表需要设置 VOL indicator override
    chart.overrideIndicator({ /* ... */ });
  }
  
  chart.setSymbol({ ticker: unref(options.code), name: unref(options.name) || unref(options.code) });
  
  currentPeriod.value = period;
  chart.setPeriod(periodToKlinecharts(period) as any);
  applyChartStyles();
  if (period !== 'minute') {
    applyCandlestickStyles();
  }
}
```

**改动文件**: 1 个文件（`useChart.ts`）  
**风险**: 中（klinecharts v10 beta API 可能有未预期的行为）

---

## 5. 实施顺序与时间估算

```
Phase 1 (P0 — 0.5h)
  └─ F4: Ticker 拖拽空 catch 修复
      └─ 独立、无依赖、零风险

Phase 2 (P1 — 5h)
  ├─ B11: move_watch 原子化 (2h)
  │   ├─ 1. db/mod.rs 新增 3 个方法
  │   └─ 2. commands/watchlist.rs 简化
  └─ F3: Ticker 事件驱动 (3h)
      ├─ 1. 后端 6 个 command 添加 AppHandle + emit
      └─ 2. TickerBar 替换轮询为事件监听

Phase 3 (P2 — 9.5h)
  ├─ B6: 统一归一化 (1.5h) — 独立纯重构
  ├─ D2: 共享 Client (1h) — 独立重构
  ├─ F1: 错误边界 (1h) — 独立前端改动
  ├─ F10: 图表复用 (2h) — 独立前端改动
  └─ D1: 结构化错误 (4h) — ⚠️ 影响面大，最后执行
      ├─ Step 1: AppError 类型定义
      ├─ Step 2: DataSource trait + 适配器
      ├─ Step 3: search.rs
      ├─ Step 4: cache/mod.rs
      └─ Step 5: commands 层

预计总工时: ~15h
```

**建议执行策略**:
- Phase 1 立即执行，零风险
- Phase 2 在下一个开发迭代中执行
- Phase 3 逐步执行，D1（结构化错误）作为最后一个，因为它影响面最大且需要回归测试

---

## 附录: 各修复项依赖关系

```
F4 ───────────────────────────────────────── (无依赖)
B11 ──────────────────────────────────────── (无依赖)
F3 ───────────────────────────────────────── (无依赖，可与 B11 并行)
B6 ───────────────────────────────────────── (无依赖)
D2 ───────────────────────────────────────── (无依赖，可与 B6 并行)
F1 ───────────────────────────────────────── (无依赖)
F10 ──────────────────────────────────────── (无依赖)
D1 ──── 依赖 B6(建议先完成归一化重构) ────── (有依赖)
```

---

*计划生成于 2026-06-20。基于 [CODE_REVIEW_REPORT.md](CODE_REVIEW_REPORT.md) 第 8.2 和 8.3 节。*
