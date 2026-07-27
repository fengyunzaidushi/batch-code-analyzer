# Batch Code Analyzer 事实来源

本文与信息图只使用仓库内的公开项目资料，不包含 API Key、源码正文或个人数据。

## 产品定位

- 本仓库用于开发跨 Windows、macOS、Linux 的本地批量代码文件 AI 分析工具。
- 产品闭环是“扫描文件—构造请求—调用模型—保存结果—管理运行记录”。
- 产品不修改用户源代码，不推断 Git 开发过程，不提供云端任务队列或团队协作。

## 已公开的产品能力

- 登记多个本地代码仓库，并在项目之间切换。
- 按 `.gitignore`、文件类型、大小和安全规则筛选项目文件。
- 为项目或单文件设置提示词与模型。
- 批量调用兼容 OpenAI Responses API 的服务。
- 查看 Markdown 结果、Token、耗时、错误、重试与主备切换历史。
- 将每次运行的结果安全地写入独立输出目录。
- 从仓库 README、`AGENTS.md` 等项目文档构建项目上下文。
- 使用 Mock Provider 进行开发和测试，不依赖真实收费模型 API。

## 技术架构

- 桌面外壳：Tauri 2。
- 前端：React、Vite、TypeScript、TanStack Query/Table/Virtual、Zustand。
- 核心后端：Rust、Tokio、reqwest。
- 数据库：SQLite + sqlx。
- 安全存储：系统 Keychain/Credential Manager/Secret Service，必要时 Stronghold 降级。
- 发布：GitHub Actions、Tauri Updater。

## 可靠性与安全边界

- React 只负责展示和用户交互；仓库扫描、文件读取、路径校验、模型请求和业务状态变更均在 Rust 中完成。
- SQLite 是运行状态的权威来源；JSON/Markdown 是可迁移配置与用户可读导出。
- Run 创建后，文件清单、文件哈希、提示词、模型、上下文版本、API 路由和重试策略不可变。
- 自动重试、备用档案切换和人工重试均新增 Attempt，不覆盖历史 Attempt。
- API Key 不进入普通配置、SQLite 明文字段、日志、错误信息、导出文件或测试快照。
- 所有输入和输出路径经过仓库边界、符号链接和路径逃逸检查。
- 首期只允许一个活动 Run；中断且结果未知的请求不得自动重发。

## 典型用途

- 批量解释代码文件的职责、数据流和模块关系。
- 按统一模板提取接口、依赖、风险或技术债信息。
- 生成注释建议、测试建议或代码审查报告。
- 对大型仓库进行逐文件结构化分析并导出结果。

## 发布与启动

- 推送版本标签后，GitHub Actions 会构建三平台安装包并上传到正式 Release。
- Windows：`.msi`、`.exe`（NSIS）。
- macOS：Intel 和 Apple Silicon `.dmg`。
- Linux：`.AppImage`、`.deb`。
- 开发者可运行 `pnpm tauri:build` 在本机生成安装包。
