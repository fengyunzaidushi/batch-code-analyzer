# TASK-0101：实现 Project、FileRecord 与 Run Repository

- Status: Ready
- Owner: Unassigned
- Branch: feat/m1-domain-db
- Dependencies: TASK-0002, TASK-0003

## 目标

实现项目、文件记录、上下文、Run、Task 和 Attempt 的事务化 Repository，不包含调度和 Tauri Command。

## 必读文档

- `docs/prd.md`：7、8、9.2 节
- `docs/architecture.md`：6、7、20 节
- `docs/contracts/database-schema.md`
- `docs/contracts/task-state-machine.md`

## 允许修改

```text
crates/persistence/src/repositories/**
crates/persistence/tests/**
crates/domain/**  # 仅经 Owner 批准的小型补充
```

## 禁止修改

```text
crates/persistence/migrations/**
docs/contracts/**
apps/**
```

## 验收标准

- [ ] Project canonical path 唯一；
- [ ] Run + Task 创建在单事务内完成；
- [ ] Task 领取不存在重复领取；
- [ ] Attempt sequence 唯一且只追加；
- [ ] Run 统计可从 Task 重算；
- [ ] 崩溃恢复查询能识别未结束对象。
