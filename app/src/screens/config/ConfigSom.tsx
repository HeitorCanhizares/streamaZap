// Tarefa config-05: seção 'Som e avisos' e lista de atalhos da tela de configurações
// (porte fiel de docs/design/4-configuracoes.html, linhas 217-241). Toda mudança sai por onChange.
import type { ReactNode } from 'react'
import type { ConfigSecaoProps } from './ConfigPerfil'

// Atalhos fixos da tela. Não são configuráveis nesta versão.
const ATALHOS: { tecla: string; descricao: string }[] = [
  { tecla: 'F11', descricao: 'Tela cheia (ou duplo clique no vídeo)' },
  { tecla: 'Esc', descricao: 'Sair da tela cheia' },
  { tecla: 'M', descricao: 'Silenciar a sala toda' },
  { tecla: 'C', descricao: 'Ligar ou desligar sua câmera' },
]

// Avisos ligáveis. A ponte guarda o volume como fração (0 a 1); a tela mostra 0 a 100.
const AVISOS: { chave: 'message_sound' | 'notify_room_open' | 'mute_when_minimized'; label: string; hint: string }[] = [
  { chave: 'message_sound', label: 'Som de nova mensagem', hint: 'Aquele plim quando alguém manda algo no chat.' },
  { chave: 'notify_room_open', label: 'Avisar quando um amigo abrir sala', hint: 'Notificação do Windows com o nome da sala.' },
  { chave: 'mute_when_minimized', label: 'Silenciar quando minimizar', hint: 'O som volta quando você abrir a janela de novo.' },
]

// Trilho 44x26 do design (função sw() do script x-dc): ligado à direita em verde, desligado à esquerda.
function trilho(on: boolean): { track: string; knob: string } {
  return {
    track: 'flex: 0 0 44px; width: 44px; height: 26px; border-radius: 999px; padding: 3px; display: flex; justify-content: ' + (on ? 'flex-end' : 'flex-start') + '; background: ' + (on ? '#3BF08A' : '#2A443C'),
    knob: 'width: 20px; height: 20px; border-radius: 50%; background: ' + (on ? '#03140A' : '#A3BDB1'),
  }
}

export function ConfigSom({ settings, onChange }: ConfigSecaoProps): ReactNode {
  const sound = settings.sound
  const volumePct = Math.round(sound.initial_volume * 100)

  return (
    <section
      aria-labelledby="h-som"
      style={{
        flex: '1 1 400px', minWidth: 0, display: 'flex', flexDirection: 'column', gap: 16,
        padding: 22, borderRadius: 18, background: '#0F1916', border: '1px solid #1F332D',
      }}
    >
      <h2 id="h-som" style={{ margin: 0, fontSize: 16, fontWeight: 700 }}>Som e avisos</h2>

      <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
        <label htmlFor="volini" style={{ fontSize: 12.5, color: '#A3BDB1' }}>Volume inicial de cada transmissão</label>
        <div style={{ display: 'flex', alignItems: 'center', gap: 12 }}>
          <input
            id="volini"
            className="zp-range"
            type="range"
            min={0}
            max={100}
            step={1}
            value={volumePct}
            onChange={(e) => onChange({ sound: { ...sound, initial_volume: Number(e.target.value) / 100 } })}
            style={{ flex: 1 }}
          />
          <span style={{ flex: '0 0 44px', textAlign: 'right', fontFamily: "'JetBrains Mono', monospace", fontSize: 13 }}>
            {`${volumePct}%`}
          </span>
        </div>
      </div>

      {AVISOS.map((a) => {
        const on = sound[a.chave]
        const t = trilho(on)
        return (
          <button
            key={a.chave}
            type="button"
            role="switch"
            aria-checked={on}
            onClick={() => onChange({ sound: { ...sound, [a.chave]: !on } })}
            style={{
              display: 'flex', alignItems: 'center', gap: 12, padding: '12px 14px', borderRadius: 14,
              border: '1px solid #1F332D', background: '#0A100E', textAlign: 'left', color: 'inherit',
              font: 'inherit', cursor: 'pointer', width: '100%',
            }}
          >
            <span style={{ flex: 1, display: 'flex', flexDirection: 'column', gap: 2 }}>
              <span style={{ fontWeight: 600 }}>{a.label}</span>
              <span style={{ fontSize: 12, color: '#A3BDB1' }}>{a.hint}</span>
            </span>
            <span style={{ ...parseEstilo(t.track) }}>
              <span style={{ ...parseEstilo(t.knob) }} />
            </span>
          </button>
        )
      })}

      <div style={{ display: 'flex', flexDirection: 'column', gap: 10, marginTop: 4 }}>
        <h3
          style={{
            margin: 0, fontFamily: "'JetBrains Mono', monospace", fontWeight: 400, fontSize: 11,
            letterSpacing: '0.14em', textTransform: 'uppercase', color: '#7E978B',
          }}
        >
          Atalhos
        </h3>
        <dl style={{ margin: 0, display: 'grid', gridTemplateColumns: 'auto 1fr', gap: '10px 14px', alignItems: 'center', fontSize: 13 }}>
          {ATALHOS.map((a) => (
            <div key={a.tecla} style={{ display: 'contents' }}>
              <dt><kbd>{a.tecla}</kbd></dt>
              <dd style={{ margin: 0, color: '#A3BDB1' }}>{a.descricao}</dd>
            </div>
          ))}
        </dl>
      </div>
    </section>
  )
}

// Converte a string de estilo do trilho em objeto de estilo do React (chave: valor; separados por ';').
function parseEstilo(css: string): Record<string, string> {
  const out: Record<string, string> = {}
  for (const parte of css.split(';')) {
    const i = parte.indexOf(':')
    if (i < 0) continue
    const chave = parte.slice(0, i).trim().replace(/-([a-z])/g, (_, c: string) => c.toUpperCase())
    const valor = parte.slice(i + 1).trim()
    if (chave) out[chave] = valor
  }
  return out
}
