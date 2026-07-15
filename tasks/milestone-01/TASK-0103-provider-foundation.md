# TASK-0103：实现 API Profile、SecretStore 与 Responses Provider

- Status: Ready
- Owner: Unassigned
- Branch: feat/m1-model-provider
- Dependencies: TASK-0002, TASK-0003

## 目标

实现 API 档案非敏感元数据、安全密钥引用和 OpenAI Responses API 的非流式适配器。

## 必读文档

- `docs/prd.md`：4.3、5.6、6.1、6.3 节
- `docs/architecture.md`：11.6～11.9、13、16、17 节
- `docs/contracts/error-codes.md`

## 允许修改

```text
crates/model-providers/**
crates/secret-store/**
crates/api-profiles/**
```

## 行为要求

1. API Key 只通过 SecretRef 访问；
2. 支持模型列表和非流式 Responses 请求；
3. 支持超时和 CancellationToken；
4. 将供应商错误分类为稳定 ProviderError；
5. 403 必须细分，不统一当作认证失败；
6. 不保存完整请求源码；
7. 使用 Mock Server 测试正常、429、401、403、5xx、非法 JSON 和中断。

## 验收标准

- [ ] 日志和错误中无明文 API Key；
- [ ] Retry-After 可解析；
- [ ] Token 字段缺失时仍可正常返回；
- [ ] 取消请求产生明确状态；
- [ ] 不调用真实收费 API。
