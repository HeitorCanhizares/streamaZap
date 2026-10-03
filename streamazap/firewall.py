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


# Usamos a API COM do Firewall (HNetCfg.FwPolicy2) e comparamos o caminho EXPANDIDO
# (o Windows pode guardar "%ProgramFiles%\..."), sem diferenciar maiúsculas. A busca
# por caminho exato do Get-NetFirewallApplicationFilter não encontrava as regras.
_FIND_RULES = r"""
$p = [IO.Path]::GetFullPath({program})
$fw = New-Object -ComObject HNetCfg.FwPolicy2
$mine = @($fw.Rules | Where-Object {{ $_.ApplicationName -and ([Environment]::ExpandEnvironmentVariables($_.ApplicationName) -ieq $p) }})
"""

CHECK_SCRIPT = (
    "$ErrorActionPreference = 'SilentlyContinue'\n"
    + _FIND_RULES
    + r"""
$in = @($mine | Where-Object {{ $_.Enabled -and $_.Direction -eq 1 }})
# Profiles: 1 domínio, 2 privada, 4 pública. A rede do Radmin costuma ser pública.
$block = @($in | Where-Object {{ $_.Action -eq 0 }}).Count
$allow = @($in | Where-Object {{ $_.Action -eq 1 -and (($_.Profiles -band 6) -eq 6) }}).Count
$rules = @($mine | ForEach-Object {{ '{{0}}|dir={{1}}|acao={{2}}|perfis={{3}}|ativa={{4}}' -f $_.Name, $_.Direction, $_.Action, $_.Profiles, $_.Enabled }})
$third = @(Get-CimInstance -Namespace root/SecurityCenter2 -ClassName FirewallProduct | ForEach-Object {{ $_.displayName }})
@{{ block = $block; allow = $allow; rules = $rules; thirdParty = $third; program = $p }} | ConvertTo-Json -Compress
"""
)

FIX_SCRIPT = (
    "$ErrorActionPreference = 'Stop'\n"
    + "try {{\n"
    + _FIND_RULES
    + r"""
$removed = 0
foreach ($rule in $mine) {{ $fw.Rules.Remove($rule.Name); $removed++ }}
foreach ($dir in 1, 2) {{
    $r = New-Object -ComObject HNetCfg.FWRule
    $r.Name = 'StreamaZap'
    $r.ApplicationName = $p
    $r.Direction = $dir
    $r.Action = 1
    $r.Profiles = 0x7FFFFFFF
    $r.Enabled = $true
    $fw.Rules.Add($r)
}}
"ok removidas=$removed" | Out-File -Encoding utf8 {result}
}} catch {{ "erro: $_" | Out-File -Encoding utf8 {result} }}
"""
)


def parse_check_output(output: str) -> FirewallStatus:
    data = json.loads(output.strip().splitlines()[-1])
    log.info("firewall: programa=%s regras=%s", data.get("program"), data.get("rules"))
    third = data.get("thirdParty") or []
    if isinstance(third, str):
        third = [third]
    # O próprio Windows Defender às vezes aparece na lista; não é "terceiro".
    third = [name for name in third if name and "defender" not in name.lower() and "windows" not in name.lower()]
    return FirewallStatus(True, blocked=int(data.get("block", 0)) > 0, allowed=int(data.get("allow", 0)) > 0, third_party=third)


def check(program: str | None = None) -> FirewallStatus:
    program = program or program_path()
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


def fix_script(program: str, result_file: str) -> str:
    return FIX_SCRIPT.format(program=_ps_quote(program), result=_ps_quote(result_file))


def fix(program: str | None = None, elevate: bool = True) -> bool:
    """Remove bloqueios e libera o StreamaZap. Com `elevate`, pede permissão de administrador (UAC)."""
    program = program or program_path()
    if program is None:
        return False
    import os
    import tempfile

    result_file = os.path.join(tempfile.gettempdir(), "streamazap-firewall.txt")
    try:
        os.remove(result_file)
    except OSError:
        pass
    script = fix_script(program, result_file)
    if elevate:
        script = (
            "Start-Process powershell -Verb RunAs -Wait -WindowStyle Hidden "
            f"-ArgumentList '-NoProfile','-ExecutionPolicy','Bypass','-EncodedCommand','{_encoded(script)}'"
        )
    try:
        result = _run_powershell(script, timeout=120)
    except Exception as exc:  # noqa: BLE001
        log.warning("correção do firewall falhou: %s", exc)
        return False
    if result.returncode != 0:
        log.info("correção do firewall cancelada ou falhou: %s", result.stderr.strip()[:300])
        return False
    try:
        with open(result_file, encoding="utf-8-sig") as f:
            outcome = f.read().strip()
    except OSError:
        outcome = "sem resultado (permissão negada?)"
    log.info("correção do firewall: %s", outcome)
    return outcome.startswith("ok")
