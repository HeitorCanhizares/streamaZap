import type { ReactNode } from 'react'
import { ACESSOS, type Acesso } from './criarModel'

export interface CriarNomeAcessoProps { nome: string; onNome: (v: string) => void; acesso: Acesso; onAcesso: (a: Acesso) => void; senha: string; onSenha: (v: string) => void }

// Seções 02 "Nome da sala" e 03 "Quem pode entrar" lado a lado (2-criar-sala.html 87-108).

const MONO = "'JetBrains Mono', monospace"

export function CriarNomeAcesso({ nome, onNome, acesso, onAcesso, senha, onSenha }: CriarNomeAcessoProps): ReactNode {
  const bloco = { display: 'flex', flexDirection: 'column' as const, gap: 10, padding: 20, borderRadius: 18, background: 'var(--superficie)', border: '1px solid var(--borda)' }
  return (
    <div style={{ display: 'flex', flexWrap: 'wrap', gap: 18 }}>
      <section style={{ ...bloco, flex: '1 1 280px', minWidth: 0 }}>
        <label htmlFor="nome" style={{ display: 'flex', alignItems: 'baseline', gap: 10, fontWeight: 600 }}>
          <span style={{ fontFamily: MONO, fontSize: 11, color: 'var(--verde)' }}>02</span>
          Nome da sala
        </label>
        <input
          id="nome"
          value={nome}
          onChange={(e) => onNome(e.target.value)}
          style={{
            height: 46, padding: '0 14px', borderRadius: 12, border: '1px solid var(--borda-forte)',
            background: 'var(--bg)', color: 'var(--texto)', fontSize: 15, fontWeight: 600,
          }}
        />
        <span style={{ fontSize: 12, color: 'var(--texto-3)' }}>É o nome que a galera vê na lista.</span>
      </section>

      <section style={{ ...bloco, flex: '1 1 340px', minWidth: 0 }}>
        <h2 style={{ margin: 0, display: 'flex', alignItems: 'baseline', gap: 10, fontSize: 14, fontWeight: 600 }}>
          <span style={{ fontFamily: MONO, fontSize: 11, color: 'var(--verde)' }}>03</span>
          Quem pode entrar
        </h2>
        <div role="group" aria-label="Acesso" style={{ display: 'grid', gridTemplateColumns: 'repeat(2, minmax(0, 1fr))', gap: 10 }}>
          {ACESSOS.map((info) => {
            const ativo = acesso === info.value
            return (
              <button
                key={info.value}
                type="button"
                className="zp-card"
                aria-pressed={ativo}
                onClick={() => onAcesso(info.value)}
                style={{
                  display: 'flex', flexDirection: 'column', alignItems: 'flex-start', gap: 4,
                  padding: '12px 14px', borderRadius: 14, textAlign: 'left',
                  background: ativo ? 'rgba(59,240,138,0.08)' : 'var(--bg)',
                  border: `1px solid ${ativo ? 'var(--verde)' : 'var(--borda)'}`,
                  color: 'inherit', font: 'inherit', cursor: 'pointer',
                }}
              >
                <span style={{ fontWeight: 700, fontSize: 14 }}>{info.label}</span>
                <span style={{ fontSize: 12, color: 'var(--texto-2)', lineHeight: 1.35 }}>{info.hint}</span>
              </button>
            )
          })}
        </div>
        {acesso === 'senha' && (
          <>
            <label
              htmlFor="senha"
              style={{ position: 'absolute', width: 1, height: 1, padding: 0, margin: -1, overflow: 'hidden', clip: 'rect(0 0 0 0)', border: 0, whiteSpace: 'nowrap' }}
            >
              Senha da sala
            </label>
            <input
              id="senha"
              type="password"
              placeholder="Crie uma senha"
              value={senha}
              onChange={(e) => onSenha(e.target.value)}
              style={{
                height: 44, padding: '0 14px', borderRadius: 12, border: '1px solid var(--borda-forte)',
                background: 'var(--bg)', color: 'var(--texto)', fontSize: 15, letterSpacing: '0.1em',
              }}
            />
          </>
        )}
      </section>
    </div>
  )
}
