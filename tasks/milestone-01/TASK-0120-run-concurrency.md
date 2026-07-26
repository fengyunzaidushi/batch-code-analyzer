# TASK-0120：Run 有界并发执行

- Status: Done（全量基线门禁存在既有阻塞，见验证记录）
- Owner: Codex
- Branch: feat/m1-run-results-ui
- Dependencies: TASK-0112, TASK-0119

## 目标

让 Run 执行器真正使用创建时冻结的并发快照，并将新项目的默认并发数调整为 3，
使三个文件可以同时进入处理状态且不会超过配置上限。

## 必读文档

- `AGENTS.md`
- `docs/prd.md`：2.4、3、4.3、8.1、8.3、10.3、11.1
- `docs/architecture.md`：14.1～14.4
- `docs/contracts/ipc-contract.md`：4.5
- `docs/contracts/task-state-machine.md`
- `tasks/milestone-01/TASK-0112-run-execution-attempts.md`
- `tasks/milestone-01/TASK-0119-failure-retry.md`

## 允许修改

```text
tasks/milestone-01/TASK-0120-run-concurrency.md
docs/prd.md
docs/architecture.md
docs/contracts/ipc-contract.md
crates/app-core/**
crates/persistence/Cargo.toml
crates/persistence/src/database.rs
crates/persistence/src/repositories/**
crates/persistence/tests/**
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
crates/ipc-contracts/**
packages/ipc-types/**
apps/desktop/**
docs/contracts/error-codes.md
tauri.conf.json
.github/workflows/**
```

## 输入与依赖

- Run 快照中的 `snapshot.concurrency`；
- 已有原子 `claim_next_task`、统一状态机和 Run 统计重算；
- TASK-0119 的单 Task 自动重试、人工重试和取消令牌；
- 仓库内本地 Mock Provider。

## 输出接口

- `RunExecutionService` 的有界并发调度行为；
- 新项目 `ExecutionDefaults.concurrency = 3`；
- 执行器内部错误时将活动 Run 和已领取 Task 收敛为中断状态的 Repository 操作。

## 行为要求

1. 执行器最多同时领取并执行 `run.snapshot.concurrency` 个 Task；该值在 Run 创建后不可变。
2. 新项目默认并发数为 3；已有项目配置和已创建 Run 快照保持不变。
3. 单个 Task 的自动重试在原 worker 内串行执行，不额外占用并发槽位，也不覆盖 Attempt。
4. 任一 worker 完成后才领取下一项；取消后停止领取，并等待在飞 worker 收敛。
5. 所有 worker 结束后统一重算统计并完成 Run；执行器内部错误不得遗留 Running Task。
6. `snapshot.concurrency = 0` 的旧数据安全按 1 个 worker 执行，不形成永久排队。
7. 模型请求允许并发，但同一进程中的 SQLite 写事务必须串行，避免延迟事务升级产生
   `SQLITE_BUSY` 并中断 Run。

## 不在范围内

- 多个正式 Run 同时运行；
- 按 API 档案分别限流、动态修改已创建 Run 的并发数；
- 暂停/恢复、备用档案健康和辅助请求统一队列；
- 前端新增并发设置控件或公共 IPC DTO 变更。

## 验收标准

- [x] 三个 Task、并发 3 时，Mock Provider 观察到三个同时在飞请求；
- [x] 三个 Task、并发 2 时，最大在飞请求不超过 2；
- [x] 自动重试保持同一 Task worker 和追加式 Attempt 语义；
- [x] 取消和内部错误后不继续领取新 Task，不遗留 Running Task；
- [x] 磁盘 SQLite 多连接下的写事务跨 `Database` clone 串行，不再因并发状态提交中断 Run；
- [x] 新项目默认并发为 3，PRD、架构和 IPC 文档已同步；
- [x] 格式化、前端门禁、范围内静态检查和相关测试通过；
- [ ] 全 Workspace Clippy 与测试通过（被任务范围外既有基线问题阻断）；
- [x] 没有越界修改。

## 验证记录

- `cargo fmt --all -- --check`：通过。
- `cargo clippy -p batch-code-analyzer-app-core -p batch-code-analyzer-persistence
  --all-targets --no-deps -- -D warnings`：通过。
- `cargo test -p batch-code-analyzer-app-core`：31 个测试通过。
- `cargo test -p batch-code-analyzer-persistence --test repositories`：9 个测试通过。
- `cargo test -p batch-code-analyzer-persistence
  disk_database_serializes_write_transactions_across_clones`：磁盘 SQLite 多连接写门闩
  回归测试通过。
- `pnpm format:check`、`pnpm lint`、`pnpm typecheck`、`pnpm test`、
  `pnpm ipc:check`：通过，前端 41 个测试通过。
- `cargo clippy --workspace --all-targets -- -D warnings`：被未修改的
  `crates/secret-store/src/lib.rs` 7 条既有 Pedantic 告警阻断。
- `cargo test --workspace`：本任务及相关测试通过；未修改的
  `crates/persistence/src/database.rs` 有 3 个 Windows 临时 SQLite 清理测试因文件占用
  (`os error 32`) 失败。

## 交付格式

1. 修改文件；
2. 已实现行为；
3. 测试命令与结果；
4. 已知限制；
5. 契约变更建议；
6. 合并注意事项。
