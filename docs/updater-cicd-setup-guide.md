# 自动更新 CI/CD 配置指南

> QuantDesktop 仓库已设为 Public，更新检测和下载均直接使用 GitHub Releases，无需镜像仓库。

---

## 架构

```
quant-desktop (Public Repo)
┌──────────────────────────────────────────────┐
│ CI (v* tag)                                  │
│  1. build (Win/Mac/Linux 矩阵)               │
│  2. release: sign + latest.json + 创建 Release │
└──────────────────────┬───────────────────────┘
                       │ GitHub Releases API (公开，无需认证)
                       ▼
               QuantDesktop App
               (检查更新 → 下载 → 安装)
```

---

## 第一步：生成签名密钥对

在项目根目录执行：

```bash
npx tauri signer generate \
  -w ~/.tauri/quant-desktop.key \
  -p "786541437" \
  --ci \
  --force
```

### 密钥文件说明

| 文件 | 用途 | 存放位置 |
|------|------|---------|
| `~/.tauri/quant-desktop.key` | 私钥（加密） | 本地 + GitHub Secret |
| `~/.tauri/quant-desktop.key.pub` | 公钥 | 已写入 `src-tauri/tauri.conf.json` |

### 查看私钥内容

**Git Bash:**
```bash
cat ~/.tauri/quant-desktop.key
```

复制输出的全部内容，下一步要用。

---

## 第二步：添加 GitHub Secrets

仓库已 Public，只需 2 个 Secret：

打开 `https://github.com/Leaderxin/quant-desktop/settings/secrets/actions`，点 **New repository secret**：

### 2.1 添加 TAURI_PRIVATE_KEY

| 字段 | 值 |
|------|-----|
| **Name** | `TAURI_PRIVATE_KEY` |
| **Secret** | 粘贴第一步 `cat ~/.tauri/quant-desktop.key` 输出的全部内容 |

### 2.2 添加 TAURI_KEY_PASSWORD

| 字段 | 值 |
|------|-----|
| **Name** | `TAURI_KEY_PASSWORD` |
| **Secret** | `786541437` |

### 验证 Secrets 列表

配置完成后页面应显示：

| Name | 状态 |
|------|------|
| `TAURI_PRIVATE_KEY` | ✅ |
| `TAURI_KEY_PASSWORD` | ✅ |

> 不需要 `PUBLIC_REPO_PAT`——仓库公开后，Release 创建使用内置 `GITHUB_TOKEN`，无需额外配置。

---

## 第三步：验证配置

### 3.1 更新 CHANGELOG.md

在项目根目录的 `CHANGELOG.md` 中添加新版本记录：

```markdown
## v1.2.0 (2026-06-21)
### Added
- 版本更新检测及自动更新机制
- 启动时自动检查更新，交易时段智能抑制弹窗
- 更新对话框展示完整 CHANGELOG
- 托盘菜单"检查更新"入口
### Fixed
- 修复已知问题，优化性能
```

### 3.2 推送 tag 触发构建

```bash
git add CHANGELOG.md
git commit -m "docs: update CHANGELOG for v1.2.0"
git tag v1.2.0
git push origin master
git push origin v1.2.0
```

### 3.3 监控 CI 运行

打开 `https://github.com/Leaderxin/quant-desktop/actions`，查看 Release workflow：

构建流程（2 个 job）：
1. **build** — Windows / macOS / Linux 三平台矩阵构建，上传 artifacts
2. **release** — 下载 artifacts → 提取 CHANGELOG → 签名生成 latest.json → 创建 Release

### 3.4 验证 Release

打开 `https://github.com/Leaderxin/quant-desktop/releases`，确认：
- 新 Release `v1.2.0` 已创建
- 安装包文件（`.exe`/`.msi`/`.dmg` 等）已上传
- `latest.json` 已上传

### 3.5 验证 latest.json

下载 `latest.json`，内容格式应为：

```json
{
  "version": "1.2.0",
  "notes": "### Added\n- 版本更新检测...",
  "pub_date": "2026-06-21T...",
  "platforms": {
    "windows-x86_64": {
      "signature": "...",
      "url": "https://github.com/Leaderxin/quant-desktop/releases/download/v1.2.0/..."
    }
  }
}
```

---

## 故障排查

### CI sign 步骤失败

| 错误信息 | 可能原因 | 解决 |
|---------|---------|------|
| `A public key has been found, but no private key` | `TAURI_PRIVATE_KEY` 未设置或名称不对 | 检查 Secret Name 是否完全一致 |
| `Bad password` | 密钥密码错误 | 确认 `TAURI_KEY_PASSWORD` 值与生成时一致 |

### 应用端检查不到更新

| 现象 | 可能原因 | 解决 |
|------|---------|------|
| 始终显示"已是最新版本" | `latest.json` 未上传到 Release | 检查 Release Assets 是否包含 `latest.json` |
| `endpoint did not respond with a successful status code` | Release 不存在或仓库非 Public | 确认仓库为 Public，Release 已创建 |
| 下载失败 | 签名验证不通过 | 确认 `tauri.conf.json` 中 pubkey 与 CI 私钥匹配 |

---

## 后续日常发布流程

```bash
# 1. 更新版本号
# 编辑 src-tauri/Cargo.toml 和 src-tauri/tauri.conf.json 的 version

# 2. 更新 CHANGELOG.md

# 3. 提交 + 打 tag
git add .
git commit -m "chore: bump version to v1.x.0"
git tag v1.x.0
git push origin master
git push origin v1.x.0
```

推送 tag 后 CI 自动构建，构建完成后应用即可检测到更新。

---

## 配置清单

| 序号 | 操作 | 位置 | 状态 |
|------|------|------|------|
| 1 | 生成签名密钥对 | 本地 `npx tauri signer generate` | ☐ |
| 2 | 添加 Secret `TAURI_PRIVATE_KEY` | Settings → Secrets → Actions | ☐ |
| 3 | 添加 Secret `TAURI_KEY_PASSWORD` | Settings → Secrets → Actions | ☐ |
| 4 | 更新 `CHANGELOG.md` | 项目根目录 | ☐ |
| 5 | 推送 `v*` tag 触发构建 | `git tag v1.2.0 && git push origin v1.2.0` | ☐ |
| 6 | 验证 Release 和 `latest.json` | `https://github.com/Leaderxin/quant-desktop/releases` | ☐ |
