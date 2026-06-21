# 版本更新检测及自动更新 — 实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 为 QuantDesktop Tauri 2 应用添加基于 GitHub Releases 的版本更新检测和自动更新能力，包括版本检查、CHANGELOG 展示、静默下载安装、代理检测。

**Architecture:** 新增 `updater` Rust 模块（代理检测 + 更新编排），改造 CI 推送到公开镜像仓库 `QuantDesktopRelease`，新增 Vue 更新对话框（Naive UI NModal + NCard 风格），通过 Pinia store 管理更新状态。

**Tech Stack:** Rust (tauri-plugin-updater, semver, reqwest), Vue 3 + Naive UI + Pinia, GitHub Actions

**Spec:** [2026-06-20-updater-design.md](../specs/2026-06-20-updater-design.md)

---

### Task 1: 添加 Rust 依赖

**Files:**
- Modify: `src-tauri/Cargo.toml`

- [ ] **Step 1: 添加 tauri-plugin-updater 和 semver 依赖**

在 `src-tauri/Cargo.toml` 的 `[dependencies]` 节末尾添加：

```toml
tauri-plugin-updater = "2"
semver = "1"
```

- [ ] **Step 2: 添加 build-dependencies 的 isolation feature**

在 `src-tauri/Cargo.toml` 的 `[build-dependencies]` 节中修改：

```toml
[build-dependencies]
tauri-build = { version = "2", features = ["isolation"] }
```

- [ ] **Step 3: 验证依赖解析**

```bash
cargo check --manifest-path src-tauri/Cargo.toml
```
预期：依赖下载成功，没有版本冲突。

- [ ] **Step 4: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/Cargo.lock
git commit -m "chore: add tauri-plugin-updater and semver dependencies"
```

---

### Task 2: 配置 tauri.conf.json

**Files:**
- Modify: `src-tauri/tauri.conf.json`

- [ ] **Step 1: 添加 plugins.updater 配置节**

在 `src-tauri/tauri.conf.json` 中，`"app"` 节之后添加：

```json
{
  "plugins": {
    "updater": {
      "endpoints": [
        "https://github.com/Leaderxin/QuantDesktopRelease/releases/latest/download/latest.json"
      ],
      "pubkey": "PASTE_YOUR_PUBLIC_KEY_HERE_AFTER_GENERATION",
      "windows": {
        "installMode": "passive"
      }
    }
  }
}
```

注意：`pubkey` 的占位符将在生成密钥对后替换。`plugins` 是顶层 key，与 `"app"`、`"build"`、`"bundle"` 平级。

- [ ] **Step 2: 在 bundle 中添加 createUpdaterArtifacts**

在 `"bundle"` 节中添加：

```json
"createUpdaterArtifacts": true,
```

放在 `"active": true` 之后。

- [ ] **Step 3: 验证 JSON 格式正确**

```bash
npx tauri dev --help | head -5
```
预期：Tauri CLI 启动无错误。

- [ ] **Step 4: Commit**

```bash
git add src-tauri/tauri.conf.json
git commit -m "feat: configure tauri-plugin-updater in tauri.conf.json"
```

---

### Task 3: 添加 UpdateInfo 领域类型

**Files:**
- Modify: `src-tauri/src/domain/mod.rs`
- Modify: `src/types/index.ts`

- [ ] **Step 1: 在 Rust domain 中添加 UpdateInfo**

在 `src-tauri/src/domain/mod.rs` 末尾添加：

```rust
/// Update check result returned to frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateInfo {
    pub current_version: String,
    pub latest_version: String,
    pub release_date: String,
    pub notes: String,
    pub release_url: String,
    pub download_size: Option<u64>,
}
```

- [ ] **Step 2: 在 TypeScript types 中添加 UpdateInfo**

在 `src/types/index.ts` 末尾添加：

```typescript
export interface UpdateInfo {
  current_version: string;
  latest_version: string;
  release_date: string;
  notes: string;
  release_url: string;
  download_size: number | null;
}
```

- [ ] **Step 3: 验证**

```bash
cargo check --manifest-path src-tauri/Cargo.toml
```
预期：编译通过。

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/domain/mod.rs src/types/index.ts
git commit -m "feat: add UpdateInfo type for update check results"
```

---

### Task 4: 创建代理自动检测模块

**Files:**
- Create: `src-tauri/src/updater/proxy.rs`
- Create: `src-tauri/src/updater/mod.rs`（仅模块声明）

- [ ] **Step 1: 创建 updater 模块入口 `src-tauri/src/updater/mod.rs`**

仅声明子模块（后续任务逐步填充）：

```rust
pub mod proxy;
mod commands;
pub use commands::*;
```

- [ ] **Step 2: 创建 `src-tauri/src/updater/proxy.rs`**

