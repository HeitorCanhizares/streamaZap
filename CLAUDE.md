# StreamaZap: guia para agentes

App de compartilhamento de tela, janela, câmera e som por app, entre amigos (Radmin/LAN e servidor
online). Estamos migrando do app Python (`streamazap/`) para a **v3 em Rust + Tauri**.

- Plano mestre: `docs/arquitetura-v3.md` (leia antes de qualquer mudança grande).
- Contrato do servidor e do protocolo: `docs/api/`.
- Planos dos marcos e tarefas: `docs/planos/`.
- Design da nova interface: `docs/design/` (HTML de referência + prints).

## Hierarquia de agentes

| Papel | Modelo | Arquivo | Faz |
|---|---|---|---|
| Arquiteto | Opus | `.claude/agents/zap-arquiteto.md` | Plano de marco: frentes, interfaces, números, testes de aceite |
| Decompositor | Sonnet | `.claude/agents/zap-decompositor.md` | Esqueleto compilável + tarefas pequenas com dono de arquivo |
| Implementador | Haiku | `.claude/agents/zap-implementador.md` | Uma tarefa: código + testes + verificação |
| Revisor | Sonnet | `.claude/agents/zap-revisor.md` | Confere cada tarefa, assume as escaladas, fecha o marco |

O ciclo completo roda pelo workflow salvo `.claude/workflows/zap-marco.js` (skill `zap-marco`).
Trabalho pequeno (um bug, um ajuste) não precisa da hierarquia: faça direto.

## Estrutura

```
crates/zap-proto   tipos e formatos do protocolo (sem I/O)
crates/zap-rate    algoritmos de qualidade/latência + simulador de enlace (sem I/O)
crates/zap-net     QUIC (quinn), descoberta LAN, cliente da API do servidor
crates/zap-gpu     D3D11, texturas, conversor para NV12 (Windows)
crates/zap-capture monitor (DXGI), janela (WGC), câmera (Windows)
crates/zap-encode  H.264 por Media Foundation (Windows)
crates/zap-audio   Opus, loopback por app e saída de áudio
crates/zap-media   pipelines de vídeo/áudio + backend sintético
crates/zap-sys     firewall, interfaces de rede, Radmin/VPN (Windows)
crates/zap-core    orquestração da sala, threads, estado para a interface
app/src-tauri      app Tauri (comandos, eventos, updater)
app/src            interface React + TypeScript
```

Dependências só descem: `proto ← rate ← net ← core ← app`; `media` e `sys` só são usados por
`core`; `gpu`, `capture`, `encode` e `audio` só por `media`. `zap-proto` e `zap-rate` não usam tokio, quinn nem `windows`: algoritmos recebem o tempo
como parâmetro.

## Convenções

- Rust 2021, MSRV = estável atual. Erros com `thiserror` nas bibliotecas e `anyhow` só no app.
- Sem `unwrap()`/`expect()` fora de testes. `unsafe` só em `zap-media`/`zap-sys`, com
  `// SAFETY:`.
- Código do Windows atrás de `#[cfg(windows)]`, sempre com uma alternativa que compila no Linux
  (stub ou backend sintético).
- Comentários, docs, logs e textos da interface em **português**. Nomes de código em inglês.
- Dependências novas vão no `[workspace.dependencies]` da raiz e entram nos crates com
  `workspace = true`.
- Threads: nada de lock no callback de áudio; canais sempre limitados.

## Verificação (rode antes de commitar)

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo check --workspace --target x86_64-pc-windows-gnu   # pega erro no código do Windows
```

No front-end (`app/`): `npm run check` (tsc) e `npm test` (vitest).

**Disco cheio** ("No space left on device", "Disk full", ENOSPC): `target/` é só saída de build e
pode ser apagado sem pedir — rode `cargo clean` (ou apague `target/debug/incremental`) e continue.
Não apague mais nada.

O app Python antigo ainda tem seus testes (`pytest`) até ser removido no marco M6.
