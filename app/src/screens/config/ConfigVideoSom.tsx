// Tarefa ui-25: seções 'Vídeo e câmera', 'Som e avisos' e lista de atalhos da tela de configurações
// (porte de docs/design/4-configuracoes.html). Toda mudança sai por onChange; nada é mutado aqui.
import type { ReactNode } from 'react'
import type { Preset, Sources } from '../../bridge/types'
import { Segmented } from '../../components/Segmented'
import { Select } from '../../components/Select'
import { Slider } from '../../components/Slider'
import { Toggle } from '../../components/Toggle'
import type { ConfigSecaoProps } from './ConfigPerfil'

// Presets de qualidade padrão das salas, na ordem do design.
const PRESETS: { value: Preset; label: string }[] = [
  { value: '480p30', label: '480p · 30' },
  { value: '720p30', label: '720p · 30' },
  { value: '720p60', label: '720p · 60' },
  { value: '1080p60', label: '1080p · 60' },
]

// Atalhos fixos da tela. Não são configuráveis nesta versão.
const ATALHOS: { tecla: string; descricao: string }[] = [
  { tecla: 'F11', descricao: 'Tela cheia (ou duplo clique no vídeo)' },
  { tecla: 'Esc', descricao: 'Sair da tela cheia' },
  { tecla: 'M', descricao: 'Silenciar a sala toda' },
  { tecla: 'C', descricao: 'Ligar ou desligar sua câmera' },
]

// Faixa da suavização fixa, em milissegundos, conforme a spec.
const SMOOTH_MIN = 0
const SMOOTH_MAX = 1000
const SMOOTH_PASSO = 50

export function ConfigVideoSom({ settings, onChange, sources }: ConfigSecaoProps & { sources: Sources }): ReactNode {
  const video = settings.video
  const sound = settings.sound
  const suavizacaoDesligada = video.smoothing_auto

  const sons: { chave: 'message_sound' | 'notify_room_open' | 'mute_when_minimized'; label: string; hint: string }[] = [
    { chave: 'message_sound', label: 'Som de mensagem', hint: 'Aquele plim quando alguém manda algo no chat.' },
    { chave: 'notify_room_open', label: 'Avisar quando abrir sala', hint: 'Notificação do Windows com o nome da sala.' },
    { chave: 'mute_when_minimized', label: 'Silenciar ao minimizar', hint: 'O som volta quando você abrir a janela de novo.' },
  ]

  return (
    <div style={{ display: 'flex', flexWrap: 'wrap', gap: 20, alignItems: 'stretch' }}>
      <section
        aria-labelledby="h-video"
        style={{
          flex: '1 1 460px', minWidth: 0, display: 'flex', flexDirection: 'column', gap: 16,
          padding: 22, borderRadius: 18, background: '#0F1916', border: '1px solid #1F332D',
        }}
      >
        <h2 id="h-video" style={{ margin: 0, fontSize: 16, fontWeight: 700 }}>Vídeo e câmera</h2>

        <div role="group" aria-labelledby="cam-label" style={{ display: 'flex', flexDirection: 'column', gap: 10 }}>
          <span id="cam-label" style={{ fontSize: 12.5, color: '#A3BDB1' }}>Câmera</span>
          <Select
            options={[
              { value: '', label: 'Nenhuma' },
              ...sources.cameras.map((c) => ({ value: c.id, label: c.name })),
            ]}
            value={video.camera_id ?? ''}
            onChange={(v) => onChange({ video: { ...video, camera_id: v === '' ? null : v } })}
          />
          <Toggle
            checked={video.mirror_preview}
            label="Espelhar prévia"
            onChange={(mirror_preview) => onChange({ video: { ...video, mirror_preview } })}
          />
        </div>

        <div role="group" aria-labelledby="q-label" style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
          <span id="q-label" style={{ fontSize: 12.5, color: '#A3BDB1' }}>Qualidade padrão das suas salas</span>
          <Segmented
            options={PRESETS}
            value={video.default_preset}
            onChange={(v) => onChange({ video: { ...video, default_preset: v as Preset } })}
          />
        </div>

        <div
          style={{
            display: 'flex', flexDirection: 'column', gap: 10, padding: 14, borderRadius: 14,
            background: '#0A100E', border: '1px solid #1F332D',
          }}
        >
          <Toggle
            checked={video.smoothing_auto}
            label="Suavização automática"
            onChange={(smoothing_auto) => onChange({ video: { ...video, smoothing_auto } })}
          />
          <div
            role="group"
            aria-labelledby="smooth-label"
            style={{
              display: 'flex', alignItems: 'center', gap: 12,
              // Com a suavização automática, o slider fica apagado. O handler também não grava nada.
              opacity: suavizacaoDesligada ? 0.5 : 1,
            }}
          >
            <span id="smooth-label" style={{ flex: '0 0 96px', fontSize: 12.5, color: '#A3BDB1' }}>Suavização</span>
            <div style={{ flex: 1, minWidth: 0 }}>
              <Slider
                value={video.smoothing_ms}
                min={SMOOTH_MIN}
                max={SMOOTH_MAX}
                step={SMOOTH_PASSO}
                onChange={(smoothing_ms) => {
                  if (suavizacaoDesligada) return
                  onChange({ video: { ...video, smoothing_ms } })
                }}
              />
            </div>
            <span
              style={{
                flex: '0 0 72px', textAlign: 'right', fontFamily: "'JetBrains Mono', monospace",
                fontSize: 13, color: '#E6F4EC',
              }}
            >
              {`${video.smoothing_ms} ms`}
            </span>
          </div>
        </div>
      </section>

      <section
        aria-labelledby="h-som"
        style={{
          flex: '1 1 400px', minWidth: 0, display: 'flex', flexDirection: 'column', gap: 16,
          padding: 22, borderRadius: 18, background: '#0F1916', border: '1px solid #1F332D',
        }}
      >
        <h2 id="h-som" style={{ margin: 0, fontSize: 16, fontWeight: 700 }}>Som e avisos</h2>

        <div role="group" aria-labelledby="vol-label" style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
          <span id="vol-label" style={{ fontSize: 12.5, color: '#A3BDB1' }}>Volume inicial de cada transmissão</span>
          <div style={{ display: 'flex', alignItems: 'center', gap: 12 }}>
            <div style={{ flex: 1, minWidth: 0 }}>
              <Slider
                value={Math.round(sound.initial_volume * 100)}
                min={0}
                max={100}
                step={1}
                onChange={(v) => onChange({ sound: { ...sound, initial_volume: v / 100 } })}
              />
            </div>
            <span
              style={{
                flex: '0 0 44px', textAlign: 'right', fontFamily: "'JetBrains Mono', monospace", fontSize: 13,
              }}
            >
              {`${Math.round(sound.initial_volume * 100)}%`}
            </span>
          </div>
        </div>

        {sons.map((s) => (
          <div
            key={s.chave}
            style={{
              display: 'flex', flexDirection: 'column', gap: 6, padding: '12px 14px', borderRadius: 14,
              border: '1px solid #1F332D', background: '#0A100E',
            }}
          >
            <Toggle
              checked={sound[s.chave]}
              label={s.label}
              onChange={(v) => onChange({ sound: { ...sound, [s.chave]: v } })}
            />
            <span style={{ fontSize: 12, color: '#A3BDB1' }}>{s.hint}</span>
          </div>
        ))}

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
    </div>
  )
}
