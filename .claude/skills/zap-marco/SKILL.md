---
name: zap-marco
description: Implementa um marco ou funcionalidade grande do StreamaZap com a hierarquia de agentes (arquiteto Opus → decompositor Sonnet → implementador Sonnet, Haiku no mecânico → revisor um nível acima, sem editar). Use quando pedirem "faça o marco M2", "implemente a câmera", "/zap-marco M3" ou qualquer trabalho grande no StreamaZap v3.
---

# Rodar um marco com a hierarquia de agentes

1. Descubra o marco e o objetivo: o pedido do usuário + a tabela "Marcos" de
   `docs/arquitetura-v3.md`. Veja se já existe plano em `docs/planos/` para ele.
2. Se não há plano, ou o plano precisa de revisão grande, deixe o arquiteto fazer: chame o
   Workflow salvo `zap-marco` com `args: {marco, objetivo}`.
3. Se o plano já existe e lista as frentes, pule o arquiteto:
   `args: {marco, plano: "docs/planos/<arquivo>.md", frentes: [{id, crate, resumo, depende_de}]}`.
4. Para rodar só parte do marco: `so_frentes: ["rate", "net"]`.
5. Quando o workflow terminar, leia o `fechamento`, a `revisao` da integração e a lista de
   tarefas que falharam. Corrija o que sobrou (ou rode de novo só as frentes com falha), rode as
   verificações do `CLAUDE.md`, faça commit e conte ao usuário o que ficou pronto e o que não ficou.

Regras do fluxo (já embutidas no workflow):
- o decompositor cria um esqueleto compilável antes das tarefas; cada tarefa é dona dos seus
  arquivos;
- nunca rodam duas tarefas no mesmo crate ao mesmo tempo;
- implementação no Sonnet por padrão; Haiku só nas tarefas marcadas como mecânicas;
- quem revisa fica um nível acima de quem fez (Haiku → Sonnet, Sonnet → Opus, Opus → Opus) e
  **não edita**: devolve achados, e o conserto volta para o implementador;
- duas recusas (ou um bloqueio) sobem a implementação um nível, e a revisão sobe junto;
- as tarefas só compilam e testam o próprio crate; o workspace inteiro roda uma vez, na
  integração, que o Opus revisa (com uma rodada de conserto, se ele pedir);
- neste PC, `.claude/settings.local.json` põe `CARGO_BUILD_JOBS=2` para o cargo não esgotar a
  memória.
