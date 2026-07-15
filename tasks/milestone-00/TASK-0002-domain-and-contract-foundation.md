# TASK-0002：建立领域枚举、状态机接口与 IPC 类型生成

- Status: Ready
- Owner: Unassigned
- Branch: feat/m0-domain-contracts
- Dependencies: TASK-0001

## 目标

建立 Project、FileRecord、Run、Task、Attempt、ContextVersion 的核心 ID/枚举，以及 Rust 到 TypeScript 的 DTO 类型生成链路。

## 必读文档

- `AGENTS.md`
- `docs/architecture.md`：6、10、15、16 节
- `docs/contracts/ipc-contract.md`
- `docs/contracts/task-state-machine.md`
- `docs/contracts/error-codes.md`

## 允许修改

```text
crates/domain/**
crates/ipc-contracts/**
packages/ipc-types/**
```

## 行为要求

1. 定义强类型 ID，而非在内部到处传裸字符串；
2. 实现 RunStatus、TaskStatus、AttemptStatus；
3. 提供状态机合法转换验证，不访问数据库；
4. 定义 `IpcError` 与通用分页 DTO；
5. 建立 Rust → TypeScript 类型生成；
6. CI 可检查生成文件是否最新；
7. 不实现 Repository、Tauri Command 或 UI。

## 验收标准

- [ ] 状态机合法和非法转换均有单元测试；
- [ ] Rust 枚举能稳定生成 TypeScript；
- [ ] 前后端不存在手写重复枚举；
- [ ] 错误结构不包含敏感内部字段。
