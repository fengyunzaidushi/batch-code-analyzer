# TASK-0128：客户端全局常用提示词库

- Status: Done（全量 Rust 基线门禁存在既有阻塞，见验证记录）
- Owner: Codex
- Branch: feat/m1-run-results-ui
- Dependencies: TASK-0113, TASK-0123

## 目标

将已保存的提示词从项目私有数据迁移为客户端全局常用提示词库，使任一项目（例如
`acg-faka`）都能选择此前在另一项目中保存的提示词，同时保持各项目默认提示词和既有 Run
快照独立且不可变。

## 必读文档

- `AGENTS.md`
- `docs/prd.md`：4.1、4.2、7.1、8.4
- `docs/architecture.md`：13.1、13.2、14.2、19.1
- `docs/contracts/database-schema.md`
- `docs/contracts/ipc-contract.md`：4.1、4.3、4.5
- `docs/decisions/0002-sqlite-source-of-truth.md`
- `docs/decisions/0005-run-snapshot-immutability.md`
- `tasks/milestone-01/TASK-0113-project-run-settings.md`
- `tasks/milestone-01/TASK-0123-complete-analysis-prompt.md`

## 允许修改

```text
tasks/milestone-01/TASK-0128-global-prompt-library.md
docs/prd.md
docs/architecture.md
docs/contracts/ipc-contract.md
crates/persistence/src/**
crates/app-core/**
crates/ipc-contracts/**
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

## 输入与依赖

- 已存在的 SQLite `prompt_library` 表是客户端全局提示词库的权威存储；
- 旧版本将预设错误地写入 `Project.filter_rules.prompt_presets`；
- `Project.default_prompt` 仍是项目级默认值，Task 的 `prompt_snapshot` 仍是 Run 创建时的
  不可变快照；
- 现有 `project_prompt_save` 和 `project_prompt_select` IPC 命令由前端调用。

## 输出接口

不新增 Migration、DTO 字段或错误码。保留现有 IPC 命令名称和请求形状，但其预设选择器读取
全局库：

- `project_prompt_save` 在全局库创建命名提示词，并将其内容设为当前项目默认；
- `project_prompt_select` 从全局库选择提示词，并将其内容设为当前项目默认；
- `ProjectDetailDto.promptPresets` 返回完整客户端全局库，而 `activePromptId` 仅表示当前项目
  选择的全局提示词 ID。

## 行为要求

1. 新保存的提示词在任意项目的选择器中可见；切换或保存到一个项目不得静默改写其他项目的
   `defaultPrompt`。
2. 新库使用 SQLite 的 `prompt_library` 表；不得将新预设写入项目 `filter_rules_json` 或
   `.batch-analysis/project.json`。
3. 首次读取全局库时，导入既有项目私有预设；保留其 ID，名称冲突且内容不同则使用稳定的
   项目名后缀，避免静默丢失。重复导入必须幂等。
4. 重名保存不得静默覆盖全局内容，返回现有的校验错误；用户可改名后重试。
5. SQLite 成功提交后才更新项目返回值和写入项目配置镜像；镜像不得再携带全局库内容。
6. 旧项目的默认提示词、文件覆盖和已创建 Run/Task 提示词快照不得被迁移或选择操作改写。
7. 前端应将选择器和操作文案标为“常用提示词”，避免继续暗示预设仅属于当前项目。

## 不在范围内

- 编辑或删除全局提示词；
- 内置提示词和完整的名称冲突交互弹窗；
- 修改数据库 Schema、Run/Task 模型、提示词生成请求或 API Key 存储；
- 调用真实收费模型 API。

## 验收标准

- [x] 在项目 A 保存的提示词可在项目 B 读取和选择；
- [x] 选择全局提示词只改变目标项目默认值；
- [x] 项目私有遗留预设被无损、幂等导入全局库；
- [x] 重名保存、缺失项目和缺失提示词返回稳定失败；
- [x] 已创建 Task 的提示词快照不受影响；
- [x] 前端成功、失败和跨项目路径有覆盖；
- [x] 格式化、lint、类型检查和相关测试通过；
- [x] `git diff` 不含越界修改。

## 验证记录

- `pnpm format:check`、`pnpm lint`、`pnpm typecheck`、`pnpm test`、`pnpm ipc:check`：
  通过，前端 54 个测试通过。
- `cargo fmt --all -- --check`：通过。
- `cargo test -p batch-code-analyzer-app-core`：通过，38 个测试通过；新增跨项目全局库、遗留
  ID 导入、同名冲突后缀与幂等导入覆盖。
- `cargo test -p batch-code-analyzer-ipc-contracts`、`cargo test -p batch-code-analyzer-desktop`：
  通过。
- App Core、Persistence、IPC Contracts 与 Desktop 的范围内 Clippy（`--no-deps`）通过。
- `cargo clippy --workspace --all-targets -- -D warnings`：被未修改的
  `crates/secret-store/src/lib.rs` 7 条既有 Pedantic 告警阻断。
- `cargo test --workspace`：除未修改的 Persistence Windows 临时 SQLite 清理测试外均通过；
  `crates/persistence/src/database.rs` 的 3 个测试因 `os error 32` 失败。

## 交付格式

1. 修改文件；
2. 已实现行为；
3. 测试命令与结果；
4. 已知限制；
5. 契约变更建议；
6. 合并注意事项。
