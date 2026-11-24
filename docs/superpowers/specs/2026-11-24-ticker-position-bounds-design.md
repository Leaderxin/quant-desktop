# 行情条位置边界管理设计文档

日期：2026-11-24
状态：待实现

## 目标

行情条（`ticker` 窗口）的位置受统一边界管理：

1. 三端（Windows / macOS / Linux）都不能被拖到所有显示器之外——最终位置必须让整条行情条完整可见。
2. Windows 下不能停在任务栏所在区域（被任务栏遮住）。边界按**显示器工作区**计算，而不是显示器矩形。
3. 允许多显示器：可以自由拖到扩展屏（含左侧/上方扩展屏的负坐标），位置能被记住并在下次启动/显示时恢复；显示器配置变化后能自动回到可用位置。

成功标准：

- 单屏四边拖动都停得住，不会有一截露在屏幕外。
- 左右并排、上下错位、负坐标布局下都能拖过去；重启后仍在原来那块屏。
- 拖到任务栏那一侧时停在任务栏上方，不钻进去。
- 拔掉扩展屏后再次显示行情条，落在仍存在的某块屏内，不丢窗口。

## 背景与现状

行情条是无边框、不可缩放、`alwaysOnTop`、`skipTaskbar` 的独立窗口，高度随内容变化（`useTickerWindowHeight`），拖拽走 Tauri 的 `startDragging()`（Windows 上是 `WM_NCLBUTTONDOWN` + `HTCAPTION` 的模态移动循环）。位置以物理像素存在 `settings` 的 `ticker_x` / `ticker_y`。

现有实现有两个真实缺陷：

1. **多显示器位置记忆是坏的。** [lib.rs](../../../src-tauri/src/lib.rs) 的 `Moved` 处理器（约 962–988 行）与两处恢复逻辑（setup 约 1002–1013 行、`set_ticker_visible` 约 373–383 行）都只取 `primary_monitor()` 并假设它原点在 `(0, 0)`：
   - 右侧扩展屏：`pos.x > 主屏宽` 时可见性判定失败，位置**不保存**；
   - 左侧/上方扩展屏：`pos.x.max(0)` 把负坐标压成 0，恢复时 `sx + tw > 0` 又拒绝负坐标，位置**无法恢复**。
2. **几何基准是启动时快照。** `mon_w/mon_h` 在 `setup` 时捕获一次，之后改分辨率、插拔显示器、跨 DPI 全部使用过期值；`Moved` 处理器在拖动过程中每移动一像素同步写两次 SQLite。

此外，默认位置 `mon_h - th - 60` 是手写的"预留任务栏高度"，不读取真实工作区。

## 平台调研

基于仓库实际依赖（tauri 2.11.2 / tao 0.35.3 / tauri-runtime-wry 2.11.2）。

| 能力 | Windows | macOS | Linux |
|---|---|---|---|
| 工作区 | ✅ `Monitor::work_area()` → `GetMonitorInfoW.rcWork` | ✅ `NSScreen.visibleFrame` | ✅ `gdk_monitor.workarea()` |
| 枚举/就近显示器 | ✅ `available_monitors()` / `monitor_from_point()` | ✅ | ✅ |
| 拖拽中实时拦截 | ✅ `WM_MOVING` + `SetWindowSubclass` | ⚠️ tao 持有 `NSWindowDelegate`，替换风险高 | ⚠️ X11 移动循环归 WM 管，程序内 `move` 会与 WM 抢 |
| 原生 Wayland 定位 | — | — | ❌ 客户端无法设置位置 |

关键结论：

- **工作区三端都有现成实现**，tauri-runtime-wry 已按平台正确换算：Windows 是 `rcWork`，macOS 是 `visibleFrame`，Linux 是 GDK workarea。不需要平台特判去扣任务栏。
- **Windows 任务栏无法可靠压过。** `Shell_TrayWnd` 自己也是 topmost，而行情条刻意不抢焦点（`focus: false`、`WS_EX_TOOLWINDOW`），z 序上会被任务栏盖住。能压住的做法只有周期性重刷 `HWND_TOPMOST`（Rainmeter 的 stay-topmost 即如此）或注册 AppBar，前者会挡住用户的任务栏、脆弱，后者会改变全系统工作区。因此需求 2 的正确解法是**把钳制边界从显示器矩形换成工作区**，而不是做 z 序 hack。
- **自动隐藏任务栏**：`rcWork` 通常等于整块屏幕，行情条可以贴到物理底边；任务栏滑出时仍会临时盖住它，属于平台行为。
- **原生 Wayland**：无法设置窗口位置（[linux_backend.rs](../../../src-tauri/src/linux_backend.rs) 已通过 `GDK_BACKEND=x11,wayland` 优先切 XWayland）；没有 XWayland 时钳制退化为 no-op，不报错、不刷日志。

### 为什么不做前端逐帧拖拽

