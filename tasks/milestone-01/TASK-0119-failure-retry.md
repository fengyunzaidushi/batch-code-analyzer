# TASK-0119：失败自动重试与人工重试

- Status: Done（全量基线门禁存在既有阻塞，见验证记录）
- Owner: Codex
- Branch: feat/m1-run-results-ui
- Dependencies: TASK-0112, TASK-0116

## 目标

补齐原始产品设计中的失败重试闭环：可重试的 Provider 错误按照 Run 冻结策略在
同一 Task 下自动新增 Attempt；自动重试耗尽后，用户可以在运行结果页使用原快照
人工重试失败 Task，并查看完整的追加式 Attempt 历史。

## 必读文档

- `AGENTS.md`
- `docs/prd.md`：4.3、4.5、5.3、5.5、5.6、9
- `docs/architecture.md`：14.2～14.6、15
- `docs/contracts/ipc-contract.md`：4.5、4.7
- `docs/contracts/task-state-machine.md`
- `docs/contracts/error-codes.md`：4.7
- `tasks/milestone-01/TASK-0112-run-execution-attempts.md`
- `tasks/milestone-01/TASK-0116-run-results-ui.md`

## 允许修改

```text
tasks/milestone-01/TASK-0119-failure-retry.md
docs/contracts/ipc-contract.md
docs/contracts/task-state-machine.md
docs/architecture.md
crates/domain/**
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
docs/contracts/error-codes.md
tauri.conf.json
.github/workflows/**
```

## 输入与依赖

- Run 快照中的 `retryPolicy.retryCountPerProfile`；
- `ProviderError` 的稳定错误码、`retryable` 和 `Retry-After`；
- 已有 `TaskTransition::ManualRetry` 与追加式 Attempt Repository；
- `RunExecutionService`、`task_get` 和运行结果页。

## 输出接口

- `RunTransition::ManualRetryRequested`；
- `task_retry(TaskRetryRequest) -> TaskRetryResponse`；
- 运行结果页失败 Task 的“重试”操作。

## 行为要求

1. 网络、超时、`429`、`5xx` 和无效响应按冻结的每档案重试次数执行；每次真实
   网络请求新增 Attempt。
2. `Retry-After` 优先；缺失时使用 5、10、20 秒退避并加入约 ±20% 抖动；取消
   Run 时停止等待，不继续发送。
3. 中间失败 Attempt 为 `failed_retryable`；策略耗尽或不可重试时为
   `failed_terminal`，Task 才进入 `failed`。
4. 人工重试仅允许最新错误仍标记为可重试的 `failed` Task；原子地执行
   `CompletedWithErrors -> Running` 和 `Failed -> Queued`，不覆盖旧 Attempt。
5. 人工重试复用 Run 创建时冻结的文件、提示词、模型、路由、超时和重试策略；
   跨项目 Task 按不存在处理。
6. 同一时间仍只允许一个活动 Run；重复点击或非法状态返回稳定错误码。

## 不在范围内

- 备用 API Profile 切换和 Run 内档案健康/冷却；
- cancelled/interrupted Task 的重新排队与重复计费确认；
- 成功 Task 的重新生成、新 Task 版本和批量重试；
- 并发调度、暂停、恢复和后台事件推送。

## 验收标准

- [x] 自动重试成功、耗尽和不可重试路径有测试；
- [x] 人工重试成功、非法状态、不可重试错误和跨项目访问有测试；
- [x] React 重试交互、防重复提交和 Attempt 刷新有测试；
- [x] 不调用真实收费模型 API，测试只使用本地 Provider；
- [x] 格式化、前端 lint/类型检查、范围内 Clippy 和相关测试通过；
- [ ] 全 Workspace Clippy 与测试通过（被任务范围外既有基线问题阻断）；
- [x] 没有越界修改，公共 IPC 与生成 TypeScript 类型同步；
- [x] 文档已同步。

## 验证记录

- `pnpm install --frozen-lockfile`、`pnpm format:check`、`pnpm lint`、
  `pnpm typecheck`、`pnpm test`、`pnpm ipc:check`：通过，前端 41 个测试通过。
- `cargo fmt --all -- --check`：通过。
- 本任务涉及包的 `cargo clippy ... --no-deps -- -D warnings`：通过。
- 自动重试 6 个、人工重试 4 个、Persistence Repository 集成 8 个专项测试：通过。
- `cargo clippy --workspace --all-targets -- -D warnings`：被未修改的
  `crates/secret-store/src/lib.rs` 7 条既有 Pedantic 告警阻断。
- `cargo test --workspace`：本任务测试通过；未修改的
  `crates/persistence/src/database.rs` 有 3 个 Windows 临时 SQLite 清理测试因文件占用
  (`os error 32`) 失败，单线程重跑仍可复现。

## 交付格式

1. 修改文件；
2. 已实现行为；
3. 测试命令与结果；
4. 已知限制；
5. 契约变更建议；
6. 合并注意事项。
