# apps

应用组合层。计划包含：

```text
apps/desktop/   # Tauri 2 + React 桌面应用
```

约束：

- 只组合 UI、Command 和依赖注入；
- 不承载大量领域逻辑；
- 不直接执行 SQL；
- 不直接读取仓库或 API Key。
