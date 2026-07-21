# Tauri IPC 契约（v0.1）

## 1. 目标

本契约定义 React 前端与 Rust 核心之间的稳定边界。Rust 是 DTO、枚举和错误码的权威来源，TypeScript 类型由构建流程自动生成到 `packages/ipc-types`。

## 2. 通用规则

- Command 用于请求—响应；Event 用于长任务进度和状态变化。
- 命令使用 `snake_case`，事件使用 `<domain>://<event>`。
- 所有请求与响应 DTO 必须可序列化并具有稳定字段。
- 所有 Event Payload 必须包含 `schemaVersion` 与 `updatedAt`。
- 所有失败统一返回 `IpcError`，不得向 UI 暴露原始数据库错误、完整本地路径、API Key 或供应商原始敏感响应。
- 前端不得通过 Tauri 文件系统插件直接遍历用户仓库。
- 列表命令必须分页，结果正文按需读取。

## 3. 通用类型

```ts
export interface IpcError {
  schemaVersion: 1;
  code: string;
  category:
    | 'validation'
    | 'project'
    | 'scan'
    | 'security'
    | 'persistence'
    | 'provider'
    | 'scheduler'
    | 'output'
    | 'recovery'
    | 'internal';
  message: string;
  retryable: boolean;
  switchProfile: boolean;
  correlationId: string;
  details?: Record<string, unknown>;
}

export interface PageRequest {
  cursor?: string;
  limit: number; // 1..500
}

export interface PageResponse<T> {
  items: T[];
  nextCursor?: string;
  total: number;
}
```

## 4. Command 清单

### 4.1 Project

```text
project_list
project_add
project_get
project_update_run_settings
project_prompt_save
project_prompt_select
project_update
project_remove
project_relocate
```

最低语义：

- `project_add`：输入用户选择的目录，完成 canonical path 校验；重复目录返回已有项目。
- `project_get`：按需返回当前项目详情；绝对仓库路径不进入项目列表摘要。
- `project_update_run_settings`：更新未来 Run 使用的主 API Profile 和项目默认模型；
  Profile 引用必须存在，响应返回更新后的 `ProjectDetailDto` 和配置镜像写入警告。
- `project_prompt_save`：将命名提示词保存到当前项目的提示词库，并立即设为项目默认；
- `project_prompt_select`：从当前项目提示词库选择一个预设，并将其内容设为项目默认；
- `project_remove`：只移除客户端登记，不删除仓库或历史输出。
- `project_relocate`：重新绑定不可用项目，必须校验项目 ID 或仓库配置的一致性。

### 4.2 Scan

```text
scan_start
scan_cancel
scan_get_report
```

`scan_start` 返回操作 ID，不等待完整扫描结束；进度通过 Event 提供。任一项目同一时间最多一个扫描操作。

本地实现使用以下稳定 DTO：

```ts
interface ScanStartRequest {
  projectId: ProjectId;
  temporaryExcludedPatterns?: string[];
}

interface ScanStartResponse {
  schemaVersion: 1;
  operationId: string;
  projectId: ProjectId;
}

interface ScanReportDto {
  schemaVersion: 1;
  operationId: string;
  projectId: ProjectId;
  status: 'running' | 'completed' | 'cancelled' | 'failed';
  visitedEntries: number;
  scannedFiles: number;
  includedFiles: number;
  excludedByReason: Record<string, number>;
  unreadableFiles: string[];
  unsupportedEncodingFiles: string[];
  sensitiveFiles: string[];
  symlinkFiles: string[];
  invalidGitignoreRules: string[];
  cancelled: boolean;
  fileCount: number | null;
  generation: number | null;
  errorCode: string | null;
  updatedAt: Rfc3339Timestamp;
  rules: ScanRuleSummaryDto;
}

interface ScanRuleSummaryDto {
  builtinDirectories: string[];
  builtinExtensions: string[];
  gitignoreRules: string[];
  temporaryExcludedPatterns: string[];
  sensitiveDetectionEnabled: boolean;
}
```

完成或取消前不会提交正式扫描代次；进度和最终报告通过
`scan://progress` Event 发送，`scan_get_report` 可按 operation ID 查询最近状态。
临时排除模式只在当前项目会话的扫描请求中生效，不会修改仓库 `.gitignore` 或持久化为项目规则。

### 4.3 Context

```text
context_generate
context_update_manual
context_get
prompt_generate
```

`context_generate` 接收 `projectId`，在仓库根目录发现 `README*` 与 `AGENTS.md`，生成
不可变 `ContextVersion` 并更新项目当前版本引用。当前阶段只生成本地发现摘要，不调用
模型，也不返回源码原文。`context_get` 返回当前项目版本或 `null`。上下文生成是独立
辅助请求，不计入文件任务成功/失败统计。

