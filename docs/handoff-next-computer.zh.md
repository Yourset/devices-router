# 下一台电脑交接提示词

把下面这段发给下一台电脑上的 Codex，用来继续开发 Devices Router。

```text
你接手的项目是 Devices Router，仓库地址：
git@github.com:Yourset/devices-router.git

本地建议路径：
D:\development\随意开发\pc-tools\flow-keyboard-bridge

当前主线版本：v0.1.17

项目背景：
这是一个 Windows 桌面工具，用来补齐 Logitech Flow 只带鼠标、不带普通键盘跟随的问题。核心需求是：主电脑和副电脑都运行 Devices Router，鼠标移动/活动在哪台电脑，键盘就尽量跟随到哪台电脑。用户的真实场景是 vibe coding 时可以用另一台电脑开发，主电脑可以继续处理游戏/LOL/窗口等事情，所以游戏模式必须保守，不要自动抢输入。

当前技术栈：
- 主线桌面版：Tauri 2 + Rust + TypeScript
- 旧 Python 版本还保留在仓库里，但现在不要优先改旧 Python
- 主入口：
  - apps/desktop-tauri/src/main.ts
  - apps/desktop-tauri/src/styles.css
  - apps/desktop-tauri/src-tauri/src/lib.rs
  - apps/desktop-tauri/src-tauri/src/core.rs
  - apps/desktop-tauri/src-tauri/src/config.rs
  - apps/desktop-tauri/src-tauri/src/discovery.rs

v0.1.17 已完成：
- 设置页
- 启动默认模式：沿用上次 / 主电脑 / 副电脑 / 不自动启动
- 运行模式和键盘目标按钮选中态
- 鼠标跟随三档：稳定 / 平衡 / 灵敏
- 游戏模式：关闭自动鼠标切换，避免游戏时误抢输入
- 日志重复折叠
- 网络诊断：本机 LAN IP、键盘端口检测、更新端口检测
- 局域网自动更新包已能生成

上一轮验证命令：
cd apps\desktop-tauri
npm.cmd run build
cd src-tauri
cargo test

打包和局域网更新命令：
cd apps\desktop-tauri
npm.cmd run build:lan-update

当前 0.1.17 安装包位置：
apps/desktop-tauri/src-tauri/target/release/bundle/nsis/Devices Router_0.1.17_x64-setup.exe

当前 0.1.17 局域网更新包位置：
apps/desktop-tauri/src-tauri/target/release/updates/DevicesRouter_0.1.17_x64_setup.exe
apps/desktop-tauri/src-tauri/target/release/updates/manifest.json

优先继续做的事：
1. 真实手测主电脑/副电脑 v0.1.17 自动更新链路。
2. 优化网络诊断，把“端口不可达”解释成用户能懂的建议，比如防火墙、IP 错、主电脑未启动。
3. 完善日志区：复制折叠后日志与导出原始日志之间的关系要明确。
4. 考虑真正系统托盘，而不是仅启动后最小化。
5. 如果继续做鼠标转发，先写设计文档，不要直接大改核心输入链路。游戏模式下默认禁止鼠标转发。

注意事项：
- 不要回滚用户或上一台电脑的未读改动。
- 修改前先 `git status --short`。
- 每次提交前跑：
  - npm.cmd run build
  - cargo test
  - git diff --check
- 提交要小而清楚。
- 用户偏好中文沟通，解释要直接，不要太技术化。
```
