"""Verifica no GitHub se existe uma versão mais nova do instalador."""

from __future__ import annotations

import json
import logging
import re
import urllib.request

from streamazap import GITHUB_REPO, __version__

log = logging.getLogger(__name__)

RELEASES_URL = f"https://github.com/{GITHUB_REPO}/releases/latest"
API_URL = f"https://api.github.com/repos/{GITHUB_REPO}/releases/latest"


def parse_version(text: str) -> tuple[int, ...]:
    return tuple(int(n) for n in re.findall(r"\d+", text)[:3])


def is_newer(remote: str, local: str = __version__) -> bool:
    return parse_version(remote) > parse_version(local)


def check_latest(timeout: float = 5.0) -> tuple[str, str] | None:
    """Retorna (versão, url) se houver versão nova; None caso contrário ou em erro."""
    try:
        request = urllib.request.Request(API_URL, headers={"Accept": "application/vnd.github+json", "User-Agent": "StreamaZap"})
        with urllib.request.urlopen(request, timeout=timeout) as response:
            data = json.load(response)
    except Exception as exc:  # noqa: BLE001 - sem internet não é erro para o usuário
        log.info("verificação de atualização falhou: %s", exc)
        return None
    tag = data.get("tag_name", "")
    if tag and is_newer(tag):
        return tag, data.get("html_url", RELEASES_URL)
    return None
