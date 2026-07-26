# TASK-0132：用户数据保留与项目数据管理

- Status: Done（全量验证存在既有 Windows SQLite 临时文件占用失败）
- Owner: Codex
- Branch: main
- Dependencies: TASK-0003, TASK-0101, TASK-0128

## 目标

确保桌面应用升级时继续使用系统应用数据目录中的 SQLite 数据；主数据库意外缺失时可从启动备份恢复。重新登记已有仓库时从 `.batch-analysis/project.json` 恢复项目默认提示词等非敏感配置，并提供用户主动清空应用数据的入口。

## 必读文档

- `AGENTS.md`
- `docs/prd.md`：4.1、7.1、8.4
- `docs/architecture.md`：7.1、13.1、19.1
- `docs/contracts/database-schema.md`：2、9、10
- `docs/contracts/ipc-contract.md`：4.1、4.9

## 允许修改

```text
tasks/milestone-01/TASK-0132-user-data-retention-management.md
docs/architecture.md
docs/contracts/ipc-contract.md
crates/persistence/src/**
crates/app-core/src/**
crates/ipc-contracts/src/**
apps/desktop/src-tauri/src/**
apps/desktop/src/**
packages/ipc-types/src/**
```

## 禁止修改

```text
Cargo.toml
Cargo.lock
package.json
pnpm-lock.yaml
pnpm-workspace.yaml
crates/persistence/migrations/**
crates/domain/**
docs/contracts/error-codes.md
docs/contracts/database-schema.md
docs/decisions/**
tauri.conf.json
.github/workflows/**
```

## 验收标准

- [x] 升级不会覆盖现有 `app.db`；缺失时可恢复 `app.bak`。
- [x] 重新登记仓库可恢复项目默认提示词、模型和执行设置，不恢复失效的 API Profile 引用。
- [x] 用户可在工作区设置中二次确认后安排清空应用数据。
- [x] 清空仅删除下次启动前的应用数据库和备份，不删除仓库、配置镜像或结果目录。
- [x] 格式化、lint、类型检查和相关测试通过。

## 验证记录

- `pnpm format:check`、`pnpm lint`、`pnpm typecheck`、`pnpm test`、`pnpm ipc:check`：通过，前端 62 个测试通过。
- `cargo fmt --all -- --check`：通过。
- `cargo test --workspace`：新增与相关测试通过；Persistence 中 3 个既有 Windows 磁盘清理测试因 `os error 32` 失败。
- 相关 Rust 包测试（App Core、IPC Contracts、Desktop）：通过。
- 相关 Clippy：被未修改的 `crates/secret-store/src/lib.rs` 既有 6 条 Pedantic 告警阻断。
