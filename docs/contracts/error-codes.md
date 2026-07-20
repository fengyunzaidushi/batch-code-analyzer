# 错误模型与稳定错误码（v0.1）

## 1. 统一结构

```json
{
  "schemaVersion": 1,
  "code": "provider_rate_limited",
  "category": "provider",
  "message": "请求受到限流",
  "retryable": true,
  "switchProfile": true,
  "details": {
    "httpStatus": 429,
    "retryAfterSeconds": 10
  },
  "correlationId": "uuid"
}
```

`message` 用于用户展示，可以本地化；业务判断只能依赖 `code`、`retryable` 和 `switchProfile`。

## 2. 分类

```text
validation
project
scan
security
persistence
provider
scheduler
output
recovery
internal
```

## 3. 命名规则

- 全部使用 `snake_case`；
- 以领域作为前缀；
- 一个错误码只表达一个稳定语义；
- 不把 HTTP 文案或供应商原始错误直接作为错误码；
- 新增错误码需要补充测试与本文件；
- 删除或改变语义属于破坏性契约修改。

## 4. 基础错误码

### 4.1 Validation

| Code | Retry | Switch | 含义 |
| --- | --- | --- | --- |
| `validation_required_field` | 否 | 否 | 必填字段缺失 |
| `validation_invalid_value` | 否 | 否 | 字段格式或枚举非法 |
| `validation_limit_exceeded` | 否 | 否 | 数量或长度超过应用限制 |
| `validation_model_missing` | 否 | 否 | 无法解析任务实际模型 |
| `api_profile_name_duplicate` | 否 | 否 | API Profile 名称已存在 |

### 4.2 Project

| Code | Retry | Switch | 含义 |
| --- | --- | --- | --- |
| `project_not_found` | 否 | 否 | 项目不存在 |
| `project_path_unavailable` | 是 | 否 | 原仓库路径不可用 |
| `project_path_duplicate` | 否 | 否 | canonical path 已登记 |
| `project_config_readonly` | 否 | 否 | 仓库不可写，已降级到应用配置 |
| `project_relocation_mismatch` | 否 | 否 | 新路径与原项目身份不匹配 |
| `api_profile_in_use` | 否 | 否 | API Profile 仍被项目主备路由引用 |

### 4.3 Scan

| Code | Retry | Switch | 含义 |
| --- | --- | --- | --- |
| `scan_already_running` | 否 | 否 | 当前项目已有扫描 |
| `scan_cancelled` | 否 | 否 | 用户取消扫描 |
| `scan_failed` | 是 | 否 | 扫描未能完成 |
| `scan_not_found` | 否 | 否 | 扫描操作不存在 |
| `scan_file_unreadable` | 是 | 否 | 文件不可读取 |
| `scan_gitignore_invalid_rule` | 否 | 否 | 忽略规则无法解析 |
| `scan_file_too_large` | 否 | 否 | 文件超过大小限制 |
| `scan_binary_file` | 否 | 否 | 检测为二进制文件 |
| `scan_encoding_unsupported` | 否 | 否 | 不支持或无法安全识别编码 |
| `context_discovery_failed` | 是 | 否 | 项目上下文资料无法安全读取 |

### 4.4 Security

| Code | Retry | Switch | 含义 |
| --- | --- | --- | --- |
| `security_path_escape` | 否 | 否 | 路径逃逸到允许根目录外 |
| `security_symlink_outside_root` | 否 | 否 | 符号链接指向仓库外 |
| `security_sensitive_file_blocked` | 否 | 否 | 默认敏感文件被阻止 |
| `security_sensitive_confirmation_required` | 否 | 否 | 敏感文件授权请求缺少明确确认 |
| `security_secret_detected` | 否 | 否 | 文件中发现疑似秘密 |
| `security_consent_required` | 否 | 否 | 尚未确认向该服务发送源码 |
| `security_external_url_blocked` | 否 | 否 | 外部链接协议或目标不允许 |
| `security_secret_store_unavailable` | 是 | 否 | 系统安全存储不可用 |
| `security_invalid_secret_reference` | 否 | 否 | SecretRef 或包含凭据的 URL 无效 |
| `security_secret_not_found` | 否 | 否 | SecretRef 不存在 |
| `security_secret_store_failure` | 是 | 否 | 安全存储后端操作失败 |

