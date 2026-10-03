"""Diagnóstico e correção do Firewall do Windows para o StreamaZap.

Problema clássico: a rede do Radmin é classificada como "Pública" e, se alguém
clicou em "Cancelar" no aviso do Firewall, o Windows cria uma regra de BLOQUEIO
para o programa nessa rede. Regra de bloqueio vence a de liberação, então a
sala aparece para os amigos mas a conexão de vídeo dá "tempo esgotado".

Usamos o PowerShell (cmdlets NetSecurity) porque a saída do netsh é traduzida
para o idioma do Windows e é frágil de interpretar.
"""

from __future__ import annotations

import base64
import json
import logging
import subprocess
import sys
from dataclasses import dataclass, field

log = logging.getLogger(__name__)

CREATE_NO_WINDOW = 0x08000000


@dataclass
class FirewallStatus:
    supported: bool  # só no Windows, rodando pelo executável instalado/portátil
    blocked: bool = False  # existe regra de bloqueio para o StreamaZap
    allowed: bool = False  # existe regra liberando (inclusive em rede pública)
    third_party: list[str] = field(default_factory=list)  # firewalls de antivírus
    error: str | None = None

    @property
    def ok(self) -> bool:
        return not self.supported or (self.allowed and not self.blocked) or self.error is not None

    def describe(self) -> str:
        if not self.supported:
            return ""
        if self.error:
            return f"Não foi possível verificar o firewall: {self.error}"
        problems = []
        if self.blocked:
            problems.append("o Firewall do Windows está BLOQUEANDO o StreamaZap")
        elif not self.allowed:
            problems.append("o StreamaZap não está liberado no Firewall do Windows")
        text = "; ".join(problems)
        if self.third_party:
            note = f"Antivírus com firewall próprio: {', '.join(self.third_party)} — libere o StreamaZap nele também"
            text = f"{text}. {note}" if text else note
        return text[:1].upper() + text[1:] if text else ""


def program_path() -> str | None:
    if sys.platform != "win32" or not getattr(sys, "frozen", False):
        return None
    return sys.executable


def _ps_quote(text: str) -> str:
    return "'" + text.replace("'", "''") + "'"


def _encoded(script: str) -> str:
    return base64.b64encode(script.encode("utf-16-le")).decode("ascii")


def _run_powershell(script: str, timeout: float = 30) -> subprocess.CompletedProcess:
    return subprocess.run(
        ["powershell", "-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-EncodedCommand", _encoded(script)],
        capture_output=True,
        text=True,
        timeout=timeout,
        creationflags=CREATE_NO_WINDOW,
    )


CHECK_SCRIPT = r"""
$ErrorActionPreference = 'SilentlyContinue'
$p = {program}
$rules = @(Get-NetFirewallApplicationFilter -Program $p | Get-NetFirewallRule | Where-Object {{ $_.Enabled -eq 'True' -and $_.Direction -eq 'Inbound' }})
$block = @($rules | Where-Object {{ $_.Action -eq 'Block' }}).Count
$allow = @($rules | Where-Object {{ $_.Action -eq 'Allow' -and $_.Profile.ToString() -match 'Any|Public' }}).Count
$fw = @(Get-CimInstance -Namespace root/SecurityCenter2 -ClassName FirewallProduct | ForEach-Object {{ $_.displayName }})
@{{ block = $block; allow = $allow; thirdParty = $fw }} | ConvertTo-Json -Compress
"""

FIX_SCRIPT = r"""
$ErrorActionPreference = 'SilentlyContinue'
$p = {program}
Get-NetFirewallApplicationFilter -Program $p | Get-NetFirewallRule | Remove-NetFirewallRule
New-NetFirewallRule -DisplayName 'StreamaZap' -Direction Inbound -Program $p -Action Allow -Profile Any | Out-Null
New-NetFirewallRule -DisplayName 'StreamaZap (saída)' -Direction Outbound -Program $p -Action Allow -Profile Any | Out-Null
"""


def parse_check_output(output: str) -> FirewallStatus:
    data = json.loads(output.strip().splitlines()[-1])
    third = data.get("thirdParty") or []
    if isinstance(third, str):
        third = [third]
    # O próprio Windows Defender às vezes aparece na lista; não é "terceiro".
    third = [name for name in third if name and "defender" not in name.lower() and "windows" not in name.lower()]
    return FirewallStatus(True, blocked=int(data.get("block", 0)) > 0, allowed=int(data.get("allow", 0)) > 0, third_party=third)


def check() -> FirewallStatus:
    program = program_path()
    if program is None:
        return FirewallStatus(False)
    try:
        result = _run_powershell(CHECK_SCRIPT.format(program=_ps_quote(program)))
        status = parse_check_output(result.stdout)
    except Exception as exc:  # noqa: BLE001 - diagnóstico nunca pode derrubar o app
        log.warning("verificação do firewall falhou: %s", exc)
        return FirewallStatus(True, error=str(exc))
    log.info("firewall: bloqueado=%s liberado=%s terceiros=%s", status.blocked, status.allowed, status.third_party)
    return status


def fix() -> bool:
    """Remove bloqueios e libera o StreamaZap (pede permissão de administrador). Retorna se rodou."""
    program = program_path()
    if program is None:
        return False
    inner = _encoded(FIX_SCRIPT.format(program=_ps_quote(program)))
    launcher = (
        "Start-Process powershell -Verb RunAs -Wait -WindowStyle Hidden "
        f"-ArgumentList '-NoProfile','-ExecutionPolicy','Bypass','-EncodedCommand','{inner}'"
    )
    try:
        result = _run_powershell(launcher, timeout=120)
    except Exception as exc:  # noqa: BLE001
        log.warning("correção do firewall falhou: %s", exc)
        return False
    if result.returncode != 0:
        log.info("correção do firewall cancelada ou falhou: %s", result.stderr.strip()[:300])
        return False
    return True
