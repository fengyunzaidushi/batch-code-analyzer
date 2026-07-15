# packages

前端共享包。计划包含：

```text
packages/ui/          # 可复用 React UI
packages/ipc-types/   # 从 Rust 自动生成的 TypeScript DTO
```

`ipc-types` 不允许手工维护与 Rust 重复的枚举和 DTO。
