"""Atualização automática a partir das Releases do GitHub.

Ao abrir, o app consulta a última Release. Se houver versão mais nova e o app
estiver instalado pelo instalador, baixa o `StreamaZap-Setup-X.Y.Z.exe`, confere
o SHA-256 publicado pelo GitHub e roda o instalador em modo silencioso, que
substitui os arquivos e abre o app de novo já atualizado.
"""

from __future__ import annotations

import hashlib
import json
import logging
import os
import re
import sys
import tempfile
import threading
import urllib.request
from collections.abc import Callable
from dataclasses import dataclass
from pathlib import Path

from streamazap import GITHUB_REPO, __version__

log = logging.getLogger(__name__)

RELEASES_URL = f"https://github.com/{GITHUB_REPO}/releases/latest"
API_URL = f"https://api.github.com/repos/{GITHUB_REPO}/releases/latest"
INSTALLER_PATTERN = re.compile(r"^StreamaZap-Setup-.*\.exe$", re.IGNORECASE)
INSTALLER_ARGS = ["/VERYSILENT", "/SUPPRESSMSGBOXES", "/NORESTART", "/CLOSEAPPLICATIONS"]


class UpdateError(Exception):
    pass


class UpdateCancelled(UpdateError):
    pass


@dataclass(frozen=True)
class Release:
    version: str
    page_url: str
    installer_url: str | None = None
    installer_name: str | None = None
    installer_size: int = 0
    installer_sha256: str | None = None


def parse_version(text: str) -> tuple[int, ...]:
    return tuple(int(n) for n in re.findall(r"\d+", text)[:3])


def is_newer(remote: str, local: str = __version__) -> bool:
    return parse_version(remote) > parse_version(local)


def parse_release(data: dict) -> Release | None:
    tag = data.get("tag_name") or ""
    if not tag or data.get("draft") or data.get("prerelease"):
        return None
    installer = next((a for a in data.get("assets", []) if INSTALLER_PATTERN.match(a.get("name", ""))), None)
    sha256 = None
    if installer and str(installer.get("digest", "")).startswith("sha256:"):
        sha256 = installer["digest"].split(":", 1)[1].lower()
    return Release(
        version=tag.lstrip("v"),
        page_url=data.get("html_url") or RELEASES_URL,
        installer_url=installer.get("browser_download_url") if installer else None,
        installer_name=installer.get("name") if installer else None,
        installer_size=int(installer.get("size", 0)) if installer else 0,
        installer_sha256=sha256,
    )


def check_latest(timeout: float = 5.0) -> Release | None:
    """Retorna a Release se ela for mais nova que a versão em execução."""
    try:
        request = urllib.request.Request(API_URL, headers={"Accept": "application/vnd.github+json", "User-Agent": "StreamaZap"})
        with urllib.request.urlopen(request, timeout=timeout) as response:
            data = json.load(response)
    except Exception as exc:  # noqa: BLE001 - sem internet não é erro para o usuário
        log.info("verificação de atualização falhou: %s", exc)
        return None
    release = parse_release(data)
    if release and is_newer(release.version):
        return release
    return None


def install_dir() -> Path | None:
    """Pasta de instalação se o app foi instalado pelo instalador (não portátil)."""
    if sys.platform != "win32" or not getattr(sys, "frozen", False):
        return None
    folder = Path(sys.executable).parent
    return folder if any(folder.glob("unins*.exe")) else None


def can_auto_update(release: Release) -> bool:
    return install_dir() is not None and release.installer_url is not None


def download(
    release: Release,
    progress: Callable[[int, int], None] = lambda done, total: None,
    cancel: threading.Event | None = None,
    dest_dir: Path | None = None,
) -> Path:
    """Baixa o instalador e confere tamanho e SHA-256. Retorna o caminho do arquivo."""
    if not release.installer_url:
        raise UpdateError("A release não tem instalador")
    dest_dir = dest_dir or Path(tempfile.gettempdir())
    target = dest_dir / (release.installer_name or f"StreamaZap-Setup-{release.version}.exe")
    partial = target.with_name(target.name + ".part")
    digest = hashlib.sha256()
    done = 0
    request = urllib.request.Request(release.installer_url, headers={"User-Agent": "StreamaZap"})
    try:
        with urllib.request.urlopen(request, timeout=30) as response, open(partial, "wb") as out:
            total = int(response.headers.get("Content-Length") or release.installer_size or 0)
            while True:
                if cancel is not None and cancel.is_set():
                    raise UpdateCancelled("Atualização cancelada")
                chunk = response.read(256 * 1024)
                if not chunk:
                    break
                out.write(chunk)
                digest.update(chunk)
                done += len(chunk)
                progress(done, total)
        if release.installer_size and done != release.installer_size:
            raise UpdateError(f"Download incompleto ({done} de {release.installer_size} bytes)")
        if release.installer_sha256 and digest.hexdigest() != release.installer_sha256:
            raise UpdateError("O instalador baixado está corrompido (SHA-256 não confere)")
        os.replace(partial, target)
    except BaseException:
        partial.unlink(missing_ok=True)
        raise
    return target


def launch_installer(path: Path) -> None:
    """Roda o instalador silencioso. Ele pede permissão de administrador (UAC),
    fecha o app, instala e abre a nova versão."""
    import ctypes

    params = " ".join(INSTALLER_ARGS)
    result = ctypes.windll.shell32.ShellExecuteW(None, "open", str(path), params, None, 1)
    if result <= 32:
        raise UpdateError(f"Não foi possível iniciar o instalador (código {result})")
