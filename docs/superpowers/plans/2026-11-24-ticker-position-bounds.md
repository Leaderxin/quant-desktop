# 行情条位置边界管理 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 让行情条在三端都不能被拖出显示器可见范围，Windows 下不进入任务栏区域，并支持在扩展屏之间自由移动与位置恢复。

**Architecture:** 新增 `window_bounds` 模块，把"工作区并集覆盖"的几何判定做成纯函数；显示器工作区来自 `Monitor::work_area()`（Windows 的 `WM_MOVING` 路径改为直接调用 Win32 `EnumDisplayMonitors`）。恢复/显示/移动落库统一走该模块；Windows 在原生拖拽循环里实时修正候选 `RECT`，macOS/Linux 用一条去抖 worker 在移动停止 200ms 后钳制。

**Tech Stack:** Tauri 2.11 / tao 0.35 / Rust 2021、手写 Win32 FFI（user32 + comctl32）、cargo test。

**Spec:** `docs/superpowers/specs/2026-11-24-ticker-position-bounds-design.md`

## Global Constraints

- 不引入新依赖（不加 `windows` / `tempfile` crate；Windows FFI 沿用 `lib.rs` 里 `windows_util` 的手写 `extern "system"` 风格）。
- 边界一律按**显示器工作区**（`Monitor::work_area()`）计算；禁止回退到显示器矩形或 `primary_monitor()` 的尺寸假设。
- 位置仍以物理像素存 `ticker_x` / `ticker_y`，键名不变，不做数据库迁移。
- 前端零改动；`useTickerWindowHeight` 的"下边缘锚定"逻辑保持原样。
- Rust 注释与日志用中文，风格与现有代码一致。
- 每个任务一个提交；提交信息不出现任何工具署名。
- 每个 Rust 任务提交前的校验：`cargo test --manifest-path src-tauri/Cargo.toml` 与 `cargo check --manifest-path src-tauri/Cargo.toml` 均须通过。

## Review Focus

- 行情条被拖到**负坐标**（左侧/上方）扩展屏后重启：必须恢复到那块屏，而不是被压成 `0` 或弹回主屏。→ Task 1 `left_secondary_monitor_with_negative_coords_is_kept`；Task 2 恢复路径。
- 两块显示器**高度不同/垂直错位**时骑在交界上：整条必须仍完整可见，不能有像素落在两块屏之间的空洞。→ Task 1 `straddling_offset_monitors_uses_the_smallest_correction`。
- 任务栏在**上/左/右/下任意一侧**：行情条必须停在任务栏之外。→ Task 1 `taskbar_offsets_shrink_the_work_area`。
- 行情条贴在工作区底边时**高度增长**（开关迷你分时图、改每屏条数）：长高后不能钻进任务栏。→ Task 1 底边钳制测试 + Task 3 `Resized` 接线。
- **拔掉扩展屏**后重新显示行情条：必须就近落在仍存在的工作区内，不能整条消失。→ Task 1 剩余显示器布局测试 + Task 2 恢复路径。

---

## File Structure

| 文件 | 责任 | 改动 |
|---|---|---|
| `src-tauri/src/window_bounds.rs` | 工作区模型、纯几何钳制、显示器采集、恢复单点、去抖 worker、Windows `WM_MOVING` 子类 | 新建 |
| `src-tauri/src/lib.rs` | 声明模块；`set_ticker_visible` 与 setup 改调恢复单点；移除旧的 primary-monitor 钳制/落库；注册去抖 worker 与 Windows 子类 | 修改 |
| `CLAUDE.md` | 记录边界规则与单点入口 | 修改 |
| 前端 | 无 | — |

---

### Task 1: 纯几何模块与单元测试

**Files:**
- Create: `src-tauri/src/window_bounds.rs`
- Modify: `src-tauri/src/lib.rs:1-12`（模块声明）

**Interfaces:**
- Consumes: 无
- Produces:
  - `pub(crate) struct WorkArea { pub x: i32, pub y: i32, pub w: u32, pub h: u32 }`
  - `pub(crate) fn clamp_rect(pos: (i32, i32), size: (u32, u32), areas: &[WorkArea]) -> Option<(i32, i32)>`
  - `fn bottom_right_in(area: &WorkArea, size: (u32, u32), margin: i32) -> (i32, i32)`

- [ ] **Step 1: 写失败测试与可编译的桩**

新建 `src-tauri/src/window_bounds.rs`，先写数据结构和桩，再加完整测试：

