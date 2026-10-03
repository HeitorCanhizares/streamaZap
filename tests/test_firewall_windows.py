"""Testes reais no Firewall do Windows (rodam no CI do Windows, que é administrador)."""

import ctypes
import subprocess
import sys

import pytest

from streamazap import firewall

pytestmark = pytest.mark.skipif(
    sys.platform != "win32" or not ctypes.windll.shell32.IsUserAnAdmin(),
    reason="precisa de Windows como administrador",
)

PROGRAM = r"C:\StreamaZapTeste\StreamaZap.exe"


def netsh(*args):
    subprocess.run(["netsh", "advfirewall", "firewall", *args], capture_output=True, check=False)


@pytest.fixture(autouse=True)
def cleanup():
    netsh("delete", "rule", "name=all", f"program={PROGRAM}")
    yield
    netsh("delete", "rule", "name=all", f"program={PROGRAM}")


def test_detects_rule_created_like_the_installer():
    netsh("add", "rule", "name=StreamaZap", "dir=in", "action=allow", f"program={PROGRAM}", "enable=yes", "profile=any")
    status = firewall.check(PROGRAM)
    assert status.allowed and not status.blocked and status.ok


def test_detects_block_and_fix_repairs_it():
    netsh("add", "rule", "name=streamazap.exe", "dir=in", "action=block", f"program={PROGRAM}", "enable=yes", "profile=public")
    netsh("add", "rule", "name=StreamaZap", "dir=in", "action=allow", f"program={PROGRAM}", "enable=yes", "profile=any")
    status = firewall.check(PROGRAM)
    assert status.blocked and not status.ok

    assert firewall.fix(PROGRAM, elevate=False)
    status = firewall.check(PROGRAM)
    assert status.allowed and not status.blocked and status.ok


def test_missing_rule_then_fix():
    assert not firewall.check(PROGRAM).allowed
    assert firewall.fix(PROGRAM, elevate=False)
    assert firewall.check(PROGRAM).ok


def _allow_inbound_rules(profile: str) -> str:
    out = subprocess.run(
        ["powershell", "-NoProfile", "-Command", f"(Get-NetFirewallProfile -Name {profile}).AllowInboundRules"],
        capture_output=True, text=True,
    )
    return out.stdout.strip()


def test_detects_block_all_inbound_and_fix_turns_it_off():
    original = _allow_inbound_rules("Public")
    try:
        subprocess.run(["powershell", "-NoProfile", "-Command", "Set-NetFirewallProfile -Name Public -Enabled True -AllowInboundRules False"], check=True)
        netsh("add", "rule", "name=StreamaZap", "dir=in", "action=allow", f"program={PROGRAM}", "enable=yes", "profile=any")
        status = firewall.check(PROGRAM)
        assert status.allowed and "Public" in status.shielded_profiles and status.needs_fix
        assert "Bloquear todas as conexões de entrada" in status.describe()

        assert firewall.fix(PROGRAM, elevate=False)
        status = firewall.check(PROGRAM)
        assert not status.shielded_profiles and status.ok
    finally:
        value = original if original in ("True", "False", "NotConfigured") else "NotConfigured"
        subprocess.run(["powershell", "-NoProfile", "-Command", f"Set-NetFirewallProfile -Name Public -AllowInboundRules {value}"])
