# TASK-0127：运行结果请求时间与列排序

- Status: Done（全量 Rust 基线门禁存在既有阻塞，见验证记录）
- Owner: Codex
- Branch: feat/m1-run-results-request-time-sorting
- Dependencies: TASK-0116, TASK-0119

## 目标

在运行结果 Task 表格中新增“请求时间”列，精确显示到秒；“状态”和“请求时间”列支持
通过点击表头在升序、降序之间切换，方便用户按执行状态或请求先后查看文件。

## 必读文档

- `AGENTS.md`
- `docs/prd.md`：4.5、4.6、5.3、5.6
- `docs/architecture.md`：9.2、9.3、9.4
- `docs/contracts/ipc-contract.md`：4.7
- `tasks/milestone-01/TASK-0116-run-results-ui.md`
- `tasks/milestone-01/TASK-0119-failure-retry.md`

## 允许修改

```text
tasks/milestone-01/TASK-0127-run-results-request-time-sorting.md
docs/prd.md
docs/architecture.md
apps/desktop/src/app/**
apps/desktop/src/features/tasks/**
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
apps/desktop/src-tauri/**
packages/ipc-types/**
数据库 migrations
docs/contracts/**
docs/decisions/**
tauri.conf.json
.github/workflows/**
```

## 输入与依赖

- 运行结果页已有 `VirtualTaskTable` 和 `TaskSummaryDto[]` 数据源。
- `TaskSummaryDto.startedAt` 是本任务“请求时间”的唯一数据源；人工重试重新入队时该字段
  会清空，Task 再次开始处理后更新，因此展示当前 Task 最近一轮执行的开始时间。
- `TaskSummaryDto.status` 使用既有 `TaskStatus` 枚举，不新增状态或中文状态文案。
- 不得为了排序逐行调用 `task_get`，避免 10,000 行场景产生 N+1 IPC 请求。

## 输出接口

- 运行结果表格新增“请求时间”列，位置为“状态”之后、“模型”之前。
- 状态表头和请求时间表头提供可访问的排序按钮与当前排序方向标识。
- 排序只属于前端 UI 状态，不修改 Task、Run、Attempt 或 SQLite 数据。

## 行为要求

1. 请求时间按桌面系统本地时区显示为 `YYYY-MM-DD HH:mm:ss`，使用 24 小时制、两位补零，
   不显示毫秒；`startedAt = null` 时显示 `—`。
2. 初次打开运行结果时保持 `task_list` 返回的原始稳定顺序，不预设状态或时间排序。
3. 首次点击“状态”按固定业务顺序升序排列：`pending`、`queued`、`running`、
   `succeeded`、`failed`、`cancelled`、`interrupted`、`source_changed`；再次点击切换为
   逆序。后续每次点击继续在升序和降序之间切换。
4. 首次点击“请求时间”按时间升序排列，即较早请求在前；再次点击按时间降序排列，即
   较晚请求在前。无请求时间的 Task 在两个方向下都固定排在有时间的 Task 之后。
5. 同一时刻或同一状态的 Task 保持原始相对顺序；实现不得原地修改 `runTasks`，应对派生
   列表执行稳定排序。
6. 任一时刻只激活一个排序字段。点击另一可排序列后，切换到该列的升序；非活动列不显示
   升序或降序状态。
7. 当前排序在轮询刷新、Task 状态更新、人工重试以及切换历史 Run 时保持；刷新后的数据
   立即按当前规则重新排序。页面重新加载后的持久化不在本任务范围内。
8. 排序不得影响 Task 详情、结果预览、提示词预览、单项重试、批量重试或虚拟滚动的行
   标识；所有操作继续以 Task ID 为准。
9. 表头必须使用原生按钮语义，当前列通过 `aria-sort` 或等价的可访问属性暴露
   `ascending` / `descending`，视觉方向图标不得只依赖颜色表达状态。
