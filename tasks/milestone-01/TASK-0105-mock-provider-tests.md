# TASK-0105：建立 Mock Responses API 与集成测试夹具

- Status: Ready
- Owner: Unassigned
- Branch: feat/m1-mock-provider
- Dependencies: TASK-0001

## 目标

提供可在本地和 CI 使用的 Mock Responses API，禁止开发测试依赖真实收费服务。

## 必读文档

- `docs/architecture.md`：23.5、23.6 节
- `docs/contracts/error-codes.md`

## 允许修改

```text
tests/mock-provider/**
tests/fixtures/**
tests/integration/**
```

## 行为要求

Mock 场景至少包括：

- 正常响应；
- 延迟与超时；
- 429 + Retry-After；
- 401；
- 多种 403；
- 500/502/503；
- 非法 JSON；
- 缺少 Token 字段；
- 请求中途断开；
- 返回 response ID。

## 验收标准

- [ ] 每个场景可以确定性触发；
- [ ] CI 不需要外网和真实 API Key；
- [ ] 测试日志不含请求中的完整源码夹具；
- [ ] 支持并发请求和可配置延迟。
