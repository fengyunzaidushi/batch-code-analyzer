# Run、Task 与 Attempt 状态机（v0.1）

## 1. 原则

- 状态只通过 `RunStateMachine`、`TaskStateMachine` 和 Attempt 完成服务转换。
- 数据库提交成功后才能发送状态 Event。
- 非法转换必须返回稳定错误码，不得静默忽略。
- 取消、暂停、崩溃恢复和结果未知必须有不同语义。

## 2. Run 状态

```rust
pub enum RunStatus {
    Draft,
    Running,
    Pausing,
    Paused,
    Cancelling,
    Cancelled,
    Completed,
    CompletedWithErrors,
    Interrupted,
}
```

### 2.1 合法转换

| 当前状态 | 事件 | 下一状态 | 说明 |
| --- | --- | --- | --- |
| Draft | start | Running | 创建 Task 后开始调度 |
| Running | pause_requested | Pausing | 停止领取新 Task |
| Pausing | active_tasks_drained | Paused | 已在飞任务结束或取消 |
| Paused | resume | Running | 继续领取任务 |
| Running/Pausing/Paused | cancel_requested | Cancelling | 停止领取并取消允许取消的任务 |
| Cancelling | all_tasks_terminal | Cancelled | 所有任务进入终态 |
| Running | all_tasks_succeeded | Completed | 全部成功 |
| Running | all_tasks_terminal_with_errors | CompletedWithErrors | 存在失败/取消/中断 |
| CompletedWithErrors | manual_retry_requested | Running | 可重试失败 Task 人工重新排队 |
| Running/Pausing/Cancelling | process_interrupted | Interrupted | 应用异常退出或进程失联 |

`CompletedWithErrors` 在没有人工重试操作时视为稳定状态，但允许通过
`manual_retry_requested` 重新进入 `Running`。`Cancelled`、`Completed` 和
`Interrupted` 仍为终态；其中取消和中断任务的重新排队需先经过重复计费确认流程。

## 3. Task 状态

```rust
pub enum TaskStatus {
    Pending,
    Queued,
    Running,
    Succeeded,
    Failed,
    Cancelled,
    Interrupted,
    SourceChanged,
}
```

### 3.1 合法转换

| 当前状态 | 事件 | 下一状态 |
| --- | --- | --- |
| Pending | enqueue | Queued |
| Pending/Queued | cancel | Cancelled |
| Queued | claim | Running |
| Running | succeed | Succeeded |
| Running | fail | Failed |
| Running | cancel_confirmed | Cancelled |
| Running | process_interrupted | Interrupted |
| Pending/Queued/Running | source_hash_changed | SourceChanged |
| Failed/Cancelled/Interrupted | manual_retry | Queued |

### 3.2 特殊规则

- `Succeeded` 不通过 retry 转回 Queued；用户“重新生成”必须创建新 Task 版本。
- `SourceChanged` 不自动执行；用户需重新扫描并创建新 Run，或明确创建新版本。
- 实际发送前发现源码哈希与 Run 快照不一致时，Running Task 直接进入
  `SourceChanged`，不得创建 Attempt 或发送模型请求。
- 运行中的 Task 不允许重复发送。
- 自动重试不会让 Task 离开 `Running`；每次网络尝试通过 Attempt 表示。
- 人工重试保留原 Attempt，并新增 Attempt。
- 失败 Task 人工重试必须在同一事务内将父 Run 从 `CompletedWithErrors` 转为
  `Running`，并清空 Run/Task 的完成时间；不得在终态 Run 下留下 Queued Task。
- Failed Task 只有在最新 Attempt 的脱敏错误标记为 `retryable: true` 时才能人工
  重试。Cancelled/Interrupted Task 仍按 PRD 支持重新排队，但必须先完成重复计费确认，
  并通过各自合法的父 Run 状态转换实现。

## 4. Attempt 状态

建议枚举：

```text
created
dispatched
succeeded
failed_retryable
failed_terminal
cancelled
interrupted_unknown
```

状态语义：

- `created`：数据库记录已建立，尚未发送网络请求；
- `dispatched`：请求已交给网络层；
- `succeeded`：获得并持久化有效响应；
- `failed_retryable`：本次尝试失败，但策略允许重试或切换档案；
- `failed_terminal`：错误不允许继续尝试；
- `cancelled`：可确定请求已取消；
- `interrupted_unknown`：无法判断服务端是否完成，禁止自动重发。

## 5. 暂停语义

- 暂停只阻止领取新 Task；
- 默认允许已在飞请求完成；
- 并发数调低不撤销已经持有的 permit；
- Run 进入 `Paused` 前必须处理所有已领取 Task 的最终状态；
- 首期不支持关闭窗口后在后台继续运行。

## 6. 取消语义

- 未执行 Task：直接 `Cancelled`；
- 已在飞请求：触发 CancellationToken；
- 能确认取消时 Attempt 为 `cancelled`；
- 无法确认时 Attempt 与 Task 为中断/结果未知；
- 用户取消不触发自动重试和备用档案切换。

## 7. 崩溃恢复

应用启动时：

```text
查询 Running/Pausing/Cancelling Run
→ 未结束 Attempt 标记 interrupted_unknown
→ 对应 Running Task 标记 Interrupted
→ Run 标记 Interrupted
→ 重算统计
→ 向 UI 返回恢复摘要
```

恢复后不得自动重发。用户主动选择重试时新增 Attempt，并显示可能重复计费提示。

## 8. 自动重试与主备切换

每个 Task 在 `Running` 状态内执行：

```text
按 Run 快照遍历 API 路由链
→ 为每次真实请求创建 Attempt
→ 可重试错误按 Retry-After 或退避等待
→ 当前档案耗尽后按规则切换备用档案
→ 成功则 Task Succeeded
→ 本地或不可切换错误则 Task Failed
→ 全部档案失败则 Task Failed
```

认证失败、限流、服务错误和本地输入错误的具体决策以 `error-codes.md` 为准。

## 9. 稳定状态机错误码

```text
run_invalid_transition
run_active_exists
run_not_active
run_not_paused
run_already_terminal

task_invalid_transition
task_already_running
task_cannot_retry
task_cannot_regenerate
task_source_changed
```
