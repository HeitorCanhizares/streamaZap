export const meta = {
  name: 'zap-marco',
  description: 'Executa um marco do StreamaZap: arquiteto (Opus) planeja, decompositor (Sonnet) quebra, implementador (Sonnet; Haiku no mecânico) faz, revisor um nível acima confere sem editar',
  whenToUse: 'Implementar um marco ou funcionalidade grande do StreamaZap. args: {marco, objetivo?, plano?, frentes?, so_frentes?, max_paralelo?}',
  phases: [
    { title: 'Planejar', detail: 'zap-arquiteto escreve ou atualiza o plano do marco', model: 'opus' },
    { title: 'Decompor', detail: 'zap-decompositor cria o esqueleto e as tarefas de cada frente', model: 'sonnet' },
    { title: 'Implementar', detail: 'zap-implementador faz cada tarefa; zap-revisor, um nível acima, confere sem editar', model: 'sonnet' },
    { title: 'Integrar', detail: 'zap-implementador roda a verificação inteira e corrige a integração; zap-revisor confere', model: 'sonnet' },
  ],
}

// args:
//   marco         "M1" (obrigatório)
//   objetivo      texto livre para o arquiteto (quando não há plano)
//   plano         caminho do plano em docs/planos/ (pula ou revisa o planejamento)
//   frentes       [{id, crate, resumo, depende_de}] — com `plano`, pula o arquiteto
//   so_frentes    ["rate", ...] — executa só essas frentes
//   max_paralelo  tarefas simultâneas (padrão 4; nunca duas no mesmo crate). O cargo compila uma
//                 de cada vez no mesmo target/ (fila pelo lock), e CARGO_BUILD_JOBS limita a
//                 memória de cada compilação (.claude/settings.local.json neste PC)
//   fechamento    instruções do fechamento no lugar do padrão (ex.: só verificações do front-end)
//   tarefas       {frente_id: [tarefa]} já decompostas (continuar um marco sem decompor de novo)

const A = args || {}
if (!A.marco) throw new Error('args.marco é obrigatório (ex.: "M1")')
const MAX = A.max_paralelo || 4
const papel = (nome) => `Siga o papel definido em \`.claude/agents/${nome}.md\` (leia esse arquivo primeiro).`
const json = (v) => JSON.stringify(v, null, 2)

const FRENTE = {
  type: 'object',
  properties: {
    id: { type: 'string' },
    crate: { type: 'string' },
    resumo: { type: 'string' },
    depende_de: { type: 'array', items: { type: 'string' } },
  },
  required: ['id', 'crate', 'resumo'],
}
const PLANO_SCHEMA = {
  type: 'object',
  properties: { plano: { type: 'string' }, frentes: { type: 'array', items: FRENTE } },
  required: ['plano', 'frentes'],
}
const TAREFA = {
  type: 'object',
  properties: {
    id: { type: 'string' },
    titulo: { type: 'string' },
    crate: { type: 'string' },
    arquivos: { type: 'array', items: { type: 'string' } },
    depende_de: { type: 'array', items: { type: 'string' } },
    nivel: { type: 'string', enum: ['haiku', 'sonnet'] },
    spec: { type: 'string' },
    testes: { type: 'string' },
    verificacao: { type: 'string' },
  },
  required: ['id', 'titulo', 'crate', 'arquivos', 'depende_de', 'nivel', 'spec', 'testes', 'verificacao'],
}
const TAREFAS_SCHEMA = {
  type: 'object',
  properties: {
    esqueleto_ok: { type: 'boolean' },
    tarefas: { type: 'array', items: TAREFA },
    notas: { type: 'string' },
  },
  required: ['esqueleto_ok', 'tarefas'],
}
const IMPL_SCHEMA = {
  type: 'object',
  properties: {
    status: { type: 'string', enum: ['feito', 'bloqueado'] },
    arquivos_alterados: { type: 'array', items: { type: 'string' } },
    verificacao: { type: 'string' },
    notas: { type: 'string' },
  },
  required: ['status', 'verificacao'],
}
// O revisor não edita: devolve achados e o conserto volta para quem implementou.
const ACHADO = {
  type: 'object',
  properties: {
    onde: { type: 'string' },
    problema: { type: 'string' },
    regra: { type: 'string' },
  },
  required: ['onde', 'problema', 'regra'],
}
const REV_SCHEMA = {
  type: 'object',
  properties: {
    veredito: { type: 'string', enum: ['aprovado', 'refazer'] },
    achados: { type: 'array', items: ACHADO },
    verificacao: { type: 'string' },
  },
  required: ['veredito', 'achados', 'verificacao'],
}
const FECHAMENTO_SCHEMA = {
  type: 'object',
  properties: {
    ok: { type: 'boolean' },
    resumo: { type: 'string' },
    arquivos_alterados: { type: 'array', items: { type: 'string' } },
    pendencias: { type: 'array', items: { type: 'string' } },
  },
  required: ['ok', 'resumo'],
}
const achadosEmTexto = (rev) =>
  (rev.achados || []).map((a) => `- ${a.onde}: ${a.problema} (regra: ${a.regra})`).join('\n') +
  (rev.verificacao ? `\n\nVerificação do revisor:\n${rev.verificacao}` : '')

