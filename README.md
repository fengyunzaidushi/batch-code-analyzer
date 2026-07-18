# Batch Code Analyzer

本仓库用于开发跨 Windows、macOS、Linux 的本地批量代码文件 AI 分析工具。

## 产品能力

用户可以登记多个本地代码仓库，按 `.gitignore`、文件类型、大小和安全规则筛选项目文件；为项目或单文件设置提示词与模型；批量调用兼容 OpenAI Responses API 的服务；查看 Markdown 结果、Token、耗时、错误、重试与主备切换历史；并将每次运行的结果安全地写入独立输出目录。

## 技术栈

- 桌面外壳：Tauri 2
- 前端：React、Vite、TypeScript、TanStack Query/Table/Virtual、Zustand
- 核心后端：Rust、Tokio、reqwest
- 数据库：SQLite + sqlx
- 安全存储：系统 Keychain/Credential Manager/Secret Service，必要时 Stronghold 降级
- 发布：GitHub Actions、Tauri Updater

## 文档导航

| 文档                                   | 用途                                     |
| -------------------------------------- | ---------------------------------------- |
| `AGENTS.md`                            | 所有 AI Agent 和开发者必须遵守的仓库规则 |
| `docs/prd.md`                          | 产品需求、范围、默认决策和验收标准       |
| `docs/architecture.md`                 | 总体架构、模块划分、数据设计、发布方案   |
| `docs/contracts/ipc-contract.md`       | Tauri Command、Event 和 DTO 约定         |
| `docs/contracts/database-schema.md`    | SQLite 表、索引、事务和迁移约定          |
| `docs/contracts/task-state-machine.md` | Run、Task、Attempt 状态与合法转换        |
| `docs/contracts/error-codes.md`        | 稳定错误结构、分类与错误码命名           |
| `docs/decisions/`                      | 已批准的架构决策记录                     |
| `tasks/`                               | 可直接交给 Agent 的任务书与模板          |

## 推荐启动方式

不要把整个产品一次性交给一个 Agent 实现。按以下顺序推进：

1. 总控 Agent 阅读完整 PRD 与架构；
2. 完成 `tasks/milestone-00/`，建立工程骨架和公共契约；
3. 骨架、Migration、DTO、错误码和 CI 稳定后，再创建 4～6 个 worktree；
4. 按 `tasks/milestone-01/` 并行开发领域、数据库、扫描、Provider 和前端壳；
5. 集成 Agent 逐个审查、合并并运行全量测试。

## Worktree 示例

```bash
git switch -c develop

git worktree add ../bca-domain \
  -b feat/m1-domain-db develop

git worktree add ../bca-scanner \
  -b feat/m1-scanner-security develop

git worktree add ../bca-provider \
  -b feat/m1-model-provider develop

git worktree add ../bca-frontend \
  -b feat/m1-frontend-shell develop

git worktree list
```

首轮不要同时启动 20～30 个编码 Agent。公共契约未稳定前，过度并行会导致数据库、IPC、状态机和依赖发生大量冲突。

## 当前仓库状态

仓库已完成基础工程初始化，包含 pnpm monorepo、Cargo workspace、`apps/desktop`（Tauri 2 + React + Vite）以及 `crates/*` 基础模块。

## 本地启动（macOS）

### 1. 前置依赖

```bash
xcode-select --install
corepack enable
corepack prepare pnpm@11.9.0 --activate
rustup toolchain install stable
```

> 说明：Node.js 建议使用 20+（推荐 22 LTS）。

### 2. 安装依赖

```bash
pnpm install
```

### 3. 启动方式

启动桌面应用（推荐）：

```bash
pnpm tauri:dev
```

仅启动前端开发服务器：

```bash
pnpm dev
```

### 4. 启动前健康检查（可选）

```bash
cargo check --workspace
pnpm typecheck
```
