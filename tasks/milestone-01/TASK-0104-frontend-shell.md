# TASK-0104：实现 React AppShell、项目侧栏与空状态

- Status: Ready
- Owner: Unassigned
- Branch: feat/m1-frontend-shell
- Dependencies: TASK-0001, TASK-0002

## 目标

实现可扩展的桌面应用壳、项目侧栏、顶部两个固定 Tab 和任务区域骨架，不实现真实扫描或模型业务。

## 必读文档

- `docs/prd.md`：3、4.1、4.2、4.3、4.5 节
- `docs/architecture.md`：9、10 节
- `docs/contracts/ipc-contract.md`

## 允许修改

```text
apps/desktop/src/**
packages/ui/**
```

## 行为要求

1. 固定 Tab 顺序为“提示词”“API 配置”；
2. 左侧项目栏支持空态、搜索框和状态占位；
3. 右侧显示当前项目名称、根目录和状态；
4. 业务数据通过 Query/IPC 适配层获取，不直接使用 Tauri 文件系统；
5. 建立 VirtualTaskTable 占位接口；
6. Markdown 预览组件默认禁用 raw HTML 和远程图片；
7. UI 状态与业务状态分离。

## 验收标准

- [ ] 组件测试覆盖无项目、路径不可用和有活动 Run 的展示；
- [ ] 不在 Zustand 保存完整 Task/Result 业务数据；
- [ ] 不手写重复的 IPC 枚举；
- [ ] 10,000 行虚拟列表 Story/测试入口已建立。
