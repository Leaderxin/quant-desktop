# QuantDesktop 版本更新检测及自动更新 — 设计规格

**日期**: 2026-06-20
**版本**: v1.0
**状态**: 设计中

---

## 1. 概述

为 QuantDesktop Tauri 2 桌面应用添加版本更新检测和自动更新能力。以 `tauri-plugin-updater` + GitHub Releases 为核心，公开镜像仓库 `QuantDesktopRelease` 作为更新源，零服务器成本。

### 1.1 核心需求

| # | 需求 | 说明 |
|---|------|------|
| R1 | 启动时自动检查更新 | 后台静默，不阻塞启动流程 |
| R2 | 交易时段抑制弹窗 | 9:30-15:00 期间不弹窗，改为托盘提示 |
| R3 | 更新对话框展示 CHANGELOG | 显示新版本号、发布日期、变更日志 |
| R4 | 静默下载 + 自动安装 | 点击"立即更新"后后台下载，完成后自动启动安装程序 |
| R5 | 代理自动检测 | 扫描本地 Clash/V2Ray 端口 + 系统代理回退 |
| R6 | 签名验证 | 更新包须通过公私钥签名验证 |
| R7 | 公开仓库发布 | CI 构建产物推送到 `Leaderxin/QuantDesktopRelease` |

### 1.2 更新源架构

```
Private Repo (quant-desktop)
  └─ CI: build + sign + generate latest.json
       └─ push Release Assets ──→ Public Repo (QuantDesktopRelease)
                                      └─ GitHub Releases API (免费，无需认证)
                                           └─ App: check + download + install
```

---

## 2. 架构总览

```
┌──────────────────────────────────────────────────┐
│           Private Repo: quant-desktop             │
│  ┌──────────────┐                                │
│  │ CI (v* tag)  │                                │
│  │ - build      │                                │
│  │ - sign       │                                │
│  │ - latest.json│                                │
│  └──────┬───────┘                                │
└─────────┼────────────────────────────────────────┘
          │ push Release Assets
          ▼
┌──────────────────────────────────────────────────┐
│     Public Repo: QuantDesktopRelease              │
│  ┌─────────────────────────────────────────┐     │
│  │  GitHub Releases (e.g. v1.2.0)          │     │
│  │  ├── QuantDesktop_1.2.0_x64-setup.exe    │     │
│  │  ├── QuantDesktop_1.2.0_x64.msi          │     │
│  │  ├── latest.json  (签名+版本+notes)      │     │
│  │  └── QuantDesktop_1.2.0_x64.dmg          │     │
│  └─────────────────────────────────────────┘     │
└──────────┬──────────────────────────────────────┘
           │ HTTPS (公开，无需认证)
           ▼
┌──────────────────────────────────────────────────┐
│           QuantDesktop App (用户机器)              │
│  ┌──────────────┐  ┌─────────────────────────┐   │
│  │ ProxyDetect   │  │ tauri-plugin-updater    │   │
│  │ - Clash:7890  │  │ - 下载 latest.json      │   │
│  │ - V2Ray:10808 │  │ - 验证签名              │   │
│  │ - 系统代理     │  │ - 下载安装包            │   │
│  └──────────────┘  └─────────────────────────┘   │
│         │                                        │
│         ▼                                        │
│  ┌──────────────────────────────────────────┐   │
│  │  Frontend (Vue 3 + Naive UI)             │   │
│  │  - updater store (更新状态管理)           │   │
│  │  - UpdateDialog (版本+CHANGELOG+进度)     │   │
│  │  - useUpdateCheck (启动检查+交易时段)     │   │
│  └──────────────────────────────────────────┘   │
└──────────────────────────────────────────────────┘
```

---

## 3. 模块一：Rust 后端

### 3.1 新增依赖

`src-tauri/Cargo.toml`:

```toml
[dependencies]
tauri-plugin-updater = "2"
semver = "1"
```

### 3.2 新增模块结构

```
src-tauri/src/
├── updater/
│   ├── mod.rs          # 更新编排 + Tauri commands
│   └── proxy.rs        # 代理自动检测
```

### 3.3 `updater/proxy.rs` — 代理自动检测

**检测策略（优先级从高到低）**：

1. 系统代理（环境变量 `HTTP_PROXY` / `HTTPS_PROXY`）
2. 扫描已知本地代理端口（TCP connect 1s 超时）：
   - `localhost:7890` — Clash
   - `localhost:7891` — Clash (备用)
   - `localhost:10809` — Clash Meta
   - `localhost:10808` — V2Ray (socks5→http)
   - `localhost:1080` — V2Ray / 通用 SOCKS5
   - `localhost:8118` — Privoxy
   - `localhost:8080` — 通用 HTTP 代理
