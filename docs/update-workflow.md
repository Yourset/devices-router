# 自动更新流程

## 使用方式

主电脑运行 `FlowKeyboardHost.exe` 时，会同时启动一个局域网更新服务：

- TCP `8767`
- 文件目录：`updates/`
- 清单文件：`updates/manifest.json`

副电脑运行 `FlowKeyboardRemote.exe` 后，会在连接 Host 时检查 Host 上的更新清单。如果 Host 的 `updates/FlowKeyboardRemote.exe` 版本和本机 Remote 版本不同，Remote 会自动下载、退出、替换自己并重启。

## 发布新版

在开发机运行：

```powershell
.\build-exe.ps1
```

生成：

```text
dist/
  FlowKeyboardHost.exe
  FlowKeyboardRemote.exe
  updates/
    FlowKeyboardHost.exe
    FlowKeyboardRemote.exe
    manifest.json
```

把整个 `dist/` 放到主电脑使用目录。副电脑不用手动替换 Remote，它会在连接主电脑后自动更新。

## Host 自更新

Host 启动时会检查自己旁边的 `updates/manifest.json`。如果里面的 Host 版本比当前运行版本不同，会用 `updates/FlowKeyboardHost.exe` 替换当前 Host 并重启。

这意味着后续可以先把新版放进主电脑 `updates/`，再重启旧 Host，让它自己替换。

## 注意

- 这不是公网更新，只在局域网内工作。
- Host 需要允许 TCP `8767` 入站，否则 Remote 无法下载更新。
- 更新判断目前使用版本号不一致即更新，不做复杂语义版本比较。