```rust
// window_bounds.rs — 行情条位置边界管理。
//
// 边界是「显示器工作区」而不是显示器矩形：Windows 任务栏、macOS 菜单栏/Dock、
// Linux 面板都会让 `work_area` 小于 `size`，按工作区钳制就同时解决了遮挡问题。
// 多显示器之间允许窗口骑在交界上，只要矩形被**所有工作区的并集**完整覆盖 ——
// 这是「能顺利跨屏」与「整条永远可见」之间的平衡。
//
// 本模块只管几何与调用时机；平台差异只在「从哪拿工作区」和「什么时候调用」。

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct WorkArea {
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
}

pub(crate) fn clamp_rect(
    _pos: (i32, i32),
    _size: (u32, u32),
    _areas: &[WorkArea],
) -> Option<(i32, i32)> {
    None // 桩：先让测试编译，再实现
}

fn bottom_right_in(_area: &WorkArea, _size: (u32, u32), _margin: i32) -> (i32, i32) {
    (0, 0) // 桩
}

#[cfg(test)]
mod tests {
    use super::*;

    fn area(x: i32, y: i32, w: u32, h: u32) -> WorkArea {
        WorkArea { x, y, w, h }
    }

    const SIZE: (u32, u32) = (220, 40);

    #[test]
    fn fully_visible_position_is_unchanged() {
        let areas = [area(0, 0, 1920, 1040)];
        assert_eq!(clamp_rect((100, 100), SIZE, &areas), Some((100, 100)));
    }

    #[test]
    fn clamps_at_each_edge_of_a_single_work_area() {
        let areas = [area(0, 0, 1920, 1040)];
        assert_eq!(clamp_rect((-50, 100), SIZE, &areas), Some((0, 100)));
        assert_eq!(clamp_rect((1800, 100), SIZE, &areas), Some((1700, 100)));
        assert_eq!(clamp_rect((100, -50), SIZE, &areas), Some((100, 0)));
        assert_eq!(clamp_rect((100, 1030), SIZE, &areas), Some((100, 1000)));
    }

    #[test]
    fn taskbar_offsets_shrink_the_work_area() {
        // 任务栏在底部
        let areas = [area(0, 0, 1920, 1000)];
        assert_eq!(clamp_rect((100, 1010), SIZE, &areas), Some((100, 960)));

        // 任务栏在左侧
        let areas = [area(60, 0, 1860, 1040)];
        assert_eq!(clamp_rect((10, 100), SIZE, &areas), Some((60, 100)));

        // 任务栏在顶部
        let areas = [area(0, 40, 1920, 1000)];
        assert_eq!(clamp_rect((100, 10), SIZE, &areas), Some((100, 40)));
    }

    #[test]
    fn left_secondary_monitor_with_negative_coords_is_kept() {
        let areas = [area(0, 0, 1920, 1040), area(-1920, 0, 1920, 1040)];
        assert_eq!(clamp_rect((-1900, 100), SIZE, &areas), Some((-1900, 100)));
        assert_eq!(clamp_rect((-2000, 100), SIZE, &areas), Some((-1920, 100)));
    }

    #[test]
    fn right_secondary_monitor_is_kept() {
        let areas = [area(0, 0, 1920, 1040), area(1920, 0, 1920, 1040)];
        assert_eq!(clamp_rect((2200, 100), SIZE, &areas), Some((2200, 100)));
        assert_eq!(clamp_rect((3700, 100), SIZE, &areas), Some((3620, 100)));
    }

    #[test]
    fn straddling_adjacent_monitors_is_allowed() {
        let areas = [area(0, 0, 1920, 1040), area(1920, 0, 1920, 1040)];
        // 1800..2020 跨在交界上，但每个像素都在某块工作区内
        assert_eq!(clamp_rect((1800, 100), SIZE, &areas), Some((1800, 100)));
    }

    #[test]
    fn straddling_offset_monitors_uses_the_smallest_correction() {
        // B 比 A 低 100px：交界处 x>=1920 且 y<100 的区域是空洞。
        // 往下挪 50px 比往回挪 100px 更近，所以应选 (1800, 100)。
        let areas = [area(0, 0, 1920, 1040), area(1920, 100, 1920, 940)];
        assert_eq!(clamp_rect((1800, 50), SIZE, &areas), Some((1800, 100)));
    }

    #[test]
    fn gap_between_monitors_is_not_visible_area() {
        let areas = [area(0, 0, 1000, 1000), area(1200, 0, 1000, 1000)];
        // 900..1120 有 120px 落在 1000..1200 的空洞里
        assert_eq!(clamp_rect((900, 100), SIZE, &areas), Some((780, 100)));
    }

    #[test]
    fn no_monitors_returns_none() {
        assert_eq!(clamp_rect((0, 0), SIZE, &[]), None);
    }

    #[test]
    fn oversized_window_falls_back_to_area_origin() {
        let areas = [area(0, 0, 100, 100)];
        assert_eq!(clamp_rect((500, 500), (220, 40), &areas), Some((0, 0)));
    }

    #[test]
    fn bottom_right_default_stays_inside_with_margin() {
        let a = area(0, 0, 1920, 1040);
        assert_eq!(bottom_right_in(&a, SIZE, 10), (1690, 990));
    }
}
```

在 `src-tauri/src/lib.rs` 顶部（`pub mod logging;` 之后）加：

```rust
mod window_bounds;
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cargo test --manifest-path src-tauri/Cargo.toml window_bounds`
Expected: FAIL —— 桩返回 `None` / `(0, 0)`，各断言不成立。

- [ ] **Step 3: 实现几何判定**

用下面内容替换 `window_bounds.rs` 里的两个桩（保留文件头注释、`WorkArea` 与测试模块）：

