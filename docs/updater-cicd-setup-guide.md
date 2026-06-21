# 自动更新 CI/CD 配置指南

> 本文档为 QuantDesktop 自动更新机制的完整部署配置流程，涵盖私有仓库和公开镜像仓库所需的所有配置。

---

## 架构回顾

```
Private Repo (quant-desktop)           Public Repo (QuantDesktopRelease)
┌──────────────────────────┐           ┌──────────────────────────────┐
│ CI (v* tag)              │           │ GitHub Releases              │
│  1. build (Win/Mac/Linux)│           │  ├── *.exe / *.msi / *.dmg   │
│  2. sign + latest.json   │──push──→  │  ├── latest.json (签名)      │
│  3. publish to public    │           │  └── *.deb / *.AppImage       │
└──────────────────────────┘           └──────────────┬───────────────┘
                                                      │ HTTPS (公开，无需认证)
                                                      ▼
                                              QuantDesktop App
                                              (检查更新 → 下载 → 安装)
```

---

## 第一步：公开仓库准备

### 1.1 确认公开仓库存在

打开 `https://github.com/Leaderxin/QuantDesktopRelease`，确保仓库已创建且为 **Public**。

> 如果仓库不存在，在 GitHub 首页点 **New** → Repository name: `QuantDesktopRelease` → 选 **Public** → Create。

---

## 第二步：创建 Personal Access Token（PAT）

Token 用于 CI 向公开仓库推送 Release。

### 2.1 进入 Token 创建页面

```
https://github.com/settings/personal-access-tokens/new
```

或者手动：右上角头像 → **Settings** → 左侧 **Developer settings** → **Personal access tokens** → **Fine-grained tokens** → **Generate new token**

### 2.2 填写 Token 配置

| 字段 | 值 |
|------|-----|
| **Token name** | `QuantDesktop Release Publisher` |
| **Expiration** | 选一个合理时间（推荐 1 year，到期需重新生成） |
| **Resource owner** | `Leaderxin` |
| **Repository access** | **Only select repositories** |

在下方仓库列表中选择 `Leaderxin/QuantDesktopRelease`。

展开 **Repository permissions**，只需勾选：

| Permission | Access |
|------------|--------|
| **Contents** | **Read and Write** |

其余所有权限保持 **No access**（默认），不需要勾选任何其他项。

### 2.3 生成并保存

点击底部 **Generate token**，页面会显示 `github_pat_11A...` 开头的 Token。

> ⚠️ 这个 Token **只显示一次**，离开页面后将无法再次查看。立即复制保存到安全位置。

---

## 第三步：生成签名密钥对

在项目根目录执行：

```bash
npx tauri signer generate \
  -w ~/.tauri/quant-desktop.key \
  -p "786541437" \
  --ci \
  --force
```

### 3.1 密钥文件说明

| 文件 | 用途 | 存放位置 |
|------|------|---------|
| `~/.tauri/quant-desktop.key` | 私钥（加密） | 本地 + GitHub Secret |
| `~/.tauri/quant-desktop.key.pub` | 公钥 | 已写入 `src-tauri/tauri.conf.json` |

### 3.2 查看私钥内容（后续需要粘贴到 GitHub Secret）

**Windows PowerShell:**
```powershell
Get-Content ~/.tauri/quant-desktop.key
```

**Git Bash:**
```bash
cat ~/.tauri/quant-desktop.key
```

复制输出的全部内容（包括 `---BEGIN...` 到 `...END---` 整段）。

---

## 第四步：私有仓库添加 GitHub Secrets

### 4.1 打开 Secrets 配置页面

```
https://github.com/Leaderxin/quant-desktop/settings/secrets/actions
```

或者手动：打开 `https://github.com/Leaderxin/quant-desktop` → 顶部 **Settings** tab → 左侧 **Security** → **Secrets and variables** → **Actions**

> 注意：是仓库顶部的 Settings，不是右上角头像的个人 Settings。

### 4.2 添加 Secret：TAURI_PRIVATE_KEY

点击绿色的 **New repository secret** 按钮：

| 字段 | 值 |
|------|-----|
| **Name** | `TAURI_PRIVATE_KEY` |
| **Secret** | 粘贴第三步中 `cat ~/.tauri/quant-desktop.key` 输出的全部内容 |

### 4.3 添加 Secret：TAURI_KEY_PASSWORD

再次点击 **New repository secret**：

| 字段 | 值 |
|------|-----|
| **Name** | `TAURI_KEY_PASSWORD` |
| **Secret** | `786541437` |

### 4.4 添加 Secret：PUBLIC_REPO_PAT

再次点击 **New repository secret**：

| 字段 | 值 |
|------|-----|
| **Name** | `PUBLIC_REPO_PAT` |
| **Secret** | 粘贴第二步保存的 `github_pat_11A...` Token |

### 4.5 验证 Secrets 列表

配置完成后，页面应显示 3 个 Secret：

