# Devices Router

一个让一套普通键盘控制两台 Windows 电脑的小工具。

Devices Router 只转发键盘，不转发鼠标。`v0.1.31` 恢复成熟的鼠标活动跟随：鼠标在哪台电脑活动，键盘就自动跟到哪台电脑。

语言 / Language: **简体中文** | [English](README.en.md)

## 当前状态

- 平台：Windows -> Windows
- 当前主线版本：Tauri/Rust 桌面版，源码在 `apps/desktop-tauri/`
- 当前版本：`v0.1.31`
- 普通用户：下载并安装 `.exe` 安装包即可，不需要安装 Node.js、Rust、Python 或其它开发依赖。
- 连接端口：
  - TCP `8765`：键盘、控制消息、心跳
  - UDP `8766`：主电脑自动发现
  - TCP `8767`：局域网自动更新

## 功能

- 主电脑低级键盘 hook 捕获按键
- 副电脑使用 Windows `SendInput` 注入按键
- 主电脑和副电脑自动发现、自动重连
- 鼠标活动自动切换键盘目标
- 跨电脑鼠标移动、点击和滚轮转发保持停用
- 本机紧急释放快捷键和托盘安全按钮
- 主电脑/副电脑双向心跳检测连接状态
- 副电脑从主电脑局域网自动更新
- 日志复制、导出、清空
- 记住上次模式，支持开机自启动选项

## 快速使用

1. 从 Release 下载 `Devices Router_版本号_x64-setup.exe`。
2. 在主电脑和副电脑各安装一次，同一个安装包即可。
3. 主电脑打开 `Devices Router`，点击 `主电脑模式`。
4. 副电脑打开 `Devices Router`，点击 `副电脑模式`。
5. 在副电脑打开记事本、聊天框、IDE 等目标输入框。
6. 在副电脑移动鼠标，确认键盘目标自动变为副电脑。
7. 在主电脑输入文字，确认按键出现在副电脑；回主电脑移动鼠标后，确认键盘自动恢复到主电脑。
8. 任何时候都可以在主电脑按 `Ctrl+Alt+Esc`，或点击托盘里的 `立即释放控制`，强制恢复本机键盘。

开箱即用版本不需要命令行，也不需要安装开发环境。README 后面的源码运行和打包命令只给开发者使用。

也可以手动切换：

- 主电脑快捷键 `Ctrl+Alt+1`：键盘回主电脑
- 主电脑快捷键 `Ctrl+Alt+2`：键盘到副电脑
- 主电脑安全键 `Ctrl+Alt+Esc`：立即释放键盘，不依赖副电脑或网络
- 软件界面按钮：`键盘到主电脑` / `键盘到副电脑`
- 托盘按钮：`立即释放控制（Ctrl+Alt+Esc）`

## 自动更新

主电脑启动后会提供局域网更新服务。副电脑连接主电脑后会检查：

- 如果版本一致，显示已是最新版本。
- 如果主电脑版本更新，副电脑会下载、校验并安装更新包。
- 更新包清单位于主电脑本机：

```text
%LOCALAPPDATA%\Devices Router\updates\manifest.json
```

开发时发布局域网更新包：

```powershell
cd apps\desktop-tauri
powershell.exe -ExecutionPolicy Bypass -File .\scripts\prepare-lan-update.ps1
```

## 从源码运行

这一节只面向开发者。如果你只是使用软件，请下载 Release 里的安装包。

需要 Node.js、Rust、Tauri 所需 Windows 构建依赖。

```powershell
cd apps\desktop-tauri
npm install
npm run tauri -- dev
```

打包：

```powershell
cd apps\desktop-tauri
npm run tauri -- build
```

安装包输出：

```text
apps/desktop-tauri/src-tauri/target/release/bundle/nsis/
apps/desktop-tauri/src-tauri/target/release/bundle/msi/
```

## 测试

```powershell
cargo test --manifest-path apps\desktop-tauri\src-tauri\Cargo.toml
cd apps\desktop-tauri
npm run build
```

## 常见问题

### 副电脑显示未连接

优先检查：

- 两台电脑是否在同一个局域网
- 主电脑是否处于 `主电脑模式`
- Windows 防火墙是否允许 TCP `8765`、TCP `8767`、UDP `8766`
- VPN/TUN/代理虚拟网卡是否干扰了局域网发现

### 连接了但键盘不过去

看日志中是否有这些信息：

- 副电脑：`切换请求已发出`
- 主电脑：`副电脑请求：键盘到副电脑`
- 主电脑：`已转发按键`
- 副电脑：`已输入按键`

如果副电脑有“已发出”但主电脑没日志，说明控制消息没有到主电脑。如果主电脑有“已转发”但副电脑没有“已输入”，说明副电脑输入注入失败或目标输入框没有焦点。

### 为什么不是纯网页 H5

浏览器网页不能把系统级按键注入到其它 Windows 程序里。现在的界面是 Tauri 桌面壳，真正的键盘捕获和输入注入都在本地 Rust 后端里完成。

## 文档

- [中文使用教程](docs/user-guide.zh.md)
- [English User Guide](docs/user-guide.en.md)
- [中文排障手册](docs/troubleshooting.zh.md)
- [English Troubleshooting](docs/troubleshooting.en.md)
- [中文视频脚本提纲](docs/video-outline.zh.md)
- [English Video Outline](docs/video-outline.en.md)
- [开发故事线](docs/development-story.md)
- [技术复盘](docs/technical-retrospective.md)

## 已知限制

- 目前主要面向 Windows 双机使用。
- UAC、管理员权限窗口、部分游戏或安全软件保护窗口可能不接受普通模拟输入。
- 中文输入法组合态、复杂快捷键、媒体键等还需要继续打磨。
- 副电脑崩溃、断网或写入失败时，主电脑会按 fail-open 策略自动解除本地输入拦截。

## 项目定位

这是一个个人实用工具，优先解决“已经有 Logitech Flow 鼠标跨屏，但没有 Logitech 键盘”的场景。它不破解 Flow，不模拟 Logitech 设备，只做一个独立的键盘桥。

更真实的起点是：vibe coding 时想把另一台电脑当开发机用，同时这台主电脑还能打 LOL 或处理自己的窗口。鼠标能靠 Flow 过去，键盘却跟不过去，于是才有了这个小工具。详见 [项目动机](docs/motivation.zh.md)。
