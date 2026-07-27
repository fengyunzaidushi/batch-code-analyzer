# TASK-0133：项目重新定位

- Status: Done
- Owner: Codex
- Branch: develop
- Dependencies: TASK-0101, TASK-0132

## 目标

让“重新定位”真正能够把已登记项目绑定到新的本地仓库目录，同时保留原 Project ID、提示词、运行历史、任务和结果。该操作用于仓库目录被移动、恢复或重新挂载后的路径修复。

## 必读文档

- `AGENTS.md`
- `docs/prd.md`：4.1、7.2、10.1
- `docs/architecture.md`：7.1、10、13.1
- `docs/contracts/database-schema.md`：2、9、10
- `docs/contracts/ipc-contract.md`：4.1、7

## 允许修改

```text
tasks/milestone-01/TASK-0133-project-relocation.md
docs/contracts/ipc-contract.md
crates/app-core/src/**
crates/ipc-contracts/src/**
crates/persistence/src/**
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

## 行为契约

- 前端只调用目录选择器；路径规范化、边界检查和配置镜像读取均在 Rust 完成。
- 请求接收 `projectId` 和新目录；成功后返回更新后的 `ProjectDetailDto` 与镜像写入警告。
- 新目录必须是可读取的真实目录，并通过 `SafeRoot` 校验。
- 新目录已经登记为其他项目时返回 `project_path_duplicate`，不得修改任一项目。
- 新目录存在 `.batch-analysis/project.json` 且包含项目 ID 时，ID 必须与请求项目一致；不一致或镜像损坏时拒绝重定位，不覆盖目标镜像。
- 目标目录没有项目镜像时允许重定位，数据库提交成功后尝试创建镜像。
- 重定位只更新项目路径与 `pathStatus`，不修改 Project ID、Run/Task/Attempt、结果目录、提示词、模型或 API Profile 引用。
- 重定位不自动扫描；前端显示新路径，用户可手动执行重新扫描。
- 取消目录选择不产生请求、不改变状态。

## 验收标准

- [x] 点击可用的“重新定位”按钮会打开目录选择器；取消后状态不变。
- [x] 选择有效新目录后，项目 ID 和历史数据不变，路径状态变为 `available`。
- [x] 目标镜像 ID 不匹配、目标目录重复登记、路径非法时有稳定错误提示且数据库不变。
- [x] 成功重定位后目标目录镜像可写入；写入失败只返回警告，不回滚数据库。
- [x] Rust、IPC 和前端测试覆盖成功、取消、冲突、镜像不匹配和路径失败。
- [x] 格式化、lint、类型检查、IPC 生成检查和全量测试通过。

## 完成记录

- 已新增 `project_relocate` IPC 请求/响应 DTO、Tauri command 与 TypeScript 类型导出。
- Rust `ProjectService::relocate_project` 已完成目录安全校验、重复登记检查、项目镜像一致性校验、数据库路径更新和镜像写入告警；保留 Project ID、项目设置与 Run/Task/Attempt 历史，不自动扫描。
- 前端已接通目录选择器、重定位请求、加载状态、错误展示和成功后的路径状态刷新；取消选择不会发送 IPC 请求，按钮在路径不可用时仍可用。
- App Core 测试覆盖成功重定位、目标镜像 ID 不匹配和重复目录；Tauri command 测试覆盖重定位错误码与安全提示；前端测试覆盖按钮回调委托、目录选择取消和成功后的路径更新。
- `pnpm format:check`、`pnpm lint`、`pnpm typecheck`、`pnpm test`、`pnpm ipc:check`：通过，前端 64 个测试通过。
- `cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets -- -D warnings`：通过。
- `cargo test --workspace -- --test-threads=1`：通过；共覆盖工作区各 Rust crate 测试。并行默认线程运行时有一次既有 Windows SQLite 临时文件锁抖动，单独重跑及串行重跑均通过。
