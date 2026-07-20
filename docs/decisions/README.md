# 架构决策记录

本目录保存已经批准、会长期影响实现的技术决策。它相当于 ADR（Architecture Decision Record）目录。

## 状态

- `Proposed`：待评审；
- `Accepted`：已批准并必须遵守；
- `Superseded`：被新决策替代；
- `Deprecated`：不再推荐，但旧实现可能仍存在。

## 修改流程

1. 复制 `ADR-template.md`；
2. 使用递增编号；
3. 说明背景、决策、备选方案、后果和迁移方式；
4. 总控或架构 Owner 审核；
5. 影响公共契约时，同步修改 `docs/contracts/`；
6. 破坏性变更必须说明兼容和数据迁移方案。

当前已建立的基础决策：

- 0001：使用 Tauri 2 + React + Rust；
- 0002：SQLite 是运行状态权威来源；
- 0003：首期单活动 Run；
- 0004：仓库文件系统只由 Rust 访问；
- 0005：Run 快照创建后不可变。
- 0006：用户显式操作时可从 SecretStore 临时回显 API Key，禁止明文持久化。
