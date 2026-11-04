// Tarefa ui-05
import type { ReactNode } from 'react'

export interface ScenePreviewProps { scene: 'game' | 'race' | 'movie' | 'code' }

// Miniaturas 16:9 das cenas, desenhadas em SVG (copiadas de docs/design/componente-previa.html).
// aria-hidden: são decorativas, o texto ao redor já descreve a transmissão.
export function ScenePreview({ scene }: ScenePreviewProps): ReactNode {
  return (
    <svg viewBox="0 0 160 90" preserveAspectRatio="xMidYMid slice" style={{ display: 'block', width: '100%', height: '100%' }} aria-hidden="true">
      {scene === 'game' && <Game />}
      {scene === 'race' && <Race />}
      {scene === 'movie' && <Movie />}
      {scene === 'code' && <Code />}
    </svg>
  )
}

function Game(): ReactNode {
  return (
    <>
      <rect width="160" height="90" fill="#1B2A3A" />
      <polygon points="0,60 26,38 44,50 70,24 98,48 120,32 160,56 160,90 0,90" fill="#243B4C" />
      <polygon points="0,72 40,56 80,68 120,54 160,70 160,90 0,90" fill="#2F4A3E" />
      <rect x="18" y="52" width="14" height="22" fill="#20332C" />
      <rect x="128" y="48" width="18" height="26" fill="#20332C" />
      <rect x="0" y="80" width="160" height="10" fill="#1A2B24" />
      <polygon points="108,90 120,70 138,64 152,70 148,90" fill="#0E1820" />
      <g stroke="#E6F4EC" strokeWidth={0.8} strokeLinecap="round">
        <line x1="80" y1="40" x2="80" y2="43" />
        <line x1="80" y1="49" x2="80" y2="52" />
        <line x1="74" y1="46" x2="77" y2="46" />
        <line x1="83" y1="46" x2="86" y2="46" />
      </g>
      <rect x="6" y="80" width="32" height="3" rx="1.5" fill="#0A100E" opacity={0.6} />
      <rect x="6" y="80" width="23" height="3" rx="1.5" fill="#3BF08A" />
      <circle cx="148" cy="12" r="8" fill="#0A100E" opacity={0.55} stroke="#5CD6FF" strokeWidth={0.6} />
      <circle cx="148" cy="12" r="1.3" fill="#3BF08A" />
      <circle cx="151" cy="9" r="0.9" fill="#FF5D6E" />
      <text x="104" y="84" fontSize={6} fill="#E6F4EC" fontFamily="JetBrains Mono, monospace">24/90</text>
    </>
  )
}

function Race(): ReactNode {
  return (
    <>
      <rect width="160" height="90" fill="#2A2140" />
      <circle cx="120" cy="30" r="12" fill="#FF9E5C" />
      <polygon points="0,50 30,36 60,46 96,32 130,44 160,38 160,90 0,90" fill="#3A2B52" />
      <rect x="0" y="48" width="160" height="42" fill="#1D2A22" />
      <polygon points="66,48 94,48 152,90 8,90" fill="#2B2F36" />
      <polygon points="66,48 70,48 18,90 8,90" fill="#FF5D6E" opacity={0.7} />
      <polygon points="90,48 94,48 152,90 142,90" fill="#FF5D6E" opacity={0.7} />
      <g fill="#E6F4EC">
        <rect x="79.4" y="51" width="1.2" height="4" />
        <rect x="79.2" y="60" width="1.6" height="6" />
        <rect x="79" y="72" width="2" height="9" />
      </g>
      <rect x="64" y="64" width="32" height="14" rx="3" fill="#5CD6FF" />
      <rect x="68" y="66" width="24" height="4" rx="1" fill="#0A100E" opacity={0.6} />
      <rect x="66" y="76" width="6" height="4" rx="1" fill="#0E1820" />
      <rect x="88" y="76" width="6" height="4" rx="1" fill="#0E1820" />
      <text x="152" y="82" textAnchor="end" fontSize={11} fontWeight={600} fill="#E6F4EC" fontFamily="JetBrains Mono, monospace">212</text>
      <text x="152" y="87" textAnchor="end" fontSize={4} fill="#A3BDB1" fontFamily="JetBrains Mono, monospace">KM/H</text>
    </>
  )
}

