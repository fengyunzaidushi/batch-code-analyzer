# TASK-0121：可配置并发与批量取消规模验收

- Status: Done（全量基线门禁存在既有阻塞，见验证记录）
- Owner: Codex
- Branch: feat/m1-run-results-ui
- Dependencies: TASK-0113, TASK-0119, TASK-0120

## 目标

让用户在项目运行设置中控制未来 Run 的文件请求并发数，并验证包含数百个排队 Task
的 Run 可以通过一次取消操作原子收敛，满足后续 10～30 并发和大型仓库的使用场景。

## 必读文档

- `AGENTS.md`
- `docs/prd.md`：2.4、3、4.3、4.6、5.1、5.4、10.3、11.1
- `docs/architecture.md`：6.3、14.1～14.6、23.2～23.3
- `docs/contracts/ipc-contract.md`：4.1、4.5
- `docs/contracts/task-state-machine.md`
- `tasks/milestone-01/TASK-0113-project-run-settings.md`
- `tasks/milestone-01/TASK-0120-run-concurrency.md`

## 允许修改

```text
tasks/milestone-01/TASK-0121-concurrency-settings-batch-cancel.md
docs/prd.md
docs/architecture.md
docs/contracts/ipc-contract.md
crates/app-core/**
crates/ipc-contracts/**
crates/persistence/tests/**
apps/desktop/src-tauri/src/**
apps/desktop/src/app/**
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
crates/persistence/src/**
docs/contracts/error-codes.md
tauri.conf.json
.github/workflows/**
```

## 输入与依赖

- 项目的 `ExecutionDefaults.concurrency` 和 Run 的不可变 `snapshot.concurrency`；
- TASK-0120 已实现的滑动窗口式有界并发执行器；
- 已有 `project_update_run_settings`、`run_preview` 和 `run_cancel` IPC；
- 已有 Repository 单事务 Run 取消操作。

## 公共契约变更建议

- `ProjectRunSettingsUpdateRequest` 增加必填 `concurrency: u16`；
- `ProjectDetailDto` 增加 `concurrency: u16`；
- `RunPreviewResponse` 增加 `concurrency: u16`；
- 继续复用 `validation_invalid_value`，不新增公共错误码；
- `run_cancel` 保持 Run 级单次操作，不新增逐 Task 批量 IPC。

## 行为要求

1. 新项目默认并发为 3；用户可将项目并发设置为 1～30 的整数。
2. Rust 领域服务必须执行范围校验，不能只依赖前端输入控件；0、31 及其他越界值返回
   `validation_invalid_value`，且不得修改 SQLite 或配置镜像。
3. 并发设置只影响之后创建的 Run；已经创建的 Run 继续使用冻结快照。
4. Run 预览显示本次创建后实际冻结的并发数。
5. 对数百个文件，执行器保持至多 N 个 worker 在飞；任一 worker 结束后从队列领取下一项，
   不一次创建数百个网络 Future。
6. 一次 `run_cancel` 必须在一个事务内取消该 Run 的全部 queued Task；300 个 queued Task
   全部进入 `cancelled`，Run 进入 `cancelled`，不遗留 queued/running Task，也不新增 Attempt。
7. 项目 JSON 镜像包含非敏感的 `executionDefaults`，SQLite 仍为运行状态权威来源。

## 不在范围内

- 动态修改已经创建或正在执行的 Run 并发快照；
- 同时运行多个正式 Run、按 API Profile 分别限流；
- 新增多选 Task 取消 IPC；
- 修改取消状态机、数据库 Migration 或生产 Repository SQL；
- 将运行状态或 Attempt 历史改为 Markdown 权威存储。

## 验收标准

- [x] 并发 1 和 30 可保存，0 和 31 被 Rust 拒绝；
- [x] 项目详情、保存响应和 JSON 配置镜像包含并发设置；
- [x] 已创建 Run 的并发快照不随项目设置变化，新 Run 使用新设置；
- [x] 前端数字控件限制为 1～30，非法值不能提交；
- [x] Run 预览显示实际并发数；
- [x] 300 个 queued Task 可由一次取消全部原子收敛且没有 Attempt；
- [x] IPC TypeScript 生成文件与 Rust DTO 一致；
- [x] 格式化、范围内静态检查、相关单元测试和集成测试通过；
- [x] 没有越界修改。

## 验证记录

- `pnpm install --frozen-lockfile`：通过，依赖已是最新且未修改锁文件。
- `pnpm format:check`、`pnpm lint`、`pnpm typecheck`、`pnpm test`、
  `pnpm ipc:check`：通过，前端 6 个测试文件、42 个测试通过。
- `cargo fmt --all -- --check`：通过。
- `cargo clippy -p batch-code-analyzer-app-core -p batch-code-analyzer-ipc-contracts
  -p batch-code-analyzer-persistence -p batch-code-analyzer-desktop --all-targets --no-deps
  -- -D warnings`：通过。
- `cargo test -p batch-code-analyzer-app-core`：31 个测试通过。
- `cargo test -p batch-code-analyzer-persistence --test repositories`：10 个测试通过，
  包含 300 queued Task 批量取消测试。
- `cargo test -p batch-code-analyzer-desktop`：11 个测试通过。
- `cargo clippy --workspace --all-targets -- -D warnings`：被未修改的
  `crates/secret-store/src/lib.rs` 7 条既有 Pedantic 告警阻断。
- `cargo test --workspace`：本任务及运行到的相关测试通过；未修改的
  `crates/persistence/src/database.rs` 有 3 个 Windows 临时 SQLite 文件清理测试因文件占用
  (`os error 32`) 失败。

## 交付格式

1. 修改文件；
2. 已实现行为；
3. 测试命令与结果；
4. 已知限制；
5. 契约变更建议；
6. 合并注意事项。
