# TASK-0130：扫描文件列表排序

- Status: Done
- Owner: Codex
- Branch: feat/m1-run-results-ui
- Dependencies: TASK-0129

## 目标

扫描完成后，文件列表支持按状态和预估 Token 排序，方便用户集中查看敏感文件或定位较大的代码文件。

## 允许修改

```text
tasks/milestone-01/TASK-0130-file-list-sorting.md
apps/desktop/src/features/tasks/FileTreeTable.tsx
apps/desktop/src/features/tasks/FileTreeTable.test.tsx
apps/desktop/src/styles.css
```

## 行为要求

1. “状态”和“大小 / 预估 Token”表头可点击排序。
2. 首次点击升序，再次点击同一列切换降序；切换列重新从升序开始。
3. Token 排序使用现有文件 Token 估算值。
4. 状态排序按业务状态分组，敏感文件归为同一组，并保持相同值的稳定顺序。
5. 未排序时保留目录树；排序后使用全局文件列表，并显示完整相对路径。
6. 排序只改变当前视图，不修改服务端文件顺序或扫描结果。

## 验收标准

- [x] Token 列可升序/降序排序；
- [x] 状态列可排序且敏感文件连续排列；
- [x] 表头暴露当前排序方向；
- [x] 目录树默认行为和文件操作不回归；
- [x] 格式化、lint、类型检查和前端测试通过；
- [x] 没有公共契约或越界修改。

