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

## Frentes desta execução (estado do código depois do M3)

O app Tauri já existe (`app/src-tauri`, comandos em `commands/`, `diagnostics_run` devolve erro
"chega no M5"), `zap-sys` está vazio e o front-end já tem os botões (Configurações: "Procurar
atualização", "Atualizar sozinho ao abrir"; Diagnóstico: cards de servidor/rede/firewall/amigos/
salas, "Corrigir firewall", "Copiar relatório").

| Id | Trava (`crate`) | Depende de | Arquivos principais |
|---|---|---|---|
| `sys` | `zap-sys` | — | `crates/zap-sys/src/{firewall,elevate,net,vpn}.rs` (+ stub não-Windows) |
| `app-update` | `streamazap` | `sys` | `app/src-tauri/` (Cargo.toml, tauri.conf.json, capabilities, `commands/update.rs`, `commands/diag.rs`, `setup.rs` p/ `--corrigir-firewall`) |
| `core-diag` | `zap-core` | `sys` | `crates/zap-core/src/diag.rs` (monta o relatório: rede, VPN, firewall, amigos/presença, salas, versão) |
| `ui-m5` | `app-ui-*` por área | `app-update`, `core-diag` | `app/src/bridge` (tipos/comandos de update e diag), `screens/config` (atualização), `screens/diag` (dados reais), aviso de atualização pronta |
| `pacote` | `pacote` | `app-update` | `.github/workflows/release-v3.yml`, `app/src-tauri/windows/hooks.nsh` (migração do Inno), `README.md` (seção v3) |

Regras do updater no código:
- `tauri-plugin-updater` 2 e `tauri-plugin-process` 2. Em `tauri.conf.json`:
  `bundle.createUpdaterArtifacts: true`, `bundle.windows.nsis.installMode: "currentUser"`,
  `plugins.updater.endpoints` = estável e beta (seção "CI de release") e
  `plugins.updater.pubkey` = placeholder `"COLE_AQUI_A_CHAVE_PUBLICA"` (o dono gera com
  `npx tauri signer generate` e cola; documentar no README). Com o placeholder, `update_check`
  devolve erro claro "atualização não configurada nesta build" em vez de quebrar.
- Comandos: `update_check() -> {disponivel, versao, notas, tamanho}`, `update_download()` (emite
  `zap://update` com progresso 0–100), `update_install_now()` (recusa com erro se o núcleo está numa
  sala), `update_install_on_exit()` (marca; o `RunEvent::Exit` instala). Estado da atualização vai
  no evento `zap://update` `{fase: "nada"|"baixando"|"pronta"|"instalando"|"erro", progresso, versao, erro}`.
- Ao abrir, se `settings.update.auto`: checa em segundo plano 10 s depois; se houver, baixa; quando
  pronta, instala **ao fechar** (nunca reinicia sozinho), e a interface mostra "Atualização X
  pronta — instala ao fechar" com botão "Reiniciar agora" (desabilitado numa sala).
- Canal: `settings.update.channel` (`estavel`/`beta`) escolhe o endpoint.
- Protocolo: se a descoberta vir `other_versions` com protocolo maior, o snapshot avisa
  (`errors` ou campo próprio) e a interface oferece atualizar.

Firewall (`zap-sys`, Windows, COM `INetFwPolicy2`, sem PowerShell):
- `status(exe: &Path) -> FirewallStatus { suportado, bloqueado, liberado, bloquear_tudo_por_perfil: [dominio, privado, publico], categoria_radmin: Option<String>, regras: Vec<RegraResumo> }`.
- `corrigir(exe) -> Resultado` (precisa de admin): remove regras de **bloqueio** do exe, cria
  "streamaZap (UDP)" e "streamaZap (TCP)" de entrada para o exe em todos os perfis, e desliga
  "Bloquear todas as conexões de entrada" só se estiver ligado no perfil da rede do Radmin.
- `elevate::relaunch_elevated(args)` (`ShellExecuteExW` com `runas`, espera terminar, lê o
  resultado de um arquivo JSON temporário).
- `net`: interfaces IPv4 (nome amigável, IP, se é Radmin `26.0.0.0/8`), `vpn`: adaptadores de VPNs
  conhecidas (NordVPN, ProtonVPN, Mullvad, WireGuard, OpenVPN/TAP, Surfshark, Windscribe,
  ExpressVPN, Hamachi, ZeroTier, Tailscale) e antivírus com firewall próprio (WMI
  `root/SecurityCenter2`, `FirewallProduct`), como o app Python faz.
- Testes: lógica de interpretação (pura) em Linux; no Windows, `status` do próprio executável de
  teste roda sem erro.

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