前端拿到鼠标位移后逐帧 `setPosition` 也能实时钳制，但会替换掉为顺滑而选的原生拖拽循环，并与现有的"点击/拖动阈值"逻辑纠缠；Linux 原生 Wayland 下 `setPosition` 本身无效。不采用。

## 设计

### 1. 边界模型：工作区并集

窗口矩形必须被**所有显示器工作区的并集**完整覆盖：

- 单屏时等价于完全包含在工作区内；
- 相邻显示器之间允许骑在交界上（只要没有像素落在屏幕外），跨屏因此顺滑，不会出现"先在交界处卡住、越过中线再跳到另一块屏"的瞬移；
- 显示器之间不连续的空洞区域不会被误判为可见。

判定使用精确覆盖检查，不做采样。

### 2. 新模块 `src-tauri/src/window_bounds.rs`

#### 2.1 纯几何

```rust
pub struct WorkArea { pub x: i32, pub y: i32, pub w: u32, pub h: u32 }

/// 返回使窗口被工作区并集完整覆盖、且位移最小的左上角。
/// 已经完整可见 → 原样返回；没有显示器 → None；窗口比所有工作区都大时
/// 退回重叠面积最大者的左上角（保证至少锚在可见区域）。
pub fn clamp_rect(pos: (i32, i32), size: (u32, u32), areas: &[WorkArea]) -> Option<(i32, i32)>
```

算法：

1. `covered_by_union(rect, areas)`：取窗口与各工作区的所有 x 边界排序去重，逐条带合并 y 区间，与窗口高度精确比对。
2. 已覆盖 → 返回原位（"允许骑在交界上"的关键）。
3. 未覆盖 → 枚举候选位移：每块工作区的左/右/上/下边界对齐值外加 `0`，做笛卡尔积（≤ (2N+1)²，N ≤ 10），过滤出仍满足覆盖的候选，取位移平方最小者。
4. 一个候选都不可用（窗口比任何工作区都大）→ 返回重叠面积最大的工作区左上角，保证窗口至少锚在可见区域。

坐标使用 `i32`，内部边界运算用 `i64` 防止 `left + width` 溢出。该函数不依赖 Tauri，三端 CI 都能跑单测。

#### 2.2 显示器工作区来源

| 路径 | 实现 |
|---|---|
| 启动/显示恢复、去抖 worker | `window.available_monitors()` → `Monitor::work_area()` |
| Windows `WM_MOVING`（UI 线程、模态循环内） | 手写 `EnumDisplayMonitors` + `GetMonitorInfoW` |

两条来源都只产出 `Vec<WorkArea>`，几何判断仍只有一份。`WM_MOVING` 不复用 Tauri 的显示器 API：虽然 tauri-runtime-wry 在 UI 线程上会直接执行消息（不会死锁），但在原生回调里依赖事件循环转发是不必要的风险，且 Win32 枚举本身足够便宜。

#### 2.3 Windows 实时钳制

- 在 ticker 窗口创建后，用 `SetWindowSubclass` 挂一个**无状态**子类过程。tao 自己也用 `SetWindowSubclass`，子类链通过 `DefSubclassProc` 正常衔接；tao 不处理 `WM_MOVING`，不存在语义冲突。
- `WM_MOVING`：`lParam` 指向系统给出的候选 `RECT`，枚举工作区 → `clamp_rect` → 原地修正 `RECT`（宽高不变，只平移）→ 交还 `DefSubclassProc`。拖拽过程中实时贴边，不会先跑出屏幕或钻到任务栏后面。
- `WM_NCDESTROY` 里 `RemoveWindowSubclass` 收尾。
- FFI 全部手写 `extern "system"`（`user32` / `comctl32`），沿用 [lib.rs](../../../src-tauri/src/lib.rs) 中 `WS_EX_TOOLWINDOW` 的写法，**不新增 `windows` crate 依赖**（`store` feature 才需要它，见 Cargo.toml 注释）。
- 该路径只负责"拖拽中"；由键盘/程序触发的移动（如 Win+Shift+方向、前端高度锚定）走 2.4 的去抖路径兜底。

#### 2.4 去抖 worker

- 单条常驻线程 + `Condvar`，由 `WindowEvent::Moved | Resized` 更新时间戳唤醒。
- 最后一次几何变化后 200ms 无新事件 → 读取 `outer_position` / `outer_size` / `available_monitors` → `clamp_rect` → 有位移则 `set_position` → 位置与上次落库值不同才写 `ticker_x` / `ticker_y`。
- 监听 `Resized`：高度随内容变化时前端把下边缘钉住，长高可能把窗口顶进任务栏；只监听 `Moved` 会漏掉这条路径。
- Windows 上拖拽已由 `WM_MOVING` 实时钳制，worker 在这里只负责落库；macOS/Linux 由它完成实际钳制。
- 200ms 阈值是取舍：X11/macOS 没有可靠的"拖拽结束"信号，停顿超过阈值时窗口会在拖拽中途贴边。阈值写成常量并注释原因。
- worker 与进程同寿命：窗口销毁后不再有事件，无需显式停止。
- 幂等性：钳制触发的 `set_position` 会再次产生 `Moved`，但下一轮读到的已是合法位置，`clamp_rect` 原样返回、不产生位移、不写库，不会形成循环。
- 旧实现"每移动一像素写两次 SQLite"的行为随之消失。

