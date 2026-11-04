// Tarefa previa-03
import type { ReactNode } from 'react'

export function SceneMovie(): ReactNode {
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
