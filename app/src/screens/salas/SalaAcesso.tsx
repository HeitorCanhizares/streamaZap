// Tarefa salas-09: card de acesso (design 1-salas.html, linhas 188-221).
import type { CSSProperties, ReactNode } from 'react'
import { corPing } from './model'
import type { SalaVM } from './model'
import { IconeAviso, IconeCadeado, IconeCadeadoAberto, IconeSeta } from './icons'

export interface SalaAcessoProps { sala: SalaVM; senha: string; onSenha: (s: string) => void; ocupado: boolean; erro: string | null; onEntrar: () => void }

const MONO = "'JetBrains Mono', monospace"
const titulo: CSSProperties = { display: 'flex', alignItems: 'center', gap: 8, fontWeight: 600 }
const caixa: CSSProperties = { display: 'flex', flexDirection: 'column', gap: 3, padding: '10px 12px', borderRadius: 12, background: '#0A100E' }
const rotulo: CSSProperties = { fontSize: 11, color: '#7E978B', letterSpacing: '0.08em', textTransform: 'uppercase' }

export function SalaAcesso({ sala, senha, onSenha, ocupado, erro, onEntrar }: SalaAcessoProps): ReactNode {
  const trancada = sala.trancada && !sala.versaoAviso
  const aberta = !sala.trancada && !sala.versaoAviso
  const avisoVersao = sala.versaoAviso !== ''
  const openText = sala.modo === 'online'
    ? 'Qualquer pessoa com o app entra pelo servidor, de qualquer lugar.'
    : 'Qualquer pessoa na mesma rede do Radmin entra direto, sem senha.'

  return (
    <div style={{ flex: '1 1 260px', maxWidth: 340, display: 'flex', flexDirection: 'column', gap: 14, padding: 22, borderRadius: 18, background: '#0F1916', border: '1px solid #1F332D' }}>
      {trancada && (
        <>
          <span style={titulo}><IconeCadeado size={18} cor="#FFB547" />Sala com senha</span>
          <div style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
            <label htmlFor="senha" style={{ fontSize: 12.5, color: '#A3BDB1' }}>Senha da sala</label>
            <input
              id="senha"
              type="password"
              placeholder="Digite a senha"
              value={senha}
              onChange={(e) => onSenha(e.target.value)}
              onKeyDown={(e) => { if (e.key === 'Enter') onEntrar() }}
              style={{ height: 46, padding: '0 14px', borderRadius: 12, border: '1px solid #2A443C', background: '#0A100E', color: '#E6F4EC', fontSize: 15, letterSpacing: '0.1em' }}
            />
            <span style={{ fontSize: 12, color: '#7E978B' }}>Não sabe? Pede pro {sala.host} no zap.</span>
          </div>
        </>
      )}
      {aberta && (
        <>
          <span style={titulo}><IconeCadeadoAberto size={18} cor="#3BF08A" />Sala aberta</span>
          <span style={{ color: '#A3BDB1', lineHeight: 1.5 }}>{openText}</span>
        </>
      )}
      {avisoVersao && (
        <span style={{ display: 'flex', gap: 10, padding: 12, borderRadius: 12, background: 'rgba(255,181,71,0.1)', color: '#F4E3C4', lineHeight: 1.45, fontSize: 13 }}>
          <span style={{ flex: '0 0 18px', marginTop: 1, display: 'flex' }}><IconeAviso size={18} cor="#FFB547" /></span>
          <span>Essa sala está na versão {sala.versaoAviso}. Todo mundo precisa estar na mesma versão para entrar.</span>
        </span>
      )}
      {!avisoVersao && (
        <button
          type="button"
          className="zp-cta"
          disabled={ocupado}
          onClick={onEntrar}
          style={{ height: 52, borderRadius: 14, border: 'none', background: '#3BF08A', color: '#03140A', fontWeight: 700, fontSize: 16, display: 'flex', alignItems: 'center', justifyContent: 'center', gap: 10, boxShadow: '0 0 24px rgba(59,240,138,0.25)', cursor: ocupado ? 'not-allowed' : 'pointer', opacity: ocupado ? 0.6 : 1 }}
        >
          Entrar na sala<IconeSeta size={20} />
        </button>
      )}
      {avisoVersao && (
        <button
          type="button"
          disabled
          style={{ height: 52, borderRadius: 14, border: '1px solid #2A443C', background: '#142320', color: '#A3BDB1', fontWeight: 600, cursor: 'not-allowed' }}
        >
          Atualize para entrar
        </button>
      )}
      {erro !== null && <p role="alert" style={{ margin: 0, color: '#FF5D6E', fontSize: 13 }}>{erro}</p>}
      <div style={{ display: 'grid', gridTemplateColumns: 'repeat(2, minmax(0, 1fr))', gap: 10, marginTop: 4 }}>
        <div style={caixa}>
          <span style={rotulo}>Ping</span>
          <span style={{ fontFamily: MONO, fontSize: 16, fontWeight: 600, color: corPing(sala.ping) }}>{sala.ping} ms</span>
        </div>
        <div style={caixa}>
          <span style={rotulo}>Suavização</span>
          <span style={{ fontFamily: MONO, fontSize: 16, fontWeight: 600 }}>auto</span>
        </div>
      </div>
    </div>
  )
}
