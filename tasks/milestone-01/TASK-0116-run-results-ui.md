# TASK-0116：Run / Task / Result IPC 与结果展示

- Status: Done
- Owner: Codex
- Branch: feat/m1-run-results-ui
- Dependencies: TASK-0111, TASK-0112, TASK-0113, TASK-0115

## 目标

补齐正式 Run 执行后的可观察闭环：用户可以查看项目的 Run 历史、Run 下的
Task 状态、Attempt 脱敏元数据和已生成的 Markdown 结果。任务执行仍复用现有
`RunExecutionService`、Domain 状态机和 SQLite 结果文件，不在本任务重新实现
调度或 Provider。

当前后端已经能够创建 Run、执行 Task 并写入结果，但前端执行完成后没有读取
Task/Attempt/结果内容的 IPC，任务区的“待处理”和“最近 Run”仍是占位状态。

## 允许修改

```text
tasks/milestone-01/TASK-0116-run-results-ui.md
crates/app-core/**
crates/persistence/src/repositories/**
crates/ipc-contracts/**
apps/desktop/src-tauri/src/commands/**
apps/desktop/src/app/**
apps/desktop/src/features/tasks/**
apps/desktop/src/ipc/**
apps/desktop/src/styles.css
packages/ipc-types/src/**
docs/contracts/ipc-contract.md
```

`docs/contracts/ipc-contract.md`、公共 DTO 和生成类型由集成负责人审核；不得
在本任务中修改 Workspace 依赖、锁文件或数据库 Migration。

## 禁止修改

```text
数据库 migrations
模型 Provider、SecretStore 和扫描器
Run/Task 状态机及执行调度策略
API Key、源代码内容、完整请求体或未脱敏错误日志
前端业务以外的桌面权限和 CI 配置
```

## IPC 方向

具体命名可在实现前按现有契约统一，但至少覆盖以下能力：

- 按 Project 查询 Run 历史，按稳定时间和 ID 倒序返回；
- 查询单个 Run 的摘要和 Task 列表；
- 查询单个 Task 的摘要和 Attempt 列表；
- 读取当前 Task 结果的安全 Markdown 内容；
- 结果不存在、路径越界、文件不可读和数据库不可用返回稳定脱敏错误；
- DTO 不返回 API Key、SecretRef、源文件内容、请求原文或绝对路径栈信息。

结果内容读取必须以 SQLite 中的 `current_result_path` 和所属 Run 输出目录为
依据，重新通过仓库边界检查；不得信任前端传入的任意文件路径。Markdown 在
前端展示时按安全文本/受限 Markdown 处理，禁止脚本、事件属性和
`javascript:` 链接。

## React 行为

1. 执行前后都能刷新当前项目的 Run 与 Task 数据；初版允许短轮询，不要求新增
   全局实时 Event。
2. 文件任务区显示相对路径、Task 状态、实际模型、结果状态、Attempt 数和
   错误摘要；保留现有文件纳入/排除和敏感授权操作。
3. 成功 Task 可以打开 Markdown 结果预览，显示结果版本、输出位置的脱敏摘要、
   Token 和耗时；不直接展示源文件绝对路径。
4. 失败、取消、处理中、无结果和结果文件缺失状态分别展示；错误文案只使用
   稳定 IPC 错误码映射。
5. Run 执行完成后不丢失当前项目选择和文件筛选状态，并能重新打开历史 Run。

## 测试要求

- Persistence：Run/Task/Attempt 列表顺序、跨项目隔离、缺失记录和结果路径
  越界拒绝；
- Application/Tauri：成功读取结果、结果缺失、数据库不可用和内部错误脱敏；
- IPC：Rust/TypeScript DTO 生成无漂移，结果 DTO 不包含密钥或任意路径输入；
- React：空历史、执行中、成功结果、失败 Attempt、结果读取失败和切换 Run；
- 使用仓库内 Mock Provider，不调用真实收费模型 API。

## 不在范围内

- 提示词生成、随机文件测试和真实模型上下文摘要；
- Run 暂停、继续、取消、崩溃恢复和后台事件推送；
- 自动重试、备用 API Profile 切换和人工重试创建新 Attempt；
- 单文件提示词/模型编辑、结果批量操作和 10,000 行虚拟滚动优化；
- 新增数据库表或迁移。

## 验收标准

- [x] 可查看当前项目 Run 历史和每个 Run 的 Task 状态；
- [x] 成功结果可通过安全 IPC 打开 Markdown 预览；
- [x] Attempt 元数据和错误均脱敏，且 API Key/SecretRef 不进入 DTO；
- [x] 结果路径越界、结果缺失、跨项目访问和数据库不可用均有稳定错误；
- [x] 执行完成后前端能刷新并保留当前项目和筛选状态；
- [x] `pnpm format:check`、`pnpm lint`、`pnpm typecheck`、`pnpm test`、
  `cargo fmt --check`、`cargo clippy --workspace --all-targets -- -D warnings`
  和 `cargo test --workspace` 通过；
- [x] 无越界修改，公共 IPC 契约和生成 TypeScript 类型保持同步。

## 交付格式

1. 修改文件和 IPC Command/Event 列表；
2. Run、Task、Attempt 和 Markdown 结果的安全边界；
3. Rust 与 TypeScript DTO 对应关系；
4. 成功、失败、缺失结果和路径安全测试结果；
5. 未实现的运行控制和后续 TASK 接口说明；
6. 合并顺序与公共契约冲突说明。