`prompt_generate` 接收当前项目和用户的分析目标，使用项目主 API Profile、实际可读取的
密钥、项目默认模型和当前上下文摘要生成一个可编辑候选。命令只返回候选提示词，不保存
项目配置，也不创建 Run；用户确认后由前端回填当前提示词输入框，再可通过
`project_prompt_save` 保存为项目预设。

### 4.4 API Profile

```text
api_profile_list
api_profile_save
api_profile_secret_put
api_profile_secret_get
api_profile_test
api_profile_delete
api_models_fetch
```

- API Key 只通过 SecretStore 持久化；SQLite、项目 JSON 和普通配置只保存不透明
  `SecretRef`，不得保存 API Key 明文。
- `api_profile_list` 只返回是否已配置密钥和脱敏摘要。
- `api_profile_secret_get` 只允许在用户明确点击“显示 API Key”后调用。它从 SecretStore
  读取并通过专用一次性响应返回当前值；不得记录日志、写入缓存、并入普通 Profile DTO
  或跨 Profile 保留。隐藏、切换 Profile 或保存完成后，前端应清除未编辑的回显值。
- 删除仍被项目引用的档案必须失败并返回稳定错误码。

`api_profile_save` 只保存名称、Base URL、协议和默认模型等非敏感元数据。
`api_profile_secret_put` 是一次性写入命令：密钥只进入 SecretStore，命令不返回密钥。
`api_profile_secret_get` 的专用响应是“显式用户回显”的唯一例外，不得复用于列表、保存、
测试或模型请求 DTO。`api_profile_test` 与 `api_models_fetch`
通过 Provider 的模型列表请求验证连接并缓存脱敏模型元数据。

### 4.5 Run

```text
run_preview
run_create
run_execute
run_list
run_get
run_pause
run_resume
run_cancel
run_get
run_list
```

- `run_preview` 返回任务数量、排除数量、预计使用配置和阻塞项，不创建 Run。
- `run_create` 在事务内创建不可变快照与 Task。
- 首期存在活动 Run 时，第二次 `run_create` 返回 `run_active_exists`。
- `run_cancel` 接收已有 `runId`，原子取消排队 Task、将已领取 Task 标记为中断，
  并把 Run 收敛为 `cancelled`；若存在进程内请求，会同时触发请求取消令牌。
- Run 预览、创建和执行前必须确认主 API Profile 的密钥引用当前可由 SecretStore
  读取；只有数据库中存在引用但当前会话无法读取时，也必须返回
  `security_secret_not_found`，不得创建必然失败的 Run。

`run_preview` 和 `run_create` 接收 `projectId`，可选地接收当前提示词和模型覆盖。
预览响应只包含相对路径、文件大小、内容哈希、解析后的模型和阻塞原因，不包含源文件内容。
创建成功后返回 `RunSummaryDto` 和创建的 Task 数量；Run 初始状态为 `running`，Task 初始状态为
`queued`，实际模型请求由后续调度器任务负责。

`run_execute` 只接收已有的 `runId`，要求 Run 处于 `running` 状态。执行器按顺序领取
queued Task，并在每次真实请求前追加 `created` Attempt。Provider 成功时先原子写入结果
Markdown，再提交 Attempt、Task 和 Run 统计；Provider、源码读取或结果写入失败时保存脱敏
错误摘要并将 Task 收敛为 `failed`。命令只返回最终 `RunSummaryDto`，不返回源码、密钥或
完整 Provider 响应。

执行前置校验或持久化失败时，执行器会把仍处于活动状态的 Run 收敛为 `interrupted`，
避免创建成功但永远占用活动 Run 限制。

### 4.6 File

```text
file_list
file_update_override
file_set_included
file_authorize_sensitive
```

`file_list` 接收 `projectId`、可选的数字游标和 `1..=500` 的 `limit`，返回
`PageResponse<FileRecordSummaryDto>`。摘要只包含相对路径、文件状态、纳入标记和结果状态，
不返回源码内容或绝对仓库路径。

`file_update_override` 只更新未来 Run 的单文件覆盖，不修改已创建 Run 的 Task 快照。

`file_set_included` 接收 `projectId`、`fileId` 和 `included`，返回
`{ file: FileRecordSummaryDto }`。手动排除会持久化为当前 FileRecord 的用户覆盖，
并在后续扫描中保留。敏感、二进制、过大、不可读取、编码不支持和已删除文件不能
通过普通纳入命令绕过安全阻止。

