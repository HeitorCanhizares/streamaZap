# Como a Anthropic monta seus agentes (pesquisa para decidir o workflow do StreamaZap)

Pesquisa de 2026-11-05. Só fontes da própria Anthropic: posts de engenharia, documentação do
Claude Code e os plugins oficiais (`anthropics/claude-plugins-official`).

**Adotadas em 2026-11-05:** as propostas 1, 2 e 3 (ver "Propostas"), nos arquivos
`.claude/agents/zap-{revisor,implementador,decompositor}.md`, `.claude/workflows/zap-marco.js`,
`.claude/skills/zap-marco/SKILL.md` e `.claude/settings.local.json`. As outras continuam em
aberto.

## Resumo

1. **Quem avalia nunca é quem fez.** Em todos os textos, gerar e avaliar ficam em agentes
   separados, com contexto separado, porque o agente elogia o próprio trabalho.
2. **Modelo grande para revisar e escrever regras, modelo menor para o volume.** Na migração do
   Bun, Sonnet implementou e os modelos maiores revisaram e escreveram o livro de regras. O Haiku
   aparece só em tarefas mecânicas: checar se um PR precisa de revisão, listar arquivos.
3. **Script verifica, agente julga.** Compilador, testes e diffs decidem o que está certo. Os
   revisores ficam com o que exige julgamento, e cada achado é conferido por outro agente antes
   de contar.
4. **Achado repetido é corrigido na regra, não no código.** Quando o mesmo problema volta, a
   correção vai para o livro de regras e o lote é gerado de novo.
5. **O harness encolhe quando o modelo melhora.** A cada modelo novo, eles tiram uma peça por
   vez e veem se ela ainda carregava peso.

## O que cada papel faz lá

| Papel | O que a Anthropic manda fazer | Modelo | Fonte |
|---|---|---|---|
| Planejador | Expande um pedido curto numa spec de produto completa: ambiciosa no escopo, mas **alto nível na parte técnica**, porque detalhe errado no plano se propaga em cascata. Sem ele, o gerador faz menos do que devia | Opus | [Harness design][harness] |
| Explorador | 2 ou 3 em paralelo, cada um rastreia um caminho do código (entrada até os dados) e devolve a lista de arquivos-chave. O orquestrador lê esses arquivos antes de seguir | Sonnet, só leitura | [feature-dev][fdev] |
| Perguntas | Fase "CRÍTICA, não pule": antes de projetar, lista o que está subespecificado (bordas, erros, integração, escopo) e **espera a resposta** do humano | o orquestrador | [feature-dev][fdev] |
| Arquiteto | 2 ou 3 em paralelo com focos diferentes: mudança mínima, arquitetura limpa, equilíbrio. O orquestrador recomenda um e o humano escolhe | Sonnet | [feature-dev][fdev] |
| Teste de estresse da spec | Antes de soltar o volume: um agente traduz 3 arquivos seguindo as regras, outro traduz como um engenheiro sênior faria, um terceiro tira regras novas do diff. **A saída é jogada fora**; só as regras ficam | modelo grande | [Migrações][migr] |
| Gerador / implementador | Uma funcionalidade por vez. Na migração, lotes de arquivos, com `// TODO(port)` no que não resolve | Sonnet (12 em paralelo no caso do Mike) | [Harness design][harness], [Migrações][migr] |
| Contrato de sprint | Antes de codar, o gerador propõe o que vai entregar e como provar; o avaliador revisa até concordarem (um sprint teve 27 critérios) | — | [Harness design][harness] |
| Avaliador | Usa o app rodando (Playwright), dá nota por critério com **limite mínimo em cada um**: um critério abaixo reprova o sprint. Calibrado com exemplos para ser cético | Opus | [Harness design][harness] |
| Revisores adversários | Dois revisores em contextos separados; divergência vai para um terceiro. Cada achado cita a regra que ele viola | modelo grande | [Migrações][migr] |
| Validador de achado | No `/code-review` oficial: 2 agentes Sonnet de conformidade com o CLAUDE.md + 2 Opus caçando bugs no diff; **cada bug achado ganha um subagente que tenta confirmar**, e o que não se confirma é descartado. Só entra o que é "sinal alto" | Haiku na triagem, Sonnet e Opus na revisão | [code-review][cr] |
| Revisor com ferramentas limitadas | Revisor com `tools: Read, Grep, Glob`: não edita, e isso é garantido pelo Claude Code, não pelo prompt. Pedir sempre "ignore estilo", senão ele acha algo e o código engorda | — | [Boas práticas][bp] |
| Corretores | Pegam erro de compilação, crash ou teste falhando, em paralelo, e passam por revisão adversária | — | [Migrações][migr] |
| Daemon de build | **Não é agente.** É o único processo que compila: junta os patches dos corretores, compila uma vez e roda os testes afetados | script | [Migrações][migr] |
| Papéis de manutenção | No compilador C com 16 agentes: um remove duplicação, um cuida de desempenho, um da eficiência do código gerado, um critica o design como desenvolvedor Rust, um cuida da documentação | — | [Compilador C][cc] |
| Simplificador | Depois que a funcionalidade está pronta, simplifica o código mexido sem mudar o comportamento | Opus | [code-simplifier][cs] |
| Caçadores especializados de PR | `silent-failure-hunter` (erro engolido, fallback ruim), `pr-test-analyzer` (cobertura das bordas), `type-design-analyzer` (invariantes nos tipos), `comment-analyzer` (comentário desatualizado) | herda | [pr-review-toolkit][prt] |