```rust
#[derive(Debug, Clone, Copy)]
struct Rect {
    x: i64,
    y: i64,
    w: i64,
    h: i64,
}

fn area_rect(a: &WorkArea) -> Rect {
    Rect {
        x: a.x as i64,
        y: a.y as i64,
        w: a.w as i64,
        h: a.h as i64,
    }
}

/// 把窗口矩形钳回「所有工作区的并集」内，返回位移最小的左上角。
///
/// 已完整可见时原样返回（包括骑在两块相邻显示器交界上的情况）；
/// 没有任何可行解（没有显示器，或窗口比所有工作区都大）时：
/// - 没有显示器 → `None`；
/// - 窗口过大 → 返回重叠面积最大者的左上角，至少锚在可见区域。
pub(crate) fn clamp_rect(
    pos: (i32, i32),
    size: (u32, u32),
    areas: &[WorkArea],
) -> Option<(i32, i32)> {
    if areas.is_empty() {
        return None;
    }
    let rect = Rect {
        x: pos.0 as i64,
        y: pos.1 as i64,
        w: size.0 as i64,
        h: size.1 as i64,
    };
    if rect.w <= 0 || rect.h <= 0 {
        return Some(pos);
    }
    if covered(rect, areas) {
        return Some(pos);
    }

    // 最小修正一定发生在某条显示器边界上：把候选位移限制为「0 或与某条边对齐」。
    let mut xs = vec![rect.x];
    let mut ys = vec![rect.y];
    for a in areas {
        let r = area_rect(a);
        xs.push(r.x);
        xs.push(r.x + r.w - rect.w);
        ys.push(r.y);
        ys.push(r.y + r.h - rect.h);
    }
    xs.sort_unstable();
    xs.dedup();
    ys.sort_unstable();
    ys.dedup();

    let mut best: Option<((i32, i32), i64)> = None;
    for &x in &xs {
        for &y in &ys {
            let candidate = Rect { x, y, ..rect };
            if !covered(candidate, areas) {
                continue;
            }
            let dx = x - rect.x;
            let dy = y - rect.y;
            let cost = dx * dx + dy * dy;
            if best.map_or(true, |(_, c)| cost < c) {
                best = Some(((x as i32, y as i32), cost));
            }
        }
    }
    if let Some((p, _)) = best {
        return Some(p);
    }

    let mut best_overlap = 0i64;
    let mut fallback = (areas[0].x, areas[0].y);
    for a in areas {
        let overlap = overlap_area(rect, a);
        if overlap > best_overlap {
            best_overlap = overlap;
            fallback = (a.x, a.y);
        }
    }
    Some(fallback)
}

/// 精确判断矩形是否被工作区并集完整覆盖。
///
/// 按所有 x 边界切成条带，每个条带内任取一点检查 y 区间的并集是否覆盖
/// `[top, bottom]` —— 条带内每块工作区要么整段覆盖、要么完全不覆盖，所以
/// 取条带左端点就足够，不需要采样。
fn covered(rect: Rect, areas: &[WorkArea]) -> bool {
    let left = rect.x;
    let right = rect.x + rect.w;
    let top = rect.y;
    let bottom = rect.y + rect.h;

    let mut xs = vec![left, right];
    for a in areas {
        let r = area_rect(a);
        if r.x + r.w > left && r.x < right {
            xs.push(r.x.clamp(left, right));
            xs.push((r.x + r.w).clamp(left, right));
        }
    }
    xs.sort_unstable();
    xs.dedup();

    for pair in xs.windows(2) {
        let strip_left = pair[0];
        if pair[1] <= strip_left {
            continue;
        }
        let mut spans: Vec<(i64, i64)> = areas
            .iter()
            .map(area_rect)
            .filter(|r| r.x <= strip_left && strip_left < r.x + r.w)
            .map(|r| (r.y, r.y + r.h))
            .collect();
        if spans.is_empty() {
            return false;
        }
        spans.sort_unstable();
        if spans[0].0 > top {
            return false;
        }
        let mut reach = spans[0].1;
        for &(span_top, span_bottom) in &spans[1..] {
            if span_top > reach {
                return false;
            }
            reach = reach.max(span_bottom);
            if reach >= bottom {
                break;
            }
        }
        if reach < bottom {
            return false;
        }
    }
    true
}

fn overlap_area(rect: Rect, a: &WorkArea) -> i64 {
    let r = area_rect(a);
    let left = rect.x.max(r.x);
    let right = (rect.x + rect.w).min(r.x + r.w);
    let top = rect.y.max(r.y);
    let bottom = (rect.y + rect.h).min(r.y + r.h);
    if right <= left || bottom <= top {
        0
    } else {
        (right - left) * (bottom - top)
    }
}

/// 工作区右下角、向内留 `margin` 的默认位置（首启与存档全失效时用）。
fn bottom_right_in(area: &WorkArea, size: (u32, u32), margin: i32) -> (i32, i32) {
    let max_x = area.x as i64 + area.w as i64 - size.0 as i64;
    let max_y = area.y as i64 + area.h as i64 - size.1 as i64;
    (
        (max_x - margin as i64).max(area.x as i64) as i32,
        (max_y - margin as i64).max(area.y as i64) as i32,
    )
}
```

- [ ] **Step 4: 运行测试确认通过**

Run: `cargo test --manifest-path src-tauri/Cargo.toml window_bounds`
Expected: PASS，11 个测试全绿。

- [ ] **Step 5: 提交**

```bash
git add src-tauri/src/window_bounds.rs src-tauri/src/lib.rs
git commit -m "feat(ticker): 工作区并集的位置钳制纯几何实现"
```

