import hashlib
import threading

import pytest

from streamazap import updater


def release_json(tmp_path, content=b"instalador", digest=None):
    path = tmp_path / "StreamaZap-Setup-0.2.0.exe"
    path.write_bytes(content)
    digest = digest or "sha256:" + hashlib.sha256(content).hexdigest()
    return {
        "tag_name": "v0.2.0",
        "html_url": "https://example/releases/v0.2.0",
        "assets": [
            {"name": "StreamaZap-0.2.0-portable.zip", "browser_download_url": "x", "size": 1},
            {"name": path.name, "browser_download_url": path.as_uri(), "size": len(content), "digest": digest},
        ],
    }


def test_version_compare():
    assert updater.is_newer("v0.1.10", "0.1.9")
    assert updater.is_newer("0.2.0", "0.1.99")
    assert not updater.is_newer("0.1.0", "0.1.0")
    assert not updater.is_newer("0.1.4", "0.1.4-dev")


def test_parse_release_picks_installer(tmp_path):
    release = updater.parse_release(release_json(tmp_path))
    assert release.version == "0.2.0"
    assert release.installer_name == "StreamaZap-Setup-0.2.0.exe"
    assert release.installer_sha256 == hashlib.sha256(b"instalador").hexdigest()
    assert updater.parse_release({"tag_name": "v1.0.0", "prerelease": True}) is None


def test_download_verifies_hash(tmp_path):
    out = tmp_path / "out"
    out.mkdir()
    release = updater.parse_release(release_json(tmp_path))
    progress = []
    path = updater.download(release, lambda done, total: progress.append(done), dest_dir=out)
    assert path.read_bytes() == b"instalador"
    assert progress[-1] == len(b"instalador")

    bad = updater.parse_release(release_json(tmp_path, digest="sha256:" + "0" * 64))
    with pytest.raises(updater.UpdateError, match="SHA-256"):
        updater.download(bad, dest_dir=out)
    assert not list(out.glob("*.part"))


def test_download_cancel(tmp_path):
    out = tmp_path / "out"
    out.mkdir()
    cancel = threading.Event()
    cancel.set()
    with pytest.raises(updater.UpdateCancelled):
        updater.download(updater.parse_release(release_json(tmp_path)), cancel=cancel, dest_dir=out)


def test_portable_or_dev_build_does_not_auto_update(tmp_path):
    release = updater.parse_release(release_json(tmp_path))
    assert updater.install_dir() is None  # rodando pelo Python, não pelo instalador
    assert not updater.can_auto_update(release)
