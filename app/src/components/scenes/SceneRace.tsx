// Tarefa previa-02
import type { ReactNode } from 'react'

export function SceneRace(): ReactNode {
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
