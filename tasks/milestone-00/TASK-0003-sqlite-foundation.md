# TASK-0003：建立 SQLite、Migration 与 Repository 基础设施

- Status: Ready
- Owner: Unassigned
- Branch: feat/m0-sqlite-foundation
- Dependencies: TASK-0001, TASK-0002

## 目标

实现数据库初始化、首个 Schema Migration、事务辅助层和测试数据库工具。

## 必读文档

- `docs/architecture.md`：7、8、20 节
- `docs/contracts/database-schema.md`
- `docs/decisions/0002-sqlite-source-of-truth.md`

## 允许修改

```text
crates/persistence/**
crates/persistence/migrations/**
```

## 行为要求

1. 配置 WAL、foreign_keys、busy_timeout；
2. 创建架构定义的核心表和索引；
3. 提供 Migration 版本检查；
4. 提供临时测试数据库；
5. 提供事务接口，但不实现完整业务 Repository；
6. Migration 失败返回稳定错误并支持只读恢复入口设计。

## 验收标准

- [ ] 空数据库可迁移到最新版本；
- [ ] 重复启动不会重复执行 Migration；
- [ ] 外键和唯一约束测试通过；
- [ ] 更高 Schema 版本拒绝写入；
- [ ] 测试不依赖用户本地数据库。
