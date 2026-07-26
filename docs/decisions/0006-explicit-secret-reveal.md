# ADR-0006：允许用户显式临时回显 API Key

- 状态：Accepted
- 日期：2026-07-20
- 负责人：Codex
- 相关任务：API Profile 密钥可见性

## 背景

API Key 当前保存在操作系统 SecretStore 中，SQLite 只保存不透明 `SecretRef`。用户需要在
编辑 API Profile 时核对已配置密钥，同时本地数据库需要支持可迁移的密钥密文存储，但普通
Profile DTO、配置和日志都不能长期携带明文。

## 决策

新增 `api_profile_secret_get` 专用 IPC。只有用户点击“显示 API Key”时，Rust 才按当前
Profile 的 `SecretRef` 从 SecretStore 读取密钥并返回一次性响应。普通 Profile 列表继续只
返回 `hasSecret`。新的 SQLite SecretStore 将 API Key 用 AEAD 加密后写入 `encrypted_secrets`，
包装密钥引用写入元数据表，包装密钥本身仍由 OS SecretStore 托管；旧 Keyring 引用继续兼容。

前端只在当前 API Key 输入框中保存该临时值：再次隐藏、切换 Profile 或保存完成后清除未
编辑的回显值。该响应不得进入日志、错误、遥测、普通状态缓存、SQLite、项目 JSON 或导出。

API Key 不写入 SQLite 明文字段。SQLite 只保存认证密文、随机 nonce 和引用；Windows
Credential Manager、macOS Keychain 或 Linux Secret Service 托管包装密钥。

## 备选方案

### SQLite 明文存储

实现简单，但数据库备份、调试工具和文件复制都会直接暴露密钥，违反现有安全边界，因此不采用。

### 永不回显，只允许覆盖

安全面更小，但用户无法核对当前配置，不满足本次产品需求，因此不采用。

## 后果

### 正面影响

- 用户可以核对和继续编辑已保存 API Key；
- 密钥可在 SQLite 中以不可直接使用的密文持久化，包装密钥仍由操作系统安全存储保护；
- 普通列表和项目配置继续保持无密钥。

### 负面影响与成本

- 回显期间密钥存在于前端内存和输入控件中；
- 需要专用 IPC、显式用户操作和生命周期清理测试；
- 无法阻止用户主动截图或手工复制已经选择显示的密钥。

## 兼容与迁移

新增 `0002_encrypted_secrets.sql`，现有 `SecretRef` 继续有效；旧 Keyring 引用按原路径
读取，未配置密钥的 Profile 返回 `security_secret_not_found`。

## 验证方式

- 测试只有显式调用才返回密钥；
- 测试 Profile 列表和 SQLite 行不包含明文；
- 测试隐藏、切换 Profile 和保存后清除临时回显；
- 检查日志、错误和测试快照不包含完整测试密钥。
