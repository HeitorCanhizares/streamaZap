// Tarefa previa-01
import type { ReactNode } from 'react'

export function SceneGame(): ReactNode {
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
