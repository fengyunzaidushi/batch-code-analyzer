# TASK-0126：失败结果原因预览

- Status: Done（全量基线门禁存在既有阻塞，见验证记录）
- Owner: Codex
- Branch: feat/m1-run-results-ui
- Dependencies: TASK-0116, TASK-0119

## 目标

失败 Task 也提供“查看结果”操作。用户点击后直接看到失败原因和 Attempt 明细，而不是
因为没有 Markdown 结果文件而只看到“当前 Task 没有可读取的结果”。

## 必读文档

- `AGENTS.md`
- `docs/prd.md`：4.5、5.3、5.6、6.3
- `docs/architecture.md`：9.4、9.5、10.2
- `docs/contracts/ipc-contract.md`：4.7、4.8
- `docs/contracts/error-codes.md`：4、5
- `tasks/milestone-01/TASK-0116-run-results-ui.md`
- `tasks/milestone-01/TASK-0119-failure-retry.md`

## 允许修改

```text
tasks/milestone-01/TASK-0126-failure-result-reason-preview.md
docs/prd.md
docs/architecture.md
docs/contracts/ipc-contract.md
apps/desktop/src/app/**
apps/desktop/src/styles.css
```

## 禁止修改

```text
Cargo.toml
Cargo.lock
package.json
pnpm-lock.yaml
pnpm-workspace.yaml
crates/**
packages/ipc-types/**
数据库 migrations
docs/contracts/error-codes.md
docs/contracts/database-schema.md
docs/contracts/task-state-machine.md
docs/decisions/**
tauri.conf.json
.github/workflows/**
```

## 设计约束

- 成功 Task 继续通过 `result_read` 按需读取 Markdown，不改变结果文件安全边界。
- 失败 Task 通过现有 `task_get` 读取按 sequence 升序排列的脱敏 Attempt 历史，不新增
  IPC、DTO、数据库字段或错误码。
- 优先显示最后一次带错误的 Attempt；同时展示每次 Attempt 的档案、模型、HTTP 状态、
  耗时、稳定错误码和失败原因。
- 已知错误码映射为明确中文原因。未知错误只有在 `sanitized = true` 时才显示 message；
  未脱敏内容不得进入 UI。
- 失败 Task 点击“查看结果”不得调用 `result_read`，避免用“结果不存在”覆盖真实原因。

## 验收标准

- [x] 失败 Task 显示“查看结果”操作；
- [x] 点击后显示最近一次失败原因和完整 Attempt 失败明细；
- [x] 已知错误码显示明确中文原因和稳定错误码；
- [x] 未脱敏错误消息不显示；
- [x] 成功 Markdown 结果读取与安全渲染行为不变；
- [x] PRD、架构和 IPC 契约同步；
- [x] 格式化、lint、类型检查和相关测试通过；
- [x] 没有越界修改。

## 验证记录

- `pnpm format:check`、`pnpm lint`、`pnpm typecheck`、`pnpm test`、
  `pnpm ipc:check`：通过，前端 51 个测试通过。
- 失败结果专项测试：AppShell 与 App 集成共 31 个测试通过，覆盖失败按钮、最近失败原因、
  多 Attempt 明细、未脱敏 message 屏蔽，以及失败 Task 不调用 `result_read`。
- `cargo fmt --all -- --check`：通过。
- Desktop 11 个 Rust 测试和 `--no-deps` 范围内 Clippy 通过。
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
