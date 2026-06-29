# QuantDesktop — Microsoft Store 发布方案文档

> 版本：v1.1  
> 日期：2026-06-29（修订）  
> 适用产品：QuantDesktop v1.2.5+  
> 目标平台：Microsoft Store（Windows 10 1809+ / Windows 11）

---

## 目录

1. [概述](#1-概述)
2. [费用与收益](#2-费用与收益)
3. [开发者账号注册](#3-开发者账号注册)
4. [应用名称预留](#4-应用名称预留)
5. [MSIX 打包流程](#5-msix-打包流程)
6. [商店展示素材准备](#6-商店展示素材准备)
7. [提交审核流程](#7-提交审核流程)
8. [常见被拒原因与对策](#8-常见被拒原因与对策)
9. [发布后维护](#9-发布后维护)
10. [CI/CD 自动化构建](#10-cicd-自动化构建)
11. [附录](#11-附录)
12. [常见账号问题排查](#12-常见账号问题排查)

---

## 1. 概述

### 1.1 产品信息

| 属性 | 值 |
|------|-----|
| 产品名称 | QuantDesktop - 免费实时A股看盘 |
| 应用标识符 | `com.leaderxin.quant-desktop` |
| 当前版本 | 1.2.5 |
| 技术栈 | Tauri 2 + Vue 3 + Rust |
| 安装格式 | `.msi`（WiX）→ 封装为 `.msix` |
| 操作系统 | Windows 10 (1809+) / Windows 11 |

### 1.2 为什么选择 Microsoft Store

- **月活 2.5 亿+用户**，Win10/Win11 内置，触达面极广
- **完全免费**：注册费、签名费、托管费全部免除（2025-2026 政策）
- **无需软著**：不要求计算机软件著作权证书
- **自动签名**：提交 MSIX 格式，微软认证后免费重新签名，消除 SmartScreen 警告
- **自动更新**：系统每 24 小时自动检查更新，用户无需手动下载
- **可信分发**：Store 认证过的应用天然获得 Windows 信任

### 1.3 技术路线

```
Tauri CLI 构建 (.msi)
       ↓
MSIX Packaging Tool 封装 (.msix)
       ↓
编辑 AppxManifest.xml（注入 Partner Center 产品标识）
       ↓
上传 Partner Center → 微软认证 → 自动签名 → 上架
```

关键决策：**选择 MSIX/PWA 应用类型**（而非 EXE/MSI 类型），以享受微软免费签名。

---

## 2. 费用与收益

### 2.1 费用明细

| 费用项 | 金额 | 说明 |
|--------|------|------|
| 个人开发者注册费 | **免费** | 2024 年 9 月起永久免费（原 $19） |
| 企业开发者注册费 | **免费** | 2026 年 5 月起永久免费（原 $99） |
| MSIX 代码签名 | **免费** | 微软认证后重新签名 |
| 应用托管/CDN | **免费** | 微软提供全球 CDN |
| 应用更新分发 | **免费** | Store 内置更新机制 |
| **总费用** | **¥0** | — |

### 2.2 收益分成

| 场景 | 分成 |
|------|------|
| 应用免费 + 无内购 | 无分成（0%） |
| 使用自有支付系统 | **100% 归开发者** |
| 使用微软支付系统 | 开发者 85% / 微软 15%（非游戏） |

> QuantDesktop 定位为免费工具软件，无内购，不涉及分成。

---

## 3. 开发者账号注册

### 3.1 注册入口

**注册专用入口**：[https://developer.microsoft.com/microsoft-store/register/](https://developer.microsoft.com/microsoft-store/register/)

点击"注册"，使用 Microsoft 账号（个人账号或 Entra ID 工作账号）登录后进入注册向导。

> ⚠️ **重要**：登录 `https://partner.microsoft.com` 后默认进入的是微软所有合作伙伴项目的总门户（CSP、商业市场、ISV 等）。作为 Windows 应用开发者，你需要导航到 **"Windows apps and games"** 才能看到应用管理功能。也可以直接访问仪表板：[https://partner.microsoft.com/en-us/dashboard/windows/overview](https://partner.microsoft.com/en-us/dashboard/windows/overview)

### 3.2 账号类型选择

| | 个人账户 | 企业账户 |
|---|---|---|
| 适用对象 | 个人开发者 | 公司/组织 |
| 登录方式 | Microsoft 个人账号 | Microsoft 个人账号 **或** Microsoft Entra ID（工作账号，2026 年 5 月起支持） |
| 显示名称 | 个人真实姓名 | 公司名称 |
| 所需材料 | 身份证/护照 + 自拍 | 营业执照 + D-U-N-S 编号（推荐）+ 公司域邮箱 |
| 验证周期 | 几小时（自动） | 1-3 天（人工审核，使用 Entra ID 可加速） |
| 费用 | 免费 | 免费 |
| 团队管理 | 不支持 | 使用 Entra ID 可分配角色，多人管理 |

### 3.3 注册步骤

1. 打开 [https://developer.microsoft.com/microsoft-store/register/](https://developer.microsoft.com/microsoft-store/register/)，点击"注册"，使用 Microsoft 账号登录（企业用户可使用 Microsoft Entra ID 工作账号）
2. 选择账户类型：**个人** 或 **公司**
3. 填写开发者信息：
   - 个人：真实姓名、国家/地区、联系邮箱
   - 公司：公司全称、D-U-N-S 编号（如有，可加速）、营业执照扫描件
4. 身份验证：
   - 个人：手机/邮箱验证 + 证件扫描 + 自拍
   - 公司：电子邮件验证 + 人工审核
5. 完成注册，进入 Partner Center 仪表板

### 3.4 注意事项

- 公司账户强烈建议准备 **D-U-N-S 编号**（可免费申请），否则需上传商业登记文件，审核时间更长
- 公司账户建议使用 **公司域名的邮箱**，避免额外身份验证步骤
- 验证机会只有 **3 次**，材料务必准备完整正确
- 账户一旦创建，身份类型不可更改

---

## 5. MSIX 打包流程

> ⚠️ Tauri 2 原生不支持 MSIX 输出，需要通过 MSIX Packaging Tool 将 `.msi` 封装为 `.msix`。

### 5.1 前提条件

- 已完成 Tauri 构建，生成 `.msi` 安装包
- 从 Microsoft Store 安装了 **MSIX Packaging Tool**（免费）
- 已在 Partner Center 预留应用名称，获取了产品标识信息

### 5.2 获取产品标识信息

在 Partner Center → 选择应用 → "产品管理" → "产品标识" 中获取以下三个关键值：

| 字段 | 示例值 |
|------|--------|
| **程序包/标识/名称** | `12345Leaderxin.QuantDesktop` |
| **发布者** | `CN=XXXXXXXX-XXXX-XXXX-XXXX-XXXXXXXXXXXX` |
| **发布者显示名称** | `Leaderxin` |

> ⚠️ **发布者字段必须从 Partner Center 精确复制粘贴**，一个字符都不能错。

### 5.3 构建 Tauri 应用

```bash
# 安装依赖
npm ci

# 构建生产版本（生成 .msi）
npm run tauri:build
```

构建输出路径：`src-tauri/target/release/bundle/msi/quant-desktop_1.2.5_x64_zh-CN.msi`

### 5.4 使用 MSIX Packaging Tool 封装

#### 步骤 1：启动工具

从开始菜单启动 **MSIX Packaging Tool**。

#### 步骤 2：创建新包

选择 **"Application package"** → 选择 **"Create a package on this machine"**。

#### 步骤 3：选择安装程序

- 浏览选择 Tauri 生成的 `.msi` 文件
- **签名选项：跳过**（微软会在认证后签名）

#### 步骤 4：填写包信息

| 字段 | 值 |
|------|-----|
| Package Name | `com.leaderxin.quant-desktop`（与 tauri.conf.json 的 identifier 对齐） |
| Package Display Name | `QuantDesktop` |
| Publisher Display Name | `<从 Partner Center 复制>` |
| Version | `1.2.5.0`（4段格式） |

#### 步骤 5：指定安装位置

默认 `C:\Program Files\QuantDesktop\`，保持默认即可。

#### 步骤 6：包含依赖项

- WebView2 运行时：选择 **"Skip"** 或 **"Include"**
  - **推荐 Skip**：Windows 10 1809+ 和 Windows 11 已内置 WebView2，无需包含
  - 如果目标用户可能使用更早的 Windows 版本，选择 Include（包体积会增加约 150MB）
- Visual C++ 运行时：通常不需要（Tauri 已静态链接）

#### 步骤 7：生成 .msix

完成向导后，工具会生成 `.msix` 文件和对应的 `AppxManifest.xml`。

### 5.5 修正 AppxManifest.xml

生成 MSIX 后，需要手动验证并修正关键字段：

```xml
<?xml version="1.0" encoding="utf-8"?>
<Package xmlns="http://schemas.microsoft.com/appx/manifest/foundation/windows10"
         xmlns:rescap="http://schemas.microsoft.com/appx/manifest/foundation/windows10/restrictedcapabilities"
         xmlns:desktop="http://schemas.microsoft.com/appx/manifest/desktop/windows10">

  <Identity
    Name="12345Leaderxin.QuantDesktop"          <!-- 必须与 Partner Center 完全一致 -->
    Publisher="CN=XXXXXXXX-XXXX-XXXX-XXXX-XXXXXXXXXXXX"  <!-- 必须与 Partner Center 完全一致 -->
    Version="1.2.5.0" />                        <!-- 必须是 4 段式版本号 -->

  <Properties>
    <DisplayName>QuantDesktop</DisplayName>
    <PublisherDisplayName>Leaderxin</PublisherDisplayName>
    <Logo>StoreLogo.png</Logo>
  </Properties>

  <Dependencies>
    <TargetDeviceFamily Name="Windows.Desktop"
                        MinVersion="10.0.17763.0"   <!-- Win10 1809，MSIX 桌面支持的最小版本 -->
                        MaxVersionTested="10.0.22621.0" />
  </Dependencies>

  <Resources>
    <Resource Language="zh-CN" />
  </Resources>

  <Applications>
    <Application Id="QuantDesktop"
                 Executable="quant-desktop.exe"
                 EntryPoint="Windows.FullTrustApplication">
      <uap:VisualElements
        DisplayName="QuantDesktop - 免费实时A股看盘"
        Description="A股实时行情桌面监测工具"
        Square150x150Logo="Square150x150Logo.png"
        Square44x44Logo="Square44x44Logo.png"
        BackgroundColor="transparent">
      </uap:VisualElements>
      <Extensions>
        <desktop:Extension Category="windows.startupTask"
                           Executable="quant-desktop.exe"
                           EntryPoint="Windows.FullTrustApplication">
          <desktop:StartupTask
            TaskId="QuantDesktop"
            Enabled="false"
            DisplayName="QuantDesktop" />
        </desktop:Extension>
      </Extensions>
    </Application>
  </Applications>

  <Capabilities>
    <Capability Name="internetClient" />
    <rescap:Capability Name="runFullTrust" />
  </Capabilities>
</Package>
```

关键检查清单：

- [ ] `Identity Name` = Partner Center 的程序包标识名称
- [ ] `Identity Publisher` = Partner Center 的发布者（精确复制）
- [ ] `Version` 为 4 段格式（`Major.Minor.Build.Revision`）
- [ ] `MinVersion` = `10.0.17763.0`
- [ ] `EntryPoint` = `Windows.FullTrustApplication`
- [ ] 声明了 `<rescap:Capability Name="runFullTrust" />`
- [ ] 声明了 `<Capability Name="internetClient" />`

### 5.6 本地验证（可选但推荐）

1. 使用自签名证书签名 MSIX：
   ```powershell
   # 创建自签名证书
   New-SelfSignedCertificate -Type Custom -Subject "CN=LocalTest" `
     -KeyUsage DigitalSignature -FriendlyName "MSIX Test" `
     -CertStoreLocation "Cert:\CurrentUser\My" `
     -TextExtension @("2.5.29.37={text}1.3.6.1.5.5.7.3.3", "2.5.29.19={text}")

   # 导出证书
   $cert = Get-ChildItem -Path Cert:\CurrentUser\My | Where-Object { $_.FriendlyName -eq "MSIX Test" }
   Export-Certificate -Cert $cert -FilePath "$env:USERPROFILE\Desktop\TestCert.cer"

   # 安装证书到受信任的根证书颁发机构
   Import-Certificate -FilePath "$env:USERPROFILE\Desktop\TestCert.cer" `
     -CertStoreLocation Cert:\LocalMachine\Root
   ```

2. 双击 `.msix` 文件安装测试
3. 确认应用正常启动、联网、双窗口均可显示
4. 确认托盘图标、自动隐藏等行为正常
5. 卸载后检查无残留

---

## 6. 商店展示素材准备

### 6.1 必需素材清单

#### 图标

| 文件名 | 尺寸 | 说明 |
|--------|------|------|
| `StoreLogo.png` | 50×50 px | Store 列表中的小图标 |
| `Square44x44Logo.png` | 44×44 px | 开始菜单小图标 |
| `Square150x150Logo.png` | 150×150 px | 产品详情页图标 |
| `Wide310x150Logo.png` | 310×150 px | 宽幅展示图标（可选但强烈推荐） |
| `Square310x310Logo.png` | 310×310 px | 大图标（可选） |

#### 应用截图

| 要求 | 说明 |
|------|------|
| 数量 | 至少 1 张，推荐 4-6 张 |
| 分辨率 | 推荐 1366×768 px 或 1920×1080 px |
| 格式 | PNG（推荐）或 JPG |
| 大小 | 单张 ≤ 50MB |
| 内容 | 展示核心功能，不要包含非应用内容 |

### 6.2 QuantDesktop 推荐截图方案

| 序号 | 截图内容 | 展示要点 |
|------|----------|----------|
| 1 | 主界面全貌 | 自选股列表 + 指数栏 + 行情数据（深色主题） |
| 2 | 个股详情 - 分时图 | 分时走势图 + 深度面板 |
| 3 | 个股详情 - K线图 | 日K/周K/月K 切换效果 |
| 4 | 个股详情 - 深度数据 | 五档买卖盘口 |
| 5 | Ticker 条 + 系统托盘 | 桌面小窗 + 状态栏托盘展示 |
| 6 | 浅色主题 | 展示双主题切换效果 |

> 提示：使用 Windows 截图工具（Win+Shift+S）截取，确保截图环境干净（关闭其他窗口、统一的桌面背景）。

### 6.3 文本素材

#### 应用名称

```
QuantDesktop - 免费实时A股看盘
```

> 限制 ≤ 256 字符，建议简洁。Store 中通常显示 30-50 字符。

#### 简短描述（一句话，搜索结果中显示）

```
免费、极简的 A 股实时行情桌面监测工具，支持分时图、K 线图与五档深度数据，深色/浅色双主题。
```

> 限制 ≤ 100 字符。

#### 详细描述（产品详情页，支持 HTML 基础标签）

```html
<h3>📈 实时 A 股行情，就在你的桌面</h3>

<p>QuantDesktop 是一款 <strong>完全免费</strong>、<strong>无广告</strong> 的 A 股实时行情桌面监测工具。
轻量、极简，常驻桌面一角，让你随时掌握自选股动态。</p>

<h3>✨ 核心功能</h3>

<ul>
  <li><strong>实时行情</strong> — 自选股实时价格、涨跌幅、成交量一键查看</li>
  <li><strong>分时图</strong> — 当日股价走势实时更新，把握盘中动态</li>
  <li><strong>K 线图</strong> — 日 K / 周 K / 月 K 自由切换，经典蜡烛图展示</li>
  <li><strong>五档深度</strong> — 买一至买五 / 卖一至卖五，盘口深度一目了然</li>
  <li><strong>主要指数</strong> — 上证指数、深证成指、创业板指等 7 大指数实时同步</li>
  <li><strong>Ticker 悬浮条</strong> — 桌面小窗轮播自选股，不占工作空间</li>
  <li><strong>深色 / 浅色主题</strong> — 适配白天黑夜使用习惯</li>
  <li><strong>系统托盘常驻</strong> — 最小化到托盘，关闭不退出</li>
</ul>

<h3>🛡️ 安全与隐私</h3>

<p>QuantDesktop 不收集、不存储、不上传任何用户个人信息。所有行情数据均来自公开市场数据接口，本地缓存仅用于加速加载。无广告，无推送，无后台遥测。</p>

<h3>💻 系统要求</h3>

<table>
  <tr><td>操作系统</td><td>Windows 10 版本 1809 或更高 / Windows 11</td></tr>
  <tr><td>处理器</td><td>1 GHz 或更快</td></tr>
  <tr><td>内存</td><td>512 MB 或更大</td></tr>
  <tr><td>存储</td><td>约 50 MB 可用空间</td></tr>
  <tr><td>网络</td><td>需要互联网连接以获取实时行情数据</td></tr>
</table>

<h3>📊 数据来源</h3>

<p>行情数据来自腾讯证券及新浪财经公开行情接口，仅供个人参考，不构成投资建议。</p>

<h3>🔗 开源 & 反馈</h3>

<p>QuantDesktop 是开源软件（MIT 协议）。</p>
<p>GitHub: <a href="https://github.com/Leaderxin/quant-desktop">github.com/Leaderxin/quant-desktop</a></p>
<p>如有建议或问题，欢迎通过 GitHub Issues 反馈。</p>
```

#### 搜索关键词

```
股票, A股, 行情, 看盘, 自选股, K线, 分时图, 深度数据, 桌面工具, 免费, stock, trading, china
```

> 最多 7 个关键词（英文算一个词），选择高热度、高搜索量的词。

#### 隐私政策 URL

需要提供一个可公开访问的 URL。最简单的方案是在 GitHub 仓库创建一个页面：

```markdown
# QuantDesktop 隐私政策

**最后更新日期：2026年6月22日**

## 数据收集

QuantDesktop **不收集、不存储、不上传**任何用户个人信息，包括但不限于：

- 姓名、邮箱、电话号码
- 地理位置、IP 地址
- 设备标识符
- 浏览记录、使用习惯

## 行情数据

本应用展示的所有股票行情数据均来自第三方公开市场数据接口（腾讯证券、新浪财经）。数据请求以匿名方式进行，不包含用户身份信息。

## 本地存储

应用配置（主题偏好、自选股列表、窗口位置）仅存储在您的本地计算机上，不会上传到任何服务器。

## 第三方服务

本应用不集成任何第三方分析、广告或追踪 SDK。

## 免责声明

本应用展示的行情数据仅供参考，不构成投资建议。投资有风险，入市需谨慎。

## 联系我们

如有任何隐私相关问题，请通过 GitHub Issues 联系我们：
https://github.com/Leaderxin/quant-desktop/issues
```

将此文件发布到 `https://leaderxin.github.io/quant-desktop/privacy.html` 或类似的公开 URL。

---

## 7. 提交审核流程

### 7.1 提交表单（6 个标签页）

#### 标签 1：定价和可用性

| 配置项 | 推荐值 |
|--------|--------|
| 价格 | **免费** |
| 分发市场 | 全球（241 个市场）或仅中国市场 |
| 试用版 | 不设置 |
| 发布日期 | 审核通过后立即发布 |

#### 标签 2：属性

| 配置项 | 推荐值 |
|--------|--------|
| 类别 | **实用工具**（比"金融"审核更宽松） |
| 子类别 | 财务与投资 / 个人理财 |
| 硬件要求 | 键盘、鼠标（最低要求） |
| 声明 | 不涉及加密、不访问位置、不访问联系人 |

#### 标签 3：年龄分级

完成 IARC（国际年龄分级联盟）问卷，约 2 分钟：

- 应用是否包含暴力内容？→ 否
- 是否包含性内容？→ 否
- 是否包含酒精/烟草/药物使用？→ 否
- 是否包含赌博？→ 否
- 是否包含用户生成内容？→ 否
- 是否收集个人信息？→ 否

预期结果：**3+（适合所有年龄）**

#### 标签 4：程序包

- 上传打包好的 `.msix` 或 `.msixupload` 文件
- 等待系统自动验证（文件名 + 签名 + manifest 完整性检查）
- 如有错误，根据提示修正后重新上传

#### 标签 5：应用商店一览

- 上传所有图标和截图
- 填写应用名称、简短描述、详细描述
- 添加搜索关键词
- 填写隐私政策 URL

#### 标签 6：提交选项

- 认证备注：可以写一些给审核人员的备注（如测试账号、特殊功能说明）
  - QuantDesktop 建议写：`本应用需要网络连接以获取实时市场数据。应用启动后可能在 3-5 秒内显示数据。`
- 发布计划：默认"审核通过后立即发布"

### 7.2 点击"提交以进行认证"

### 7.3 审核周期

| 阶段 | 预计时间 |
|------|----------|
| 自动预检查 | 几分钟 |
| 安全测试 | 几小时 |
| 内容合规检查 | 1-2 个工作日 |
| 技术合规检查 | 1-2 个工作日 |
| 总计 | **1-3 个工作日** |

审核通过后，应用大约在 **15 分钟** 内上架，用户即可在 Microsoft Store 搜索到。

---

## 8. 常见被拒原因与对策

### 8.1 技术类问题

| 问题 | 原因 | 解决方案 |
|------|------|----------|
| Publisher 不匹配 | Manifest 中 Publisher 与 Partner Center 不一致 | 直接从 Partner Center 复制粘贴 |
| 版本号格式错误 | 版本号不是 4 段格式 | 改为 `1.2.5.0` 格式 |
| 缺少 runFullTrust | `AppxManifest.xml` 未声明此能力 | 添加 `<rescap:Capability Name="runFullTrust"/>` |
| 应用启动崩溃 | WebView2 未找到或运行时错误 | 确保 WebView2 已安装或内嵌 |
| WACK 测试失败 | Windows App Certification Kit 检查不通过 | 提交前用 WACK 自测 |

### 8.2 内容类问题

| 问题 | 原因 | 解决方案 |
|------|------|----------|
| 功能描述与实际不符 | 描述夸大了功能 | 确保截图与功能描述一致 |
| 隐私政策缺失 | 未提供隐私政策 URL | 添加隐私政策页面 |
| 应用内包含动态代码 | Tauri WebView 加载了外部 JavaScript | 确保 CSP 配置正确，不从外部加载未授权脚本 |
| 类别选择不当 | 选择了金融类别但功能不匹配 | 改为"实用工具"类别 |

### 8.3 金融类应用特别注意

- 如果审核人员将应用归类为"金融服务"，可能要求提供金融资质证明
- **对策**：在应用描述和认证备注中明确说明这是**数据展示工具**，不涉及交易、投资建议、资金操作
- 在应用中添加清晰的免责声明：**"数据仅供参考，不构成投资建议"**

### 8.4 申诉与重新提交

- 被拒后可在 Partner Center 查看具体被拒原因
- 修改后重新上传程序包并提交
- 如有争议，可通过 Partner Center 的"联系支持"申诉

---

## 9. 发布后维护

### 9.1 版本更新流程

1. 在 Tauri 项目中更新版本号 `tauri.conf.json` 和 `package.json`
2. 构建新版本 MSI
3. 封装为 MSIX（4 段版本号必须递增，如 `1.2.6.0`）
4. Partner Center → 应用 → 程序包 → 上传新包
5. 填写更新说明
6. 提交审核（更新审核通常比首次更快，0.5-1 个工作日）

### 9.2 用户评价管理

- 定期查看 Store 评价和评分
- 通过评价中的反馈改进产品
- 可以在 GitHub Issues 中回复用户，注明已在 Store 版本中修复
- 不建议在 Store 评价中直接回复（可能被视作垃圾信息）

### 9.3 分析数据

Partner Center 提供免费的分析仪表板（无需集成任何 SDK）：

- 下载量（按市场、语言、OS 版本）
- 使用次数
- 评分与评价趋势
- 崩溃报告（可选，需集成 Windows SDK）

### 9.4 双渠道维护策略

QuantDesktop 需要维护两套更新渠道：

| | **Microsoft Store 版本** | **GitHub 直接下载版** |
|---|---|---|
| 分发方式 | Store 自动更新 | Tauri updater |
| 更新检查 | 系统每 24h 检查 | 应用启动时 + 定时检查 |
| 签名 | 微软签名 | 需自行签名（可选） |
| 安装路径 | `C:\Program Files\WindowsApps\` | 用户自定义 |

建议策略：

- 通过 Rust 编译时的 feature flag 或 Tauri 的 `#[cfg()]` 区分两个渠道
- Store 版本：**禁用**内置 Tauri updater（使用 Store 更新机制）
- GitHub 版本：**保持**现有的 Tauri updater

> 实际实现：在 Tauri 构建配置中，可以通过环境变量控制 updater 是否启用。Store 构建时设置环境变量禁用 updater 插件。

---

## 10. CI/CD 自动化构建

### 10.1 目标

在 GitHub Actions 中自动构建 MSIX 包，减少手动操作。

### 10.2 构建脚本参考

```yaml
# .github/workflows/store-release.yml
name: Microsoft Store Release

on:
  workflow_dispatch:
    inputs:
      version:
        description: 'Version (e.g. 1.2.5)'
        required: true

jobs:
  build-store:
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v4

      - name: Setup Node
        uses: actions/setup-node@v4
        with:
          node-version: 22

      - name: Setup Rust
        uses: dtolnay/rust-toolchain@stable

      - name: Install dependencies
        run: npm ci

      - name: Build Tauri MSI
        run: npm run tauri:build
        env:
          TAURI_SIGNING_PRIVATE_KEY: ${{ secrets.TAURI_SIGNING_PRIVATE_KEY }}
          TAURI_SIGNING_PRIVATE_KEY_PASSWORD: ${{ secrets.TAURI_SIGNING_PRIVATE_KEY_PASSWORD }}
          # 禁用内置 updater（Store 版本）
          TAURI_DISABLE_UPDATER: true

      - name: Package as MSIX
        shell: pwsh
        run: |
          $msiPath = Get-ChildItem -Path "src-tauri/target/release/bundle/msi" -Filter "*.msi" | Select-Object -First 1
          & "C:\Program Files (x86)\Windows Kits\10\bin\10.0.22621.0\x64\makeappx.exe" `
            pack `
            -d temp_msix `
            -p quant-desktop.msix `
            -l

      - name: Upload MSIX artifact
        uses: actions/upload-artifact@v4
        with:
          name: quant-desktop-msix
          path: quant-desktop.msix
```

### 10.3 使用 `makeappx.exe` 手动打包（无需 GUI 工具）

```powershell
# 1. 创建临时目录结构
$staging = "msix-staging"
New-Item -ItemType Directory -Force -Path $staging
Copy-Item -Recurse "src-tauri/target/release/bundle/msi-extracted/*" $staging

# 2. 确保 AppxManifest.xml 在根目录
# （从模板生成或从 MSIX Packaging Tool 导出后手动维护）

# 3. 打包
makeappx.exe pack /d $staging /p "quant-desktop.msix" /l
```

---

## 11. 附录

### 11.1 时间线估算

| 阶段 | 预计耗时 | 备注 |
|------|----------|------|
| 开发者账号注册 | 0.5 小时 | 个人账号自动验证快 |
| 应用名称预留 | 5 分钟 | 一次性操作 |
| Tauri 构建 (MSI) | 5-15 分钟 | 取决于机器性能 |
| MSIX 封装 (初次) | 1-2 小时 | 首次需要安装工具、学习流程 |
| MSIX 封装 (后续) | 15 分钟 | 熟悉后快速完成 |
| 素材准备 | 2-4 小时 | 截图 + 文案撰写 |
| 填写提交表单 | 30 分钟 | 6 个标签页逐一填写 |
| 等待审核 | 1-3 个工作日 | 首次可能较慢 |
| **总计** | **约 1 周** | 含审核等待时间 |

### 11.2 相关链接

| 资源 | 链接 |
|------|------|
| Microsoft Partner Center（仪表板） | https://partner.microsoft.com/en-us/dashboard/windows/overview |
| Microsoft Store 开发者注册 | https://developer.microsoft.com/microsoft-store/register/ |
| 发布第一个 Windows 应用（官方文档） | https://learn.microsoft.com/zh-cn/windows/apps/package-and-deploy/publish-first-app |
| 将 Win32 应用分发到 Store | https://learn.microsoft.com/zh-cn/windows/apps/distribute-through-store/how-to-distribute-your-win32-app-through-microsoft-store |
| Tauri 官方 Microsoft Store 指南 | https://v2.tauri.app/learn/distribute/microsoft-store/ |
| MSIX Packaging Tool | Microsoft Store 免费下载 |
| 个人开发者免费上架实战参考 | https://huayemao.run/posts/337 |
| Windows App Certification Kit (WACK) | 包含在 Windows SDK 中 |
| QuantDesktop GitHub | https://github.com/Leaderxin/quant-desktop |

### 11.3 素材规格速查表

| 资源 | 尺寸 | 格式 | 大小限制 |
|------|------|------|----------|
| StoreLogo | 50×50 px | PNG | ≤ 200KB |
| Square44x44Logo | 44×44 px | PNG | ≤ 200KB |
| Square150x150Logo | 150×150 px | PNG | ≤ 200KB |
| Wide310x150Logo | 310×150 px | PNG | ≤ 200KB |
| Square310x310Logo | 310×310 px | PNG | ≤ 200KB |
| 应用截图 | ≥ 1366×768 px | PNG | ≤ 50MB |
| 应用名称 | — | 文本 | ≤ 256 字符 |
| 简短描述 | — | 文本 | ≤ 100 字符 |
| 详细描述 | — | HTML | ≤ 10,000 字符 |
| 搜索关键词 | — | 文本 | 最多 7 个 |

### 11.4 版本号对比

| 来源 | 格式 | 示例 |
|------|------|------|
| `tauri.conf.json` | `MAJOR.MINOR.PATCH` | `1.2.5` |
| `Cargo.toml` | `MAJOR.MINOR.PATCH` | `1.2.5` |
| `AppxManifest.xml` (MSIX) | `MAJOR.MINOR.BUILD.REVISION` | `1.2.5.0` |
| Partner Center 提交 | 同 MSIX | `1.2.5.0` |

每次发布 Store 版本时，MSIX 的 Revision 号需要递增：`1.2.5.0` → `1.2.5.1`。

---

## 12. 常见账号问题排查

Partner Center 的账号验证和后台状态同步偶尔会出现卡顿，以下是最常见的几个问题和解决方案。

### 12.1 登录后看不到"Windows apps and games"

**症状**：登录 `https://partner.microsoft.com` 后只看到商业市场（Marketplace）、CSP 等选项，找不到应用发布入口。

**原因**：`partner.microsoft.com` 是微软所有合作伙伴项目的总门户。Windows 应用发布只是其中一个模块，新手容易走错。

**解决**：
1. 直接访问专用仪表板地址：[https://partner.microsoft.com/en-us/dashboard/windows/overview](https://partner.microsoft.com/en-us/dashboard/windows/overview)
2. 确认注册时选择了正确的账号类型（Windows 应用开发者，而非 CSP/ISV）
3. 如仍不可用，可能是账号尚未完成验证（见 12.3）

### 12.2 "提交认证"按钮灰色无法点击

**症状**：所有标签页已经填写完毕（显示绿色勾），但最终的"提交以进行认证"按钮始终是灰色的。

**可能原因与解决**：

1. **未接受更新后的协议条款** → 导航到 **Account Settings → Agreements**（账户设置 → 协议），检查是否有待接受的更新条款。需要用账号所有者邮箱对应的账号登录才能签署。
2. **所有标签页确实需要全部变绿** → 特别检查容易遗漏的"Submission Options"标签页 —— 即使内容为空，也需要点进去打开一下并保存。
3. **浏览器缓存问题** → 退出登录，清除浏览器缓存和 Cookie，使用隐私/无痕窗口重新登录。
4. **Partner Center 已知 bug**：使用 `runFullTrust` 受限能力的应用，有时"Submission Options"会一直显示"Incomplete"。这是 Partner Center 前端的 bug，需要联系支持解决。

### 12.3 账号长期处于"未验证"状态

**症状**：注册已超过 30 分钟（甚至数天），账号仍显示 verification pending，页面提示"You can't submit to the Store until your account is verified"。

**解决步骤**：

1. **检查邮箱**（含垃圾邮件箱），是否有微软发来的验证请求邮件
2. **个人账号**：确认身份证/护照照片清晰、自拍照面部清晰且与证件匹配
3. **企业账号**：确认 D-U-N-S 编号有效，或营业执照扫描件清晰可读；使用公司域名的邮箱
4. **通过官方支持渠道提交工单**：
   - Partner Center 内：**Help (?) → Contact Support** → 选择"Account"类别
   - 或访问：[https://developer.microsoft.com/en-us/windows/support/](https://developer.microsoft.com/en-us/windows/support/)
5. **备用方案**：如果 Partner Center 支持入口不可用（链接跳转错误等），可到 [Azure Portal](https://portal.azure.com) 创建支持请求，选择"订阅/账号"类别，描述中提到"Dev Center/Partner Center provisioning issue"，附上你的 Publisher ID

> ⚠️ 身份验证只有 **3 次**机会，材料务必准备完整正确再提交。

### 12.4 税务/付款资料阻止提交（即使是免费应用）

**症状**：页面提示"需要更新税务和付款信息"，但进入 Payout & Tax 页面发现所有资料显示为 **Complete**（已完成）。

**原因**：这是 Partner Center 的已知问题 —— 后端验证系统与前端 UI 状态不同步。

**解决**：
1. 重新填写并保存税务资料（即使是重复的），触发一次后端重新验证
2. 等待 24 小时内后端同步（通常会自动恢复）
3. 如超 24 小时仍未解决，通过 Partner Center 的 **Help (?) → Contact Support** 提交工单

### 12.5 提交后长期卡在"Publishing"状态

**症状**：应用已通过认证（Certification passed），进入 Publishing 阶段，但卡在"可能需要 30 分钟"的提示数天无变化。

**解决**：
1. 退出登录 → 清除缓存 → 隐私模式重新登录查看状态
2. 尝试 **撤回提交并重新提交**（Withdraw and re-submit）
3. 如果上述方法无效且超过 2 天，通过 Partner Center **Help (?) → Contact Support** 提交工单，注明 Product ID 和 Submission ID

### 12.6 程序包上传持续报错

**症状**：上传 MSIX 或填写 EXE/MSI URL 后，Pacakges 页面持续显示 "Submission failed. Please try again later"。

**解决**：
1. 确保每个架构/语言组合只上传了 **一个**程序包 —— 重复条目会导致报错
2. 删除旧的或重复的包条目，重新添加
3. 如果自检（病毒扫描、代码签名验证）已通过但仍报错，这很可能是后端问题，需要通过支持工单上报（附上 Package ID）

### 12.7 联系支持的推荐方式

| 渠道 | 适用场景 |
|------|----------|
| **Partner Center → Help (?) → Contact Support** | 首选，适用于大多数问题 |
| **developer.microsoft.com/windows/support** | 账号验证、提交、认证相关问题 |
| **Azure Portal → 创建支持请求** | 当 Partner Center 支持入口不可用时的备用方案 |
| **Microsoft Q&A（learn.microsoft.com/answers）** | 公开社区求助，用于紧急且其他渠道无响应时 |
| **`reportapp@microsoft.com`** | **仅**用于政策/认证纠纷申诉，不用于技术问题 |

提交工单时请务必附上：
- **Seller ID / Publisher ID**
- **Product ID** 和 **Submission ID**（如适用）
- 问题发生的**完整时间线**
- 错误信息的**截图**
- 之前所有相关工单的**编号**

---

> **文档维护**：本文档应在每次 Store 提交流程变更后更新。如有问题或补充，欢迎提交 PR。
