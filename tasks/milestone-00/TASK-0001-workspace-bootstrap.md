# TASK-0001：初始化 Monorepo、Tauri 与 React 工程

- Status: Ready
- Owner: Unassigned
- Branch: feat/m0-workspace-bootstrap
- Dependencies: None

## 目标

建立可启动的 Tauri 2 + React + Vite + TypeScript + Rust Workspace 空壳，并让基础检查命令可运行。

## 必读文档

- `AGENTS.md`
- `docs/architecture.md`：2、3、4、5、21、25 节
- `docs/decisions/0001-tauri-react-rust.md`

## 允许修改

```text
apps/**
crates/**
packages/**
Cargo.toml
Cargo.lock
package.json
pnpm-lock.yaml
pnpm-workspace.yaml
rust-toolchain.toml
.gitignore
```

## 行为要求

1. 创建 pnpm Workspace 和 Cargo Workspace；
2. 在 `apps/desktop` 初始化 Tauri 2 与 React/Vite；
3. TypeScript 开启 `strict`、`noUncheckedIndexedAccess`、`exactOptionalPropertyTypes`；
4. 创建最小 `crates/domain`、`crates/app-core`、`packages/ipc-types` 占位结构；
5. 提供一个最小健康检查 Command 和前端展示；
6. 不实现业务功能；
7. 不向前端开放任意文件系统或 Shell 权限。

## 验收标准

- [ ] `pnpm install` 成功；
- [ ] 前端开发服务器和 Tauri 开发模式可启动；
- [ ] `cargo check --workspace` 通过；
- [ ] `pnpm typecheck` 通过；
- [ ] 根目录没有业务逻辑堆积；
- [ ] 目录符合 `docs/architecture.md`。
