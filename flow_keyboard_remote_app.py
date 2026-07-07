from flow_keyboard_bridge.client import run_client
from flow_keyboard_bridge.gui import BridgeWindow
from flow_keyboard_bridge.updates import check_local_self_update


def main() -> None:
    check_local_self_update("remote")
    app = BridgeWindow(
        "Flow Keyboard Bridge - Remote",
        "副电脑模式：自动寻找主电脑。连上后打开目标输入框即可。",
        lambda: run_client(None, 8765, True, 8),
    )
    app.run()


if __name__ == "__main__":
    main()
