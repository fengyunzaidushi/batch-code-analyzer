# TASK-0112：Run Execution / Attempt Persistence

- Status: Done
- Owner: Codex
- Branch: feat/m1-run-execution
- Dependencies: TASK-0103, TASK-0105, TASK-0111

## 目标

让已创建的 Run 可以使用本地 Mock/Responses Provider 顺序执行 queued Task，并持久化 Attempt、Markdown 结果和最终 Run 状态。

## 必读文档

- `AGENTS.md`
- `docs/prd.md`：5.2～5.6、6.4、7.3
- `docs/architecture.md`：13、14、15、17
- `docs/contracts/task-state-machine.md`
- `docs/contracts/database-schema.md`：7、8、10
- `crates/model-providers/src/lib.rs`
- `crates/persistence/src/repositories/mod.rs`
- `crates/domain/src/entities.rs`

## 允许修改

```text
tasks/milestone-01/TASK-0112-run-execution-attempts.md
Cargo.lock
crates/app-core/**
crates/model-providers/**
crates/persistence/src/repositories/**
crates/persistence/tests/**
apps/desktop/src-tauri/Cargo.toml
apps/desktop/src-tauri/src/**
apps/desktop/src/app/**
apps/desktop/src/ipc/**
packages/ipc-types/src/**
docs/contracts/ipc-contract.md
```

## 禁止修改

```text
crates/persistence/migrations/**
暂停、继续、取消和崩溃恢复
多档案自动切换、自动重试策略和并发调度
真实收费服务测试、API Key 或源码日志
```

## 行为要求

1. `run_execute` 只接受已有 Run ID，要求 Run 为 `running`。
2. 顺序领取 queued Task；每个真实请求前先追加 `created` Attempt。
3. Provider 成功时先原子写入 Markdown，再提交 Attempt `succeeded`、Task `succeeded` 和结果路径。
4. Provider 失败时保存稳定错误摘要，Attempt 使用 `failed_retryable` 或 `failed_terminal`，Task 进入 `failed`。
5. 所有状态更新经过 Domain 状态机；Run 无剩余 queued/running Task 时进入 `completed` 或 `completed_with_errors`。
6. 请求体只在内存中构造，日志和 IPC 不返回源码、API Key 或完整 Provider 响应。

## 不在范围内

- 并发执行和全局 Semaphore；
- 自动重试、备用 Profile、暂停/取消和恢复；
- Context 摘要生成；
- 结果读取/打开文件夹 IPC；
- 完整运行历史页面。

## 验收标准

- [x] Mock Provider 成功、失败、结果写入和 Attempt 追加路径有测试；
- [x] Task/Attempt/Run 状态和统计保持一致；
- [x] 结果文件原子写入，失败不伪造成功状态；
- [x] IPC 只返回 Run 摘要和稳定错误；
- [x] 全量格式、lint、类型、前端和 Rust 测试通过；
- [x] 没有越界修改。