// ── 1. Planejar (Opus) ────────────────────────────────────────────────
phase('Planejar')
let plano
if (A.plano && A.frentes) {
  plano = { plano: A.plano, frentes: A.frentes }
  log(`Plano pronto: ${A.plano} (${A.frentes.length} frentes)`)
} else {
  plano = await agent(
    `${papel('zap-arquiteto')}\n\nMarco: ${A.marco}.\n` +
      (A.objetivo ? `Objetivo: ${A.objetivo}\n` : '') +
      (A.plano ? `Plano existente para revisar e completar: ${A.plano}\n` : '') +
      'Escreva (ou atualize) o plano em docs/planos/ e devolva o caminho e as frentes ' +
      '(uma por crate ou pasta do front-end, com as dependências entre elas).',
    { label: 'arquiteto', phase: 'Planejar', model: 'opus', schema: PLANO_SCHEMA },
  )
  if (!plano) throw new Error('O arquiteto não devolveu um plano')
}
const frentes = A.so_frentes ? plano.frentes.filter((f) => A.so_frentes.includes(f.id)) : plano.frentes
log(`Frentes: ${frentes.map((f) => f.id).join(', ')}`)

// ── 2+3. Decompor (Sonnet) e Implementar (Sonnet ou Haiku → revisão um nível acima) em esteira ──
// Uma frente só é decomposta depois das frentes de que depende (o esqueleto dela usa os tipos
// delas e as tarefas podem depender das tarefas delas). As tarefas de uma frente entram na agenda
// assim que ela é decomposta: a implementação de `proto` não espera a decomposição de `net`.
phase('Decompor')
const decomp = {}
const visitando = new Set()
function decompor(f) {
  if (decomp[f.id]) return decomp[f.id]
  if (visitando.has(f.id)) throw new Error(`Ciclo entre frentes em ${f.id}`)
  visitando.add(f.id)
  const deps = (f.depende_de || []).map((d) => frentes.find((x) => x.id === d)).filter(Boolean)
  decomp[f.id] = Promise.all(deps.map(decompor)).then((anteriores) => {
    const contexto = anteriores
      .filter(Boolean)
      .flatMap((r) => r.tarefas.map((t) => `- ${t.id} (${t.crate}): ${t.titulo}`))
      .join('\n')
    return agent(
      `${papel('zap-decompositor')}\n\nPlano: ${plano.plano}\nFrente: ${json(f)}\n` +
        (contexto ? `\nTarefas já planejadas nas frentes de que esta depende (pode usar em depende_de):\n${contexto}\n` : '') +
        '\nNão edite o Cargo.toml da raiz a menos que seja indispensável (outras frentes rodam ao mesmo tempo).' +
        '\nCrie o esqueleto compilável e devolva as tarefas.',
      { label: `decompor ${f.id}`, phase: 'Decompor', model: 'sonnet', schema: TAREFAS_SCHEMA },
    )
  })
  return decomp[f.id]
}

// Quem implementa num nível é revisado no nível acima (Haiku → Sonnet, Sonnet → Opus; o Opus é
// revisado pelo Opus, em outro contexto). Implementação padrão no Sonnet; Haiku só no mecânico.
const NIVEIS = ['haiku', 'sonnet', 'opus']
const acimaDe = (m) => NIVEIS[Math.min(NIVEIS.indexOf(m) + 1, NIVEIS.length - 1)]

