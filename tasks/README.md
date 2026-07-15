# Agent 任务系统

## 1. 使用方式

每个编码 Agent 必须收到一个独立任务书。任务书要明确目标、必读章节、允许修改目录、禁止修改目录、输入输出接口、验收标准和交付格式。

推荐提示词：

```text
你正在独立 worktree 中工作。

先阅读：
1. AGENTS.md
2. 当前任务书
3. 任务书列出的 PRD、架构和契约章节

严格限制：
- 只修改任务书允许的目录；
- 不修改公共 Migration、IPC、错误码和 Workspace 锁文件；
- 发现契约不足时停止扩展范围并提出变更建议；
- 不通过删除测试或降低检查标准完成任务。

完成实现、测试和自检后，按任务书规定输出交付摘要。
```

## 2. 状态

任务文件顶部使用：

```text
Status: Ready | In Progress | Blocked | Review | Done
Owner: Agent/Person
Branch: feat/...
Dependencies: TASK-...
```

## 3. 并行规则

- Milestone 00 串行或由一个总控 Agent 完成；
- Milestone 01 在公共契约稳定后，最多开启 4～6 个 worktree；
- 有依赖的任务不得抢跑；
- 公共文件由单一 Owner 修改；
- 集成 Agent 负责合并和全量验证。

## 4. 文件

- `TASK-template.md`：复制后创建新任务；
- `milestone-00/`：工程地基与公共契约；
- `milestone-01/`：首轮可并行模块。
