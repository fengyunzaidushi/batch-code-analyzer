# TASK-0004：建立 CI 与质量门禁

- Status: Ready
- Owner: Unassigned
- Branch: feat/m0-ci-quality
- Dependencies: TASK-0001

## 目标

在 GitHub Actions 中持续检查 Rust、TypeScript 和三平台构建基础兼容性。

## 必读文档

- `AGENTS.md`
- `docs/architecture.md`：21、22、23、25、28 节

## 允许修改

```text
.github/workflows/**
package.json
Cargo.toml
scripts/**
```

## 行为要求

1. Rust：fmt、clippy、test；
2. Web：format check、lint、typecheck、test；
3. 检查生成的 IPC 类型无漂移；
4. 建立 Windows、macOS、Linux 构建矩阵；
5. 首期不要求签名和发布；
6. 缓存依赖但不能缓存用户秘密；
7. CI 不调用真实模型 API。

## 验收标准

- [ ] Pull Request 能运行所有基础检查；
- [ ] 任一失败会阻止合并；
- [ ] 三个平台至少执行 `cargo check` 与前端构建；
- [ ] Workflow 中不存在明文密钥。