---

### Task 2: 工作区采集、恢复单点与移动落库

**Files:**
- Modify: `src-tauri/src/window_bounds.rs`
- Modify: `src-tauri/src/lib.rs`（`set_ticker_visible` 约 361-388、setup 约 928-1013）

**Interfaces:**
- Consumes: Task 1 的 `WorkArea` / `clamp_rect` / `bottom_right_in`
- Produces:
  - `pub(crate) fn restore_ticker_position(window: &tauri::WebviewWindow, db: &Database, logical_height: u32)`
  - `pub(crate) fn clamp_and_save(window: &tauri::WebviewWindow, db: &Database) -> Option<(i32, i32)>`
  - 内部：`fn work_areas(window: &tauri::WebviewWindow) -> Vec<WorkArea>`、`fn work_area_of(&tauri::Monitor) -> WorkArea`、`fn read_saved_position(&Database) -> Option<(i32, i32)>`、`fn save_position(&Database, (i32, i32))`

- [ ] **Step 1: 写失败的存取测试**

在 `window_bounds.rs` 的 `mod tests` 顶部加导入与临时库工具，并加测试：

```rust
    use crate::db::{keys, Database};
    use std::sync::atomic::{AtomicU64, Ordering};

    static SEQ: AtomicU64 = AtomicU64::new(0);

    fn temp_db(tag: &str) -> (std::path::PathBuf, Database) {
        let seq = SEQ.fetch_add(1, Ordering::SeqCst);
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "qd-bounds-{tag}-{}-{nanos}-{seq}",
            std::process::id()
        ));
        let db = Database::open(dir.clone()).unwrap();
        (dir, db)
    }

    #[test]
    fn saved_position_round_trips_and_bad_values_are_ignored() {
        let (dir, db) = temp_db("position");

        assert_eq!(read_saved_position(&db), None, "没有存档时应返回 None");

        save_position(&db, (-1900, 120));
        assert_eq!(
            read_saved_position(&db),
            Some((-1900, 120)),
            "负坐标扩展屏的位置必须原样读回"
        );

        db.set_setting(keys::TICKER_X, "oops").unwrap();
        assert_eq!(read_saved_position(&db), None, "坏值视为没有存档");

        drop(db);
        std::fs::remove_dir_all(&dir).ok();
    }
```

同时在模块顶部加 `use crate::db::{keys, Database};`（非测试构建也要用）。

此步骤完成后 `read_saved_position` / `save_position` 尚不存在，测试编译失败即可进入下一步。

- [ ] **Step 2: 运行测试确认失败**

Run: `cargo test --manifest-path src-tauri/Cargo.toml window_bounds`
Expected: FAIL —— `read_saved_position` / `save_position` 未定义（编译错误）。

- [ ] **Step 3: 实现工作区采集、存取与恢复单点**

在 `window_bounds.rs` 的 `WorkArea` 之后追加：

```rust
use crate::db::{keys, Database};

/// 工作区右下角的默认边距（物理像素）。
const DEFAULT_MARGIN: i32 = 10;

fn work_area_of(monitor: &tauri::Monitor) -> WorkArea {
    let wa = monitor.work_area();
    WorkArea {
        x: wa.position.x,
        y: wa.position.y,
        w: wa.size.width,
        h: wa.size.height,
    }
}

/// 当前所有显示器的工作区。取不到时返回空表，调用方必须把空表当成「无法判定」。
fn work_areas(window: &tauri::WebviewWindow) -> Vec<WorkArea> {
    match window.available_monitors() {
        Ok(monitors) => monitors.iter().map(work_area_of).collect(),
        Err(e) => {
            log::warn!("[bounds] available_monitors failed: {e}");
            Vec::new()
        }
    }
}

fn read_saved_position(db: &Database) -> Option<(i32, i32)> {
    let x = db.get_setting(keys::TICKER_X).ok().flatten()?;
    let y = db.get_setting(keys::TICKER_Y).ok().flatten()?;
    Some((x.parse().ok()?, y.parse().ok()?))
}

fn save_position(db: &Database, pos: (i32, i32)) {
    if let Err(e) = db.set_setting(keys::TICKER_X, &pos.0.to_string()) {
        log::warn!("[bounds] 保存 ticker_x 失败: {e}");
    }
    if let Err(e) = db.set_setting(keys::TICKER_Y, &pos.1.to_string()) {
        log::warn!("[bounds] 保存 ticker_y 失败: {e}");
    }
}

/// 启动 / 显示行情条时的位置恢复（唯一入口）。
///
/// 存档越界（显示器被拔掉、分辨率变化、负坐标布局变化）时钳回工作区并集；
/// 完全不可用时退回主显示器工作区右下角。恢复后会立刻写回库，避免下次显示
/// 继续拿一个非法存档来校验。
pub(crate) fn restore_ticker_position(
    window: &tauri::WebviewWindow,
    db: &Database,
    logical_height: u32,
) {
    let areas = work_areas(window);
    if areas.is_empty() {
        log::warn!("[bounds] 读不到显示器信息，保持行情条当前位置");
        return;
    }

    // set_size 是投递到事件循环的请求，紧接着读 outer_size() 可能还是旧值，
    // 所以按逻辑尺寸 × 缩放换算目标物理尺寸（与 set_ticker_size 的口径一致）。
    let scale = window.scale_factor().unwrap_or(1.0);
    let size = (
        (crate::datasource::TICKER_WIDTH as f64 * scale).round().max(1.0) as u32,
        (logical_height as f64 * scale).round().max(1.0) as u32,
    );

    let saved = read_saved_position(db);
    let target = match saved.and_then(|p| clamp_rect(p, size, &areas)) {
        Some(p) => p,
        None => {
            let primary = window
                .primary_monitor()
                .ok()
                .flatten()
                .map(|m| work_area_of(&m))
                .or_else(|| areas.first().copied())
                .unwrap_or(areas[0]);
            bottom_right_in(&primary, size, DEFAULT_MARGIN)
        }
    };

    if saved != Some(target) {
        log::info!("[bounds] 行情条位置 {:?} -> {:?}", saved, target);
    }
    if let Err(e) = window.set_position(tauri::PhysicalPosition::new(target.0, target.1)) {
        log::warn!("[bounds] 设置行情条位置失败: {e}");
        return;
    }
    save_position(db, target);
}

/// 把当前窗口钳回工作区并落库；返回落库后的位置。幂等。
///
/// 保存前先读库比较，避免「钳制触发 Moved → 再钳制」这轮重复写。
pub(crate) fn clamp_and_save(
    window: &tauri::WebviewWindow,
    db: &Database,
) -> Option<(i32, i32)> {
    let pos = window.outer_position().ok()?;
    let size = window.outer_size().ok()?;
    let areas = work_areas(window);
    if areas.is_empty() {
        return None;
    }

    let current = (pos.x, pos.y);
    let target = clamp_rect(current, (size.width, size.height), &areas)?;
    if target != current {
        if let Err(e) = window.set_position(tauri::PhysicalPosition::new(target.0, target.1)) {
            log::warn!("[bounds] 钳制行情条位置失败: {e}");
            return None;
        }
        log::debug!("[bounds] 行情条位置 {:?} -> {:?}", current, target);
    }
    if read_saved_position(db) != Some(target) {
        save_position(db, target);
    }
    Some(target)
}
```

