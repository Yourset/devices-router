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
    assert manifest.file_for("remote").size is None
    assert manifest.file_for("remote").sha256 is None


def test_parse_manifest_accepts_utf8_bom():
    manifest = parse_manifest(
        b'\xef\xbb\xbf{"files":{"host":{"version":"0.5.0","path":"FlowKeyboardHost.exe"}}}'
    )

    assert manifest.file_for("host").version == "0.5.0"


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


def test_parse_manifest_reads_integrity_fields():
    manifest = parse_manifest(
        b'{"files":{"remote":{"version":"0.6.2","path":"FlowKeyboardRemote.exe","size":123,"sha256":"abc"}}}'
    )

    update_file = manifest.file_for("remote")
    assert update_file.size == 123
    assert update_file.sha256 == "abc"
