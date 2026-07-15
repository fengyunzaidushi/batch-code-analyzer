# TASK-0102：实现仓库扫描、Git Ignore 与安全过滤

- Status: Ready
- Owner: Unassigned
- Branch: feat/m1-scanner-security
- Dependencies: TASK-0002

## 目标

实现可取消的仓库扫描管线，支持嵌套 `.gitignore`、编码/二进制检测、哈希、敏感文件与路径安全。

## 必读文档

- `docs/prd.md`：4.4、6、9.1、10.1、10.5 节
- `docs/architecture.md`：11.2、12、18、23.6 节
- `docs/contracts/error-codes.md`
- `docs/decisions/0004-rust-filesystem-boundary.md`

## 允许修改

```text
crates/repository-scanner/**
crates/security-core/**
```

## 行为要求

1. 默认不跟随符号链接；
2. 支持根目录和嵌套 `.gitignore`；
3. 应用内置排除、用户排除、大小和包含后缀规则；
4. 检测二进制和编码；
5. 计算 BLAKE3；
6. 检测常见敏感文件和秘密模式，只返回掩码结果；
7. 生成按原因统计的导入报告；
8. 10,000 文件扫描不阻塞 UI 所在线程。

## 验收标准

- [ ] 符号链接不能读取仓库外文件；
- [ ] `!` 否定规则有测试；
- [ ] 不可读、过大、二进制和敏感文件均可追溯；
- [ ] 取消扫描后不留下半完成的正式扫描代次；
- [ ] 测试覆盖 Windows 路径保留名和大小写冲突逻辑。
