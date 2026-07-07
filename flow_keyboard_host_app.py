from flow_keyboard_bridge.gui import BridgeWindow
from flow_keyboard_bridge.server import run_server


def main() -> None:
    app = BridgeWindow(
        "Flow Keyboard Bridge - Host",
        "主电脑模式：保持这个窗口打开。Ctrl+Alt+2 转发到副电脑，Ctrl+Alt+1 回到本机。",
        lambda: run_server("0.0.0.0", 8765),
    )
    app.run()


if __name__ == "__main__":
    main()

