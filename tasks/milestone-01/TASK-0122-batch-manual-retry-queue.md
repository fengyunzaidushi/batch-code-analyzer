# TASK-0122：批量重试与连续单项重试

- Status: Done（全量基线门禁存在既有阻塞，见验证记录）
- Owner: Codex
- Branch: feat/m1-run-results-ui
- Dependencies: TASK-0119, TASK-0120, TASK-0121

## 目标

补齐 PRD 已定义但 TASK-0119 未实现的批量重试，并移除运行结果页的人工重试全表锁。
用户既可以一次重试当前 Run 的全部失败 Task，也可以连续点击多个单项重试；每个 Task
仍复用原 Run 快照并追加 Attempt，不覆盖历史。

## 必读文档

- `AGENTS.md`
- `docs/prd.md`：2.1、4.6、5.3、5.6、10.3
- `docs/architecture.md`：14.2～14.6、15
- `docs/contracts/ipc-contract.md`：4.5、4.7
- `docs/contracts/task-state-machine.md`
- `tasks/milestone-01/TASK-0119-failure-retry.md`
- `tasks/milestone-01/TASK-0120-run-concurrency.md`

## 允许修改

```text
tasks/milestone-01/TASK-0122-batch-manual-retry-queue.md
docs/prd.md
docs/architecture.md
docs/contracts/ipc-contract.md
crates/app-core/**
crates/persistence/src/repositories/**
crates/persistence/tests/**
crates/ipc-contracts/**
apps/desktop/src-tauri/src/**
apps/desktop/src/app/**
apps/desktop/src/ipc/**
apps/desktop/src/styles.css
packages/ipc-types/src/**
```

## 禁止修改

```text
Cargo.toml（Workspace 根）
Cargo.lock
package.json
pnpm-lock.yaml
pnpm-workspace.yaml
crates/persistence/migrations/**
crates/domain/**
docs/contracts/error-codes.md
tauri.conf.json
.github/workflows/**
```

## 公共契约变更建议

- 新增 `task_retry_batch(TaskRetryBatchRequest) -> TaskRetryBatchResponse`；
- 请求包含 `projectId`、`runId` 和同一 Run 下的 `taskIds`，数量为 `1..=10,000`；
- 响应包含最终 Run、实际提交的 Task ID 和被跳过的 Task ID；
- 继续复用 `task_not_found`、`task_cannot_retry`、`run_active_exists` 和
  `validation_limit_exceeded`，不新增公共错误码。

## 行为要求

1. 批量命令在一个数据库事务内验证 Project/Run/Task 归属、活动 Run 冲突和最新
   Attempt；符合条件的 `failed` Task 转为 `queued`，Run 从
   `completed_with_errors` 转为 `running`。
2. 同一批次中状态不允许或最新错误不可重试的 Task 被跳过；Task 不存在、跨 Run 或跨
   Project 时整个请求失败且不产生部分更新。没有任何可提交 Task 时返回
   `task_cannot_retry`。
3. 批量重排提交后只启动一个 Run 执行器，并继续遵守冻结的并发上限；真实请求前才追加
   Attempt，源码哈希变化时不发送请求。
4. 单项重试保持逐个 IPC 提交，但前端维护待提交队列；一项执行时其他失败行仍可点击，
   每行只能加入一次，队列按点击顺序继续执行。
5. “重试全部失败”一次提交当前选中 Run 的失败 Task，并显示目标数量；批量执行期间禁止
   重复批量提交。
6. 用户取消正在执行的重试 Run 时，前端尚未提交的单项重试队列必须清空。

## 不在范围内

- 跨 Run、跨 Project 批量重试；
- cancelled/interrupted Task 的重复计费确认流程；
- 成功 Task 重新生成；
- 动态修改 Run 快照或同时启动多个执行器；
- 新增数据库 Migration 或错误码。

## 验收标准

- [x] 两个以上可重试失败 Task 可在一个事务内重新排队并按 Run 并发执行；
- [x] 不可重试项被跳过，跨 Run/不存在项原子失败；
- [x] 批量与单项重试均追加 Attempt 并保留旧历史；
- [x] 一项重试中，其他失败行仍可点击并进入待提交队列；
- [x] 结果页提供“重试全部失败（N）”且防重复提交；
- [x] 取消 Run 会清空尚未提交的前端重试队列；
- [x] IPC TypeScript 生成文件与 Rust DTO 一致；
- [x] 格式化、范围内静态检查和相关测试通过；
- [x] 没有越界修改。

## 验证记录

- `pnpm install --frozen-lockfile`、`pnpm format:check`、`pnpm lint`、
  `pnpm typecheck`、`pnpm test`、`pnpm ipc:check`：通过，前端 45 个测试通过。
- `cargo fmt --all -- --check`：通过。
- Persistence、App Core、IPC Contracts、Desktop 四个受影响包的
  `cargo clippy ... --all-targets --no-deps -- -D warnings`：通过。
- App Core 32 个、Persistence Repository 12 个、Desktop 11 个、IPC Contracts
  4 个相关 Rust 测试通过。
- `cargo clippy --workspace --all-targets -- -D warnings`：被未修改的
  `crates/secret-store/src/lib.rs` 7 条既有 Pedantic 告警阻断。
- `cargo test --workspace`：本任务测试通过；未修改的
  `crates/persistence/src/database.rs` 有 3 个 Windows 临时 SQLite 清理测试因文件占用
  (`os error 32`) 失败。

## 交付格式

1. 修改文件；
2. 已实现行为；
3. 测试命令与结果；
4. 已知限制；
5. 契约变更建议；
6. 合并注意事项。
