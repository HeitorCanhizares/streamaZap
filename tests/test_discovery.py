import json

from streamazap import config, netutil
from streamazap.discovery import RoomBrowser, build_announcement, parse_announcement


def test_announcement_roundtrip():
    data = build_announcement("id1", "Sala", "PC", 47800, 2, True)
    msg = parse_announcement(data)
    assert msg["name"] == "Sala" and msg["locked"] is True and msg["viewers"] == 2


def test_old_version_rooms_listed_as_incompatible():
    old = json.dumps({"app": config.DISCOVERY_MAGIC, "v": 1, "id": "velha", "name": "Antiga", "host": "PC", "port": 47800})
    browser = RoomBrowser()
    browser.handle_datagram(old.encode(), "26.1.1.1", 50000)
    browser.handle_datagram(build_announcement("nova", "Nova", "PC", 47800, 0, False), "26.1.1.2", 50001)
    rooms = browser.rooms()
    assert [r.name for r in rooms] == ["Nova", "Antiga"]  # compatíveis primeiro
    assert rooms[0].compatible and not rooms[1].compatible
    assert rooms[0].callback_ports == {"26.1.1.2": 50001}


def test_ignores_foreign_packets():
    assert parse_announcement(b"lixo") is None
    assert parse_announcement(json.dumps({"app": "outro", "v": 1, "id": "x", "port": 1}).encode()) is None
    bad_port = json.dumps({"app": config.DISCOVERY_MAGIC, "v": config.PROTOCOL_VERSION, "id": "x", "port": 0})
    assert parse_announcement(bad_port.encode()) is None


def test_browser_merges_interfaces_and_expires(monkeypatch):
    browser = RoomBrowser()
    data = build_announcement("id1", "Sala", "PC", 47800, 0, False)
    browser.handle_datagram(data, "192.168.0.10")
    browser.handle_datagram(data, "26.1.2.3")
    rooms = browser.rooms()
    assert len(rooms) == 1
    assert rooms[0].addresses == ["192.168.0.10", "26.1.2.3"]
    monkeypatch.setattr(config, "ROOM_TIMEOUT", -1)
    assert browser.rooms() == []


def test_broadcast_address():
    assert netutil.broadcast_address("26.10.20.30", "255.0.0.0") == "26.255.255.255"
    assert netutil.broadcast_address("192.168.1.5", "255.255.255.0") == "192.168.1.255"
    assert netutil.broadcast_address("10.0.0.1", "255.255.255.255") is None


def test_broadcast_targets_include_radmin():
    interfaces = [
        netutil.Interface("Ethernet", "192.168.1.5", "192.168.1.255"),
        netutil.Interface("Radmin VPN", "26.10.20.30", "26.255.255.255"),
    ]
    assert interfaces[1].is_radmin
    assert netutil.broadcast_targets(interfaces) == ["192.168.1.255", "26.255.255.255", "255.255.255.255"]
