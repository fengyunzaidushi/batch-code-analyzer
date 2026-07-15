# AGENTS.md

## 1. 项目定位

本项目是一个本地桌面端“批量代码文件 AI 分析工具”。它负责登记本地代码仓库、筛选真实项目文件、按文件调用兼容 OpenAI Responses API 的模型，并保存 Markdown 结果、运行统计与请求历史。

本项目不负责：

- 修改用户源代码；
- 推断真实程序员的 Git 开发过程；
- 云端任务队列或团队协作；
- 同时运行多个正式 Run；
- 在首期支持流式模型响应。

## 2. 权威文档与优先级

发生冲突时，按以下顺序处理：

1. `docs/prd.md`：产品行为与验收标准的权威来源；
2. `docs/architecture.md`：技术架构与实现约束的权威来源；
3. `docs/contracts/*.md`：跨模块接口、状态与错误码的权威来源；
4. `docs/decisions/*.md`：已批准的架构决策；
5. 当前 `tasks/**/*.md`：本次任务范围；
6. 现有代码与测试。

任务文件不得擅自覆盖 PRD、架构或公共契约。发现冲突时，不自行猜测；记录冲突、影响范围与建议方案，由总控或集成负责人处理。

## 3. 强制架构边界

- React 只负责展示和用户交互，不直接读取仓库、SQLite 或 API Key。
- 仓库扫描、文件读取、路径校验、模型请求和业务状态变更均在 Rust 中完成。
- SQLite 是运行状态的权威来源；JSON/Markdown 是可迁移配置与用户可读导出。
- Rust 领域实体不得原样暴露给前端，必须通过稳定 DTO。
- Run 创建后，其文件清单、文件哈希、提示词、模型、上下文版本、API 路由和重试策略不可变。
- 所有自动重试、备用档案切换和人工重试均新增 Attempt，不覆盖历史 Attempt。
- 所有 Run/Task 状态变化必须通过统一状态机，不得直接修改状态字符串。
- 数据库事务提交成功后，才能更新内存状态并发送 Tauri Event。
- API Key 不得出现在普通配置、SQLite 明文字段、日志、错误信息、导出文件或测试快照中。
- 所有输入和输出路径必须经过仓库边界、符号链接和路径逃逸检查。
- 首期只允许一个活动 Run；中断且结果未知的请求不得自动重发。

## 4. Agent 工作规则

开始编码前必须：

1. 阅读本文件；
2. 阅读任务书列出的 PRD、架构和契约章节；
3. 检查现有代码、测试和相邻模块接口；
4. 输出一个简短实施计划；
5. 确认任务书允许修改的目录。

开发过程中：

- 只修改任务书允许的目录；
- 不顺手重构任务范围外代码；
- 不自行修改数据库 Migration、公共 IPC DTO、公共错误码、Workspace 依赖或锁文件；
- 需要修改公共契约时，先提交契约变更建议，不在模块代码中偷渡；
- 不通过删除测试、跳过测试、降低 lint 或放宽类型检查完成任务；
- 不调用真实收费模型 API，使用仓库内 Mock Provider；
- 日志和测试夹具不得包含真实密钥、真实客户源码或个人数据；
- 新增行为必须覆盖成功路径、失败路径和边界条件。

## 5. 完成标准

每个任务完成前必须：

1. 运行格式化；
2. 运行 lint 和静态检查；
3. 运行相关单元测试；
4. 运行相关集成测试；
5. 检查 `git diff` 是否包含越界修改；
6. 更新受影响的接口文档或 ADR；
7. 给出修改摘要、测试结果、已知限制和合并注意事项。

项目初始化后，最低检查命令应包括：

```bash
pnpm install
pnpm lint
pnpm typecheck
pnpm test

cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

## 6. Git 与 Worktree 规则

- `main`：稳定发布分支；
- `develop`：当前集成分支；
- 功能分支格式：`feat/<milestone>-<module>`；
- 修复分支格式：`fix/<issue>-<summary>`；
- 一个 worktree 对应一个明确任务或紧密相关的任务组；
- 公共文件必须有单一 Owner；
- 合并前先 rebase 或同步最新 `develop`，再运行本模块测试；
- 集成 Agent 负责最终合并、冲突解决和全量测试。

以下文件默认由总控或集成负责人维护，普通模块 Agent 不得直接修改：

```text
Cargo.toml
Cargo.lock
package.json
pnpm-lock.yaml
pnpm-workspace.yaml
数据库 migrations
docs/contracts/**
AGENTS.md
tauri.conf.json
.github/workflows/**
```

## 7. 任务交付格式

完成任务时返回：

1. 修改文件列表；
2. 已实现行为；
3. 执行的命令和测试结果；
4. 未实现或受限行为；
5. 对公共契约的建议；
6. 合并顺序和潜在冲突。
