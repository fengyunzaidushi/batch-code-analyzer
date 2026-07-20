# TASK-0113：Project Run Settings Bridge

- Status: Done
- Owner: Codex
- Branch: feat/m1-project-run-settings
- Dependencies: TASK-0106, TASK-0110, TASK-0111

## 目标

补齐当前项目与全局 API Profile 之间的绑定，使用户可以保存主 API Profile 和项目默认模型，并解除 Run 预览的配置阻塞。

## 允许修改

```text
tasks/milestone-01/TASK-0113-project-run-settings.md
crates/app-core/**
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
模型 Provider、任务调度器和 Run 执行策略
备用 Profile 路由、自动重试和并发配置
Workspace 依赖与锁文件
```

## 行为要求

1. `project_update_run_settings` 接收项目 ID、可选主 Profile ID 和可选默认模型。
2. Profile 引用必须存在；公共 DTO 不返回 SecretRef 或 API Key。
3. SQLite 提交成功后再写项目配置镜像；镜像失败不回滚数据库。
4. React API 配置页展示当前项目路由设置并在保存后更新本地项目详情。
5. Run 预览继续通过 Domain Project 读取路由和模型，不在前端拼装运行配置。

## 验收标准

- [x] 主 Profile 和默认模型可保存、读取并用于 Run 预览；
- [x] 不存在的 Project/Profile 返回稳定脱敏错误；
- [x] 配置镜像不包含 API Key；
- [x] React 覆盖保存与无 Profile 状态；
- [x] 全量格式、lint、类型和测试通过；
- [x] 没有越界修改。
