# ADR-0001：使用 Tauri 2 + React + Rust

- 状态：Accepted
- 日期：2026-07-15

## 背景

产品需要扫描大量本地文件、调用模型服务、安全保存密钥、可靠恢复任务，并分发 Windows、macOS 和 Linux 安装包。

## 决策

使用 Tauri 2 作为桌面外壳，React + Vite + TypeScript 构建 UI，Rust 实现文件系统、任务调度、模型请求、持久化和安全边界。

## 备选方案

- Electron：全 TypeScript 更快，但安装体积、内存和本地核心隔离不符合长期目标。
- Flutter：跨平台 UI 完整，但现有 Web 表格、Markdown 和编辑器生态迁移成本更高。

## 后果

- 团队需要维护 Rust 与 TypeScript 两套工具链；
- 三个平台使用不同系统 WebView，必须持续运行 UI 兼容测试；
- 本地核心具备更清晰的权限边界和较好的扫描/调度性能。
