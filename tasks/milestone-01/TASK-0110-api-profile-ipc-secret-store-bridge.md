# TASK-0110：API Profile IPC / SecretStore Bridge

- Status: Ready
- Owner: Unassigned
- Branch: feat/m1-api-profile-bridge
- Dependencies: TASK-0002, TASK-0003, TASK-0004, TASK-0103, TASK-0105

## 目标

API 配置页从占位状态接入 Rust 核心：用户可以查看和维护 API Profile 非敏感元数据、测试连接，并由 SecretStore 管理密钥引用；前端永远不会读取或展示 API Key 明文。

## 必读文档

- `AGENTS.md`
- `docs/prd.md`：4.3、5.6、6.1、6.3、8.3
- `docs/architecture.md`：11.6～11.9、13、16、17、18
- `docs/contracts/ipc-contract.md`：4.4、7、8
- `docs/contracts/error-codes.md`：2、4.4、4.6、5
- `docs/contracts/database-schema.md`：4、5、7、9
- `tasks/milestone-01/TASK-0103-provider-foundation.md`
- `tasks/milestone-01/TASK-0105-mock-provider-tests.md`
- `crates/api-profiles/src/lib.rs`
- `crates/secret-store/src/lib.rs`
- `crates/model-providers/src/lib.rs`
- `crates/persistence/migrations/0001_initial_schema.sql`
- `apps/desktop/src/app/AppShell.tsx`

## 允许修改

```text
tasks/milestone-01/TASK-0110-api-profile-ipc-secret-store-bridge.md
Cargo.toml
Cargo.lock
apps/desktop/src-tauri/Cargo.toml
apps/desktop/src-tauri/src/**
crates/domain/**
crates/persistence/src/repositories/**
crates/persistence/src/rows.rs
crates/persistence/tests/**
crates/app-core/**
crates/ipc-contracts/**
apps/desktop/src/app/**
apps/desktop/src/ipc/**
apps/desktop/src/styles.css
packages/ipc-types/src/**
docs/contracts/ipc-contract.md
docs/contracts/error-codes.md
```

## 禁止修改

```text
crates/persistence/migrations/**
任务调度器、Run/Task 执行流程
文件扫描和文件读取实现
真实收费模型 API
GitHub Actions、Tauri capabilities 以外的无关配置
```

`api_profiles` 表已由现有 Migration 创建。本任务不得新增或修改 Migration；如果现有字段不足，必须先提交数据库契约变更建议并停止扩大实现范围。

## 输入与依赖

- 将 `crates/api-profiles`、`crates/secret-store` 和必要的 `crates/model-providers` 注册到根 Workspace，并由桌面应用显式依赖。
- Repository 必须返回领域层实体或明确的领域适配类型，不得把 SQLite Row 直接转换成 IPC DTO。
- 复用 `ApiProfile` 的 Base URL 校验、协议枚举、模型缓存和 `SecretRef`；Provider 使用 `ResolvedApiProfile`。
- API Profile 元数据保存在 SQLite `api_profiles` 表；API Key 只通过 `SecretStore` 保存。

## 输出接口

新增并自动生成 TypeScript 的稳定 DTO，至少包括：

- `ApiProfileSummaryDto`：ID、名称、协议、Base URL、默认模型、模型缓存时间、`hasSecret`、连接测试摘要；不得包含 API Key 或可直接取回密钥的值。
- `ApiProfileSaveRequest` / `ApiProfileSaveResponse`：保存或更新非敏感元数据，返回脱敏摘要。
- `ApiProfileListResponse`：稳定分页或列表响应。
- `ApiProfileTestRequest` / `ApiProfileTestResponse`：通过 Mock Provider/本地测试端点验证连接，返回脱敏状态和模型摘要。
- `ApiProfileDeleteRequest` / `ApiProfileDeleteResponse`：删除未被项目引用的档案；被引用时返回稳定错误。
- Tauri Commands：`api_profile_list`、`api_profile_save`、`api_profile_test`、`api_profile_delete`、`api_models_fetch`。

## 密钥边界与待确认契约

1. 任何公共响应 DTO、SQLite 普通字段、日志、错误、测试快照和导出文件都不得包含 API Key。
2. 密钥写入必须是一次性、写入即忘的 SecretStore 操作；读取命令只能返回 `hasSecret` 和脱敏摘要。
3. 当前仓库只有抽象 `SecretStore` 和 `MemorySecretStore`，没有三平台生产 Keychain/Credential Manager/Secret Service 后端。实现必须明确标记 `session_only` 或 `unavailable`，不得伪装成持久安全存储。
4. 若需要从前端录入新密钥，必须先确定“写入专用命令”或等价的受控输入边界；不得把 API Key 字段加入普通公共 DTO 后直接回传。该契约决定应在实现前记录到 `docs/contracts/ipc-contract.md` 或 ADR。

## 行为要求

1. 名称为空、Base URL 非 HTTP(S)、包含 URL 凭据或协议不支持时返回稳定校验错误。
2. 列表和保存响应只包含非敏感元数据与 `hasSecret`；不得泄露 `SecretRef` 的可用密钥值。
3. 保存操作在 SQLite 事务提交成功后再更新内存状态；失败不得留下半成品 Profile。
4. `api_profile_test` 使用 Mock Provider 或仓库内测试服务，不调用真实收费 API；错误只返回稳定 Provider/Security 错误码和脱敏摘要。
5. 删除仍被 Project 的主档案或备用档案引用时必须阻止，并返回稳定错误码；不得级联删除项目配置。
6. 模型缓存只保存模型 ID、展示名、归属方和更新时间，不保存响应原文或密钥。
7. API 配置页显示空状态、会话密钥状态、保存成功、测试成功/失败、删除阻止和数据库不可用状态。

## 不在范围内

- Run Preview、Run 创建、Task 调度和实际批量请求；
- 主备路由切换和自动重试策略；
- API Profile 拖拽排序和项目路由编辑；
- 三平台生产 SecretStore 后端的完整实现；
- 单文件提示词/模型覆盖；
- 修改数据库 Migration。

## 验收标准

- [ ] 根 Workspace 能编译 `api-profiles`、`secret-store` 和所需 Provider crate；
- [ ] API Profile Repository 成功、更新、列表、删除和被引用阻止路径有测试；
- [ ] SecretStore 不可用/会话模式有明确状态测试；
- [ ] DTO 生成无漂移，且不存在 API Key 字段或明文快照；
- [ ] Tauri Command 覆盖校验、数据库不可用、Provider 错误脱敏和删除阻止；
- [ ] React 覆盖空状态、保存、测试、删除阻止和安全错误展示；
- [ ] 使用 Mock Provider，不调用真实收费 API；
- [ ] `pnpm format:check`、`pnpm lint`、`pnpm typecheck`、`pnpm test`、`cargo fmt --check`、`cargo clippy --workspace --all-targets -- -D warnings` 和 `cargo test --workspace` 通过；
- [ ] 没有越界修改，文档契约已同步。

## 交付格式

1. 修改文件；
2. 已实现行为；
3. Rust 与 TypeScript DTO 对应关系；
4. SecretStore 和 API Key 安全边界；
5. 测试命令与结果；
6. 未实现或受限行为；
7. 契约冲突和合并注意事项。
