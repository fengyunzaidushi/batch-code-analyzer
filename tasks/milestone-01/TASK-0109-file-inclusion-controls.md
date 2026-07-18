# TASK-0109：File Inclusion and Filter Controls

- Status: Done
- Owner: Codex
- Branch: feat/m1-file-inclusion
- Dependencies: TASK-0102, TASK-0106, TASK-0107, TASK-0108

## 目标

用户可以在文件树中手动排除普通文件或恢复普通扫描排除项，选择结果持久化到 SQLite，并在重新扫描后保留用户手动排除。

## 必读文档

- `AGENTS.md`
- `docs/prd.md`：4.4、4.5、4.6、5.1
- `docs/architecture.md`：6.2、6.3、12.1、12.3
- `docs/contracts/ipc-contract.md`：4.6、7、8
- `docs/contracts/error-codes.md`：2、4.3、4.4
- `crates/domain/src/entities.rs`
- `crates/persistence/src/repositories/mod.rs`
- `apps/desktop/src/features/tasks/FileTreeTable.tsx`

## 允许修改

```text
tasks/milestone-01/TASK-0109-file-inclusion-controls.md
crates/app-core/**
crates/persistence/src/repositories/**
crates/ipc-contracts/**
apps/desktop/src-tauri/src/commands/**
apps/desktop/src-tauri/src/lib.rs
apps/desktop/src/app/**
apps/desktop/src/features/tasks/**
apps/desktop/src/ipc/**
apps/desktop/src/styles.css
docs/contracts/ipc-contract.md
docs/contracts/error-codes.md
packages/ipc-types/src/**
```

## 禁止修改

```text
数据库 migrations
模型 Provider
Run/Task 执行器
API Key 或 SecretStore
前端业务页面以外的无关模块
```

## 输入与依赖

- 复用 `FileRecord.included`、`FileRecord.source_status` 和 `exclusion_reason`。
- 复用 `Repository` 的 Domain Entity ↔ SQLite Row 边界。
- 用户手动排除使用内部持久化原因 `user_excluded`，不新增数据库字段。
- `file_set_included` 只修改当前 FileRecord，不影响已经创建的 Run/Task 快照。

## 输出接口

- Rust/TypeScript `FileSetIncludedRequest` 与 `FileSetIncludedResponse`。
- Tauri `file_set_included` Command。
- React `setFileIncluded` IPC 适配和文件树复选框。

## 行为要求

1. `included: false` 可以排除任何现有 FileRecord。
2. 恢复纳入仅允许普通、可读且非安全阻止的文件；敏感、二进制、过大、编码不支持、不可读取和已删除文件保持阻止。
3. 项目 ID 与文件 ID 必须在 Rust 中校验，错误不得暴露绝对路径或数据库诊断。
4. 用户手动排除在后续扫描中保持，扫描器的新安全排除优先级高于手动排除。
5. UI 更新失败时保持服务端状态并展示脱敏错误。

## 不在范围内

- 单文件提示词或模型覆盖；
- 自定义 FilterRules 编辑器；
- Run 预览、创建和执行；
- 敏感文件授权确认流程；
- 数据库 Migration。

## 验收标准

- [ ] Repository 与 Application 覆盖成功、跨项目和不存在文件路径；
- [ ] 敏感等安全阻止文件不能通过普通 IPC 恢复纳入；
- [ ] Rust 与 TypeScript DTO 生成无漂移；
- [ ] React 覆盖排除、恢复纳入、失败展示和禁用安全文件；
- [ ] 格式化、lint、类型检查、前端和 Rust 测试通过；
- [ ] 没有越界修改。

## 交付格式

1. 修改文件；
2. 已实现行为；
3. 测试命令与结果；
4. 已知限制；
5. 契约变更建议；
6. 合并注意事项。
