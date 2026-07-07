# Flow Keyboard Bridge 开发故事线

## 一句话

因为 Logi Flow 只让罗技生态键盘跟随鼠标跨电脑，而我们不想买新键盘，所以做了一个旁路键盘桥：Logi Flow 继续管鼠标，这个工具只负责把主电脑键盘送到副电脑。

## 起点

用户已经配置好了 Logi Flow，鼠标可以在两台电脑之间切换，但普通键盘不能跟随。目标不是破解 Logi Flow，也不是重写一个完整 KVM，而是做一个最小可用工具：

- 主电脑使用实体键盘。
- 副电脑接收键盘输入。
- 不干扰 Logi Flow 的鼠标切换。
- 尽量做到双击就能用。

## 第一阶段：手动版

第一版是命令行工具：

- Host 跑在主电脑，监听键盘。
- Remote 跑在副电脑，接收按键。
- `Ctrl+Alt+2` 把键盘转发到副电脑。
- `Ctrl+Alt+1` 切回主电脑。

这个阶段证明了核心链路可行：键盘事件可以跨局域网发送，副电脑也能模拟输入。

## 第二阶段：连接问题

最早的连接问题来自几个现实细节：

- 用户填错了 IP，副电脑连到了 `192.168.1.23`，而主电脑真实地址是 `192.168.31.18`。
- Windows 防火墙没有放行 TCP `8765` 和 UDP `8766`。
- 自动发现一开始只靠 UDP 广播，容易被防火墙或网络环境挡住。
- 副电脑上存在 Mihomo/TUN 虚拟网卡，程序误把 `198.18.0.0/24` 当作局域网扫描。

后续改进：

- Remote 不填 IP 时自动搜索。
- 广播失败后自动扫描本地网段。
- 忽略 `198.18.0.0/15` 这类代理虚拟网段。
- 防火墙放行 TCP `8765` 和 UDP `8766`。

## 第三阶段：从命令行到无脑版

命令行测试能用，但体验不好。于是增加 GUI 双击版：

- `FlowKeyboardHost.exe`：主电脑运行。
- `FlowKeyboardRemote.exe`：副电脑运行。

窗口里显示连接状态，不再要求用户敲命令或填 IP。

## 第四阶段：按键注入问题

副电脑曾经连续报：

```text
WinError 87 参数错误
```

根因是 Windows `SendInput` 的 `INPUT` 结构体大小写错了。64 位 Windows 需要 40 字节，但最初实现只有 32 字节。

修复后：

- 补齐 `MOUSEINPUT` / `KEYBDINPUT` / `HARDWAREINPUT` union。
- 加测试确认 64 位 `INPUT` 大小为 40 字节。
- 改用更稳的 Unicode / ScanCode 注入路径。

## 第五阶段：隔离主电脑输入

最初版本只是监听键盘，所以 remote 模式下主电脑窗口也可能收到输入。后来改成 Windows 低级键盘钩子：

- local 模式：不拦截，本机正常输入。
- remote 模式：吞掉主电脑本机输入，只发送给副电脑。
- `Ctrl+Alt+1` 作为回本机逃生键。
- `Ctrl+Alt+2` 切到副电脑。
- `Ctrl+Alt+Esc` 退出。

## 第六阶段：尝试自动跟随鼠标

用户希望键盘能跟随 Logi Flow 鼠标位置。

当前策略：

- Remote 检测副电脑鼠标移动，通知 Host 切 remote。
- Host 轮询本机鼠标位置，一旦主电脑鼠标移动，切 local。

这不是接入 Logi Flow 协议，而是通过两边鼠标活动推断“用户现在在哪台电脑上”。

## 当前状态

当前本地提交：

```text
dece6c1 Detect host mouse return by polling cursor
2e09018 Add remote input isolation
8b87121 Add flow keyboard bridge
```

当前可用版本：

- 主电脑：`dist/FlowKeyboardHost.exe`
- 副电脑：`dist/FlowKeyboardRemote.exe`

验证记录：

- 单元测试：`21 passed`
- 已打包双击版 exe
- 已实现连接、自动发现、远端输入、本机输入隔离、鼠标活动切换

