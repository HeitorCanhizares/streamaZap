// Espelho da tabela de travas de zap-core (plan_gate.rs); o teste do núcleo é a fonte. Tarefa bridge-10.
import type { PlanId, PresetLock, QualityLocks } from '../types'

// Ordem de `PRESETS` no núcleo: 720p30, 720p60, 1080p30, 1080p60.
// Os planos 'nenhum' e 'espectador' não transmitem: travam tudo em HD e Full HD.
const TRAVAS_SEM_TRANSMISSAO: PresetLock[] = [
  { preset: '720p30', needs: 'hd' },
  { preset: '720p60', needs: 'hd' },
  { preset: '1080p30', needs: 'fullhd' },
  { preset: '1080p60', needs: 'fullhd' },
]

// Plano 'hd' transmite até 720p60; 1080p continua travado até 'fullhd'.
const TRAVAS_HD: PresetLock[] = [
  { preset: '1080p30', needs: 'fullhd' },
  { preset: '1080p60', needs: 'fullhd' },
]

function copiaTravas(lista: PresetLock[]): PresetLock[] {
  return lista.map((t) => ({ ...t }))
}

export function travasDoPlano(id: PlanId): QualityLocks {
  const travas = id === 'fullhd' ? [] : id === 'hd' ? TRAVAS_HD : TRAVAS_SEM_TRANSMISSAO
  return {
    can_publish: id !== 'nenhum' && id !== 'espectador',
    screen: copiaTravas(travas),
    camera: copiaTravas(travas),
  }
}