3. 都不可用时直连

**接口**：

```rust
pub fn build_proxied_client() -> reqwest::Client { ... }
pub fn detect_proxy_url() -> Option<String> { ... }
```

### 3.4 `updater/mod.rs` — 更新编排

**Tauri 命令**：

```rust
// 检查更新，返回版本信息 + changelog
#[tauri::command]
async fn check_update(app: tauri::AppHandle) -> Result<UpdateInfo, String>

// 下载并安装更新
#[tauri::command]
async fn install_update(app: tauri::AppHandle) -> Result<(), String>

// 查询当前是否处于交易活跃时段
#[tauri::command]
fn is_trading_session() -> bool
```

**UpdateInfo 结构体**（定义于 `domain/mod.rs`）：

```rust
pub struct UpdateInfo {
    pub current_version: String,    // "1.1.1"
    pub latest_version: String,     // "1.2.0"
    pub release_date: String,       // "2026-06-20"
    pub notes: String,              // CHANGELOG 内容 (Markdown)
    pub release_url: String,        // GitHub Release 链接
    pub download_size: Option<u64>, // 安装包大小（字节）
}
```

**交易时段判断**：复用 `market_clock` 模块，`is_trading_session()` 在 `MorningTrade` / `AfternoonTrade` 时返回 `true`。

### 3.5 `lib.rs` 集成

```rust
// 注册 updater plugin
.plugin(tauri_plugin_updater::Builder::new()
    .endpoints(&[
        "https://github.com/Leaderxin/QuantDesktopRelease/releases/latest/download/latest.json"
    ])
    .build())

// 注册新命令
.invoke_handler(tauri::generate_handler![
    // ... 现有命令 ...
    updater::check_update,
    updater::install_update,
    updater::is_trading_session,
])
```

**启动后后台检查**：在 `setup()` 中 spawn 一个 tokio task，调用 `check_update()`，与本地版本比较，根据交易时段决定弹窗或托盘提示。

### 3.6 `tauri.conf.json` 配置

```json
{
  "plugins": {
    "updater": {
      "endpoints": [
        "https://github.com/Leaderxin/QuantDesktopRelease/releases/latest/download/latest.json"
      ],
      "pubkey": "YOUR_PUBLIC_KEY_HERE",
      "windows": {
        "installMode": "passive"
      }
    }
  },
  "bundle": {
    "active": true,
    "targets": "all",
    "createUpdaterArtifacts": true,
    "icon": ["..."],
    "windows": {
      "wix": {},
      "nsis": {}
    }
  }
}
```

---

## 4. 模块二：CI/CD 改造

### 4.1 密钥体系

| 密钥 | 存储位置 | 用途 |
|------|---------|------|
| `TAURI_PRIVATE_KEY` | Private Repo GitHub Secrets | CI 签名 `latest.json` |
| `TAURI_KEY_PASSWORD` | Private Repo GitHub Secrets | 私钥密码（如有） |
| `PUBLIC_REPO_PAT` | Private Repo GitHub Secrets | 写入公开仓库 `QuantDesktopRelease` |

**生成密钥对**：

```bash
npx tauri signer generate -w ~/.tauri/quant-desktop.key
# Private key → copy to TAURI_PRIVATE_KEY secret
# Public key  → paste into tauri.conf.json plugins.updater.pubkey
```

### 4.2 latest.json 格式

```json
{
  "version": "1.2.0",
  "notes": "### ✨ 新增\n- K线图表支持 日/周/月 切换\n...",
  "pub_date": "2026-06-20T12:00:00Z",
  "platforms": {
    "windows-x86_64": {
      "signature": "dw...base64...sig",
      "url": "https://github.com/Leaderxin/QuantDesktopRelease/releases/download/v1.2.0/QuantDesktop_1.2.0_x64-setup.exe"
    },
    "darwin-x86_64": {
      "signature": "...",
      "url": "https://github.com/Leaderxin/QuantDesktopRelease/releases/download/v1.2.0/QuantDesktop_1.2.0_x64.dmg"
    },
    "linux-x86_64": {
      "signature": "...",
      "url": "https://github.com/Leaderxin/QuantDesktopRelease/releases/download/v1.2.0/QuantDesktop_1.2.0_amd64.AppImage"
    }
  }
}
```

### 4.3 CI 工作流改造

在 `.github/workflows/release.yml` 的 `build` job 之后插入 `sign-and-publish` job：

