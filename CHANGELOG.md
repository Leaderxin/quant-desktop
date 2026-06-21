# Changelog

## v1.2.1 (2025-06-21)
### Added
- 版本更新检测及自动更新机制 (tauri-plugin-updater)
- 启动时自动检查更新，交易时段(9:30-15:00)智能抑制弹窗
- 更新对话框展示完整 CHANGELOG
- 本地代理自动检测 (Clash/V2Ray/系统代理)
- 托盘菜单"检查更新"入口
- 状态栏版本显示 + 手动检查更新按钮
- 更新下载进度实时展示
- GitHub Release 详情链接
### Changed
- 仓库公开，简化 CI/CD 流程
- 移除冗余 semver 依赖
- 统一错误色使用 CSS token --color-error
