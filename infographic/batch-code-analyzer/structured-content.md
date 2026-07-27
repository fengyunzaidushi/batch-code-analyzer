# Batch Code Analyzer：把批量代码分析变成一个可追溯的本地工作流

## Overview

Batch Code Analyzer 是一个跨 Windows、macOS、Linux 的本地桌面端批量代码文件 AI 分析工具。它将“扫描文件—构造请求—调用模型—保存结果—管理运行记录”串成一个可控、可追溯的工作流。

## Learning Objectives

The viewer will understand:

1. 这个工具适合什么问题，以及它明确不做什么。
2. 一次批量分析从本地仓库到 Markdown 结果的完整路径。
3. 为什么架构把文件系统、模型请求、状态和密钥集中在 Rust/SQLite 等受控边界内。

---

## Section 1: 它解决的不是“写代码”，而是“理解一整个仓库”

**Key Concept**: 把重复的逐文件分析工作变成统一模板、统一记录的本地流程。

**Content**:

- 批量解释代码文件的职责、数据流和模块关系。
- 按统一模板提取接口、依赖、风险或技术债信息。
- 生成注释建议、测试建议或代码审查报告。
- 对大型仓库进行逐文件结构化分析并导出结果。
- 不编辑、格式化或覆盖源代码文件。

**Visual Element**:

- Type: 产品场景模块
- Subject: 左侧是本地仓库文件树，右侧是多份 Markdown 分析结果。
- Treatment: 用“重复劳动 → 批量工作流”的方向箭头连接。

**Text Labels**:

- Headline: “先把问题说清楚：它分析代码，不改代码”
- Labels: “职责”“数据流”“依赖”“风险”“技术债”

---

## Section 2: 七步闭环

**Key Concept**: 每个文件都经过扫描、筛选、构造请求、执行和落盘，并留下运行记录。

**Content**:

1. 登记本地代码仓库。
2. 按 `.gitignore`、文件类型、大小和安全规则筛选项目文件。
3. 设置项目默认提示词、单文件覆盖和项目上下文。
4. 选择兼容 OpenAI Responses API 的服务、模型、并发和重试策略。
5. 创建 Run，冻结文件哈希、提示词、模型、上下文版本和 API 路由。
6. 按并发限制执行每个文件对应的 AI 请求。
7. 保存 Markdown 结果、Token、耗时、错误、重试与主备切换历史。

**Visual Element**:

- Type: numbered process flow
- Subject: 7 个连续节点，节点 5 用冻结快照图标强调不可变。
- Treatment: 从左到右或从上到下的工程流程线。

**Text Labels**:

- Headline: “一次分析，七个可追溯步骤”
- Labels: “登记”“扫描”“配置”“冻结”“执行”“记录”“导出”

---

## Section 3: 项目上下文，让模型看到文件在仓库里的位置

**Key Concept**: README、AGENTS.md 等项目资料先形成上下文摘要，再参与单文件分析。

**Content**:

- 导入或重新扫描项目时，自动发现仓库根目录下的 `README`、`README.md`、`README.*` 和 `AGENTS.md`。
- 系统从启用资料中生成“项目上下文摘要”。
- 默认提示词要求结合上下文说明文件职责、输入输出、协作模块及可能影响。
- ContextVersion 在 Run 创建时固定，自动重试继续使用原版本。

**Visual Element**:

- Type: context funnel
- Subject: README/AGENTS.md 文档汇入“摘要”节点，再进入文件请求。
- Treatment: 文档卡片带有“仅作为资料”标识，避免与系统指令混淆。

**Text Labels**:

- Headline: “不是孤立看函数，而是理解它为何存在”
- Labels: “README”“AGENTS.md”“上下文摘要”“文件职责”“数据流”

---

## Section 4: 四层工程架构

**Key Concept**: 前端展示、Rust 核心、SQLite 状态和外部模型服务各司其职。

**Content**:

- Tauri 2：跨 Windows、macOS、Linux 的桌面外壳。
- React、Vite、TypeScript：负责展示和用户交互。
- Rust、Tokio、reqwest：负责仓库扫描、路径校验、模型请求和业务状态变更。
- SQLite + sqlx：作为运行状态的权威来源。
- OS Keychain/Credential Manager/Secret Service：保存密钥；必要时 Stronghold 降级。
- Provider Adapter：兼容 OpenAI Responses API，首期使用非流式请求。

**Visual Element**:

