# TASK-0125：正式分析空 Instructions 与条件上下文

- Status: Done（全量基线门禁存在既有阻塞，见验证记录）
- Owner: Codex
- Branch: feat/m1-run-results-ui
- Dependencies: TASK-0123, TASK-0124

## 目标

正式单文件分析请求和请求预览始终使用空字符串 `instructions`；只有 Task 冻结的
ContextVersion 中确实存在已纳入的 `AGENTS.md` 或 `README*` 文件时，`input` 才包含
“项目上下文摘要”分段，不再发送“发现 0 个项目上下文文件”等占位内容。

## 必读文档

- `AGENTS.md`
- `docs/prd.md`：4.2、5.2、6.3
- `docs/architecture.md`：12.3、13.1、19.1
- `docs/contracts/ipc-contract.md`：4.7
- `tasks/milestone-01/TASK-0118-prompt-generation-empty-instructions.md`
- `tasks/milestone-01/TASK-0123-complete-analysis-prompt.md`
- `tasks/milestone-01/TASK-0124-complete-request-prompt-preview.md`

## 允许修改

```text
tasks/milestone-01/TASK-0125-empty-instructions-conditional-context.md
docs/prd.md
docs/architecture.md
docs/contracts/ipc-contract.md
crates/app-core/**
apps/desktop/src/app/**
```

## 禁止修改

```text
Cargo.toml
Cargo.lock
package.json
pnpm-lock.yaml
pnpm-workspace.yaml
crates/persistence/migrations/**
crates/domain/**
crates/ipc-contracts/**
packages/ipc-types/**
docs/contracts/error-codes.md
docs/contracts/database-schema.md
docs/contracts/task-state-machine.md
docs/decisions/**
tauri.conf.json
.github/workflows/**
```

## 契约调整

- `ProviderRequest.instructions` 对正式分析请求显式设置为 `""`，与提示词生成请求一致；
- `TaskRequestPreviewResponse.instructions` 字段保留，但值固定为空字符串；
- `input` 的项目上下文分段成为条件分段，仅在冻结 ContextVersion 含已纳入的
  `AGENTS.md` 或 `README*` 来源时出现；
- 不修改 DTO 结构、数据库 Schema 或公共错误码。

## 行为要求

1. 正式请求、自动重试、人工重试和请求预览共用相同的空 `instructions`。
2. 判断上下文是否存在必须依据 Task/Run 冻结 ContextVersion 的 `sourceFiles`，不得解析
   摘要文本或读取项目当前版本。
3. 仅 `included = true` 且文件名为大小写不敏感的 `AGENTS.md` 或以 `README` 开头的来源
   才允许加入项目上下文摘要。
4. 没有符合来源时，`input` 直接从“用户任务目标”开始，不发送空标题、占位语句或
   “0 个文件”摘要。
5. 有符合来源时仍发送完整冻结摘要，并保持与用户任务、路径和源码的明确边界。
6. 提示词生成请求行为不变；完整请求仍只在内存和显式预览 IPC 中存在。

## 不在范围内

- 删除 `instructions` JSON 字段；
- 修改上下文发现、生成或持久化流程；
- 将其他项目文件自动视作上下文来源；
- 修改模型、重试、Token 或数据库契约；
- 调用真实收费模型 API。

## 验收标准

- [x] 正式分析实际请求 JSON 的 `instructions` 为 `""`；
- [x] 无 `AGENTS.md/README*` 时请求和预览均不含项目上下文分段；
- [x] 有已纳入上下文来源时仍包含冻结摘要；
- [x] 未纳入或无关来源不会触发上下文分段；
- [x] 自动重试和人工重试请求保持一致；
- [x] PRD、架构和 IPC 契约同步；
- [x] 格式化、lint、类型检查和相关测试通过；
- [x] 没有越界修改。

## 验证记录

- `pnpm install --frozen-lockfile`、`pnpm format:check`、`pnpm lint`、
  `pnpm typecheck`、`pnpm test`、`pnpm ipc:check`：通过，前端 48 个测试通过。
- `cargo fmt --all -- --check`：通过。
- App Core 37 个、IPC Contracts 4 个、Desktop 11 个 Rust 测试通过。
- App Core、IPC Contracts 和 Desktop 的范围内 Clippy 通过；对含既有告警依赖的 crate
  使用 `--no-deps` 验证本次代码。
- `cargo clippy --workspace --all-targets -- -D warnings`：被未修改的
  `crates/secret-store/src/lib.rs` 7 条既有 Pedantic 告警阻断。
- `cargo test --workspace`：本任务及先执行到的包通过；未修改的
  `crates/persistence/src/database.rs` 有 3 个 Windows 临时 SQLite 清理测试因
  `os error 32` 失败。

## 交付格式

1. 修改文件；
2. 已实现行为；
3. 测试命令与结果；
4. 已知限制；
5. 契约变更建议；
6. 合并注意事项。
