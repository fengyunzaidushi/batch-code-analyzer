# TASK-0114：Scan Rule Visibility / Temporary Overrides

- Status: Done
- Owner: Codex
- Branch: feat/m1-scan-rule-overrides
- Dependencies: TASK-0102, TASK-0109, TASK-0113

## 目标

让扫描规则可解释，并允许用户为当前项目会话添加临时排除模式。普通规则排除的文件可以在文件树中手动恢复纳入；敏感、路径安全和不可读取类阻止继续保持硬安全边界。

## 允许修改

```text
tasks/milestone-01/TASK-0114-scan-rule-overrides.md
crates/repository-scanner/**
crates/app-core/**
crates/ipc-contracts/**
apps/desktop/src-tauri/src/**
apps/desktop/src/app/**
apps/desktop/src/ipc/**
apps/desktop/src/features/tasks/**
apps/desktop/src/styles.css
packages/ipc-types/src/**
docs/contracts/ipc-contract.md
```

## 禁止修改

```text
crates/persistence/migrations/**
模型 Provider、Run 执行器和调度器
API Key、源码日志和敏感内容导出
Workspace 依赖与锁文件
```

## 行为要求

1. 扫描报告返回内置目录、内置扩展名、有效 `.gitignore` 模式、临时用户模式和敏感检测状态。
2. `scan_start` 可接收当前项目会话的临时排除模式；模式只影响本次会话扫描，不写入仓库配置。
3. 文件树显示排除原因；`.gitignore`、临时用户模式和非安全纳入规则排除的文件可以手动恢复。
4. 敏感、符号链接、不可读取、编码不支持、二进制和路径安全阻止不得通过普通 checkbox 绕过。
5. 敏感文件只显示风险类型和脱敏位置，不显示命中的秘密原文。

## 验收标准

- [x] Scanner 报告规则目录和临时模式；
- [x] React 可添加/移除临时排除模式并随扫描提交；
- [x] 文件树显示规则类别且普通排除可恢复；
- [x] 敏感和硬安全阻止仍由 Rust 拒绝；
- [x] IPC 类型和契约同步；
- [x] 全量格式、lint、类型和测试通过；
- [x] 没有越界修改。