```rust
use reqwest::{Client, Proxy};
use std::env;
use std::net::TcpStream;
use std::time::Duration;

/// Known local proxy ports — ordered by priority (most common first)
const PROXY_PORTS: &[(u16, &str)] = &[
    (7890, "Clash"),
    (10809, "Clash Meta"),
    (7891, "Clash (alt)"),
    (1080, "V2Ray/SOCKS5"),
    (10808, "V2Ray HTTP"),
    (8118, "Privoxy"),
    (8080, "Generic HTTP"),
];

/// Build a `reqwest::Client` with proxy auto-detection.
/// Detection order: system env vars → local proxy ports → direct
pub fn build_proxied_client() -> Client {
    let builder = Client::builder()
        .timeout(Duration::from_secs(30))
        .connect_timeout(Duration::from_secs(10));

    // 1. Try system proxy
    if let Some(proxy_url) = system_proxy_url() {
        if let Ok(proxy) = Proxy::all(&proxy_url) {
            log::info!("[updater] Using system proxy: {}", proxy_url);
            return builder.proxy(proxy).build()
                .expect("Failed to build reqwest client with system proxy");
        }
    }

    // 2. Scan local proxy ports
    for &(port, name) in PROXY_PORTS {
        if is_port_open(port) {
            let proxy_url = format!("http://127.0.0.1:{}", port);
            if let Ok(proxy) = Proxy::all(&proxy_url) {
                log::info!("[updater] Using local proxy ({}) at {}", name, proxy_url);
                return builder.proxy(proxy).build()
                    .expect("Failed to build reqwest client with local proxy");
            }
        }
    }

    // 3. Direct connection
    log::info!("[updater] No proxy detected, using direct connection");
    builder.build().expect("Failed to build reqwest client")
}

/// Get system proxy URL from environment variables
fn system_proxy_url() -> Option<String> {
    env::var("HTTPS_PROXY")
        .or_else(|_| env::var("https_proxy"))
        .or_else(|_| env::var("HTTP_PROXY"))
        .or_else(|_| env::var("http_proxy"))
        .ok()
        .filter(|s| !s.is_empty())
}

/// Check if a TCP port is open on localhost (1s timeout)
fn is_port_open(port: u16) -> bool {
    TcpStream::connect_timeout(
        &format!("127.0.0.1:{}", port).parse().unwrap(),
        Duration::from_secs(1),
    )
    .is_ok()
}

/// Detect the first available proxy URL (for diagnostics/logging)
pub fn detect_proxy_url() -> Option<String> {
    system_proxy_url().or_else(|| {
        PROXY_PORTS.iter().find_map(|&(port, name)| {
            if is_port_open(port) {
                Some(format!("http://127.0.0.1:{} ({})", port, name))
            } else {
                None
            }
        })
    })
}
```

- [ ] **Step 3: 验证编译**

```bash
cargo check --manifest-path src-tauri/Cargo.toml
```
预期：编译通过。

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/updater/
git commit -m "feat: add proxy auto-detection module for updater"
```

---

### Task 5: 创建更新编排 commands

**Files:**
- Create: `src-tauri/src/updater/commands.rs`
- Modify: `src-tauri/src/updater/mod.rs`

- [ ] **Step 1: 更新 `src-tauri/src/updater/mod.rs`**

```rust
pub mod proxy;
pub mod commands;
pub use commands::*;
```

- [ ] **Step 2: 创建 `src-tauri/src/updater/commands.rs`**

```rust
use crate::datasource::market_clock::MarketSession;
use crate::domain::UpdateInfo;
use semver::Version;
use tauri::{AppHandle, Emitter};
use tauri_plugin_updater::UpdaterExt;

/// Check for update. Returns UpdateInfo if a newer version is available,
/// or null if the current version is already the latest.
#[tauri::command]
pub async fn check_update(app: AppHandle) -> Result<Option<UpdateInfo>, String> {
    let current_version = app
        .config()
        .version
        .clone()
        .unwrap_or_else(|| env!("CARGO_PKG_VERSION").to_string());

    let updater = app
        .updater()
        .map_err(|e| format!("Updater init failed: {}", e))?;

    let Some(update) = updater
        .check()
        .await
        .map_err(|e| format!("Update check failed: {}", e))?
    else {
        log::info!("[updater] No update available (current: {})", current_version);
        return Ok(None);
    };

    let latest_version = update.version.clone();
    log::info!(
        "[updater] Update found: {} -> {}",
        current_version,
        latest_version
    );

    // Parse versions for comparison
    let cur_ver = parse_semver(&current_version);
    let new_ver = parse_semver(&latest_version);

    if new_ver <= cur_ver {
        log::info!("[updater] Remote version {} is not newer than current {}", latest_version, current_version);
        return Ok(None);
    }

    let info = UpdateInfo {
        current_version,
        latest_version: latest_version.clone(),
        release_date: update.date.clone().unwrap_or_default(),
        notes: update.body.clone().unwrap_or_default(),
        release_url: format!(
            "https://github.com/Leaderxin/QuantDesktopRelease/releases/tag/v{}",
            latest_version
        ),
        download_size: None, // populated during download
    };

    Ok(Some(info))
}

/// Download and install the update. The updater plugin handles
/// downloading, signature verification, and launching the installer.
#[tauri::command]
pub async fn install_update(app: AppHandle) -> Result<(), String> {
    let updater = app
        .updater()
        .map_err(|e| format!("Updater init failed: {}", e))?;

    let Some(update) = updater
        .check()
        .await
        .map_err(|e| format!("Update check failed: {}", e))?
    else {
        return Err("No update available".into());
    };

    log::info!("[updater] Downloading update v{}...", update.version);

    // Emit progress events during download
    let handle = app.clone();
    update
        .on_before_exit(move || {
            log::info!("[updater] Installer launched, exiting app");
            // Give installer a moment to start before exit
            std::process::exit(0);
        })
        .download_and_install(
            |downloaded, total| {
                let percent = if total > 0 {
                    ((downloaded as f64 / total as f64) * 100.0) as u32
                } else {
                    0
                };
                let _ = handle.emit("update-download-progress", serde_json::json!({
                    "downloaded": downloaded,
                    "total": total,
                    "percent": percent.min(100),
                }));
            },
            || {
                log::info!("[updater] Download complete, launching installer");
            },
        )
        .await
        .map_err(|e| format!("Download/install failed: {}", e))?;

    Ok(())
}

#[tauri::command]
pub fn is_trading_session() -> bool {
    let session = MarketSession::current();
    matches!(session, MarketSession::MorningTrade | MarketSession::AfternoonTrade)
}