function Movie(): ReactNode {
  return (
    <>
      <rect width="160" height="90" fill="#10233A" />
      <circle cx="42" cy="30" r="9" fill="#F2E86D" opacity={0.9} />
      <g fill="#0B1828">
        <rect x="0" y="52" width="18" height="26" />
        <rect x="20" y="44" width="12" height="34" />
        <rect x="34" y="56" width="20" height="22" />
        <rect x="58" y="38" width="10" height="40" />
        <rect x="70" y="50" width="24" height="28" />
        <rect x="98" y="42" width="14" height="36" />
        <rect x="114" y="54" width="20" height="24" />
        <rect x="138" y="46" width="22" height="32" />
      </g>
      <g fill="#F2E86D" opacity={0.55}>
        <rect x="23" y="48" width="2" height="2" />
        <rect x="61" y="44" width="2" height="2" />
        <rect x="61" y="52" width="2" height="2" />
        <rect x="102" y="48" width="2" height="2" />
        <rect x="142" y="52" width="2" height="2" />
      </g>
      <rect x="0" y="0" width="160" height="11" fill="#000000" />
      <rect x="0" y="78" width="160" height="12" fill="#000000" />
      <text x="80" y="74" textAnchor="middle" fontSize={5} fill="#FFFFFF" fontFamily="Space Grotesk, sans-serif">— Você viu aquilo lá em cima?</text>
    </>
  )
}

// Cena de editor de código: fundo escuro, barra lateral, abas e linhas coloridas
function Code(): ReactNode {
  return (
    <>
      <rect width="160" height="90" fill="#0F1916" />
      <rect x="0" y="0" width="26" height="90" fill="#0A100E" />
      <rect x="26" y="0" width="134" height="9" fill="#0A100E" />
      <rect x="28" y="2" width="26" height="7" fill="#0F1916" />
      <g fill="#2A443C">
        <rect x="5" y="14" width="14" height="2" rx="1" />
        <rect x="5" y="20" width="10" height="2" rx="1" />
        <rect x="5" y="32" width="12" height="2" rx="1" />
      </g>
      <rect x="5" y="26" width="16" height="2" rx="1" fill="#3BF08A" opacity={0.7} />
      <rect x="34" y="16" width="20" height="2.4" rx="1" fill="#5CD6FF" />
      <rect x="57" y="16" width="30" height="2.4" rx="1" fill="#3BF08A" />
      <rect x="40" y="23" width="14" height="2.4" rx="1" fill="#FFE37A" />
      <rect x="57" y="23" width="44" height="2.4" rx="1" fill="#A3BDB1" />
      <rect x="40" y="30" width="26" height="2.4" rx="1" fill="#5CD6FF" />
      <rect x="69" y="30" width="18" height="2.4" rx="1" fill="#FFE37A" />
      <rect x="46" y="37" width="38" height="2.4" rx="1" fill="#A3BDB1" />
      <rect x="46" y="44" width="22" height="2.4" rx="1" fill="#3BF08A" />
      <rect x="71" y="44" width="34" height="2.4" rx="1" fill="#FFE37A" />
      <rect x="40" y="51" width="16" height="2.4" rx="1" fill="#5CD6FF" />
      <rect x="34" y="58" width="28" height="2.4" rx="1" fill="#3BF08A" />
      <rect x="65" y="58" width="12" height="2.4" rx="1" fill="#A3BDB1" />
      <rect x="40" y="65" width="48" height="2.4" rx="1" fill="#FFE37A" />
      <rect x="26" y="74" width="134" height="16" fill="#0A100E" />
      <text x="30" y="84" fontSize={4.4} fill="#3BF08A" fontFamily="JetBrains Mono, monospace">$ python main.py</text>
      <rect x="72" y="80.6" width="3" height="4.4" fill="#3BF08A" />
    </>
  )
}