注意：模块顶部只保留一份 `use crate::db::{keys, Database};`（Step 1 若已加过不要重复）。

- [ ] **Step 4: 运行测试确认通过**

Run: `cargo test --manifest-path src-tauri/Cargo.toml window_bounds`
Expected: PASS（含新增的 `saved_position_round_trips_and_bad_values_are_ignored`）。

- [ ] **Step 5: 把 `set_ticker_visible` 的位置恢复改成单点调用**

在 `src-tauri/src/lib.rs` 的 `set_ticker_visible` 中，把"位置:优先恢复上次保存的坐标……"整段（从 `let mon = window.primary_monitor()` 到默认位置 `if !restored { ... }` 结束）替换为：

```rust
    // 位置恢复与边界校验只有一个入口：显示器工作区并集钳制。
    // 负坐标扩展屏、任务栏与拔屏后的回退都在 window_bounds 里处理。
    window_bounds::restore_ticker_position(&window, db, height);
```

- [ ] **Step 6: 把 setup 的位置恢复与移动落库改成新几何**

在 `src-tauri/src/lib.rs` 的 setup 中，把从 `// 钳制与「可见比例」都按事件发生时的真实尺寸算` 开始，到默认位置 `let _ = ticker.set_position(...)` 结束的整段（含 `ticker_physical_size`、`mon`、`scale`、`tw/th`、旧 `Moved` 处理器、旧恢复分支）替换为：

```rust
                // 位置恢复与边界校验只有一个入口（工作区并集钳制）。
                window_bounds::restore_ticker_position(
                    &ticker,
                    &db,
                    initial_ticker_height,
                );

                // 位置变化后按工作区并集钳制并落库。
                // Task 3 会把它换成去抖 worker；这里先保持「每次移动即处理」，
                // 让中间状态也能正确记住扩展屏位置。
                let ticker_for_move = ticker.clone();
                let db_for_move = db.clone();
                let _ = ticker.on_window_event(move |event| {
                    if matches!(event, tauri::WindowEvent::Moved(_)) {
                        window_bounds::clamp_and_save(&ticker_for_move, &db_for_move);
                    }
                });
```

- [ ] **Step 7: 编译并跑全量 Rust 测试**

Run:
```bash
cargo check --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml
```
Expected: 均通过；无未使用变量/函数警告（旧的 `ticker_physical_size` 已随整段删除）。

- [ ] **Step 8: 提交**

```bash
git add src-tauri/src/window_bounds.rs src-tauri/src/lib.rs
git commit -m "refactor(ticker): 位置恢复收敛到工作区边界单点"
```

---

### Task 3: 去抖 worker 与 `Resized` 兜底

**Files:**
- Modify: `src-tauri/src/window_bounds.rs`
- Modify: `src-tauri/src/lib.rs`（setup 中 Task 2 留下的 `on_window_event` 块）

**Interfaces:**
- Consumes: `clamp_and_save(&WebviewWindow, &Database) -> Option<(i32, i32)>`
- Produces: `pub(crate) fn spawn_settle_worker(window: tauri::WebviewWindow, db: std::sync::Arc<Database>)`