```yaml
sign-and-publish:
  needs: build
  runs-on: ubuntu-latest
  if: startsWith(github.ref, 'refs/tags/v')
  permissions:
    contents: write
  steps:
    - uses: actions/download-artifact@v4
      with:
        pattern: quant-desktop-*
        merge-multiple: true

    - uses: actions/checkout@v4

    - name: Extract changelog for this version
      id: changelog
      run: |
        VERSION="${{ github.ref_name }}"
        node scripts/extract-changelog.mjs "$VERSION" > changelog-notes.md

    - name: Sign and generate latest.json
      env:
        TAURI_PRIVATE_KEY: ${{ secrets.TAURI_PRIVATE_KEY }}
        TAURI_KEY_PASSWORD: ${{ secrets.TAURI_KEY_PASSWORD }}
      run: |
        VERSION="${{ github.ref_name }}"
        NOTES=$(cat changelog-notes.md | jq -Rs '.')
        npx tauri signer sign \
          --private-key <(echo "$TAURI_PRIVATE_KEY") \
          --password "$TAURI_KEY_PASSWORD" \
          --generate-latest-json \
          --version "$VERSION" \
          --notes "$NOTES" \
          --output latest.json

    - name: Publish to public release repo
      uses: softprops/action-gh-release@v2
      with:
        repository: Leaderxin/QuantDesktopRelease
        token: ${{ secrets.PUBLIC_REPO_PAT }}
        files: |
          release/**/*
          latest.json
        generate_release_notes: true
```

### 4.4 新增脚本 `scripts/extract-changelog.mjs`

从项目根目录 `CHANGELOG.md` 提取指定版本的变更内容：

```javascript
// 输入: CHANGELOG.md
// 输出: 特定版本的 changelog 文本
// 格式约定 (Keep a Changelog 风格):
// ## v1.2.0 (2026-06-20)
// ### Added
// - xxx
// ### Fixed
// - yyy
```

---

## 5. 模块三：前端

### 5.1 新增文件

```
src/
├── components/
│   └── updater/
│       └── UpdateDialog.vue      # 主更新弹窗
├── stores/
│   └── updater.ts               # 更新状态管理
└── composables/
    └── useUpdateCheck.ts        # 启动检查 + 交易时段判断
```

### 5.2 数据流

```
App.vue onMounted()
  └─ useUpdateCheck().performStartupCheck()
       ├─ invoke('check_update') → UpdateInfo | null
       ├─ 比较版本 (semver)
       ├─ 若是旧版本，invoke('is_trading_session') → bool
       └─ 交易时段?
            ├─ Yes → 托盘提示 "发现新版本 v{version}"
            └─ No  → 显示 UpdateDialog
                       ├─ [立即更新] → invoke('install_update') + 监听进度
                       │    └─ 下载完成 → 自动启动安装程序
                       ├─ [稍后提醒] → dismissUpdate() (24h 冷却)
                       └─ [GitHub]   → openReleasePage() (系统浏览器)
```

### 5.3 `src/stores/updater.ts`

```typescript
// 状态
updateStatus: 'idle' | 'checking' | 'available' | 'downloading' | 'ready' | 'error'
updateInfo: UpdateInfo | null
downloadProgress: number          // 0-100
downloadBytes: { current: number, total: number }
lastCheckTime: string
nextReminderTime: string          // 下次可提醒时间戳，24h 冷却

// 动作
checkForUpdate()                  // invoke('check_update')
downloadAndInstall()              // invoke('install_update') + 事件监听
dismissUpdate()                   // 设置 nextReminderTime = now + 24h
openReleasePage()                 // opener 打开 GitHub Release URL
```

### 5.4 `src/components/updater/UpdateDialog.vue`

**组件树**：

```
NModal (maskClosable=false, displayDirective="if")
  └─ NCard (bordered=false, closable, width=480px)
       ├─ Header: 版本徽章 + "发现新版本" 标题 + [×] 关闭
       ├─ VersionComparison: v{current} → v{latest} + 发布日期
       ├─ ChangelogBox:
       │    ├─ Section 标题 "更新内容"
       │    └─ ScrollContainer (max-height: 240px, overflow-y: auto)
       │         └─ MarkdownRenderer (轻量正则渲染)
       ├─ ProgressBar: (v-if="downloading")
       │    ├─ 进度条 track + fill
       │    ├─ 百分比 + 字节数
       │    └─ 状态文案："正在下载更新..."
       └─ Footer:
            ├─ [在 GitHub 查看详情] (link variant)
            └─ [稍后提醒] (secondary) [立即更新 v{x.y.z}] (primary)
```

**交互状态**：

