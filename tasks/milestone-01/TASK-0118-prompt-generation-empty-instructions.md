# TASK-0118：Prompt Generation Empty Instructions

- Status: Review
- Owner: Codex
- Branch: feat/m1-run-results-ui
- Dependencies: TASK-0103, TASK-0115

## 目标

让“生成提示词”调用兼容 OpenAI Responses API 时始终发送空的 `instructions` 字段，避免附加默认系统指令。

## 必读文档

- `AGENTS.md`
- `docs/prd.md`：5.3“生成提示词”、5.4“项目上下文”
- `docs/architecture.md`：9.1、9.2
- `docs/contracts/ipc-contract.md`：4.3 Context
- `crates/app-core/src/lib.rs`
- `crates/model-providers/src/lib.rs`

## 允许修改

```text
tasks/milestone-01/TASK-0118-prompt-generation-empty-instructions.md
crates/app-core/src/lib.rs
```

## 禁止修改

```text
Cargo.toml
Cargo.lock
package.json
pnpm-lock.yaml
docs/contracts/**
crates/model-providers/**
crates/persistence/migrations/**
apps/desktop/src-tauri/**
packages/ipc-types/**
```

## 输入与依赖

- `PromptGenerationService` 负责构造候选提示词请求。
- `OpenAiResponsesProvider` 会将未指定的 `ProviderRequest.instructions` 序列化为 `"instructions":""`。
- `prompt_generate` 的 IPC 请求与响应 DTO 保持不变。

## 输出接口

不新增或修改公共接口。`PromptGenerationService::generate` 保持原有签名与错误码。

## 行为要求

1. 生成候选提示词时，不向 `ProviderRequest` 设置系统指令。
2. 实际发出的 Responses 请求 JSON 必须包含 `"instructions":""`。
3. 用户目标与项目上下文摘要仍作为 `input` 发送；候选内容仍只返回给调用方，不持久化项目配置。
4. 空用户目标继续返回 `validation_required_field`。

## 不在范围内

- 更改正式 Run 的提示词或 `instructions` 行为；
- 调整 UI、IPC DTO、模型路由、上下文发现或 API 配置；
- 调用真实模型服务。

## 验收标准

- [x] 成功路径验证候选提示词与实际请求的空 `instructions`；
- [x] 空目标校验测试通过；
- [x] 现有 Run 执行测试保持通过；
- [ ] 全工作区 Rust 检查通过（既有 `secret-store` Clippy 规则问题阻断）；
- [ ] 全工作区 Rust 测试通过（既有 Windows SQLite 临时文件清理测试阻断）；
- [x] 前端 lint、类型检查和测试通过；
- [x] IPC 类型无漂移；
- [ ] 没有越界修改（独立 Provider 集成测试生成的未跟踪 `tests/integration/target/` 无法由当前环境清理）；
- [x] 任务文档已同步。

## 交付格式

1. 修改文件；
2. 已实现行为；
3. 测试命令与结果；
4. 已知限制；
5. 契约变更建议；
6. 合并注意事项。