- [ ] **Step 1: 实现去抖 worker**

在 `window_bounds.rs` 顶部加导入：

```rust
use std::sync::Arc;
use std::time::{Duration, Instant};
```

在文件末尾（测试模块之前）追加：

```rust
/// 停止移动多久后才认为手势结束（毫秒）。
///
/// X11/macOS 拿不到可靠的「拖拽结束」信号，只能靠静默判定。阈值太短会在
/// 拖拽中途贴边，太长则松手后回弹有明显的滞后感；200ms 是两者的折中。
const SETTLE_DELAY: Duration = Duration::from_millis(200);

struct SettleState {
    last_change: Instant,
    dirty: bool,
}

/// 监听行情条的移动与改尺寸，静默后钳制并落库。
///
/// Windows 的拖拽已由 `WM_MOVING` 实时修正，这里只负责落库；macOS/Linux
/// 的实际钳制由它完成。监听 `Resized` 是因为高度随内容变化：前端把下边缘
/// 钉住，长高可能把窗口顶进任务栏，而这条路径没有 Moved 事件。
pub(crate) fn spawn_settle_worker(window: tauri::WebviewWindow, db: Arc<Database>) {
    let signal = Arc::new((
        std::sync::Mutex::new(SettleState {
            last_change: Instant::now(),
            dirty: false,
        }),
        std::sync::Condvar::new(),
    ));

    let event_signal = signal.clone();
    let _ = window.on_window_event(move |event| {
        if matches!(
            event,
            tauri::WindowEvent::Moved(_) | tauri::WindowEvent::Resized(_)
        ) {
            let (lock, cv) = &*event_signal;
            let mut state = lock.lock().unwrap_or_else(|e| e.into_inner());
            state.last_change = Instant::now();
            state.dirty = true;
            cv.notify_all();
        }
    });

    let spawned = std::thread::Builder::new()
        .name("ticker-bounds".into())
        .spawn(move || {
            let (lock, cv) = &*signal;
            loop {
                let mut state = lock.lock().unwrap_or_else(|e| e.into_inner());
                while !state.dirty {
                    state = cv.wait(state).unwrap_or_else(|e| e.into_inner());
                }
                // 等到最后一次几何变化之后连续 SETTLE_DELAY 没有新事件。
                loop {
                    let elapsed = state.last_change.elapsed();
                    if elapsed >= SETTLE_DELAY {
                        break;
                    }
                    state = cv
                        .wait_timeout(state, SETTLE_DELAY - elapsed)
                        .unwrap_or_else(|e| e.into_inner())
                        .0;
                }
                state.dirty = false;
                drop(state);

                if let Some(pos) = clamp_and_save(&window, &db) {
                    log::debug!("[bounds] settled at {:?}", pos);
                }
            }
        });

    if let Err(e) = spawned {
        log::warn!("[bounds] 启动位置收敛线程失败，位置只在下次显示时校正: {e}");
    }
}
```

- [ ] **Step 2: 用 worker 替换 setup 里的临时处理器**

在 `src-tauri/src/lib.rs` 的 setup 中，把 Task 2 加入的 `let ticker_for_move = ...` 到 `});` 整段替换为：

```rust
                // 位置/尺寸变化后去抖钳制并落库：macOS/Linux 靠它完成实际钳制，
                // Windows 的拖拽由 WM_MOVING 实时处理，这里负责落库与键盘移动兜底。
                window_bounds::spawn_settle_worker(ticker.clone(), db.clone());
```

- [ ] **Step 3: 编译并跑全量 Rust 测试**

Run:
```bash
cargo check --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml
```
Expected: 均通过。

- [ ] **Step 4: 手动冒烟（开发模式）**

Run: `npm run tauri dev`
操作：
1. 拖动行情条到屏幕左下角，松手，等待 1 秒。
2. 在设置里开关「迷你分时线」，让窗口长高，确认底部没有钻进任务栏。

Expected: 行情条停在任务栏上方；高度变化后下边缘不越界。

- [ ] **Step 5: 提交**

```bash
git add src-tauri/src/window_bounds.rs src-tauri/src/lib.rs
git commit -m "feat(ticker): 移动与改尺寸结束后去抖钳制位置"
```

---

### Task 4: Windows `WM_MOVING` 实时钳制

**Files:**
- Modify: `src-tauri/src/window_bounds.rs`
- Modify: `src-tauri/src/lib.rs`（setup 中安装子类）

**Interfaces:**
- Consumes: `clamp_rect`、`WorkArea`
- Produces: `pub(crate) fn install_move_clamp(window: &tauri::WebviewWindow)`（仅 `cfg(target_os = "windows")`）

- [ ] **Step 1: 实现 Win32 子类**

在 `window_bounds.rs` 追加（放在去抖 worker 之后、测试模块之前）：