| 状态 | 主按钮 | 次按钮 | 进度条 |
|------|--------|--------|--------|
| available | "立即更新 v1.2.0" (primary) | "稍后提醒" | 隐藏 |
| downloading | loading + "下载中..." (disabled) | disabled | 显示百分比+字节 |
| ready | "立即安装" (success) | hidden | 100% 绿色 |
| error | "重试" (warning) | "手动下载" | 变红 + 错误信息 |

**Changelog 渲染规则**（零依赖轻量实现）：

```
输入文本 → 逐行解析：
"### "...  → <h4> + 左侧 accent 色条
"- "    ... → <li> + 圆点标记
空行     → 段落间距
其他     → <p>
```

### 5.5 `src/composables/useUpdateCheck.ts`

```typescript
export function useUpdateCheck() {
  async function performStartupCheck() {
    const info = await updaterStore.checkForUpdate()
    if (!info) return // 已是最新版本

    const isTrading = await invoke<boolean>('is_trading_session')
    if (isTrading) {
      // 托盘提示，不弹窗
      emitTrayNotification(`发现新版本 v${info.latest_version}，收盘后可更新`)
    } else {
      showUpdateDialog(info)
    }
  }
  return { performStartupCheck }
}
```

### 5.6 设计令牌（沿用项目现有 CSS 变量）

| 元素 | CSS |
|------|-----|
| 版本徽章 | `background: var(--color-accent-dim); color: var(--color-accent)` |
| Changelog 容器 | `background: var(--color-surface-2); border: 1px solid var(--color-border-0); border-radius: var(--radius-md)` |
| 进度条轨道 | `background: var(--color-surface-2); border-radius: var(--radius-sm)` |
| 进度条填充 | `background: var(--color-accent); transition: width 300ms ease-out` |
| 进度条错误 | `background: #f85149` |
| 版本箭头 | `color: var(--color-accent)` |
| 主按钮 | Naive UI `type="primary"` → `#58a6ff`（暗色主题） |
| 次按钮 | Naive UI `type="default"` 边框样式 |

### 5.7 可访问性

| 规则 | 实现 |
|------|------|
| Escape routes | Esc 关闭 / [×] 按钮 / `maskClosable` |
| Focus states | 聚焦元素 2px accent 色 outline-ring |
| Aria labels | `aria-label="关闭更新对话框"` / `aria-label="下载进度 65%"` |
| Keyboard nav | Tab 序：关闭 → changelog → GitHub链接 → 稍后提醒 → 立即更新 |
| Reduced motion | 检测 `prefers-reduced-motion`，跳过入场动画 |
| Loading feedback | 下载中按钮 `:loading` + 禁用 |
| Error recovery | 错误时显示"重试" + "手动下载"选项 |

### 5.8 动画规范

| 动画 | 时长 | 缓动 |
|------|------|------|
| 对话框入场 (scale + fade) | 200ms | ease-out |
| 对话框退场 (fade) | 150ms | ease-in |
| 进度条宽度变化 | 300ms | ease-out |
| 按钮状态过渡 | 150ms | ease |

---

## 6. 错误处理矩阵

| 场景 | 处理 |
|------|------|
| 网络不可达 | 静默失败，不弹窗，日志记录 |
| latest.json 签名验证失败 | 日志 warn，不提示用户（安全优先） |
| 下载中断 | 进度条变红，显示"下载失败，请重试"，提供重试按钮 |
| 安装程序启动失败 | 提示"无法启动安装程序"，提供 GitHub Release 链接 |
| 代理全部不可用 | 回退直连，日志记录 |
| 版本号格式异常 | semver 解析失败 → 跳过更新，日志记录 |

---

## 7. 安全考虑

- 更新包签名验证（tauri-plugin-updater 内置）
- 私钥仅存于 CI Secrets，不进入源代码
- 公钥编译进 App，无法篡改
- CSP 限制 `default-src 'self'`，不从外部加载脚本
- 安装包仅从 `github.com/Leaderxin/QuantDesktopRelease` 下载

---

## 8. 测试策略

| 测试场景 | 方法 |
|---------|------|
| 更新检查成功（有新版） | 模拟 GitHub API 返回 latest.json |
| 更新检查成功（已最新） | 返回 current_version == latest_version |
| 网络不可达 | 超时/连接拒绝 → 静默处理 |
| 签名验证失败 | 使用无效签名 → 拒绝更新 |
| 交易时段判断 | 不同时间点验证 |
| 代理检测 | 逐个端口验证 |
| 下载进度事件 | 验证百分比递增 |
| 对话框交互 | 手动：版本展示、changelog 渲染、操作按钮 |

---

## 9. 开放问题

- [ ] macOS 更新行为确认（`.dmg` 安装 vs `.app` 替换）
- [ ] Linux AppImage 自更新是否兼容
- [ ] NSIS passive 模式是否需要管理员权限