// Uma rodada: implementa (ou conserta, com `feedback`) e revisa. O revisor não edita.
async function tentar(t, modelo, feedback, rotulo) {
  const impl = await agent(
    `${papel('zap-implementador')}\n\nTarefa:\n${json(t)}` +
      (feedback ? `\n\nA tentativa anterior não passou. O que consertar:\n${feedback}` : ''),
    { label: `${t.id} ${rotulo}`, phase: 'Implementar', model: modelo, schema: IMPL_SCHEMA },
  )
  if (!impl) return { ok: false, feedback: 'O implementador anterior morreu sem resposta.' }
  if (impl.status === 'bloqueado') {
    return { ok: false, bloqueado: true, feedback: `Implementador bloqueado: ${impl.notas || ''}\n${impl.verificacao}` }
  }
  const revisor = acimaDe(modelo)
  const rev = await agent(
    `${papel('zap-revisor')}\n\nRevise a tarefa abaixo (feita pelo nível ${modelo}).\n\nTarefa:\n${json(t)}\n\nRelato do implementador:\n${json(impl)}`,
    { label: `${t.id} revisão ${revisor}`, phase: 'Implementar', model: revisor, schema: REV_SCHEMA },
  )
  if (!rev) return { ok: false, feedback: 'O revisor morreu sem resposta; a implementação não foi conferida.' }
  if (rev.veredito === 'aprovado') return { ok: true }
  return { ok: false, feedback: achadosEmTexto(rev) }
}

async function executar(t) {
  const modelo = t.nivel === 'haiku' ? 'haiku' : 'sonnet'
  let r = { ok: false, feedback: '' }
  for (let tentativa = 1; tentativa <= 2; tentativa++) {
    r = await tentar(t, modelo, r.feedback, `${modelo}${tentativa > 1 ? ' #2' : ''}`)
    if (r.ok) return { id: t.id, ok: true, por: modelo, veredito: 'aprovado' }
    if (r.bloqueado) break
  }
  // Duas recusas (ou um bloqueio) sobem a implementação um nível; a revisão sobe junto.
  const acima = acimaDe(modelo)
  log(`${t.id}: escalada para ${acima}`)
  const esc = await tentar(
    t,
    acima,
    `O nível ${modelo} não conseguiu. O que deu errado até agora:\n${r.feedback}`,
    `escalada ${acima}`,
  )
  return {
    id: t.id,
    ok: esc.ok,
    por: `${acima} (escalada)`,
    veredito: esc.ok ? 'aprovado' : 'refazer',
    feedback: esc.ok ? '' : esc.feedback,
  }
}