/// Parse a version string like "1.1.1" or "v1.1.1" into semver::Version
fn parse_semver(s: &str) -> Version {
    let s = s.strip_prefix('v').unwrap_or(s);
    Version::parse(s).unwrap_or_else(|_| Version::new(0, 0, 0))
}
```

- [ ] **Step 3: 添加 `market_clock` 模块可见性**

确保 `src-tauri/src/datasource/mod.rs` 中 `market_clock` 是 `pub mod`：

```rust
pub mod market_clock;
```

- [ ] **Step 4: 验证编译**

```bash
cargo check --manifest-path src-tauri/Cargo.toml
```
预期：编译通过。

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/updater/commands.rs src-tauri/src/updater/mod.rs
git commit -m "feat: add updater commands (check_update, install_update, is_trading_session)"
```

---

### Task 6: 集成到 lib.rs

**Files:**
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: 声明 updater 模块**

在 `src-tauri/src/lib.rs` 的模块声明区（第 1-5 行）中添加：

```rust
pub mod domain;
pub mod db;
pub mod datasource;
pub mod cache;
pub mod commands;
pub mod updater;  // <-- 新增
```

- [ ] **Step 2: 注册 updater plugin**

在 `tauri::Builder::default()` 的 plugin 注册链（第 21-24 行）中添加：

```rust
.plugin(tauri_plugin_opener::init())
.plugin(tauri_plugin_autostart::init(
    tauri_plugin_autostart::MacosLauncher::LaunchAgent,
    None::<Vec<&str>>,
))
.plugin(tauri_plugin_updater::Builder::new()
    .endpoints(&[
        "https://github.com/Leaderxin/QuantDesktopRelease/releases/latest/download/latest.json"
    ])
    .build())  // <-- 新增
```

- [ ] **Step 3: 注册新命令**

在 `invoke_handler` 宏中添加三个新命令（在现有命令列表末尾）：

```rust
.invoke_handler(tauri::generate_handler![
    // ... 现有命令保持不变 ...
    commands::window::show_main_window,
    updater::commands::check_update,       // <-- 新增
    updater::commands::install_update,     // <-- 新增
    updater::commands::is_trading_session, // <-- 新增
])
```

- [ ] **Step 4: 添加启动后更新检查（非阻塞）**

在 `setup(|app| { ... })` 闭包的末尾，`Ok(())` 之前添加：

```rust
// ── Startup update check (non-blocking) ──
let update_handle = app.handle().clone();
let update_db = db.clone();
tokio::spawn(async move {
    // Small delay to let the UI fully render first
    tokio::time::sleep(std::time::Duration::from_secs(3)).await;

    match updater::commands::check_update(update_handle.clone()).await {
        Ok(Some(info)) => {
            // Backend startup check is for logging only.
            // Frontend handles dialog display via useUpdateCheck composable
            // which gates on is_trading_session() before showing UI.
            log::info!(
                "[updater] Update available: {} -> {}",
                info.current_version,
                info.latest_version
            );
            // Note: we intentionally do NOT emit "update-available" here.
            // The frontend composable calls check_update directly and
            // decides whether to show the dialog based on trading session.
            // The "update-available" event is reserved for manual triggers
            // (tray menu, settings button) where the user explicitly asked.
        }
        Ok(None) => {
            log::info!("[updater] App is up to date");
        }
        Err(e) => {
            log::warn!("[updater] Startup check failed (non-critical): {}", e);
        }
    }
});
```

- [ ] **Step 5: 验证编译**

```bash
cargo check --manifest-path src-tauri/Cargo.toml
```
预期：编译通过。

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/lib.rs
git commit -m "feat: integrate updater plugin and commands into lib.rs"
```

---

### Task 7: 创建 CHANGELOG 提取脚本

**Files:**
- Create: `scripts/extract-changelog.mjs`

- [ ] **Step 1: 创建 `scripts/extract-changelog.mjs`**

```javascript
#!/usr/bin/env node
/**
 * Extract changelog entry for a specific version from CHANGELOG.md.
 *
 * Usage:
 *   node scripts/extract-changelog.mjs v1.2.0
 *
 * Expected CHANGELOG.md format (Keep a Changelog style):
 *   ## v1.2.0 (2026-06-20)
 *   ### Added
 *   - item 1
 *   - item 2
 *   ### Fixed
 *   - item 3
 *
 *   ## v1.1.1 (2026-06-15)
 *   ...
 */