```rust
/// 给行情条挂 Win32 移动子类：拖拽过程中直接修正系统给出的候选矩形。
///
/// 只在 Windows 编译。Windows 的任务栏是 topmost 且行情条刻意不抢焦点，
/// 靠 z 序压不过它，所以拖到任务栏附近时必须实时停住，不能等松手再弹回。
#[cfg(target_os = "windows")]
pub(crate) fn install_move_clamp(window: &tauri::WebviewWindow) {
    win32::install(window);
}

#[cfg(target_os = "windows")]
mod win32 {
    use super::{clamp_rect, WorkArea};
    use std::ffi::c_void;

    type HWND = *mut c_void;
    type HMONITOR = *mut c_void;

    const WM_MOVING: u32 = 0x0216;
    const WM_NCDESTROY: u32 = 0x0082;
    const SUBCLASS_ID: usize = 0x5144_5442; // "QDTB"

    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    struct RECT {
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
    }

    #[repr(C)]
    struct MONITORINFO {
        cb_size: u32,
        rc_monitor: RECT,
        rc_work: RECT,
        flags: u32,
    }

    type MonitorEnumProc =
        unsafe extern "system" fn(HMONITOR, *mut c_void, *mut RECT, isize) -> i32;
    type SubclassProc =
        unsafe extern "system" fn(HWND, u32, usize, isize, usize, usize) -> isize;

    #[link(name = "user32")]
    extern "system" {
        fn EnumDisplayMonitors(
            hdc: *mut c_void,
            clip: *const RECT,
            callback: MonitorEnumProc,
            data: isize,
        ) -> i32;
        fn GetMonitorInfoW(monitor: HMONITOR, info: *mut MONITORINFO) -> i32;
    }

    #[link(name = "comctl32")]
    extern "system" {
        fn SetWindowSubclass(
            hwnd: HWND,
            callback: SubclassProc,
            id: usize,
            ref_data: usize,
        ) -> i32;
        fn RemoveWindowSubclass(hwnd: HWND, callback: SubclassProc, id: usize) -> i32;
        fn DefSubclassProc(hwnd: HWND, msg: u32, wparam: usize, lparam: isize) -> isize;
    }

    unsafe extern "system" fn collect_area(
        monitor: HMONITOR,
        _hdc: *mut c_void,
        _rect: *mut RECT,
        data: isize,
    ) -> i32 {
        let out = &mut *(data as *mut Vec<WorkArea>);
        let mut info = MONITORINFO {
            cb_size: std::mem::size_of::<MONITORINFO>() as u32,
            rc_monitor: RECT::default(),
            rc_work: RECT::default(),
            flags: 0,
        };
        if GetMonitorInfoW(monitor, &mut info) != 0 {
            out.push(WorkArea {
                x: info.rc_work.left,
                y: info.rc_work.top,
                w: (info.rc_work.right - info.rc_work.left).max(0) as u32,
                h: (info.rc_work.bottom - info.rc_work.top).max(0) as u32,
            });
        }
        1 // 继续枚举
    }

    fn work_areas() -> Vec<WorkArea> {
        let mut areas: Vec<WorkArea> = Vec::new();
        unsafe {
            EnumDisplayMonitors(
                std::ptr::null_mut(),
                std::ptr::null(),
                collect_area,
                &mut areas as *mut Vec<WorkArea> as isize,
            );
        }
        areas
    }

    unsafe extern "system" fn ticker_move_proc(
        hwnd: HWND,
        msg: u32,
        wparam: usize,
        lparam: isize,
        id: usize,
        _ref_data: usize,
    ) -> isize {
        if msg == WM_MOVING && lparam != 0 {
            let rect = &mut *(lparam as *mut RECT);
            let size = (
                (rect.right - rect.left).max(0) as u32,
                (rect.bottom - rect.top).max(0) as u32,
            );
            if let Some((x, y)) = clamp_rect((rect.left, rect.top), size, &work_areas()) {
                let dx = x - rect.left;
                let dy = y - rect.top;
                if dx != 0 || dy != 0 {
                    rect.left += dx;
                    rect.right += dx;
                    rect.top += dy;
                    rect.bottom += dy;
                }
            }
            // 修改后的 RECT 交给系统继续处理；tao 不处理 WM_MOVING，链式安全。
            return DefSubclassProc(hwnd, msg, wparam, lparam);
        }
        if msg == WM_NCDESTROY {
            RemoveWindowSubclass(hwnd, ticker_move_proc, id);
        }
        DefSubclassProc(hwnd, msg, wparam, lparam)
    }

    pub(super) fn install(window: &tauri::WebviewWindow) {
        use raw_window_handle::HasWindowHandle;
        let Ok(handle) = window.window_handle() else {
            log::warn!("[bounds] 取不到行情条窗口句柄，拖拽实时钳制未启用");
            return;
        };
        let raw_window_handle::RawWindowHandle::Win32(h) = handle.as_raw() else {
            log::warn!("[bounds] 行情条窗口不是 Win32 句柄，拖拽实时钳制未启用");
            return;
        };
        let hwnd = h.hwnd.get() as HWND;
        let ok = unsafe { SetWindowSubclass(hwnd, ticker_move_proc, SUBCLASS_ID, 0) };
        if ok == 0 {
            log::warn!("[bounds] SetWindowSubclass 失败，拖拽实时钳制未启用");
        } else {
            log::info!("[bounds] 行情条拖拽实时钳制已安装（WM_MOVING）");
        }
    }
}
```

- [ ] **Step 2: 在 setup 中安装**

在 `src-tauri/src/lib.rs` 的 setup 中，`window_bounds::spawn_settle_worker(...)` 之后加：

