from flow_keyboard_bridge.gui import BridgeWindow
from flow_keyboard_bridge.server import run_server
from flow_keyboard_bridge.firewall import ensure_update_firewall_rule
from flow_keyboard_bridge.app_info import APP_VERSION
from flow_keyboard_bridge.updates import check_local_self_update, start_update_server


def run_host() -> None:
    check_local_self_update("host")
    ensure_update_firewall_rule()
    start_update_server()
    run_server("0.0.0.0", 8765)


def main() -> None:
    app = BridgeWindow(
        f"键盘跟随工具 - 主电脑 v{APP_VERSION}",
        "主电脑模式：保持窗口打开。鼠标到副电脑时键盘跟过去，鼠标回来时键盘回本机。",
        run_host,
    )
    app.run()


if __name__ == "__main__":
    main()