import { readFileSync } from 'node:fs';
import { resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const version = process.argv[2];

if (!version) {
  console.error('Usage: node scripts/extract-changelog.mjs <version>');
  console.error('Example: node scripts/extract-changelog.mjs v1.2.0');
  process.exit(1);
}

// Normalize: accept both "1.2.0" and "v1.2.0"
const tag = version.startsWith('v') ? version : `v${version}`;
const altTag = version.startsWith('v') ? version.slice(1) : version;

const changelogPath = resolve(__dirname, '..', 'CHANGELOG.md');

let content;
try {
  content = readFileSync(changelogPath, 'utf-8');
} catch {
  console.error(`CHANGELOG.md not found at ${changelogPath}`);
  process.exit(1);
}

// Match the version section header
// e.g. "## v1.2.0" or "## 1.2.0" optionally followed by date
const versionRegex = new RegExp(
  `^##\\s+(${escapeRegex(tag)}|${escapeRegex(altTag)})\\b`,
  'm'
);
const match = content.match(versionRegex);

if (!match) {
  console.error(`Version ${tag} not found in CHANGELOG.md`);
  process.exit(1);
}

const startIndex = match.index;
// Find the next version header
const nextVersionMatch = content
  .slice(startIndex + match[0].length)
  .match(/^##\s+v?\d+\.\d+\.\d+/m);

const endIndex = nextVersionMatch
  ? startIndex + match[0].length + nextVersionMatch.index
  : content.length;

const entry = content.slice(startIndex, endIndex).trim();

if (!entry) {
  console.error(`Empty changelog entry for ${tag}`);
  process.exit(1);
}

// Output the extracted entry
process.stdout.write(entry + '\n');

function escapeRegex(s) {
  return s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}
```

- [ ] **Step 2: 验证脚本可执行**

```bash
node scripts/extract-changelog.mjs --help 2>&1 || true
```
预期：显示 Usage 信息。

- [ ] **Step 3: Commit**

```bash
git add scripts/extract-changelog.mjs
git commit -m "feat: add changelog extraction script for CI"
```

---

### Task 8: 改造 CI 工作流

**Files:**
- Modify: `.github/workflows/release.yml`

- [ ] **Step 1: 在 `release` job 之前插入 `sign-and-publish` job**

在 `.github/workflows/release.yml` 中，`release` job（第 110 行附近）之前插入：

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

      - name: Setup Node.js
        uses: actions/setup-node@v4
        with:
          node-version: 22

      - name: Extract changelog for this version
        run: |
          VERSION="${{ github.ref_name }}"
          node scripts/extract-changelog.mjs "$VERSION" > changelog-notes.md
          echo "=== Changelog ==="
          cat changelog-notes.md

      - name: Flatten installer files
        run: |
          mkdir -p release
          find . -type f \( \
            -name "*.exe" -o -name "*.msi" -o \
            -name "*.dmg" -o \
            -name "*.deb" -o -name "*.AppImage" \
          \) -exec cp {} release/ \;
          ls -la release/

      - name: Sign and generate latest.json
        env:
          TAURI_PRIVATE_KEY: ${{ secrets.TAURI_PRIVATE_KEY }}
          TAURI_KEY_PASSWORD: ${{ secrets.TAURI_KEY_PASSWORD }}
        run: |
          VERSION="${{ github.ref_name }}"
          NOTES=$(jq -Rs '.' changelog-notes.md)
          echo "VERSION=$VERSION"
          echo "NOTES=$NOTES"

          # Use tauri CLI signer if available, otherwise manual approach
          npx tauri signer sign \
            --private-key <(echo "$TAURI_PRIVATE_KEY") \
            --password "$TAURI_KEY_PASSWORD" \
            --generate-latest-json \
            --version "$VERSION" \
            --notes "$NOTES" \
            release/ \
            -o latest.json

          echo "=== latest.json ==="
          cat latest.json

      - name: Publish to public release repo
        uses: softprops/action-gh-release@v2
        with:
          repository: Leaderxin/QuantDesktopRelease
          token: ${{ secrets.PUBLIC_REPO_PAT }}
          files: |
            release/*
            latest.json
          generate_release_notes: true
```

- [ ] **Step 2: 验证 YAML 语法**

```bash
# 使用任意 YAML linter 或在 GitHub Actions UI 中检查
cat .github/workflows/release.yml | python3 -c "import sys,yaml; yaml.safe_load(sys.stdin); print('YAML valid')"
```
预期：`YAML valid`

- [ ] **Step 3: Commit**

```bash
git add .github/workflows/release.yml
git commit -m "ci: add sign-and-publish job for auto-update to public release repo"
```

---

### Task 9: 创建前端 updater store

**Files:**
- Create: `src/stores/updater.ts`

- [ ] **Step 1: 创建 `src/stores/updater.ts`**

```typescript
// src/stores/updater.ts
import { defineStore } from 'pinia';
import { ref, computed } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type { UpdateInfo } from '@/types';

export const useUpdaterStore = defineStore('updater', () => {
  const updateStatus = ref<'idle' | 'checking' | 'available' | 'downloading' | 'ready' | 'error'>('idle');
  const updateInfo = ref<UpdateInfo | null>(null);
  const downloadProgress = ref(0);
  const downloadedBytes = ref(0);
  const totalBytes = ref(0);
  const lastCheckTime = ref('');
  const nextReminderTime = ref('');
  const errorMessage = ref('');
  const dialogVisible = ref(false);

  const hasUpdate = computed(() => updateStatus.value === 'available');
  const isDownloading = computed(() => updateStatus.value === 'downloading');

  async function checkForUpdate(): Promise<UpdateInfo | null> {
    updateStatus.value = 'checking';
    errorMessage.value = '';
    try {
      const result = await invoke<UpdateInfo | null>('check_update');
      if (result) {
        updateStatus.value = 'available';
        updateInfo.value = result;
        lastCheckTime.value = new Date().toISOString();
        return result;
      } else {
        updateStatus.value = 'idle';
        lastCheckTime.value = new Date().toISOString();
        return null;
      }
    } catch (e) {
      updateStatus.value = 'error';
      errorMessage.value = String(e).slice(0, 200);
      console.error('[updater] checkForUpdate failed:', e);
      return null;
    }
  }

  async function downloadAndInstall() {
    if (!updateInfo.value) return;
    updateStatus.value = 'downloading';
    downloadProgress.value = 0;
    errorMessage.value = '';

    // Listen for download progress events
    const unlisten = await listen<{ downloaded: number; total: number; percent: number }>(
      'update-download-progress',
      (event) => {
        downloadProgress.value = event.payload.percent;
        downloadedBytes.value = event.payload.downloaded;
        totalBytes.value = event.payload.total;
      }
    );

    try {
      await invoke('install_update');
      updateStatus.value = 'ready';
      downloadProgress.value = 100;
    } catch (e) {
      updateStatus.value = 'error';
      errorMessage.value = String(e).slice(0, 200);
      console.error('[updater] downloadAndInstall failed:', e);
    } finally {
      unlisten();
    }
  }

  function dismissUpdate() {
    // Set 24-hour cooldown
    const next = new Date();
    next.setHours(next.getHours() + 24);
    nextReminderTime.value = next.toISOString();
    dialogVisible.value = false;
  }

  function canRemind(): boolean {
    if (!nextReminderTime.value) return true;
    return new Date() >= new Date(nextReminderTime.value);
  }

  async function openReleasePage() {
    if (!updateInfo.value?.release_url) return;
    const { openUrl } = await import('@tauri-apps/plugin-opener');
    await openUrl(updateInfo.value.release_url);
  }

  function showDialog() {
    if (updateStatus.value === 'available' && canRemind()) {
      dialogVisible.value = true;
    }
  }

  function reset() {
    updateStatus.value = 'idle';
    updateInfo.value = null;
    errorMessage.value = '';
    downloadProgress.value = 0;
  }

  return {
    updateStatus,
    updateInfo,
    downloadProgress,
    downloadedBytes,
    totalBytes,
    lastCheckTime,
    nextReminderTime,
    errorMessage,
    dialogVisible,
    hasUpdate,
    isDownloading,
    checkForUpdate,
    downloadAndInstall,
    dismissUpdate,
    canRemind,
    openReleasePage,
    showDialog,
    reset,
  };
});
```

- [ ] **Step 2: 验证 TypeScript 编译**

```bash
npx vue-tsc --noEmit src/stores/updater.ts 2>&1 | head -20
```
预期：无类型错误。

- [ ] **Step 3: Commit**

```bash
git add src/stores/updater.ts
git commit -m "feat: add updater Pinia store"
```

---

### Task 10: 创建 useUpdateCheck composable

**Files:**
- Create: `src/composables/useUpdateCheck.ts`

- [ ] **Step 1: 创建 `src/composables/useUpdateCheck.ts`**

```typescript
// src/composables/useUpdateCheck.ts
import { invoke } from '@tauri-apps/api/core';
import { useUpdaterStore } from '@/stores/updater';

export function useUpdateCheck() {
  const updater = useUpdaterStore();

  async function performStartupCheck() {
    const info = await updater.checkForUpdate();
    if (!info) return; // Already up to date

    // Check if we should suppress the dialog (trading session)
    try {
      const isTrading = await invoke<boolean>('is_trading_session');
      if (isTrading) {
        console.log(
          `[updater] Update ${info.latest_version} available but suppressing during trading hours`
        );
        // The 'update-available' event was already emitted by backend;
        // frontend receives it but doesn't auto-show dialog during trading.
        // The tray menu / settings button can still trigger manual check.
        return;
      }
    } catch {
      // If is_trading_session call fails, show dialog anyway (safe default)
      console.warn('[updater] is_trading_session check failed, showing dialog');
    }

    // Show the update dialog
    updater.showDialog();
  }

  async function manualCheck() {
    const info = await updater.checkForUpdate();
    if (info) {
      updater.showDialog();
    } else if (updater.updateStatus === 'idle') {
      return false; // No update
    }
    return true;
  }

  return { performStartupCheck, manualCheck };
}
```

- [ ] **Step 2: 验证 TypeScript 编译**

```bash
npx vue-tsc --noEmit
```
预期：无类型错误。

- [ ] **Step 3: Commit**

```bash
git add src/composables/useUpdateCheck.ts
git commit -m "feat: add useUpdateCheck composable with trading-session gating"
```

---

### Task 11: 创建 UpdateDialog 组件

**Files:**
- Create: `src/components/updater/UpdateDialog.vue`

- [ ] **Step 1: 创建 `src/components/updater/UpdateDialog.vue`**

```vue
<script setup lang="ts">
import { computed } from 'vue';
import { NModal, NCard, NButton, NProgress, NSpace, NScrollbar, NDivider } from 'naive-ui';
import { useUpdaterStore } from '@/stores/updater';

const updater = useUpdaterStore();

const formattedNotes = computed(() => {
  if (!updater.updateInfo?.notes) return [];
  return renderMarkdownLines(updater.updateInfo.notes);
});

const progressStatus = computed(() => {
  if (updater.errorMessage) return 'error';
  if (updater.updateStatus === 'ready') return 'success';
  return undefined;
});

const downloadLabel = computed(() => {
  if (updater.updateStatus === 'downloading') {
    const dl = formatBytes(updater.downloadedBytes);
    const tot = formatBytes(updater.totalBytes);
    return tot ? `正在下载 ${dl} / ${tot}` : '正在下载...';
  }
  if (updater.updateStatus === 'ready') return '下载完成';
  return '';
});

function formatBytes(bytes: number): string {
  if (!bytes || bytes <= 0) return '';
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(0)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

function handleUpdate() {
  updater.downloadAndInstall();
}

function handleLater() {
  updater.dismissUpdate();
}

function handleViewOnGitHub() {
  updater.openReleasePage();
}

function handleRetry() {
  updater.reset();
  updater.checkForUpdate().then((info) => {
    if (info) updater.showDialog();
  });
}

/**
 * Lightweight Markdown renderer — zero dependencies.
 * Converts Keep a Changelog style markdown to structured sections.
 */
interface ChangelogSection {
  title: string;
  items: string[];
}

function renderMarkdownLines(raw: string): ChangelogSection[] {
  const sections: ChangelogSection[] = [];
  const lines = raw.split('\n');
  let currentSection: ChangelogSection | null = null;

  // Skip the version header line (e.g. "## v1.2.0")
  const skipLine = (i: number, line: string): boolean => {
    if (i === 0 && /^##\s+v?\d/.test(line)) return true;
    if (i === 1 && /^\d{4}-\d{2}-\d{2}/.test(line.trim())) return true;
    return false;
  };

  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];
    if (skipLine(i, line)) continue;

    // Section header: "### ..."
    const h3Match = line.match(/^###\s+(.+)/);
    if (h3Match) {
      currentSection = { title: h3Match[1].trim(), items: [] };
      sections.push(currentSection);
      continue;
    }

    // List item: "- ..."
    const liMatch = line.match(/^[-*]\s+(.+)/);
    if (liMatch && currentSection) {
      currentSection.items.push(liMatch[1].trim());
      continue;
    }
  }

  return sections;
}
</script>

<template>
  <NModal
    :show="updater.dialogVisible"
    :mask-closable="updater.updateStatus !== 'downloading'"
    @update:show="(v: boolean) => { if (!v) updater.dismissUpdate(); }"
  >
    <NCard
      style="width: 480px; max-width: 90vw;"
      :bordered="false"
      closable
      @close="updater.dismissUpdate()"
    >
      <template #header>
        <div class="dialog-header">
          <span class="version-badge">
            <svg viewBox="0 0 16 16" width="14" height="14" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" aria-hidden="true">
              <circle cx="8" cy="8" r="6"/>
              <path d="M8 4v4l2.5 2"/>
            </svg>
            发现新版本
          </span>
        </div>
      </template>

      <NSpace vertical :size="16">
        <!-- Version comparison -->
        <div class="version-compare">
          <span class="ver-current">{{ updater.updateInfo?.current_version }}</span>
          <svg viewBox="0 0 16 16" width="16" height="16" fill="none" aria-hidden="true" class="ver-arrow">
            <path d="M3 8h10M11 4l4 4-4 4" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
          </svg>
          <span class="ver-latest">{{ updater.updateInfo?.latest_version }}</span>
          <span class="ver-date tabular-nums">
            <svg viewBox="0 0 16 16" width="12" height="12" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" aria-hidden="true" class="date-icon">
              <rect x="2" y="3" width="12" height="11" rx="1"/>
              <path d="M5 1v3M11 1v3M2 6h12"/>
            </svg>
            {{ updater.updateInfo?.release_date || '--' }}
          </span>
        </div>

        <!-- Changelog -->
        <div class="changelog-section">
          <div class="changelog-title">更新内容</div>
          <div class="changelog-box">
            <NScrollbar style="max-height: 220px">
              <div v-if="formattedNotes.length === 0" class="changelog-empty">
                暂无更新说明
              </div>
              <div v-for="(section, si) in formattedNotes" :key="si" class="changelog-section-item">
                <h4 class="changelog-h4">{{ section.title }}</h4>
                <ul class="changelog-list">
                  <li v-for="(item, ii) in section.items" :key="ii" class="changelog-li">
                    {{ item }}
                  </li>
                </ul>
              </div>
            </NScrollbar>
          </div>
        </div>

        <!-- Download progress -->
        <div v-if="updater.updateStatus === 'downloading' || updater.updateStatus === 'error' || updater.updateStatus === 'ready'" class="progress-section">
          <NProgress
            :percentage="updater.downloadProgress"
            :status="progressStatus"
            :show-indicator="false"
            :height="6"
            :border-radius="3"
          />
          <div class="progress-info" :class="{ 'progress-error': updater.updateStatus === 'error' }">
            <template v-if="updater.updateStatus === 'error'">
              <span class="error-text">{{ updater.errorMessage || '下载失败' }}</span>
            </template>
            <template v-else>
              <span>{{ downloadLabel }}</span>
              <span class="tabular-nums">{{ updater.downloadProgress }}%</span>
            </template>
          </div>
        </div>

        <NDivider style="margin: 0" />

        <!-- Footer actions -->
        <div class="dialog-footer">
          <NButton
            text
            size="small"
            @click="handleViewOnGitHub"
          >
            在 GitHub 查看详情
          </NButton>

          <NSpace :size="8">
            <NButton
              v-if="updater.updateStatus !== 'error'"
              size="medium"
              :disabled="updater.updateStatus === 'downloading'"
              @click="handleLater"
            >
              稍后提醒
            </NButton>

            <NButton
              v-if="updater.updateStatus === 'available' || updater.updateStatus === 'downloading'"
              type="primary"
              size="medium"
              :loading="updater.updateStatus === 'downloading'"
              @click="handleUpdate"
            >
              立即更新 {{ updater.updateInfo?.latest_version }}
            </NButton>

            <NButton
              v-else-if="updater.updateStatus === 'error'"
              type="warning"
              size="medium"
              @click="handleRetry"
            >
              重试
            </NButton>

            <NButton
              v-else-if="updater.updateStatus === 'ready'"
              type="success"
              size="medium"
            >
              即将安装...
            </NButton>
          </NSpace>
        </div>
      </NSpace>
    </NCard>
  </NModal>
</template>

<style scoped>
/* ── Header ── */
.dialog-header {
  display: flex;
  align-items: center;
}

.version-badge {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 2px 10px;
  border-radius: var(--radius-sm);
  background: var(--color-accent-dim);
  color: var(--color-accent);
  font-size: var(--text-xs);
  font-weight: var(--font-weight-medium);
}

/* ── Version comparison ── */
.version-compare {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  padding: var(--space-3);
  background: var(--color-surface-2);
  border-radius: var(--radius-md);
  border: 1px solid var(--color-border-0);
}

.ver-current {
  font-family: var(--font-mono);
  font-size: var(--text-sm);
  color: var(--color-text-tertiary);
}

.ver-arrow {
  color: var(--color-accent);
  flex-shrink: 0;
}

.ver-latest {
  font-family: var(--font-mono);
  font-size: var(--text-lg);
  font-weight: var(--font-weight-semibold);
  color: var(--color-accent);
}

.ver-date {
  margin-left: auto;
  display: flex;
  align-items: center;
  gap: 4px;
  font-size: var(--text-xs);
  color: var(--color-text-tertiary);
}

.date-icon {
  color: var(--color-text-tertiary);
}

/* ── Changelog ── */
.changelog-section {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.changelog-title {
  font-size: var(--text-sm);
  font-weight: var(--font-weight-medium);
  color: var(--color-text-secondary);
}

.changelog-box {
  background: var(--color-surface-2);
  border: 1px solid var(--color-border-0);
  border-radius: var(--radius-md);
  padding: var(--space-3);
}

.changelog-empty {
  color: var(--color-text-tertiary);
  font-size: var(--text-sm);
  text-align: center;
  padding: var(--space-4);
}

.changelog-section-item {
  margin-bottom: var(--space-3);
}

.changelog-section-item:last-child {
  margin-bottom: 0;
}

.changelog-h4 {
  font-size: var(--text-sm);
  font-weight: var(--font-weight-semibold);
  color: var(--color-text-primary);
  margin: 0 0 var(--space-1) 0;
  padding-left: var(--space-2);
  border-left: 2px solid var(--color-accent);
  line-height: 1.4;
}

.changelog-list {
  list-style: none;
  padding: 0;
  margin: 0;
}

.changelog-li {
  position: relative;
  padding-left: var(--space-4);
  font-size: var(--text-sm);
  color: var(--color-text-secondary);
  line-height: 1.6;
}

.changelog-li::before {
  content: '';
  position: absolute;
  left: 6px;
  top: 10px;
  width: 4px;
  height: 4px;
  border-radius: 50%;
  background: var(--color-text-tertiary);
}

/* ── Progress ── */
.progress-section {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
}

.progress-info {
  display: flex;
  justify-content: space-between;
  font-size: var(--text-xs);
  color: var(--color-text-tertiary);
}

.progress-error {
  color: #f85149;
}

.error-text {
  color: #f85149;
}

/* ── Footer ── */
.dialog-footer {
  display: flex;
  align-items: center;
  justify-content: space-between;
}
</style>
```

- [ ] **Step 2: 验证 TypeScript 编译**

```bash
npx vue-tsc --noEmit
```
预期：无类型错误。

- [ ] **Step 3: Commit**

```bash
git add src/components/updater/UpdateDialog.vue
git commit -m "feat: add UpdateDialog component with changelog rendering"
```

---

### Task 12: 集成到 App.vue

**Files:**
- Modify: `src/App.vue`

- [ ] **Step 1: 在 App.vue 中添加启动检查**

修改 `src/App.vue` 的 `<script setup>` 部分：

在 import 区添加：
```typescript
import { useUpdaterStore } from '@/stores/updater';
import { useUpdateCheck } from '@/composables/useUpdateCheck';
import UpdateDialog from '@/components/updater/UpdateDialog.vue';
```

在 `onMounted` 中的初始化流程末尾添加：
```typescript
// After initReady and settings/watchlist/quote are done:
const { performStartupCheck } = useUpdateCheck();
performStartupCheck();
```

在 template 中，`</NConfigProvider>` 之前添加：
```html
<UpdateDialog />
```

- [ ] **Step 2: 验证完整编译**

```bash
npx vue-tsc --noEmit
```
预期：无类型错误。

- [ ] **Step 3: Commit**

```bash
git add src/App.vue
git commit -m "feat: integrate update check on app startup"
```

---

### Task 13: 添加托盘菜单入口 + 设置面板按钮

**Files:**
- Modify: `src-tauri/src/lib.rs`（托盘菜单）
- Modify: `src/components/layout/AppLayout.vue`（设置面板）

- [ ] **Step 1: 在托盘菜单添加"检查更新"项**

在 `src-tauri/src/lib.rs` 的托盘菜单构建代码中，在 `quit_item` 之前添加：

```rust
let check_update_item = MenuItemBuilder::with_id("check_update", "检查更新").build(app)?;

let menu = MenuBuilder::new(app)
    .item(&show_item)
    .item(&toggle_ticker)
    .separator()
    .item(&check_update_item)   // <-- 新增
    .item(&quit_item)
    .build()?;
```

在托盘菜单事件处理中，`"quit"` 匹配分支之前添加：

```rust
"check_update" => {
    let handle = app.handle().clone();
    tauri::async_runtime::spawn(async move {
        match crate::updater::commands::check_update(handle.clone()).await {
            Ok(Some(info)) => {
                let _ = handle.emit("update-available", &info);
            }
            Ok(None) => {
                log::info!("[updater] Manual check: already up to date");
            }
            Err(e) => {
                log::warn!("[updater] Manual check failed: {}", e);
            }
        }
    });
}
```

- [ ] **Step 2: 前端监听 `update-available` 事件并显示对话框**

在 `src/stores/updater.ts` 中添加事件监听器（在 store setup 函数体内添加，与 `downloadAndInstall()` 中的下载进度监听器并列）。注意：此监听器仅在手动触发（托盘菜单 / 设置面板按钮）时响应，启动自动检查走 `useUpdateCheck` composable 路径：

```typescript
// Listen for manual update triggers (tray menu / settings button)
// Startup auto-check goes through useUpdateCheck composable instead.
listen<UpdateInfo>('update-available', (event) => {
  updateStatus.value = 'available';
  updateInfo.value = event.payload;
  // Manual triggers always show dialog (user explicitly asked)
  dialogVisible.value = true;
}).catch((e) => console.error('[updater] Failed to listen update-available:', e));
```

注意：当收到 `update-available` 事件时，直接显示对话框（包括从托盘菜单手动触发的情况），但需要确保 Trading session 的抑制逻辑只在 `performStartupCheck` 中生效。

调整 `useUpdateCheck.ts` 中的逻辑：
- 启动检查时，先调用 `checkForUpdate()`，再调用 `is_trading_session()`
- 非交易时段才调用 `showDialog()`
- 从 `update-available` 事件直接触发的（托盘菜单）不受抑制

- [ ] **Step 3: 在设置面板/AppLayout 添加版本号和检查按钮**

在 `AppLayout.vue` 底部（或现有的 StatusBar 区域）添加当前版本号显示和手动检查按钮。复用 StatusBar 组件或直接在 AppLayout 底部添加。

如果 `StatusBar.vue` 已存在，修改 `src/components/layout/StatusBar.vue`：

```vue
<script setup lang="ts">
import { useUpdaterStore } from '@/stores/updater';
import { useUpdateCheck } from '@/composables/useUpdateCheck';

const updater = useUpdaterStore();
const { manualCheck } = useUpdateCheck();
</script>

<template>
  <div class="status-bar">
    <span class="status-version tabular-nums">
      v{{ updater.updateInfo?.current_version || '1.1.1' }}
    </span>
    <button class="status-check-btn" @click="manualCheck">
      检查更新
    </button>
  </div>
</template>

<style scoped>
.status-bar {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  padding: 2px var(--space-3);
  border-top: 1px solid var(--color-border-0);
  background: var(--color-surface-1);
  flex-shrink: 0;
}
.status-version {
  font-size: var(--text-xs);
  color: var(--color-text-tertiary);
}
.status-check-btn {
  margin-left: auto;
  font-size: var(--text-xs);
  color: var(--color-text-tertiary);
  background: transparent;
  border: none;
  cursor: pointer;
  transition: color var(--transition-fast);
}
.status-check-btn:hover {
  color: var(--color-accent);
}
</style>
```

- [ ] **Step 4: 验证编译**

```bash
cargo check --manifest-path src-tauri/Cargo.toml
npx vue-tsc --noEmit
```
预期：均通过。

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/lib.rs src/stores/updater.ts src/composables/useUpdateCheck.ts src/components/layout/StatusBar.vue
git commit -m "feat: add tray menu check-update entry and status bar version display"
```

---

### Task 14: 生成签名密钥对

**Files:**
- Modify: `src-tauri/tauri.conf.json`（替换 pubkey 占位符）

- [ ] **Step 1: 生成密钥对**

```bash
npx tauri signer generate -w ~/.tauri/quant-desktop.key
```
预期：生成私钥文件和公钥。输出类似：
```
Public key: dwL0...base64...
Private key saved to ~/.tauri/quant-desktop.key
```

- [ ] **Step 2: 将公钥写入 tauri.conf.json**

复制输出的公钥，替换 `src-tauri/tauri.conf.json` 中 `plugins.updater.pubkey` 的占位符值 `PASTE_YOUR_PUBLIC_KEY_HERE_AFTER_GENERATION`。

- [ ] **Step 3: 将私钥添加到 GitHub Secrets**

在私有仓库 `Leaderxin/quant-desktop` 的 Settings → Secrets and variables → Actions 中添加：
- `TAURI_PRIVATE_KEY`: 私钥文件内容 (`cat ~/.tauri/quant-desktop.key`)

- [ ] **Step 4: 生成 `PUBLIC_REPO_PAT`**

在 GitHub → Settings → Developer settings → Personal access tokens → Fine-grained tokens：
- Repository: `Leaderxin/QuantDesktopRelease`
- Permissions: Contents (Read and Write)
- 将生成的 PAT 添加到私有仓库的 Secrets: `PUBLIC_REPO_PAT`

- [ ] **Step 5: Commit**

```bash
git add src-tauri/tauri.conf.json
git commit -m "chore: add updater public key to tauri.conf.json"
```

---

### Task 15: 端到端验证

- [ ] **Step 1: 本地构建验证**

```bash
npm run tauri:build
```
预期：完整构建成功，无编译/打包错误。

- [ ] **Step 2: 启动应用验证**

```bash
npm run tauri dev
```
预期：
- 应用正常启动
- 3 秒后后台检查更新
- 日志显示 `[updater] App is up to date`（当前已是最新版本时）

- [ ] **Step 3: 模拟有新版本（测试对话框）**

临时修改 `src-tauri/Cargo.toml` 版本为 `0.0.1`，重启应用：
- 对话框应弹出（非交易时段）
- CHANGELOG 渲染正确
- 点击"稍后提醒"关闭
- 24h 内不再弹窗

- [ ] **Step 4: 验证托盘菜单**

右键托盘 → "检查更新"：
- 如果已是最新版本，静默
- 如果有新版本，弹出对话框

- [ ] **Step 5: 测试后恢复版本号**

将 `Cargo.toml` 和 `tauri.conf.json` 版本号恢复为 `1.1.1`。

- [ ] **Step 6: 创建 CHANGELOG.md**

```bash
cat > CHANGELOG.md << 'EOF'
# Changelog

## v1.1.1 (2026-06-20)
### Added
- 版本更新检测及自动更新机制
- 启动时自动检查更新，交易时段智能抑制弹窗
- 更新对话框展示完整 CHANGELOG
- 代理自动检测（Clash/V2Ray/系统代理）
- 托盘菜单"检查更新"入口
### Fixed
- 无

## v1.1.0
### Added
- 版本号更新
### Fixed
- 无
EOF
```

- [ ] **Step 7: Commit**

```bash
git add CHANGELOG.md
git commit -m "docs: add CHANGELOG.md"
```

---

## 实施顺序

```
Phase 1: 基础设施
  Task 1 → Task 2 → Task 3

Phase 2: Rust 后端
  Task 4 → Task 5 → Task 6

Phase 3: CI/CD
  Task 7 → Task 8

Phase 4: 前端
  Task 9 → Task 10 → Task 11 → Task 12 → Task 13

Phase 5: 收尾
  Task 14 → Task 15
```

每个 Phase 内部有依赖顺序，Phase 之间尽量独立。Phase 4（前端）可以部分和 Phase 2（Rust 后端）并行开发。

---

## 关键检查点

| 检查点 | 验证方法 |
|--------|---------|
| Cargo check 通过 | `cargo check --manifest-path src-tauri/Cargo.toml` |
| vue-tsc 通过 | `npx vue-tsc --noEmit` |
| 完整构建通过 | `npm run tauri:build` |
| 版本检查正常 | 启动应用，查看日志 `[updater]` |
| 对话框渲染 | 模拟低版本号触发 |
| 代理检测 | 开启/关闭 Clash 验证日志 |