`file_authorize_sensitive` 接收 `projectId`、`fileId` 和明确的 `confirmed: true`。
Rust 会重新校验仓库边界、符号链接、文件大小、二进制和编码，计算当前内容哈希并
只保存脱敏的风险类型与位置。授权不会返回源码或秘密原文，文件仍保留 `sensitive`
来源状态，但可以进入后续 Run。普通 `file_set_included(false)` 可以撤销授权；重新扫描
后授权默认失效，需要用户再次确认。

### 4.7 Task

```text
task_list
task_get
task_retry
task_regenerate
task_cancel
```

- `task_retry`：只对允许重试的失败、中断或取消任务创建新 Attempt。
- `task_regenerate`：创建新的 Task 版本，不覆盖原 Task。
- 运行中 Task 不允许重复提交。

`run_list` 按 Project 返回分页的 `RunSummaryDto`，`run_get` 只允许读取该 Project
所属的 Run。`task_list` 按 Run 返回分页的 `TaskSummaryDto`，`task_get` 返回一个
Task、创建该 Task 时冻结的 `promptSnapshot` 和按 sequence 升序排列的 `AttemptDto`
历史。`promptSnapshot` 只包含用户配置的分析提示词，不包含源文件内容、API Key 或
供应商完整请求体。跨 Project 的 ID 查询统一按不存在处理，不暴露其他项目是否存在。

### 4.8 Result

```text
result_read
result_open_in_folder
```

`result_read` 只接收 Project ID 与 Task ID。Rust 从 SQLite 的当前结果引用和所属
Run 输出目录重新解析路径，拒绝路径逃逸、外部符号链接、缺失和超大文件；响应只
包含结果相对路径、版本和 Markdown 正文，不包含源文件、绝对路径、请求原文或密钥。

- `result_read` 按需读取已完成结果；不得通过 `task_list` 返回完整 Markdown。
- `result_open_in_folder` 只能打开已验证位于允许输出根目录内的路径。

### 4.9 App

```text
app_get_settings
app_update_settings
health_check
```

`health_check` 返回桌面核心的受控启动状态，可用于前端启动检测、自动化测试和故障诊断。`status: ready` 仅表示 Rust 核心可响应，不表示数据库、扫描或模型服务已经可用。

```ts
export interface HealthCheckResponse {
  schemaVersion: 1;
  status: "ready" | "degraded" | "unavailable";
  appVersion: string;
  databaseStatus:
    | 'not_initialized'
    | 'ready'
    | 'migration_failed'
    | 'unavailable';
  databaseSchemaVersion: number;
}
```

数据库基础设施尚未建立时，响应必须为 `databaseStatus: 'not_initialized'` 与 `databaseSchemaVersion: 0`。

## 5. Event 清单

```text
scan://progress
scan://completed
scan://failed

run://state-changed
run://stats-changed
run://completed

task://state-changed
task://attempt-started
task://attempt-finished
task://result-written

context://state-changed
api-profile://health-changed
app://fatal-error
```

通用事件示例：

```json
{
  "schemaVersion": 1,
  "projectId": "project-uuid",
  "runId": "run-uuid",
  "taskId": "task-uuid",
  "previousStatus": "queued",
  "status": "running",
  "updatedAt": "2026-07-15T10:38:30Z",
  "correlationId": "correlation-uuid"
}
```

## 6. Task 分页契约

```ts
export interface TaskListRequest {
  runId: string;
  cursor?: string;
  limit: number; // 最大 500
  filters: {
    statuses?: string[];
    pathContains?: string;
    model?: string;
    hasPromptOverride?: boolean;
    hasModelOverride?: boolean;
    minDurationMs?: number;
    maxDurationMs?: number;
    minTokens?: number;
    maxTokens?: number;
  };
  sort: Array<{
    field: 'relativePath' | 'status' | 'durationMs' | 'totalTokens' | 'updatedAt';
    direction: 'asc' | 'desc';
  }>;
}

export interface TaskListResponse {
  items: TaskSummary[];
  nextCursor?: string;
  total: number;
}
```

## 7. 事件顺序与一致性

Rust 必须按以下顺序处理：

```text
数据库事务提交
→ 更新内存状态
→ 发送 Tauri Event
```

Event 是 UI 加速机制，不是业务状态权威来源。前端错过 Event 后，必须能通过查询 Command 恢复一致状态。

## 8. 类型生成

工程初始化阶段应选择 `ts-rs` 或等价工具，满足：

- Rust DTO 自动生成 TypeScript；
- CI 检查生成文件无漂移；
- 枚举值不允许前后端重复手写；
- 破坏性修改升级 `schemaVersion` 并记录 ADR。
