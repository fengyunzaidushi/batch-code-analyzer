# TASK-0129：文件大小与预估 Token 超长警告

- Status: Done
- Owner: Codex
- Branch: feat/m1-run-results-ui
- Dependencies: TASK-0102, TASK-0109, TASK-0114

## 目标

扫描完成后，在文件树中显示每个文件的大小与预估 Token；仍被纳入且预估 Token 超过保守阈值时，明确警告用户代码文件过长，但不改变扫描纳入状态。

## 必读文档

- `AGENTS.md`
- `docs/prd.md`：4.4、4.5、9.1、10.1
- `docs/architecture.md`：12、13.2、23.6
- `docs/contracts/ipc-contract.md`：4.6
- `tasks/milestone-01/TASK-0109-file-inclusion-controls.md`
- `tasks/milestone-01/TASK-0114-scan-rule-overrides.md`

## 允许修改

```text
tasks/milestone-01/TASK-0129-file-size-token-warning.md
apps/desktop/src/features/tasks/FileTreeTable.tsx
apps/desktop/src/features/tasks/FileTreeTable.test.tsx
apps/desktop/src/styles.css
```

## 禁止修改

```text
数据库 migrations
公共 IPC DTO 与错误码
扫描器、模型 Provider、Run/Task 执行器
Workspace 依赖与锁文件
```

## 行为要求

1. 文件树新增“大小 / 预估 Token”列，复用 `FileRecordSummaryDto.sizeBytes`，不得读取源文件。
2. Token 采用保守估算：每 2 字节约 1 Token，并明确显示为预估值。
3. 默认警告阈值为 32,000 Token；达到阈值不警告，超过阈值才警告。
4. 警告只针对仍被纳入的文件；已排除文件继续显示大小和预估 Token，但不显示超长警告。
5. 警告不自动排除文件，也不改变 Run 快照或扫描结果。

## 验收标准

- [x] 普通文件显示可读大小和预估 Token；
- [x] 已纳入且超过阈值的文件显示“代码文件过长”；
- [x] 临界值及已排除超长文件不显示警告；
- [x] 文件树在桌面和窄视口仍可横向滚动且各列不重叠；
- [x] 格式化、lint、类型检查和前端测试通过；
- [x] 没有越界修改或公共契约漂移。