| Name | 状态 |
|------|------|
| `TAURI_PRIVATE_KEY` | ✅ |
| `TAURI_KEY_PASSWORD` | ✅ |
| `PUBLIC_REPO_PAT` | ✅ |

---

## 第五步：验证配置（首次构建）

### 5.1 更新 CHANGELOG.md

在项目根目录的 `CHANGELOG.md` 中添加新版本记录：

```markdown
## v1.2.0 (2026-06-21)
### Added
- 版本更新检测及自动更新机制
- 启动时自动检查更新，交易时段智能抑制弹窗
- 更新对话框展示完整 CHANGELOG
- 代理自动检测（Clash/V2Ray/系统代理）
- 托盘菜单"检查更新"入口
### Fixed
- 修复已知问题，优化性能
```

### 5.2 推送 tag 触发构建

```bash
git add CHANGELOG.md
git commit -m "docs: update CHANGELOG for v1.2.0"
git tag v1.2.0
git push origin master
git push origin v1.2.0
```

### 5.3 监控 CI 运行

打开 `https://github.com/Leaderxin/quant-desktop/actions`，查看 `Release` workflow 运行状态。

构建流程：
1. **build** job — Windows / macOS / Linux 三平台构建
2. **sign-and-publish** job — 签名 + 生成 `latest.json` + 推送到公开仓库
3. **release** job（原有）— 私有仓库 Release 创建

### 5.4 验证公开仓库 Release

打开 `https://github.com/Leaderxin/QuantDesktopRelease/releases`，确认：
- 新 Release `v1.2.0` 已创建
- 安装包文件（`.exe`/`.msi`/`.dmg` 等）已上传
- `latest.json` 已上传

### 5.5 验证 latest.json 内容

下载并查看 `latest.json`，确认格式正确：

```json
{
  "version": "1.2.0",
  "notes": "### Added\n- 版本更新检测...",
  "pub_date": "2026-06-21T...",
  "platforms": {
    "windows-x86_64": {
      "signature": "...",
      "url": "https://github.com/Leaderxin/QuantDesktopRelease/releases/download/v1.2.0/..."
    },
    ...
  }
}
```

---

## 故障排查

### CI sign-and-publish 失败

| 错误信息 | 可能原因 | 解决 |
|---------|---------|------|
| `TAURI_PRIVATE_KEY not set` | Secret 未添加或名称不对 | 检查 Secret Name 是否完全一致 |
| `Bad password` | 密钥密码错误 | 确认 `TAURI_KEY_PASSWORD` 值与生成时一致 |
| `403 Forbidden` (推送公开仓库) | PAT 权限不足 | 检查 PAT 是否对 `QuantDesktopRelease` 有 Contents Write 权限 |
| `Repository not found` | 公开仓库不存在 | 检查 `QuantDesktopRelease` 仓库是否已创建且为 Public |

### 应用端检查不到更新

| 现象 | 可能原因 | 解决 |
|------|---------|------|
| 始终显示"已是最新版本" | `latest.json` 未生成或路径不对 | 检查公开仓库 Release 是否包含 `latest.json` |
| 下载失败 | 签名验证不通过 | 确认 `tauri.conf.json` 中 `pubkey` 与私钥匹配 |
| 国内网络无法下载 | GitHub 访问受限 | 确保已开启 Clash/V2Ray 代理 |

---

## 后续维护

### 发布新版本流程

```bash
# 1. 更新版本号
# 编辑 src-tauri/Cargo.toml 和 src-tauri/tauri.conf.json 中的 version 字段

# 2. 更新 CHANGELOG.md

# 3. 提交 + 打 tag
git add .
git commit -m "chore: bump version to v1.x.0"
git tag v1.x.0
git push origin master
git push origin v1.x.0
```

### Token 过期处理

PAT 过期后，需要：
1. 按第二步重新生成 PAT
2. 按第四步 4.4 更新 `PUBLIC_REPO_PAT` Secret

---

## 配置清单

| 序号 | 操作 | 位置 | 状态 |
|------|------|------|------|
| 1 | 创建公开仓库 `QuantDesktopRelease` | GitHub | ☐ |
| 2 | 生成 Fine-grained PAT（Contents R/W） | GitHub Settings → Developer settings | ☐ |
| 3 | 生成签名密钥对 | 本地 `npx tauri signer generate` | ☐ |
| 4 | 添加 Secret `TAURI_PRIVATE_KEY` | 私有仓库 Settings → Secrets → Actions | ☐ |
| 5 | 添加 Secret `TAURI_KEY_PASSWORD` | 私有仓库 Settings → Secrets → Actions | ☐ |
| 6 | 添加 Secret `PUBLIC_REPO_PAT` | 私有仓库 Settings → Secrets → Actions | ☐ |
| 7 | 更新 `CHANGELOG.md` | 项目根目录 | ☐ |
| 8 | 推送 `v*` tag 触发首次构建 | `git tag v1.2.0 && git push origin v1.2.0` | ☐ |
| 9 | 验证公开仓库 Release | `https://github.com/Leaderxin/QuantDesktopRelease/releases` | ☐ |
