// Estado de "Testar conexão" (Configurações > Conexão). Tarefa ui-online-07.
import { useCallback, useEffect, useRef, useState } from 'react'
import { api } from '../../bridge/api'
import type { ServerTestResult } from '../../bridge/types'

export interface UseServerTest {
  testando: boolean
  resultado: ServerTestResult | null
  testar: (endereco: string, senha: string) => Promise<void>
  limpar: () => void
}

export function useServerTest(): UseServerTest {
  const [testando, setTestando] = useState(false)
  const [resultado, setResultado] = useState<ServerTestResult | null>(null)
  // Geração do teste: respostas de testes antigos (limpos, substituídos ou
  // de um componente já desmontado) são descartadas comparando com este número.
  const geracao = useRef(0)

  useEffect(() => {
    return () => {
      geracao.current++
    }
  }, [])

  const testar = useCallback(async (endereco: string, senha: string): Promise<void> => {
    const id = ++geracao.current
    const alvo = endereco.trim()
    if (alvo === '') {
      // Sem endereço não faz sentido chamar a API: a mensagem vem da própria tela.
      setTestando(false)
      setResultado({ ok: false, name: null, ping_ms: null, code: 'sem_endereco', message: 'Preencha o endereço do servidor' })
      return
    }
    setTestando(true)
    setResultado(null)
    try {
      const r = await api.serverTest(alvo, senha)
      if (id === geracao.current) setResultado(r)
    } catch (e) {
      if (id === geracao.current) {
        setResultado({ ok: false, name: null, ping_ms: null, code: 'erro', message: e instanceof Error ? e.message : String(e) })
      }
    } finally {
      if (id === geracao.current) setTestando(false)
    }
  }, [])

  const limpar = useCallback((): void => {
    // Invalida qualquer teste em andamento para que a resposta tardia não reapareça.
    geracao.current++
    setResultado(null)
    setTestando(false)
  }, [])

  return { testando, resultado, testar, limpar }
}
