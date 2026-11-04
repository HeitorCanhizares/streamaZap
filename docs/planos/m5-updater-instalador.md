# M5: atualização segura, firewall, diagnóstico e instalador

Plano do arquiteto. Base: `docs/arquitetura-v3.md` seção 8 e os módulos Python
`streamazap/updater.py`, `streamazap/firewall.py`, `streamazap/netutil.py`,
`streamazap/ui/diagnostics.py`, `packaging/installer.iss`.

## O que melhora na atualização

| Hoje (Python) | v3 |
|---|---|
| Confere SHA-256 que vem **da mesma Release** (não protege contra Release adulterada) | Assinatura **ed25519** (minisign) com chave pública embutida no app; a privada só no segredo do CI |
| Instalador pede **administrador** a cada atualização | Instalação **por usuário** (NSIS `currentUser`): atualizar não pede nada. Admin só uma vez, em "Corrigir firewall" |
| Reinicia o app na hora, mesmo no meio de uma sala | Baixa em segundo plano; **nunca reinicia numa sala**: instala ao fechar, ou pergunta ao sair da sala |
| Só canal estável | Canais **estável** e **beta** (configuração) |
| Sem aviso de incompatibilidade | Amigo/servidor com protocolo maior → oferece atualizar na hora |
| Baixa ~150 MB (PyInstaller) | ~10 MB (Tauri) |

## Frentes

| Id | Pasta / crate | Resumo |
|---|---|---|
| `updater` | `app/src-tauri` | `tauri-plugin-updater` + `tauri-plugin-process`; comandos `update_check`, `update_download`, `update_install_on_exit`, `update_install_now`; eventos de progresso; regras de "não reiniciar em sala" |
| `firewall` | `crates/zap-sys` | Porte de `firewall.py` usando a API COM `INetFwPolicy2`: regras do executável (entrada UDP/TCP), regras de **bloqueio** criadas pelo "Cancelar", "Bloquear todas as conexões de entrada" por perfil, categoria da rede do Radmin; correção elevada |
| `rede` | `crates/zap-sys` | Interfaces (Radmin `26.0.0.0/8` primeiro), VPNs conhecidas (NordVPN, ProtonVPN, Mullvad, WireGuard, OpenVPN, Surfshark…) pelo nome do adaptador, antivírus com firewall próprio (WMI `SecurityCenter2`) |
| `diagnostico` | `crates/zap-core` + `app/src/screens` | Tela Diagnóstico completa (servidor, rede, firewall, amigos, salas) e "Copiar relatório" |
| `pacote` | `.github/workflows/release-v3.yml`, `app/src-tauri/tauri.conf.json` | NSIS por usuário, ícones, `.zip` portátil, `latest.json`, assinatura, Release |

## Correção do firewall (elevada)
O app relança a si mesmo com `ShellExecuteExW("runas", streamazap.exe, "--corrigir-firewall")`.
Nesse modo, sem janela: remove regras de bloqueio do executável, cria regras de permissão
(UDP 47800–47820 e 47801 de entrada, todos os perfis), e escreve o resultado num arquivo
temporário que o app lê para mostrar na tela. Mesmo comportamento do Python, sem PowerShell.

## Migração do app Python
O updater Python baixa o `StreamaZap-Setup-*.exe` da última Release e roda com parâmetros do Inno
(`/VERYSILENT …`). A primeira Release da v3 (3.0.0 > 0.1.x):
- publica o instalador NSIS com o mesmo padrão de nome (`StreamaZap-Setup-3.0.0.exe`); o NSIS
  ignora os parâmetros do Inno e abre normalmente (o usuário clica em Instalar uma vez);
- o instalador (hook `NSIS_HOOK_POSTINSTALL`) procura a instalação Inno antiga
  (`HKLM\…\Uninstall\{6B1F3E2A-8C4D-4F7B-9A51-2D3E8F0C7A11}_is1`) e roda o desinstalador dela em
  modo silencioso (com elevação, uma vez);
- configurações antigas não são migradas (o formato mudou); o nome do usuário sim, se achado.

## CI de release (`release-v3.yml`)
- Gatilho: tag `v3.*` ou "Run workflow". (O `build.yml` do Python continua até o M6.)
- `tauri-apps/tauri-action` em `windows-latest`: gera NSIS, `.zip` portátil (exe + recursos),
  `.sig` e `latest.json` (`updaterJsonPreferNsis: true`), publica a Release (`prerelease` no
  canal beta).
- Segredos: `TAURI_SIGNING_PRIVATE_KEY` e `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`. A chave pública
  vai em `tauri.conf.json > plugins.updater.pubkey`. Gerar com `npx tauri signer generate`.
- Endpoints: estável = `…/releases/latest/download/latest.json`; beta = arquivo `latest-beta.json`
  anexado a uma Release fixa `canal-beta`.

## Testes de aceite
1. Instalar 3.0.0 (sem admin), publicar 3.0.1 de teste, abrir: baixa, avisa, instala ao fechar,
   reabre na 3.0.1, configurações mantidas.
2. Release com assinatura inválida → recusada com mensagem.
3. Numa sala, a atualização não reinicia nada.
4. "Corrigir firewall" num PC com regra de bloqueio → regra removida, permissão criada, resultado
   na tela.
