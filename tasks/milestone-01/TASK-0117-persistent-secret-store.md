# TASK-0117：Persistent OS SecretStore Backend

- Status: Done
- Owner: Codex
- Branch: feat/m1-persistent-secret-store
- Dependencies: TASK-0110, TASK-0116

## 目标

将桌面端当前的 `SessionOnly` 内存密钥存储替换为操作系统安全存储，使用户
配置一次 API Key 后，重启应用仍可继续使用。SQLite 和仓库项目配置只保存
非敏感 API Profile 元数据以及不透明 `SecretRef`。

## 必读文档

- `AGENTS.md`
- `docs/prd.md`：6.3、7.1、7.2、8.2
- `docs/architecture.md`：17、18.2
- `docs/contracts/ipc-contract.md`：4.4
- `docs/contracts/database-schema.md`：4、10、11
- `tasks/milestone-01/TASK-0110-api-profile-ipc-secret-store-bridge.md`
- `crates/secret-store/src/lib.rs`
- `apps/desktop/src-tauri/src/lib.rs`

## 允许修改

```text
tasks/milestone-01/TASK-0117-persistent-secret-store.md
crates/secret-store/**
apps/desktop/src-tauri/**
docs/architecture.md
docs/contracts/error-codes.md
Cargo.toml
Cargo.lock
```

## 禁止修改

```text
数据库 migrations
project.json 写入 API Key
SQLite 写入 API Key 明文
普通日志、错误信息、DTO 或测试快照写入 API Key
模型 Provider、扫描器和 Run 状态机
前端业务页面和 IPC 公共 DTO
```

## 行为要求

1. 使用 OS 原生安全存储保存 API Key：Linux Secret Service、macOS Keychain、
   Windows Credential Manager。
2. `SecretStore` 的公共抽象保持不变；`MemorySecretStore` 仅用于测试。
3. 桌面应用注入 `Arc<dyn SecretStore>`，密钥环初始化失败时返回稳定的
   `security_secret_store_unavailable` 或 `security_secret_store_failure`，不
   静默降级为会话内存存储。
4. `api_profile_secret_put` 先写入安全存储，成功后再将 `SecretRef` 写入
   SQLite；任何一步失败都不得返回成功。
5. API Profile 元数据仍通过 SQLite 保存，`project.json` 不保存 API Key。
6. 应用重启后 `api_profile_list` 能通过同一 `SecretRef` 检查并使用密钥。
7. 删除密钥时只删除当前引用；共享引用的清理留给后续引用计数任务。

## 测试要求

- Memory SecretStore 回归测试继续通过；
- OS SecretStore 的错误映射、稳定引用格式和不暴露密钥测试；
- Tauri/Application 使用 trait object 注入，密钥环不可用时返回稳定错误；
- `cargo fmt --check`、`cargo clippy --workspace --all-targets -- -D warnings`、
  `cargo test --workspace`；
- 不调用真实收费模型 API，不在测试中使用真实密钥。

## 不在范围内

- API Key 迁移或写入现有 `project.json`；
- 备用 Profile、自动重试和 Run 调度；
- SecretStore 管理界面和跨设备同步；
- 修改 IPC DTO 或数据库 Migration。

## 交付格式

1. 修改文件和平台后端；
2. SecretRef 与 OS 密钥环的映射；
3. 重启后读取和不可用降级行为；
4. 测试结果和未覆盖的平台限制；
5. 后续密钥引用计数和恢复策略接口。

## 实现结果

- `KeyringSecretStore` 使用 `keyring` 的原生后端：Linux Secret Service、
  macOS Keychain、Windows Credential Manager；
- 桌面 State 改为 `Arc<dyn SecretStore>`，密钥环初始化失败时使用明确的
  `Unavailable` Store，不静默保存到内存或普通文件；
- `Cargo.toml` 和 `Cargo.lock` 已锁定 `keyring 4.1.5`；
- 当前容器没有可用 Secret Service，临时回环测试得到稳定的后端不可用错误，
  未留下密钥环条目；真实桌面环境需要安装并运行对应系统密钥环服务。
