from flow_keyboard_bridge.updates import UpdateManifest, parse_manifest


def test_parse_manifest_reads_versions_and_files():
    manifest = parse_manifest(
        b"""
        {
          "version": "0.5.0",
          "files": {
            "host": {"version": "0.5.0", "path": "FlowKeyboardHost.exe"},
            "remote": {"version": "0.5.0", "path": "FlowKeyboardRemote.exe"}
          }
        }
        """
    )

    assert isinstance(manifest, UpdateManifest)
    assert manifest.file_for("remote").version == "0.5.0"
    assert manifest.file_for("remote").path == "FlowKeyboardRemote.exe"


def test_same_version_does_not_need_update():
    manifest = parse_manifest(
        b'{"files":{"remote":{"version":"0.5.0","path":"FlowKeyboardRemote.exe"}}}'
    )

    assert manifest.file_for("remote").needs_update("0.5.0") is False


def test_different_version_needs_update():
    manifest = parse_manifest(
        b'{"files":{"remote":{"version":"0.6.0","path":"FlowKeyboardRemote.exe"}}}'
    )

    assert manifest.file_for("remote").needs_update("0.5.0") is True
