# TASK-0124：完整 AI 请求提示词预览

- Status: Done（全量基线门禁存在既有阻塞，见验证记录）
- Owner: Codex
- Branch: feat/m1-run-results-ui
- Dependencies: TASK-0116, TASK-0123

## 目标

用户在运行结果中点击“查看提示词”时，看到与该 Task 实际模型请求结构一致的完整请求
预览，包括系统指令、冻结项目上下文、用户任务目标、目标文件路径和完整代码内容，而不是
只看到 `promptSnapshot`。

## 必读文档

- `AGENTS.md`
- `docs/prd.md`：5.1、5.2、6.2、6.3、7.3、10.3
- `docs/architecture.md`：12.3、13.1、17、19.1
- `docs/contracts/ipc-contract.md`：4.7、4.8
- `docs/contracts/error-codes.md`：4.2、4.4、4.6
- `tasks/milestone-01/TASK-0116-run-results-ui.md`
- `tasks/milestone-01/TASK-0123-complete-analysis-prompt.md`

## 允许修改

```text
tasks/milestone-01/TASK-0124-complete-request-prompt-preview.md
docs/contracts/ipc-contract.md
crates/app-core/**
crates/ipc-contracts/**
apps/desktop/src-tauri/src/**
apps/desktop/src/app/**
apps/desktop/src/ipc/**
apps/desktop/src/styles.css
packages/ipc-types/src/**
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
docs/contracts/error-codes.md
docs/contracts/database-schema.md
docs/contracts/task-state-machine.md
docs/decisions/**
tauri.conf.json
.github/workflows/**
```

## 公共契约变更建议

新增显式读取命令：

```text
task_request_preview(TaskRequestPreviewRequest) -> TaskRequestPreviewResponse
```

- 请求包含 `projectId` 和 `taskId`；
- 响应包含安全的 `TaskSummaryDto`、`instructions` 和完整 `input`；
- 仅用户点击预览时调用，不扩展日常使用的 `task_get`，避免查看 Attempt 时顺带返回源码；
- 复用 `project_path_unavailable`、`security_path_escape`、`scan_file_unreadable`、
  `task_source_changed` 和 `task_not_found`，不新增错误码。

## 行为要求

1. App Core 使用 TASK-0123 的同一组装函数生成预览，禁止维护第二套提示词格式。
2. Rust 必须验证 Project/Run/Task 归属、仓库边界、文件可读性和内容哈希；源码变化时返回
   `task_source_changed`，不得用当前新内容伪装历史请求。
3. ContextVersion 使用 Task 冻结版本；版本缺失或归属不一致时不回退到项目当前上下文。
4. 前端只在点击“查看提示词”时调用新命令，并在弹窗关闭或切换 Run 后释放预览状态。
5. 弹窗明确展示 `instructions` 和 `input`，完整代码内容可滚动查看，不做省略或截断。
6. 完整请求不得进入普通 Task DTO、日志、数据库、结果文件、错误详情或测试快照。

## 不在范围内

- 保存历史请求原文或旧源码副本；
- 源码变化后展示创建 Run 时的旧内容；
- 修改模型请求结构、重试策略或 Attempt 数据模型；
- 新增数据库 Migration 或错误码；
- 调用真实收费模型 API。

## 验收标准

- [x] 点击“查看提示词”后显示系统指令和包含完整源码的请求正文；
- [x] 普通 `task_get` 仍不返回源码；
- [x] 预览与 Mock Provider 捕获到的实际请求一致；
- [x] 源码变化、路径逃逸、不可读取和跨 Project 查询安全失败；
- [x] IPC Rust/TypeScript 类型与契约文档同步；
- [x] 前端成功、加载和失败路径有测试；
- [x] 格式化、lint、类型检查和相关测试通过；
- [x] 没有越界修改。

## 验证记录

- `pnpm install --frozen-lockfile`、`pnpm format:check`、`pnpm lint`、
  `pnpm typecheck`、`pnpm test`、`pnpm ipc:check`：通过，前端 48 个测试通过。
- `cargo fmt --all -- --check`：通过。
- App Core 36 个、IPC Contracts 4 个、Desktop 11 个 Rust 测试通过。
- App Core、IPC Contracts 和 Desktop 的范围内 Clippy 均通过。
- `cargo clippy --workspace --all-targets -- -D warnings`：被未修改的
  `crates/secret-store/src/lib.rs` 7 条既有 Pedantic 告警阻断。
- `cargo test --workspace`：本任务及先执行到的包通过；未修改的
  `crates/persistence/src/database.rs` 有 3 个 Windows 临时 SQLite 清理测试因
  `os error 32` 失败。
- 现有 Tauri 开发实例已自动重编译并重新启动，Vite 服务地址为
  `http://localhost:1420`。

## 交付格式

1. 修改文件；
2. 已实现行为；
3. 测试命令与结果；
4. 已知限制；
5. 契约变更建议；
6. 合并注意事项。