10. 新增一列后调整桌面和窄屏表格网格宽度，保证时间不被截断、表头与数据列对齐，且
    现有操作区不重叠。
11. 若实现中确认产品需要的是 Provider 实际发送时刻而非 `TaskSummaryDto.startedAt`，应
    停止扩展范围并提出 `TaskSummaryDto` 契约变更建议；不得通过批量加载 Attempt 规避
    公共契约评审。

## 测试要求

- React 覆盖请求时间的本地时间格式、补零、秒级精度和空值占位。
- React 覆盖状态首次升序、再次降序、固定业务顺序和同状态稳定排序。
- React 覆盖时间首次升序、再次降序、相同时间稳定排序及空值双向置后。
- React 覆盖从状态排序切换到时间排序时仅保留一个活动排序字段。
- React 覆盖刷新数据、切换 Run 和 Task 状态变化后仍按当前方向排序。
- React 覆盖排序后打开详情、查看结果、查看提示词及重试仍传递正确 Task ID。
- 视觉或 DOM 断言覆盖新增列的表头/单元格对齐与窄屏最小宽度。

## 不在范围内

- 后端或数据库排序、分页排序参数及 Migration；
- 新增或修改 IPC Command、DTO、公共错误码；
- 对文件、模型、提示词或结果列增加排序；
- 保存跨应用重启的排序偏好；
- 修改 Run/Task/Attempt 状态机、重试策略或调度顺序；
- 将 Attempt 历史预加载到任务列表。

## 验收标准

- [x] 运行结果表格在状态后显示“请求时间”列，时间精确到秒，空值显示 `—`；
- [x] 状态列点击后可在规定业务顺序的升序和降序之间切换；
- [x] 请求时间列点击后可在时间升序和降序之间切换，空值始终置后；
- [x] 活动排序方向有清晰视觉标识和可访问语义；
- [x] 排序稳定，不修改原始数据，也不影响虚拟滚动和现有 Task 操作；
- [x] 刷新、状态更新、人工重试和切换 Run 后保持当前排序；
- [x] 桌面和窄屏下表头、时间内容与操作区无错位或重叠；
- [x] 成功路径、空值、相同值和排序切换边界均有测试；
- [x] PRD 与架构文档同步，公共 IPC 契约保持不变；
- [x] `pnpm format:check`、`pnpm lint`、`pnpm typecheck`、`pnpm test` 通过；
- [x] `cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets -- -D warnings`、
      `cargo test --workspace` 已执行并记录结果；
- [x] `git diff` 不包含允许目录之外的本任务修改。

## 验证记录

- `pnpm install --frozen-lockfile`：通过，Workspace 依赖已是最新状态，锁文件未变化。
- `pnpm format:check`、`pnpm lint`、`pnpm typecheck`、`pnpm ipc:check`：通过。
- `pnpm test`：通过，前端 7 个测试文件共 54 个测试通过。
- `AppShell.test.tsx` 专项测试：29 个测试通过，覆盖全部 Task 状态的稳定双向排序、请求
  时间本地秒级格式、相同时间稳定排序、空值双向置后、排序字段切换、刷新与历史 Run
  切换保持排序，以及排序后操作仍使用正确 Task ID。
- `cargo fmt --all -- --check`：通过。
- `cargo clippy --workspace --all-targets -- -D warnings`：被未修改的
  `crates/secret-store/src/lib.rs` 7 条既有 Pedantic 告警阻断。
- `cargo test --workspace`：本任务及先执行到的包通过；未修改的
  `crates/persistence/src/database.rs` 有 3 个 Windows 临时 SQLite 清理测试因
  `os error 32` 失败。
- Vite 开发服务器已启动于 `http://localhost:1420`，HTTP 状态为 200。

## 交付格式

1. 修改文件列表；
2. 已实现行为；
3. 执行的命令和测试结果；
4. 未实现或受限行为；
5. 对公共契约的建议；
6. 合并顺序和潜在冲突。
