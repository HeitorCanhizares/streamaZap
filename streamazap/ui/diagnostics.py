"""Janela de diagnóstico de rede: mostra o que pode estar impedindo a conexão e gera um relatório."""

from __future__ import annotations

import html
import platform
import threading

from PySide6.QtCore import QObject, Signal
from PySide6.QtWidgets import QApplication, QDialog, QHBoxLayout, QPushButton, QTextBrowser, QVBoxLayout

from streamazap import __version__, config, firewall, netutil


class _Signals(QObject):
    firewall = Signal(object)


class DiagnosticsDialog(QDialog):
    def __init__(self, main_window):
        super().__init__(main_window)
        self.main = main_window
        self.setWindowTitle("Diagnóstico de rede")
        self.resize(720, 620)
        self.status: firewall.FirewallStatus | None = None
        self.vpns: list[str] = list(getattr(main_window, "_vpns", []))  # até a verificação terminar
        self.signals = _Signals()
        self.signals.firewall.connect(self._on_firewall)

        self.view = QTextBrowser()
        self.recheck = QPushButton("Verificar de novo")
        self.recheck.clicked.connect(self.check)
        self.fix = QPushButton("🛡️  Corrigir firewall")
        self.fix.setToolTip(
            "Remove bloqueios do StreamaZap, libera a entrada em todas as redes e desliga\n"
            "“Bloquear todas as conexões de entrada” (pede permissão de administrador)."
        )
        self.fix.clicked.connect(self._fix)
        copy = QPushButton("📋  Copiar relatório")
        copy.clicked.connect(lambda: QApplication.clipboard().setText(self.report_text()))
        close = QPushButton("Fechar")
        close.clicked.connect(self.accept)
        buttons = QHBoxLayout()
        buttons.addWidget(self.recheck)
        buttons.addWidget(self.fix)
        buttons.addStretch()
        buttons.addWidget(copy)
        buttons.addWidget(close)
        layout = QVBoxLayout(self)
        layout.addWidget(self.view)
        layout.addLayout(buttons)
        self.fix.setEnabled(firewall.program_path() is not None)
        self.check()

    def check(self) -> None:
        self.recheck.setEnabled(False)
        self.render("Verificando o firewall…")
        threading.Thread(target=self._probe, daemon=True).start()

    def _probe(self) -> None:
        """Numa thread: o PowerShell do firewall e a varredura de processos (VPNs) são lentos."""
        try:
            vpns = netutil.other_vpns()
        except Exception:  # noqa: BLE001 - só um aviso
            vpns = []
        self.signals.firewall.emit((firewall.check(), vpns))

    def _on_firewall(self, result: tuple[firewall.FirewallStatus, list[str]]) -> None:
        status, self.vpns = result
        self.status = status
        self.recheck.setEnabled(True)
        self.fix.setText("🛡️  Corrigir firewall")
        self.fix.setEnabled(firewall.program_path() is not None)
        self.render()
        if hasattr(self.main, "_on_firewall_status"):
            self.main._on_firewall_status(status)

    def _fix(self) -> None:
        self.fix.setEnabled(False)
        self.fix.setText("Corrigindo… (aceite o aviso do Windows)")

        def work():
            firewall.fix()
            self._probe()

        threading.Thread(target=work, daemon=True).start()

    # -- conteúdo ------------------------------------------------------------------
    def sections(self) -> list[tuple[str, list[str]]]:
        browser = self.main.browser
        interfaces = netutil.ipv4_interfaces(max_age=0)
        net = [f"{i.name}: {i.ip}{' (Radmin)' if i.is_radmin else ''}" for i in interfaces] or ["nenhuma interface IPv4"]
        if not any(i.is_radmin for i in interfaces):
            net.append("⚠️ Radmin VPN não detectado")
        if browser.error:
            net.append(f"⚠️ {browser.error}")
        for vpn in self.vpns:
            net.append(
                f"⚠️ {vpn} detectado: se o kill switch (“bloquear conexões fora da VPN”) estiver ligado, ele bloqueia "
                "o Radmin mesmo com o Firewall liberado. Teste com ele fechado; para usar junto, desligue o kill switch "
                "e exclua o Radmin VPN e o StreamaZap no Split Tunneling."
            )

        fw: list[str] = []
        s = self.status
        if s is None:
            fw.append("verificando…")
        elif not s.supported:
            fw.append("verificação disponível só no Windows (app instalado ou portátil)")
        else:
            fw.append("✅ sem problemas encontrados" if s.ok else f"⚠️ {s.describe()}")
            if s.radmin_category:
                category = {"Public": "Pública", "Private": "Privada"}.get(s.radmin_category, s.radmin_category)
                fw.append(f"Rede do Radmin classificada como: {category}")
            fw.append(f"Programa: {s.details.get('program', '?')}")
            rules = s.details.get("rules") or []
            fw.append("Regras do StreamaZap: " + ("; ".join(rules) if rules else "nenhuma"))
            profiles = s.details.get("profiles") or []
            if profiles:
                fw.append("Perfis do firewall: " + "; ".join(profiles))
            if s.third_party:
                fw.append(f"Antivírus com firewall próprio: {', '.join(s.third_party)} — libere o StreamaZap nele")

        peers = []
        for peer in browser.peers():
            state = "✅ recebe você" if peer.hears_me else "⚠️ NÃO recebe nada do seu PC (bloqueio no PC dele)"
            peers.append(f"{peer.name} ({peer.ip}, v{peer.app_version or '?'}): {state}")
        if not peers:
            peers.append(
                "ninguém — se seus amigos estão com o StreamaZap aberto, nada da rede está chegando no seu PC "
                "(bloqueio de entrada no SEU PC ou problema no Radmin)"
            )

        rooms = [
            f"{r.name} — {r.host_name} em {', '.join(r.addresses)}:{r.port}"
            + ("" if r.compatible else f" (⚠️ versão {r.app_version or 'antiga'})")
            for r in browser.rooms()
        ] or ["nenhuma"]

        return [
            ("StreamaZap", [f"versão {__version__} (protocolo {config.PROTOCOL_VERSION}) · {platform.platform()}"]),
            ("Rede", net),
            ("Firewall", fw),
            ("Online com StreamaZap", peers),
            ("Salas encontradas", rooms),
            (
                "Como ler",
                [
                    "Se você vê a sala de alguém mas dá “tempo esgotado”, e ao lado do nome dele aparece "
                    "“NÃO recebe nada do seu PC”, o bloqueio está no PC DELE.",
                    "Teste rápido no PC bloqueado: desligar o Firewall do Windows por 1 minuto. Se funcionar, é o "
                    "firewall (use “Corrigir firewall”); se não, é antivírus ou o Radmin (veja se a conexão no Radmin "
                    "está “direta” e use o Ping do próprio Radmin).",
                ],
            ),
        ]

    def render(self, note: str = "") -> None:
        parts = [f"<p><i>{html.escape(note)}</i></p>"] if note else []
        for title, lines in self.sections():
            parts.append(f"<h3>{html.escape(title)}</h3><ul>")
            parts.extend(f"<li>{html.escape(line)}</li>" for line in lines)
            parts.append("</ul>")
        self.view.setHtml("".join(parts))

    def report_text(self) -> str:
        out = []
        for title, lines in self.sections():
            out.append(f"== {title} ==")
            out.extend(f"- {line}" for line in lines)
        return "\n".join(out)
