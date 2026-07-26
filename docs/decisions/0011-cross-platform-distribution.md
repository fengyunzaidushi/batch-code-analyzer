# ADR 0011：三平台安装包发布

## 状态

已批准

## 背景

桌面端使用 Tauri 2。仅完成前端构建不能让用户直接安装应用，需要在 Windows、macOS 和 Linux 上生成平台原生安装包，并将同一版本的产物集中到 GitHub Release。

## 决策

- Tauri bundle 常开，并生成以下产物：
  - Windows：MSI 和 NSIS 安装程序；
  - macOS：Intel 和 Apple Silicon 两个 DMG；
  - Linux：x86_64 AppImage 和 DEB。
- GitHub Actions 在推送 `v*` 标签时运行三平台矩阵构建，并创建草稿 Release 上传全部安装包。
- 发布工作流不包含签名证书。签名、公证和 Windows 签名证书通过后续仓库 Secrets 接入，不阻塞当前可下载构建。
- Linux DEB 声明 WebKitGTK 和 GTK3 运行时依赖；AppImage 作为便携格式同时发布。

## 后果

用户可以从 GitHub Release 直接下载对应平台安装包。未签名的 macOS 和 Windows 安装包可能显示系统安全警告，发布说明必须明确这一点。每次推送版本标签会自动发布 Release；签名配置完成后再补充证书校验和人工审批。

## 使用方式

```bash
git tag v0.1.0
git push origin v0.1.0
```

本地可使用 `pnpm tauri:build` 生成当前操作系统的安装包。
