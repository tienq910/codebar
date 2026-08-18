# CodeBar

Windows 10 系统托盘应用：实时显示各 AI 编程工具（Codex、Claude、Copilot 等）的额度用量、重置倒计时与花费。

## 便携使用（portable）

- **免安装**：解压 zip 到任意目录（含 U 盘）,双击 `CodeBar.exe` 即用
- 所有配置与数据保存在 **exe 同级 `data/` 目录**,不写 `%APPDATA%`、不写注册表
- 密钥经 Windows DPAPI 加密后存 `data/secrets.bin`,拷贝到其他机器后密钥自动失效（需重新接入）,配置文件仍可读

## 系统要求

- Windows 10 及以上
- WebView2 Runtime（Windows 10/21H2 之后的系统一般已内置;若缺失,从 https://developer.microsoft.com/microsoft-edge/webview2/ 下载"Evergreen Standalone"安装）

## 开发

```bash
npm install
npm run dev          # 前端 UI(浏览器,mock 数据)
npm run tauri dev    # 本地桌面窗口(macOS 可用于脚手架冒烟;Windows 特性经 CI 验证)
cargo test --manifest-path src-tauri/Cargo.toml   # 纯逻辑单测
```

Windows 编译与打包走 GitHub Actions（`.github/workflows/codebar.yml`）,产物见 Actions run 的 artifact 或 Releases。

## 排查问题

操作日志位于 **exe 同级 `data/codebar.log`**（设置 → 常规 → 「打开日志目录」可直达）,记录：

- 启动信息（版本/主题/刷新间隔/已接入列表）与第二实例唤起
- 托盘点击/菜单操作、弹窗显隐（含托盘图标坐标）
- 每轮刷新：开始/每个 provider 结果（ok 窗口数/软降级原因/错误）/耗时,自适应刷新决策
- provider 接入（扫描结果/校验成败）、断开、主题与配置变更
- 前端页面挂载、按钮操作、JS 异常与未处理的 Promise rejection

日志超过 512KB 自动轮转为 `codebar.log.old`;**绝不记录密钥/Cookie/token 内容**,可放心分享排查。
遇到问题时把该文件发给开发者即可快速定位。
