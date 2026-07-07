from flow_keyboard_bridge.h5_remote import LogBuffer, render_remote_page


def test_log_buffer_returns_only_new_entries():
    logs = LogBuffer()
    logs.write("one\n")
    logs.write("two\n")

    first = logs.entries_after(0)
    second = logs.entries_after(first[-1]["id"])

    assert [entry["text"] for entry in first] == ["one\n", "two\n"]
    assert second == []


def test_remote_page_includes_version():
    html = render_remote_page("0.8.0")

    assert "0.8.0" in html
    assert "键盘跟随工具" in html
