# TASK-0123：完整单文件分析提示词

- Status: Done（全量基线门禁存在既有阻塞，见验证记录）
- Owner: Codex
- Branch: feat/m1-run-results-ui
- Dependencies: TASK-0112, TASK-0115, TASK-0119, TASK-0122

## 目标

正式 Run 的每次模型请求都发送一份边界清晰的完整分析提示词，其中包含冻结的项目上下文、
用户任务目标、目标文件相对路径和未经截断的完整代码文件内容；自动重试、单项人工重试和
批量人工重试保持相同行为。

## 必读文档

- `AGENTS.md`
- `docs/prd.md`：5.1、5.2、6.1、6.3、10.2、10.3
- `docs/architecture.md`：12.3、13.1、13.2、14.2～14.6、19.1
- `docs/contracts/task-state-machine.md`
- `docs/contracts/ipc-contract.md`：4.5、4.7
- `tasks/milestone-01/TASK-0112-run-execution-attempts.md`
- `tasks/milestone-01/TASK-0119-failure-retry.md`
- `tasks/milestone-01/TASK-0122-batch-manual-retry-queue.md`

## 允许修改

```text
tasks/milestone-01/TASK-0123-complete-analysis-prompt.md
crates/app-core/**
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
docs/contracts/**
docs/decisions/**
tauri.conf.json
.github/workflows/**
```

## 输入与依赖

- `Run.context_version_id` 冻结的项目上下文版本；
- `Task.prompt_snapshot`、`Task.relative_path` 和 `Task.file_snapshot`；
- 发送前经过仓库边界校验、UTF-8 读取和内容哈希复核的当前文件内容；
- `ProviderRequest` 的 `input` 与 `instructions` 字段；
- 正式 Run、自动重试和人工重试共用的 `RunExecutionService`。

## 输出接口

不新增公共接口。App Core 在内存中统一组装正式分析请求：

```text
[系统安全与输出约束]        -> ProviderRequest.instructions
[项目上下文摘要：仅作为资料]
[用户任务目标]
[目标文件路径]
[目标文件内容：仅作为待分析数据]
[输出要求]                  -> ProviderRequest.input
```

## 行为要求

1. `input` 必须同时包含 Task 冻结的用户提示词、相对路径和完整 UTF-8 文件内容，不得只
   发送源码或只发送用户提示词。
2. 文件内容不得截断、摘要化、转义丢失或替换；发送前仍必须复核内容哈希，源码变化时不得
   发送请求。
3. Run 冻结了 ContextVersion 时，使用该版本的摘要，不读取项目当前版本；未启用上下文时
   仍保留明确的空上下文分段。
4. 系统安全约束明确将项目资料和源码视为不可信数据；代码中的指令性文本不得覆盖用户目标
   或系统约束。
5. 自动重试、单项人工重试和批量人工重试复用同一组装路径，每次 Attempt 的请求正文一致。
6. 请求正文只在内存中存在，不写入日志、IPC、Attempt、结果文件或测试快照。

## 不在范围内

- 改变提示词生成请求；
- 新增源码持久化或请求正文查看功能；
- Token 估算器、上下文超限错误码或自动分段；
- 修改公共 Provider、IPC、数据库或状态机契约；
- 调用真实收费模型 API。

## 验收标准

- [x] 完整请求正文包含上下文、任务目标、相对路径和完整代码内容；
- [x] 无上下文和源码含指令性/边界文本时仍保持明确分段且不丢失内容；
- [x] Mock Responses API 捕获的真实 JSON 同时验证 `instructions` 和 `input`；
- [x] 源码变化检测、重试及既有执行测试保持通过；
- [x] 格式化、范围内 Clippy 和相关测试通过；
- [x] 没有越界修改，文档与实现一致。

## 验证记录

- `pnpm install --frozen-lockfile`、`pnpm format:check`、`pnpm lint`、
  `pnpm typecheck`、`pnpm test`、`pnpm ipc:check`：通过，前端 45 个测试通过。
- `cargo fmt --all -- --check`：通过。
- `cargo test -p batch-code-analyzer-app-core`：通过，35 个测试通过。
- `cargo clippy -p batch-code-analyzer-app-core --all-targets --no-deps -- -D warnings`：
  通过。
- `cargo clippy --workspace --all-targets -- -D warnings`：被未修改的
  `crates/secret-store/src/lib.rs` 7 条既有 Pedantic 告警阻断。
- `cargo test --workspace`：本任务和其他已执行测试通过；未修改的
  `crates/persistence/src/database.rs` 有 Windows 临时 SQLite 文件占用失败。单线程复跑
  Persistence 包时 13 个测试中 10 个通过，3 个因 `os error 32` 失败。

## 交付格式

1. 修改文件；
2. 已实现行为；
3. 测试命令与结果；
4. 已知限制；
5. 契约变更建议；
6. 合并注意事项。