- Type: layered architecture diagram
- Subject: React UI → Tauri IPC → Rust services → SQLite/Secret Store/Provider Adapter。
- Treatment: 文件系统和密钥路径用边界线圈出，标记“React 不直接访问”。

**Text Labels**:

- Headline: “把权限放在该在的地方”
- Labels: “React UI”“Tauri IPC”“Rust 核心”“SQLite”“Secret Store”“Provider Adapter”

---

## Section 5: 可靠性设计：每一次请求都留下痕迹

**Key Concept**: Run、Task、Attempt 分层记录，重试不会覆盖历史。

**Content**:

- Run 创建后，其文件清单、文件哈希、提示词、模型、上下文版本、API 路由和重试策略不可变。
- 所有自动重试、备用档案切换和人工重试均新增 Attempt，不覆盖历史 Attempt。
- SQLite 是运行状态的权威来源，事务提交成功后才更新内存状态并发送事件。
- 应用异常退出后，原处理中任务标记为“已中断/结果未知”，必须由用户决定是否重新排队。
- 首期只允许一个活动 Run。

**Visual Element**:

- Type: audit trail module
- Subject: Run → Task → Attempt 的层级与时间轴。
- Treatment: 用重复请求的分叉箭头表现“新增 Attempt”，旧记录保留。

**Text Labels**:

- Headline: “失败可以重试，历史不能被抹掉”
- Labels: “Run 快照”“Task”“Attempt 1”“Attempt 2”“结果未知”

---

## Section 6: 安全边界不是附加项

**Key Concept**: 本地代码、密钥和输出路径都要经过显式边界检查。

**Content**:

- API Key 不得出现在普通配置、SQLite 明文字段、日志、错误信息、导出文件或测试快照中。
- 所有输入和输出路径必须经过仓库边界、符号链接和路径逃逸检查。
- 默认排除常见密钥文件，并对疑似敏感内容、符号链接和危险 Markdown 做安全处理。
- 模型返回内容不得触发客户端命令执行、文件写入或 API 配置修改。

**Visual Element**:

- Type: warning checklist
- Subject: 密钥、符号链接、路径逃逸、危险 Markdown 四个警示点。
- Treatment: 使用高亮色只标记风险，不制造恐吓式视觉。

**Text Labels**:

- Headline: “代码可以送去分析，密钥不能跟着走”
- Labels: “密钥隔离”“路径校验”“符号链接”“Markdown 安全”

---

## Section 7: 三个平台，一套本地工作流

**Key Concept**: 通过 GitHub Actions 构建 Windows、macOS、Linux 安装包。

**Content**:

- Windows：`.msi`、`.exe`（NSIS）。
- macOS：Intel 和 Apple Silicon `.dmg`。
- Linux：`.AppImage`、`.deb`。
- 开发者可以运行 `pnpm tauri:build` 在本机生成安装包。
- 发布工作流当前未配置代码签名；macOS 首次打开可能需要在“隐私与安全性”中允许，Windows 可能显示 SmartScreen 提示。

**Visual Element**:

- Type: platform matrix
- Subject: Windows、macOS、Linux 三个平台卡片与对应产物。
- Treatment: 用同一条发布线连接三平台，突出“本地、跨平台”。

**Text Labels**:

- Headline: “从仓库到安装包，发布链路也可见”
- Labels: “Windows”“macOS”“Linux”“GitHub Actions”“Tauri Updater”

---

## Data Points (Verbatim)

### Statistics

- “并发请求数：文件 Task 同时在飞的最大网络请求数，可配置为 `1`～`30`；`3`。”
- “文件大小上限：默认 `256 KB`。”
- “10,000 个文件可以完成扫描和虚拟表格展示。”

### Key Terms

- **Run**：用户一次正式点击执行所创建的批量分析。
- **Task**：某个 Run 中对一个文件进行一次分析。
- **Attempt**：Task 实际向某个 API 档案发送的一次网络请求。
- **ContextVersion**：一次生成或人工编辑后的项目上下文摘要。

---

## Design Instructions

### Style Preferences

- 技术手册感、蓝图网格、低饱和青绿色和少量荧光重点色。
- 标签清晰，中文可读，避免过度营销与装饰性插画。

### Layout Preferences

- 高密度模块布局，7 个模块对应产品定位、流程、上下文、架构、可靠性、安全和发布。
- 每个模块有编号或坐标标识，流程和层级关系必须一眼可见。

### Other Requirements

- Target platform: 公众号文章首图/正文配图。
- Language: zh。
- Aspect recommendation: portrait 9:16，便于手机端阅读和转发。
