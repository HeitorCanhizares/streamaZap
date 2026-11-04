// Tarefa ui-01: faixa do ping (< 60 verde, < 100 amarelo, senão vermelho).
export type PingTone = 'green' | 'yellow' | 'red'

// NaN não satisfaz nenhuma comparação, então cai em 'red' sem caso especial.
// Negativos caem em 'green' (ms < 60), como a spec pede.
export function pingTone(ms: number): PingTone {
  if (ms < 60) return 'green'
  if (ms < 100) return 'yellow'
  return 'red'
}