## Padrões que se repetem

- **Fila no disco e retomada.** Uma tarefa está pronta quando a saída dela existe; a fila é
  refeita a partir do disco ([Migrações][migr]). Os workflows salvos retomam sozinhos do que já
  terminou ([Workflows][wf]).
- **Tarefa trancada por arquivo.** No compilador C, cada agente cria `current_tasks/<tarefa>.txt`
  para pegar a tarefa, e o git resolve quando dois pegam a mesma ([Compilador C][cc]).
- **Saída curta para o agente.** Imprimir poucas linhas, mandar o resto para log, pôr "ERROR" e o
  motivo na mesma linha, ter um `--fast` que roda uma amostra fixa dos testes
  ([Compilador C][cc]).
- **Teste é o juiz e tem de ser quase perfeito.** Antes de confiar no juiz, provam que ele passa
  no original e falha num código quebrado de propósito ([Migrações][migr]).
- **Compilador lento fica fora do laço.** Quem tinha TypeScript (segundos) compilava a cada
  tarefa; quem tinha `cargo` (minutos) deixou a compilação de fora até a fase de correção
  ([Migrações][migr]).
- **O humano entra no começo.** O esforço humano fica nas regras e no teste de estresse; o Mike
  rodou a migração inteira 3 vezes, jogando fora a saída até a terceira ([Migrações][migr]).
- **Regra que não pode falhar vira hook, não texto.** CLAUDE.md abaixo de 200 linhas, para
  orientação; "sempre que X, faça Y" vai num hook (`PostToolUse` para formatar, `Stop` que
  devolve o agente ao trabalho se a checagem falhar) ([Steering][steer], [Boas práticas][bp]).
- **Poucos agentes, bem descritos.** Muitos especialistas tornam a delegação automática menos
  confiável; a `description` deve dizer *quando* usar, não *o que* o agente sabe
  ([Subagentes][sub]).
- **Na equipe do Claude Code**, todo commit é feito com Claude; o gargalo passou a ser verificação,
  revisão e segurança. Humanos ficam com fronteiras de confiança, código sensível e gosto de
  produto ([Org AI-native][org]).

## Comparação com a hierarquia `zap-*`

| Ponto | StreamaZap hoje | Anthropic | Distância |
|---|---|---|---|
| Planejamento | `zap-arquiteto` (Opus) escreve plano com assinaturas reais e números | Planejador em alto nível; detalhe técnico só depois de explorar | Nosso plano é mais detalhado, o que ajuda a dividir em tarefas, mas espalha um erro de spec por todas elas |
| Perguntas ao humano | Não há: o decompositor "decide e anota" quando o plano é ambíguo | Fase obrigatória antes de projetar | Falta |
| Teste de estresse da spec | Não há | Amostra jogada fora antes do volume | Falta |
| Esqueleto e dono de arquivo | `zap-decompositor` cria esqueleto compilável; uma tarefa por arquivo | Mapa de dependências determinístico; tarefa trancada por arquivo | Igual em espírito. Bom |
| Implementador | Haiku por padrão, Sonnet nas difíceis | Sonnet no volume; Haiku só no mecânico | Nosso padrão está um degrau abaixo |
| Revisor | Um Sonnet que **também corrige** até 30 linhas (`corrigido`) | Revisor separado de quem corrige; revisores maiores; dois adversários; achado validado | Nosso revisor avalia o próprio conserto e é menor que o recomendado |
| Verificação | Cada implementador roda `cargo test` do crate; até 4 em paralelo | Um único processo compila; agentes mandam patches | Neste PC (7,9 GB) cargo em paralelo já estourou a memória hoje |
| Avaliação no app real | Aceites e bancada, rodados no fechamento | Avaliador usa o app rodando, com nota mínima por critério | Parcial: temos os números, falta o agente que os cobra |
| Achado repetido | Corrigido na tarefa | Corrigido na regra, lote refeito | Falta. O ESTADO tem casos repetidos (testes que pulavam em silêncio em paralelo) |
| Hooks | Nenhum (`.claude/settings.json` não existe) | Hooks para formatar e para barrar o fim sem checagem | Falta |
| Revisão do harness | Desenhado para Haiku/Sonnet/Opus antigos | Tirar uma peça por vez a cada modelo novo | Nunca foi revisado; nas rodadas M4b/M4c o orquestrador corrigiu à mão depois da hierarquia |

