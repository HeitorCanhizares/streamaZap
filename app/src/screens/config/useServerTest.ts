// Estado de "Testar conexão" (Configurações > Conexão). Tarefa ui-online-07.
import type { ServerTestResult } from '../../bridge/types'

export interface UseServerTest {
  testando: boolean
  resultado: ServerTestResult | null
  testar: (endereco: string, senha: string) => Promise<void>
  limpar: () => void
}

export function useServerTest(): UseServerTest {
  throw new Error('tarefa ui-online-07')
}
