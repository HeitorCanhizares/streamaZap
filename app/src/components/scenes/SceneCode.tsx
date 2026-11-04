// Tarefa previa-04
import type { ReactNode } from 'react'

// Paleta e geometria copiadas de docs/design/componente-previa.html (linhas 88-110).
export function SceneCode(): ReactNode {
  return (
    <>
      <rect width="160" height="90" fill="#0F1621" />
      <rect x="0" y="0" width="26" height="90" fill="#0B111A" />
      <rect x="26" y="0" width="134" height="9" fill="#0B111A" />
      <rect x="28" y="2" width="26" height="7" fill="#0F1621" />
      <g>
        <rect x="5" y="14" width="14" height="2" rx="1" fill="#2A443C" />
        <rect x="5" y="20" width="10" height="2" rx="1" fill="#2A443C" />
        <rect x="5" y="26" width="16" height="2" rx="1" fill="#3BF08A" opacity={0.7} />
        <rect x="5" y="32" width="12" height="2" rx="1" fill="#2A443C" />
      </g>
      <g>
        <rect x="34" y="16" width="20" height="2.4" rx="1" fill="#B79CFF" />
        <rect x="57" y="16" width="30" height="2.4" rx="1" fill="#5CD6FF" />
        <rect x="40" y="23" width="14" height="2.4" rx="1" fill="#FFB547" />
        <rect x="57" y="23" width="44" height="2.4" rx="1" fill="#A3BDB1" />
        <rect x="40" y="30" width="26" height="2.4" rx="1" fill="#3BF08A" />
        <rect x="69" y="30" width="18" height="2.4" rx="1" fill="#FF8FB1" />
        <rect x="46" y="37" width="38" height="2.4" rx="1" fill="#A3BDB1" />
        <rect x="46" y="44" width="22" height="2.4" rx="1" fill="#5CD6FF" />
        <rect x="71" y="44" width="34" height="2.4" rx="1" fill="#FFB547" />
        <rect x="40" y="51" width="16" height="2.4" rx="1" fill="#B79CFF" />
        <rect x="34" y="58" width="28" height="2.4" rx="1" fill="#5CD6FF" />
        <rect x="65" y="58" width="12" height="2.4" rx="1" fill="#A3BDB1" />
        <rect x="40" y="65" width="48" height="2.4" rx="1" fill="#3BF08A" />
      </g>
      <rect x="26" y="74" width="134" height="16" fill="#0B111A" />
      <text x="30" y="84" fontSize={4.4} fill="#3BF08A" fontFamily="JetBrains Mono, monospace">
        $ python main.py
      </text>
      <rect x="72" y="80.6" width="3" height="4.4" fill="#3BF08A" />
    </>
  )
}