### 4.5 Persistence

| Code | Retry | Switch | 含义 |
| --- | --- | --- | --- |
| `persistence_database_unavailable` | 是 | 否 | 数据库无法打开 |
| `persistence_database_busy` | 是 | 否 | 数据库锁等待超时 |
| `persistence_migration_failed` | 否 | 否 | Migration 失败，进入恢复模式 |
| `persistence_schema_too_new` | 否 | 否 | 当前应用不支持更高 Schema |
| `persistence_transaction_failed` | 是 | 否 | 事务提交失败 |

### 4.6 Provider

| Code | Retry | Switch | 含义 |
| --- | --- | --- | --- |
| `provider_connection_failed` | 是 | 是 | 连接失败 |
| `provider_timeout` | 是 | 是 | 请求超时 |
| `provider_rate_limited` | 是 | 是 | 429；优先尊重 Retry-After |
| `provider_server_error` | 是 | 是 | 5xx |
| `provider_authentication_failed` | 否 | 是 | 401 或确定的认证失败 |
| `provider_permission_denied` | 否 | 是 | 账户或 API 权限不足 |
| `provider_model_unavailable` | 否 | 是 | 当前档案无法使用模型 |
| `provider_content_rejected` | 否 | 否 | 内容审核或策略拒绝 |
| `provider_invalid_request` | 否 | 否 | 请求参数或协议错误 |
| `provider_invalid_response` | 是 | 是 | 返回体无法解析或缺少必要内容 |
| `provider_cancelled` | 否 | 否 | 用户取消 |
| `provider_interrupted_unknown` | 否 | 否 | 结果未知，禁止自动重发 |

`403` 必须根据错误体分类为 permission、content rejection、区域/账户限制等，不允许统一映射为认证失败。

### 4.7 Scheduler

| Code | Retry | Switch | 含义 |
| --- | --- | --- | --- |
| `run_active_exists` | 否 | 否 | 首期已有活动 Run |
| `run_invalid_transition` | 否 | 否 | Run 状态转换非法 |
| `run_not_active` | 否 | 否 | 操作要求活动 Run |
| `run_not_paused` | 否 | 否 | 继续操作要求 Paused |
| `run_already_terminal` | 否 | 否 | Run 已进入终态 |
| `task_invalid_transition` | 否 | 否 | Task 状态转换非法 |
| `task_already_running` | 否 | 否 | Task 已执行中 |
| `task_cannot_retry` | 否 | 否 | 当前 Task 不允许重试 |
| `task_source_changed` | 否 | 否 | 文件内容与 Task 快照不一致 |

### 4.8 Output

| Code | Retry | Switch | 含义 |
| --- | --- | --- | --- |
| `output_directory_unavailable` | 是 | 否 | 输出根目录不可用 |
| `output_path_mapping_failed` | 否 | 否 | 无法安全映射结果路径 |
| `output_write_failed` | 是 | 否 | 原子写入失败 |
| `output_export_failed` | 是 | 否 | 模型请求成功但导出镜像失败 |

### 4.9 Recovery

| Code | Retry | Switch | 含义 |
| --- | --- | --- | --- |
| `recovery_interrupted_run_found` | 否 | 否 | 启动时发现中断 Run |
| `recovery_unknown_attempt_result` | 否 | 否 | Attempt 结果未知，需用户处理 |
| `recovery_lock_held` | 是 | 否 | 另一个应用实例持有项目或数据库锁 |
| `recovery_backup_failed` | 是 | 否 | 数据库或配置备份失败 |

### 4.10 Internal

| Code | Retry | Switch | 含义 |
| --- | --- | --- | --- |
| `internal_unexpected` | 否 | 否 | 未分类内部错误 |
| `internal_contract_violation` | 否 | 否 | 模块违反公共契约 |

## 5. 脱敏要求

进入 UI、SQLite、日志和输出前，统一移除或掩码：

- `Authorization`、Cookie、Bearer Token；
- API Key 与自定义敏感 Header；
- 数据库连接 URL；
- 文件中秘密扫描的完整命中值；
- 供应商响应中的账户和凭据字段。

允许保留：错误类别、HTTP 状态码、供应商错误类型、掩码后的尾部字符、Retry-After、模型名和 correlation ID。