#### 2.5 恢复路径单点化

启动路径与 `set_ticker_visible` 各写了一份"读库 → 校验 → 默认右下角"，合并为：

```rust
pub(crate) fn restore_ticker_position(window, db, logical_height)
```

- 有存档 → `clamp_rect` 到工作区并集；完全可见则原样恢复（扩展屏位置从此能记住）。
- 无存档 / 解析失败 → 主显示器工作区右下角，边距 10px（替代手写的 `-60`）。
- 目标显示器缩放不同造成的物理尺寸差异，由置位后的 `Moved` / DPI 变化再次触发 worker 收敛。
- 恢复失败（读不到显示器）时保持当前位置，不 panic、不阻塞启动。

#### 2.6 尺寸变化

`set_ticker_size` 本身不改位置；高度变化后的下边缘越界由 worker 的 `Resized` 路径拉回。前端 `useTickerWindowHeight` 的"下边缘锚定"逻辑不改。

#### 2.7 落库语义

- 仍用物理像素（`ticker_x` / `ticker_y`），不改键名、不做迁移。
- 只在位置收敛后写库，并跳过与上次写入相同的值。
- 写入的一定是钳制后的位置。

## 边界与降级

| 场景 | 行为 |
|---|---|
| 左侧 / 上方扩展屏（负坐标） | 正常保存与恢复（当前是坏的，本次修复） |
| 右侧扩展屏 | 正常保存与恢复（当前是坏的，本次修复） |
| 显示器被拔掉 | 下次显示时就近拉回最近的可行位置，不跳回主屏右下角 |
| 分辨率 / 缩放变化 | worker 按事件重算，物理坐标与尺寸实时读取，不缓存 |
| 自动隐藏任务栏 | 可贴物理底边；任务栏滑出时仍会临时盖住（平台行为） |
| 原生 Wayland | 无法设置位置，钳制退化为 no-op，不报错、不刷日志 |
| 覆盖 Windows 任务栏 | 不做。工作区钳制已避免遮挡；压过任务栏需要 AppBar 或重刷 topmost，代价不可接受 |
| 显示器全部消失的极端情况 | `clamp_rect` 返回 `None`，保持原位，下次事件再试 |

## 测试

纯函数 `cargo test`（三端 CI 都跑）：

- 单屏：四边越界、正常位置不产生位移；
- 负坐标左屏 / 上方屏；
- 左右并排：交界处骑跨不产生位移，越过外沿被拉回；
- 上下错位：并集覆盖时可跨屏，落入空洞被修正；
- 不同大小工作区（模拟任务栏在左/右/上/下）；
- 显示器间不连续空洞；
- 无显示器、窗口比所有工作区都大（退化路径）。

其余：

- Windows CI 的 `cargo check`（default + `--features store`）保证 FFI 编译；
- `backend-linux` job 覆盖 Linux 分支；
- 前端无需改动，现有 vitest 不受影响；
- 手动验收清单：Windows 单屏/双屏（含任务栏在左/右/上）、macOS 外接屏、Linux X11；拖四边、跨交界、拖到任务栏、拔屏、改缩放、重启还原、开关迷你分时图让高度变化。

## 改动面

| 文件 | 改动 |
|---|---|
| `src-tauri/src/window_bounds.rs` | 新增：纯几何 + 工作区采集 + Win32 子类 + 去抖 worker + 恢复单点 |
| `src-tauri/src/lib.rs` | 注册模块；setup 与 `set_ticker_visible` 改调单点；移除旧 `Moved` 落库/钳制逻辑；Windows 上安装子类 |
| `src-tauri/Cargo.toml` | 无新依赖 |
| `CLAUDE.md` | 补模块约定：工作区并集规则、恢复单点、Wayland 降级 |
| 前端 | 零改动 |

## 非目标

- 主窗口的边界校验。它同样缺少校验，但需求只提行情条；纯函数可直接复用，留待后续。
- 压过任务栏的 z 序方案。
- 原生 Wayland 的位置控制。
- "重置行情条位置"菜单/按钮。钳制与默认位已覆盖恢复场景。

## 风险

1. **Win32 子类的线程与生命周期**：过程函数无状态、只读消息参数，`WM_NCDESTROY` 注销；窗口销毁由进程退出兜底。
2. **macOS/Linux 拖拽中途停顿触发贴边**：200ms 阈值取舍，注释说明。
3. **混合 DPI**：物理坐标的边界语义在各平台上不完全一致（尤其 Linux/X11 的整数缩放），策略是事件驱动重算而非缓存，尽力而为。
4. **测试盲区**：Win32 子类与真实拖拽无法单测；靠 CI 编译 + 手动验收覆盖。
