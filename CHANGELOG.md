# Changelog

## v1.2.4 (2025-06-22)
### Fixed
- 修复自动更新下载完成后安装器未启动的 bug（on_before_exit 回调时序错误）
- 修复更新下载进度显示异常（chunk 大小未累积）
- 修复启动时行情条先出现在左上角再跳到右下角的闪烁
- 修复指数栏数据渲染抖动（CSS transition + 重复 emit + interval 振荡）
- 修复启动自动检查更新弹窗被交易时段逻辑意外抑制
### Changed
- 自适应轮询仅限交易时段启用，非交易时段使用固定间隔
- 连续无变化检测阈值从 5 次提高到 10 次
- 价格变化检测增加 0.001 浮点容差
- 调度器用 tokio::sleep 替代 tokio::time::interval 避免振荡
- 启动时数据源初始化不触发 scheduler wakeup

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
