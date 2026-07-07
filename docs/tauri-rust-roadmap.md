# Tauri/Rust 版路线

Tauri/Rust 版目标是替代原 Python Host/Remote，做成适合网友使用的正式桌面软件。

## 当前产物

源码目录：

```text
apps/desktop-tauri/
```

已能生成：

- `src-tauri/target/release/devices-router.exe`
- `src-tauri/target/release/bundle/msi/Devices Router_0.1.0_x64_en-US.msi`
- `src-tauri/target/release/bundle/nsis/Devices Router_0.1.0_x64-setup.exe`

## 已实现

- Tauri v2 桌面壳
- 现代控制台 UI
- 主电脑 / 副电脑模式按钮
- 状态和日志轮询
- Rust JSON line 协议
- 旧客户端静默 LAN 连接兼容测试
- Host TCP listener
- Remote TCP client
- Remote `SendInput` 键盘注入
- Host `WH_KEYBOARD_LL` 低级键盘 hook
- Host 鼠标移动切回本机
- Remote 鼠标移动上报
- 鼠标跟随默认配置展示

## 仍需实机验证

- 两台 Windows 电脑真实跨机连接
- Host hook 在普通窗口、管理员窗口、游戏窗口下的表现
- Remote `SendInput` 对不同输入框和输入法的表现
- 鼠标跟随防抖和冷却手感
- 防火墙自动放行
- Tauri updater / GitHub Release 更新链路

## 更新兼容原则

协议变化必须保留以下测试：

- 新 Host 不应被本机空连接踢掉真实客户端。
- 新 Host 应允许旧 LAN Remote 静默连接，以便旧客户端还有机会拿到更新。
- 新 Remote 应先发送 `client_hello`。

这避免后续从 Python 版、Tauri 版或其他方案切换时，双端因为握手变化无法自动更新。