## Propostas, da que mais rende para a que menos

1. **Revisor só lê; quem corrige é outro.** Trocar `corrigido` por: o revisor (ferramentas
   `Read, Grep, Glob, Bash` só para rodar testes) devolve achados com a regra violada; o conserto
   volta para o implementador ou para a escalada. Revisor no **Opus** para tarefas `sonnet` e no
   fechamento. Pequena mudança em `zap-revisor.md` e no `zap-marco.js`.
2. **Implementador padrão Sonnet**, Haiku só para tarefas marcadas `mecanica` (renomear, portar
   texto, testes de tabela). Mudança de uma linha no workflow e na regra de `nivel` do
   decompositor.
3. **Fila de build única.** Implementadores só rodam `cargo check -p <crate>` e o teste da própria
   tarefa com `CARGO_BUILD_JOBS=2`; o teste do workspace roda uma vez por onda, num agente só.
   Evita o estouro de memória deste PC e a briga pelo lock do `target/`.
4. **Fase de perguntas** entre o arquiteto e o decompositor: o arquiteto devolve, além do plano,
   uma lista de pontos em aberto, e o workflow para (cada fase vira um workflow separado, porque
   workflow não aceita entrada no meio).
5. **Teste de estresse da spec:** antes de soltar todas as tarefas de uma frente, rodar 1 ou 2
   como amostra, revisar contra a spec e corrigir o plano antes do resto.
6. **Achado repetido vira regra:** no fechamento, o revisor lista as classes de problema que
   apareceram mais de uma vez e propõe a linha para o `CLAUDE.md`, o arquivo do agente ou um hook.
7. **Hooks baratos:** `PostToolUse` com `rustfmt` no arquivo `.rs` editado (rápido) e
   `npx prettier`/`tsc` só quando o arquivo é do `app/src`. Nada de `cargo test` em hook neste PC.
8. **Validação de achado no fechamento:** usar o `/code-review` embutido (revisor que nunca viu a
   conversa) sobre o diff do marco, em vez de só o revisor da casa.
9. **Avaliador do app (verify):** um agente que sobe o `zap-test-server`, roda os aceites e a
   bancada e reprova se algum número do plano não bate, com o limite de cada critério escrito no
   plano.

## O que eu não adotaria agora

- **Agent teams e sessões em worktrees para cada tarefa:** cada worktree tem seu `target/`, e
  este PC não tem memória nem disco para isso. Dono de arquivo na mesma pasta já resolve.
- **Os 6 caçadores do `pr-review-toolkit` como agentes fixos:** muitos especialistas atrapalham a
  delegação. Vale chamar um deles quando o assunto pedir (o `silent-failure-hunter` combina com a
  regra de "sem `unwrap()`").
- **Laços longos do tipo "rode até passar" sem limite:** custam caro; os exemplos da própria
  documentação de workflows param depois de duas rodadas seguidas sem progresso.

## Fontes

- [Harness design for long-running apps][harness] (Anthropic Engineering, 2026-03-24)
- [How Anthropic runs large-scale code migrations with Claude Code][migr] (2026-07-16)
- [Building a C compiler with a team of parallel Claudes][cc] (Anthropic Engineering, 2026-02-05)
- [Running an AI-native engineering org][org] (2026-06-03)
- [Introducing dynamic workflows][dw] (2026-05-28) e [documentação de workflows][wf]
- [How and when to use subagents in Claude Code][sub] (2026-04-07)
- [Steering Claude Code: CLAUDE.md, skills, hooks e subagentes][steer] (2026-06-18)
- [Best practices for Claude Code][bp] (documentação)
- Plugins oficiais: [feature-dev][fdev], [code-review][cr], [pr-review-toolkit][prt],
  [code-simplifier][cs]

[harness]: https://www.anthropic.com/engineering/harness-design-long-running-apps
[migr]: https://claude.com/resources/articles/ai-code-migration
[cc]: https://www.anthropic.com/engineering/building-c-compiler
[org]: https://claude.com/blog/running-an-ai-native-engineering-org
[dw]: https://claude.com/blog/introducing-dynamic-workflows-in-claude-code
[wf]: https://code.claude.com/docs/en/workflows
[sub]: https://claude.com/resources/articles/subagents-in-claude-code
[steer]: https://claude.com/resources/articles/steering-claude-code-skills-hooks-rules-subagents-and-more
[bp]: https://code.claude.com/docs/en/best-practices
[fdev]: https://github.com/anthropics/claude-plugins-official/tree/main/plugins/feature-dev
[cr]: https://github.com/anthropics/claude-plugins-official/tree/main/plugins/code-review
[prt]: https://github.com/anthropics/claude-plugins-official/tree/main/plugins/pr-review-toolkit
[cs]: https://github.com/anthropics/claude-plugins-official/tree/main/plugins/code-simplifier
