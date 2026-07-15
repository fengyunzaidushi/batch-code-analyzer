# crates

Rust 领域和基础设施模块。建议逐步创建：

```text
domain
app-core
persistence
ipc-contracts
repository-scanner
security-core
api-profiles
secret-store
model-providers
task-scheduler
output-service
recovery-service
```

每个 crate 必须有明确公开接口、单元测试和禁止依赖方向。避免所有逻辑集中在 Tauri `lib.rs`。
