# TASK-0115：Project Context Discovery / ContextVersion IPC

- Status: Done
- Owner: Codex
- Branch: feat/m1-project-context
- Dependencies: TASK-0002, TASK-0003, TASK-0102, TASK-0111

## 目标

建立项目上下文的本地发现和 IPC 闭环：从项目根目录发现 `README*` 与 `AGENTS.md`，
生成不可变 `ContextVersion`，保存当前版本引用，并在前端展示来源与摘要状态。
本任务不调用真实模型，不实现上下文摘要 Provider。

## 允许修改

```text
tasks/milestone-01/TASK-0115-project-context.md
crates/app-core/**
crates/persistence/src/repositories/**
crates/ipc-contracts/**
apps/desktop/src-tauri/src/**
apps/desktop/src/app/**
apps/desktop/src/ipc/**
apps/desktop/src/styles.css
packages/ipc-types/src/**
docs/contracts/ipc-contract.md
```

## 禁止修改

```text
数据库 migrations
模型 Provider、SecretStore、Run/Task 执行器
API Key、源码日志和源码原文导出
Workspace 依赖与锁文件
```

## 行为要求

1. Rust 在仓库边界内发现根目录 `README`、`README.*` 和 `AGENTS.md`，不跟随符号链接。
2. 生成 `ContextVersion` 时只持久化来源相对路径、大小哈希和安全摘要，不返回源码原文。
3. ContextVersion 创建后不可修改；生成新版本时更新 Project 当前版本引用。
4. `context_generate` 和 `context_get` 返回稳定 DTO，错误不暴露绝对路径或文件内容。
5. React 显示当前上下文状态、来源文件和本地摘要，并支持重新生成。

## 验收标准

- [x] 发现规则和路径安全有测试；
- [x] ContextVersion 创建、读取和项目当前版本更新有测试；
- [x] Rust/TypeScript IPC DTO 无漂移；
- [x] React 覆盖无上下文、生成成功和错误状态；
- [x] 全量格式、lint、类型和测试通过；
- [x] 没有越界修改。