// Agenda única: decomposições e tarefas disputam o mesmo `Promise.race`. Regras: respeita
// depende_de, no máximo MAX tarefas ao mesmo tempo e nunca duas no mesmo crate (compilam o mesmo
// alvo; um arquivo pela metade de uma quebraria a verificação da outra).
const tarefas = new Map()
const feitas = new Set()
const falhas = new Set()
const ocupados = new Set()
const rodando = new Map()
const resultados = []
// `args.tarefas` = {frente_id: [tarefa, ...]}: frentes já decompostas (continuação de um marco
// que parou no meio); essas frentes não passam pelo decompositor.
const prontas = A.tarefas || {}
const emDecomposicao = new Map(
  frentes
    .filter((f) => !prontas[f.id])
    .map((f) => [f.id, decompor(f).then((r) => ({ decomp: true, f, r }))]),
)
function acolher(f, r) {
  if (!r) return log(`⚠ frente ${f.id}: decompositor sem resposta (pulada)`)
  if (!r.esqueleto_ok) log(`⚠ frente ${f.id}: esqueleto não compilou (${r.notas || 'sem notas'})`)
  for (const t of r.tarefas) {
    const desconhecidas = t.depende_de.filter((d) => !tarefas.has(d) && !r.tarefas.some((x) => x.id === d))
    if (desconhecidas.length) log(`${t.id}: dependências ignoradas (não existem): ${desconhecidas.join(', ')}`)
    t.depende_de = t.depende_de.filter((d) => !desconhecidas.includes(d))
    tarefas.set(t.id, t)
  }
  const haiku = r.tarefas.filter((t) => t.nivel === 'haiku').length
  log(`frente ${f.id}: ${r.tarefas.length} tarefas (${haiku} mecânicas, no haiku)`)
}
const pendentes = () => [...tarefas.values()].filter((t) => !feitas.has(t.id) && !falhas.has(t.id))
for (const f of frentes.filter((f) => prontas[f.id])) {
  // Dependências fora da lista já estão prontas (de uma execução anterior).
  const ids = new Set(Object.values(prontas).flat().map((t) => t.id))
  acolher(f, {
    esqueleto_ok: true,
    tarefas: prontas[f.id].map((t) => ({ ...t, depende_de: t.depende_de.filter((d) => ids.has(d)) })),
  })
}
while (emDecomposicao.size || pendentes().length) {
  for (const t of tarefas.values()) {
    if (rodando.size >= MAX) break
    if (feitas.has(t.id) || falhas.has(t.id) || rodando.has(t.id) || ocupados.has(t.crate)) continue
    if (t.depende_de.some((d) => falhas.has(d))) {
      falhas.add(t.id)
      resultados.push({ id: t.id, ok: false, por: '-', veredito: 'pulada (dependência falhou)' })
      continue
    }
    if (!t.depende_de.every((d) => feitas.has(d))) continue
    ocupados.add(t.crate)
    rodando.set(
      t.id,
      executar(t)
        .catch((e) => ({ id: t.id, ok: false, por: '-', veredito: `erro: ${e}` }))
        .then((r) => ({ t, r })),
    )
  }
  if (rodando.size === 0 && emDecomposicao.size === 0) {
    const presas = pendentes()
    if (!presas.length) break
    // Ninguém roda e ninguém pode rodar: ciclo de dependências (erro do decompositor). Como o
    // esqueleto compila, a ordem é preferência, não exigência: força a tarefa presa de menor
    // número, ignorando as dependências que ainda estão presas, e segue.
    const ids = new Set(presas.map((t) => t.id))
    const candidata = presas
      .filter((t) => t.depende_de.every((d) => feitas.has(d) || ids.has(d)))
      .sort((a, b) => a.id.localeCompare(b.id, 'pt', { numeric: true }))[0]
    if (!candidata) {
      log(`⚠ ${presas.length} tarefas presas sem saída: ${presas.map((t) => t.id).join(', ')}`)
      presas.forEach((t) => {
        falhas.add(t.id)
        resultados.push({ id: t.id, ok: false, por: '-', veredito: 'presa' })
      })
      break
    }
    const ignoradas = candidata.depende_de.filter((d) => ids.has(d))
    log(`⚠ ciclo de dependências: forçando ${candidata.id} (ignorando ${ignoradas.join(', ')})`)
    candidata.depende_de = candidata.depende_de.filter((d) => !ids.has(d))
    continue
  }
  const ev = await Promise.race([...emDecomposicao.values(), ...rodando.values()])
  if (ev.decomp) {
    emDecomposicao.delete(ev.f.id)
    acolher(ev.f, ev.r)
    continue
  }
  const { t, r } = ev
  rodando.delete(t.id)
  ocupados.delete(t.crate)
  ;(r.ok ? feitas : falhas).add(t.id)
  resultados.push(r)
  log(`${r.ok ? '✓' : '✗'} ${t.id} (${r.por}, ${r.veredito}) — ${feitas.size}/${tarefas.size}`)
}


// ── 4. Integrar (Sonnet implementa, Opus revisa) ──────────────────────
// A verificação do workspace inteiro roda aqui, uma vez, num agente só (as tarefas só verificam o
// próprio crate). Quem integra edita; o revisor confere sem editar e sem rodar o workspace de novo.
phase('Integrar')
const pedidoFechamento =
  `Fechamento do marco ${A.marco} (plano ${plano.plano}). ` +
  (A.fechamento || 'Faça a parte "Integração (fechamento de marco)" do seu papel.') +
  `\n\nResultado das tarefas:\n${json(resultados)}`
let fechamento = await agent(`${papel('zap-implementador')}\n\n${pedidoFechamento}`, {
  label: 'integração',
  phase: 'Integrar',
  model: 'sonnet',
  schema: FECHAMENTO_SCHEMA,
})
let revisao = null
if (fechamento) {
  revisao = await agent(
    `${papel('zap-revisor')}\n\nFaça a "Revisão da integração (fechamento de marco)" do seu papel.\n\n` +
      `${pedidoFechamento}\n\nRelato da integração:\n${json(fechamento)}`,
    { label: 'revisão da integração', phase: 'Integrar', model: 'opus', schema: REV_SCHEMA },
  )
  if (revisao && revisao.veredito === 'refazer') {
    const segunda = await agent(
      `${papel('zap-implementador')}\n\n${pedidoFechamento}\n\nVocê já fez a integração uma vez e a revisão ` +
        `pediu para refazer. Conserte isto e rode de novo só a verificação dos crates que mudaram:\n` +
        achadosEmTexto(revisao),
      { label: 'integração #2', phase: 'Integrar', model: 'sonnet', schema: FECHAMENTO_SCHEMA },
    )
    if (segunda) fechamento = segunda
  }
}
return { plano: plano.plano, tarefas: resultados, fechamento, revisao }
