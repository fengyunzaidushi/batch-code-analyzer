# TASK-0111：Run Preview / Run Creation IPC

- Status: Done
- Owner: Codex
- Branch: feat/m1-run-preview
- Dependencies: TASK-0002, TASK-0003, TASK-0102, TASK-0109, TASK-0110

## 目标

在不发送模型请求的前提下，完成从当前项目文件选择到不可变 Run/Task 快照创建的闭环。

## 必读文档

- `AGENTS.md`
- `docs/prd.md`：5.1～5.3、6.1、8.2
- `docs/architecture.md`：10.2、14、15、17
- `docs/contracts/ipc-contract.md`：4.5、4.7
- `docs/contracts/task-state-machine.md`
- `docs/contracts/database-schema.md`：7、8、10
- `crates/domain/src/entities.rs`
- `crates/persistence/src/repositories/mod.rs`
- `apps/desktop/src/app/AppShell.tsx`

## 允许修改

```text
tasks/milestone-01/TASK-0111-run-preview-creation.md
crates/app-core/**
crates/persistence/src/repositories/**
crates/persistence/tests/**
crates/ipc-contracts/**
apps/desktop/src-tauri/src/**
apps/desktop/src/app/**
apps/desktop/src/ipc/**
packages/ipc-types/src/**
docs/contracts/ipc-contract.md
```

## 禁止修改

```text
crates/persistence/migrations/**
任务调度器、模型请求、Attempt 执行和输出写入
数据库 Migration、公共 Workspace 依赖和锁文件
```

## 输入与依赖

- 使用 Domain 的 `RunSnapshot`、`Run`、`Task`、`FileSnapshot` 和状态枚举。
- 使用 Repository 的 `create_run_with_tasks`、`list_file_records`、`list_runs`。
- 使用项目的 `ApiRouting`、`ExecutionDefaults` 和默认提示词。
- 目标文件必须来自最新已提交扫描结果，并满足纳入、安全和内容哈希条件。

## 输出接口

- `RunPreviewRequest` / `RunPreviewResponse`；
- `RunCreateRequest` / `RunCreateResponse`；
- `RunBlockingReasonDto`、`RunPreviewTaskDto`、`RunSummaryDto`；
- Tauri Commands：`run_preview`、`run_create`；
- 前端 IPC 适配和预览确认面板。

## 行为要求

1. 预览只读，不创建 Run、Task 或 Attempt。
2. 创建前校验主 API Profile、目标文件、内容哈希、提示词、模型和输出目录。
3. 每个纳入文件创建一个 `Task`，保存文件快照、提示词快照、模型快照和哈希。
4. Run 与全部初始 Task 必须通过一个事务创建；创建后 Run 进入 `running`，Task 进入 `queued`。
5. 同一项目存在活动 Run 时返回 `run_active_exists`，不得创建第二个活动 Run。
6. 不读取或返回源文件内容，不调用真实或 Mock 模型 Provider。

## 不在范围内

- Run/Task 调度、Attempt 创建和模型请求；
- 暂停、继续、取消和崩溃恢复；
- Context 摘要生成；
- 输出 Markdown 和结果目录；
- API Profile 主备路由编辑。

## 验收标准

- [x] 预览成功、无文件、缺少 API Profile、内容哈希缺失和活动 Run 失败路径有测试；
- [x] Run/Task 快照不可变且一次事务创建；
- [x] IPC DTO 无源文件内容和 API Key；
- [x] React 可打开预览、显示阻塞原因并创建 Run；
- [x] `pnpm format:check`、`pnpm lint`、`pnpm typecheck`、`pnpm test`、`cargo fmt --check`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo test --workspace` 通过；
- [x] 没有越界修改，IPC 契约已同步。

## 交付格式

1. 修改文件；
2. 已实现行为；
3. DTO 与 Domain/Repository 的对应关系；
4. 测试命令与结果；
5. 尚未实现的调度能力；
6. 合并顺序和潜在冲突。
