# TASK-0131：数据库迁移换行兼容

- Status: Done
- Owner: Codex
- Branch: main
- Dependencies: TASK-0003

## 目标

修复 Windows 工作区换行格式变化导致的 SQLx migration checksum mismatch，避免已有数据库在启动时被错误判定为本地数据库不可用。

## 允许修改

```text
tasks/milestone-01/TASK-0131-database-migration-line-ending-compatibility.md
crates/persistence/src/database.rs
```

## 行为要求

1. 嵌入迁移使用稳定的换行格式计算校验和。
2. 已存在的 LF 迁移校验和可兼容转换到当前格式；真正的 SQL 内容变化仍然失败。
3. 不修改迁移文件、不删除用户数据库、不跳过迁移校验。

## 验收标准

- [x] 现有 CRLF 数据库可启动；
- [x] 旧 LF 数据库可启动并修正迁移元数据；
- [x] 真正的 migration checksum mismatch 仍然被拒绝；
- [x] persistence 单元测试通过。

