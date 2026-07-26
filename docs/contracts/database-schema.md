# SQLite 数据契约（v0.1）

## 1. 权威性

SQLite 是项目、文件、上下文、Run、Task、Attempt 和恢复状态的唯一运行时权威来源。仓库中的 `.batch-analysis/*.json` 与输出目录中的 JSON/Markdown 仅作为可迁移镜像和用户可读导出。

## 2. 数据库位置

```text
Windows: %APPDATA%/<AppName>/app.db
macOS:   ~/Library/Application Support/<AppName>/app.db
Linux:   ~/.local/share/<AppName>/app.db
```

启动时执行：

```sql
PRAGMA foreign_keys = ON;
PRAGMA journal_mode = WAL;
PRAGMA synchronous = NORMAL;
PRAGMA busy_timeout = 5000;
```

## 3. 核心关系

```text
Project 1 ── N FileRecord
Project 1 ── N ContextVersion
Project 1 ── N Run
Run     1 ── N Task
Task    1 ── N Attempt
Task    N ── 1 FileRecord
```

## 4. 核心表

工程初始化时以 `docs/architecture.md` 第 7.3 节 SQL 为基线创建首个 Migration，至少包含：

```text
projects
api_profiles
encrypted_secrets
secret_store_metadata
file_records
context_versions
runs
tasks
attempts
prompt_library
app_settings
```

不得在业务模块内临时创建表或绕过 Migration 修改 Schema。

## 5. 关键字段约束

### projects

- `canonical_source_directory` 全局唯一；
- API Key 默认只保存安全引用 ID；若启用加密 SQLite 后端，`encrypted_secrets` 只能保存
  AEAD 密文和随机 nonce，包装密钥引用保存在 `secret_store_metadata`，实际包装密钥仍在
  操作系统安全存储中；禁止出现 API Key 明文；
- `filter_rules_json`、`execution_defaults_json` 和 `api_routing_json` 必须有独立版本字段；
- 路径不可用时保留项目与历史 Run。

### file_records

- 唯一键：`project_id + normalized_relative_path`；
- 当前仓库事实与历史 Task 快照分离；
- `content_hash` 推荐 BLAKE3；
- 重新扫描通过 `scan_generation` 标记未出现的旧记录；
- 敏感发现仅保存类型、位置与掩码，不保存完整秘密值。

### runs

- 创建后 `snapshot_json` 不可修改；
- Run 状态必须由状态机更新；
- `stats_json` 是运行缓存，最终统计以 Task 查询重算；
- 首期通过事务或应用级锁保证最多一个活动 Run。

### tasks

- 保存创建 Run 时的文件、提示词、模型与上下文快照；
- `current_result_path` 必须位于当前 Run 的结果目录；
- 重新生成创建新 Task，并通过 `parent_task_id` 关联；
- 不覆盖历史 Task。

### attempts

- `task_id + sequence` 唯一；
- 在真实网络分发前先创建 Attempt；
- 自动重试、备用切换和人工重试都新增记录；
- 错误信息写入前必须脱敏；
- 中断且无法确定供应商结果时，状态使用 `interrupted_unknown`。

## 6. 必需索引

至少创建：

```sql
CREATE INDEX idx_files_project_status
ON file_records(project_id, source_status, result_status);

CREATE INDEX idx_runs_project_created
ON runs(project_id, created_at DESC);

CREATE INDEX idx_tasks_run_status
ON tasks(run_id, status);

CREATE INDEX idx_tasks_file
ON tasks(file_id, created_at DESC);

CREATE INDEX idx_attempts_task_sequence
ON attempts(task_id, sequence);
```

## 7. 事务边界

以下操作必须在单个事务中完成：

- 新建项目及其初始设置；
- 创建 Run、不可变快照和全部初始 Task；
- Task 从排队中领取为运行中；
- Attempt 创建与 Task 运行标记；
- 请求成功后的结果路径、Attempt 和 Task 状态更新；
- 状态转换与统计增量更新；
- 项目重新定位后的 canonical path 更新。

## 8. 并发与领取

领取 Task 必须防止重复执行：

```text
BEGIN IMMEDIATE
SELECT 可领取任务
UPDATE 选中任务为 running
COMMIT
```

具体 SQL 可依据 SQLite 版本调整，但必须由集成测试证明同一 Task 不会被两个执行单元同时领取。

## 9. Migration 规则

- 文件命名使用单调递增编号；
- 每个 Migration 只执行一次；
- 启动迁移前备份数据库；
- 迁移失败进入只读恢复模式；
- 不允许自动删除用户数据；
- 不支持 Schema 降级；
- 旧应用遇到更高 Schema 必须拒绝写入；
- Schema 变更需要数据库 Owner 审核。

## 10. JSON 镜像与输出

- Task 成功：先原子写 Markdown，再提交数据库结果路径；
- Attempt 完成：先提交 SQLite，再追加 `attempts.jsonl`；
- Run 结束：从 SQLite 重新生成完整 `manifest.json` 与 `tasks.json`；
- 导出失败不将模型请求改为失败，但必须记录 `output_export_failed`。

## 11. 禁止事项

- 不在 SQLite 保存 API Key 明文；
- 不在普通日志保存完整源码；
- 不用一个大型 `tasks.json` 代替数据库；
- 不允许前端直接执行 SQL；
- 不允许业务服务绕过 Repository 随意更新状态字段；
- 不允许删除旧 Attempt 以节省空间，清理功能需另行设计并经用户确认。