```rust
                #[cfg(target_os = "windows")]
                window_bounds::install_move_clamp(&ticker);
```

- [ ] **Step 3: 编译**

Run:
```bash
cargo check --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml
```
Expected: 通过（Windows 上 FFI 参与编译）。

- [ ] **Step 4: 手动验证实时贴边**

Run: `npm run tauri dev`
操作：
1. 把行情条向屏幕底部拖，试图压到任务栏上。
2. 向左右边缘拖，试图把整条拖出屏幕。

Expected: 拖动过程中就停住，松手前后位置一致，没有回弹；任务栏始终可见、未被覆盖。

- [ ] **Step 5: 提交**

```bash
git add src-tauri/src/window_bounds.rs src-tauri/src/lib.rs
git commit -m "feat(windows): 拖拽行情条时按工作区实时钳制"
```

---

### Task 5: 文档与全量验收

**Files:**
- Modify: `CLAUDE.md`

**Interfaces:**
- Consumes: 前四个任务完成
- Produces: 无（文档）

- [ ] **Step 1: 更新 `CLAUDE.md`**

在「### 两个窗口」表格与 CSP 段落之后追加：

```markdown
行情条的位置边界由 [window_bounds.rs](src-tauri/src/window_bounds.rs) 单点管理：边界是**显示器工作区并集**（`Monitor::work_area()`，已扣除 Windows 任务栏 / macOS 菜单栏与 Dock / Linux 面板），判定是「窗口矩形被并集完整覆盖」，因此相邻显示器之间可以骑在交界上，但整条永远不会落到屏幕外。恢复入口只有 `restore_ticker_position`（启动、显示、拔屏后回退都走它）；Windows 拖拽在 `WM_MOVING` 里实时修正候选 `RECT`，macOS/Linux 由去抖 worker 在停止移动 200ms 后钳制，`Resized` 也触发同一套逻辑（高度随内容增长时防止钻进任务栏）。原生 Wayland 无法设置窗口位置，钳制退化为 no-op。**不要**再按 `primary_monitor()` 或显示器矩形写第二份边界判断 —— 负坐标扩展屏与任务栏遮挡都会漏掉。
```

在「**`lib.rs`** — 应用装配」的"三个必须单点实现的窗口 helper"列表后补一条：

```markdown
- 行情条位置边界另有单点 `window_bounds::restore_ticker_position` / `clamp_and_save`，规则见上一节；`set_ticker_visible` 与 setup 都必须调它们，不要各自读一遍 `ticker_x/ticker_y`。
```

- [ ] **Step 2: 全量校验**

Run:
```bash
cargo test --manifest-path src-tauri/Cargo.toml
cargo check --manifest-path src-tauri/Cargo.toml
npx vue-tsc --noEmit
npm test
```
Expected: 全部通过（前端未改，两条前端校验只作为守卫）。

- [ ] **Step 3: 手动验收清单**

在 Windows 上逐项确认：

1. 单屏：向四边拖动，整条停在工作区内；底部停在任务栏上方。
2. 双屏左右排列：拖到副屏并重启，位置仍在副屏。
3. 双屏上下/错位排列：骑在交界处拖动，整条始终完整可见。
4. 任务栏切到左侧或顶部（临时改设置）：拖到那一侧不会钻到任务栏下面。
5. 开关「迷你分时线」让窗口长高：底部不进入任务栏。
6. 拔掉副屏后从托盘隐藏再显示行情条：落在主屏内，不消失。

如果有 macOS / Linux 设备，追加：外接屏拖动与恢复、X11 下松手后钳制、无 XWayland 时不崩溃。

- [ ] **Step 4: 提交**

```bash
git add CLAUDE.md
git commit -m "docs: 补充行情条位置边界管理的约定"
```

---

## Self-Review

**Spec coverage:**
- 工作区并集规则 → Task 1 `clamp_rect` / `covered`，Task 2 接线。
- Windows 任务栏 → Task 1 工作区偏移测试，所有路径用 `work_area()`。
- 多显示器 / 负坐标 → Task 1 负坐标与右侧测试，Task 2 恢复单点。
- Windows 实时钳制 → Task 4。
- macOS/Linux 去抖 → Task 3。
- 尺寸变化 → Task 3 `Resized`。
- 落库去重与钳制后保存 → Task 2 的 `clamp_and_save` / `restore_ticker_position`。
- Wayland 降级 → 无平台代码，钳制调用失败只记日志（`set_position` 失败分支）。
- 测试与非目标 → 各任务验证步骤 + Task 5 文档。

**Placeholder scan:** 计划中无 TBD/TODO；每个代码步骤都带完整实现。

**Type consistency:** `WorkArea`、`clamp_rect`、`clamp_and_save`、`restore_ticker_position`、`spawn_settle_worker`、`install_move_clamp` 的签名在 Task 1–4 间一致；`DEFAULT_MARGIN`、`SETTLE_DELAY` 只在各自任务定义一次。

**Review Focus tests:** 负坐标、错位交界、任务栏四边均已由 Task 1 的纯函数测试固定；高度增长与拔屏分别是 Task 3 `Resized` 接线和 Task 2 恢复路径，由手动验收覆盖（几何部分已被 Task 1 测试覆盖）。
