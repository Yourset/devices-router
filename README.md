# Flow Keyboard Bridge

手动版键盘桥：让 Logi Flow 继续负责鼠标切屏，这个小工具只负责把主电脑键盘转发到副电脑。

## 第一版范围

- Windows -> Windows
- 手动切换键盘目标
- 主电脑运行 server，副电脑运行 client
- 不修改、不破解、不接入 Logi Flow 本体

## 安装

两台电脑都需要 Python。

```powershell
cd D:\development\随意开发\pc-tools\flow-keyboard-bridge
.\install.ps1
```

## 运行

先在主电脑查 IP：

```powershell
ipconfig
```

在主电脑运行：

```powershell
.\run-server.ps1
```

在副电脑运行。默认会自动搜索主电脑：

```powershell
.\dist\flow-keyboard-client.exe
```

如果自动搜索失败，再手动指定 IP：

```powershell
.\run-client.ps1 -ServerHost 192.168.x.x
```

## 打包 exe

```powershell
.\build-exe.ps1
```

生成文件：

- `dist\flow-keyboard-server.exe`
- `dist\flow-keyboard-client.exe`

推荐使用双击版：

- 主电脑运行 `dist\FlowKeyboardHost.exe`
- 副电脑运行 `dist\FlowKeyboardRemote.exe`

老的命令行版仍然保留。主电脑命令行运行：

```powershell
.\dist\flow-keyboard-server.exe
```

副电脑运行，默认自动搜索：

```powershell
.\dist\flow-keyboard-client.exe
```

也可以手动指定：

```powershell
.\dist\flow-keyboard-client.exe --host 192.168.x.x
```

## 快捷键

- `Ctrl+Alt+1`：键盘留在主电脑
- `Ctrl+Alt+2`：键盘发到副电脑
- `Ctrl+Alt+Esc`：退出 server

## 测试方法

1. 两台电脑都启动程序。
2. 用 Logi Flow 把鼠标移到副电脑。
3. 在副电脑打开记事本并让它获得焦点。
4. 在主电脑按 `Ctrl+Alt+2`。
5. 用主电脑键盘输入英文，看副电脑记事本是否出现字符。
6. 按 `Ctrl+Alt+1` 切回本机。

## 已知限制

- 管理员窗口、UAC 弹窗、部分游戏可能不接受模拟输入。
- 中文输入法第一版不保证稳定，先用英文验证闭环。
- 目前不会自动跟随 Logi Flow，自动跟随要等手动版可用后再做。
- 第一版是“转发键盘”，不是强拦截键盘；主电脑当前有输入焦点的窗口可能也会收到按键。测试时建议主电脑不要把焦点放在会误输入的位置。
