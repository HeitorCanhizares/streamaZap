// Tarefa previa-05
import type { ReactNode } from 'react'

// Paleta e geometria copiadas de docs/design/componente-previa.html (linhas 115-131).
export function SceneDesktop(): ReactNode {
  return (
    <>
      <rect width="160" height="90" fill="#14324A" />
      <circle cx="128" cy="24" r="22" fill="#18405C" />
      <rect x="14" y="12" width="78" height="52" rx="3" fill="#E6F4EC" />
      <rect x="14" y="12" width="78" height="7" rx="3" fill="#C9DAD1" />
      <rect x="20" y="25" width="40" height="3" rx="1" fill="#7E978B" />
      <rect x="20" y="32" width="64" height="20" rx="2" fill="#2F4A3E" />
      <circle cx="34" cy="42" r="5" fill="#3BF08A" />
      <rect x="44" y="39" width="30" height="2.4" rx="1" fill="#A3BDB1" />
      <rect x="44" y="44" width="20" height="2.4" rx="1" fill="#A3BDB1" />
      <rect x="100" y="40" width="48" height="32" rx="3" fill="#0F1916" />
      <rect x="100" y="40" width="48" height="6" rx="3" fill="#1F332D" />
      <rect x="105" y="51" width="26" height="2.4" rx="1" fill="#3BF08A" />
      <rect x="105" y="57" width="36" height="2.4" rx="1" fill="#5CD6FF" />
      <rect x="0" y="82" width="160" height="8" fill="#0A100E" opacity={0.85} />
      <rect x="74" y="84" width="4" height="4" rx="1" fill="#3BF08A" />
      <rect x="80" y="84" width="4" height="4" rx="1" fill="#5CD6FF" />
      <rect x="86" y="84" width="4" height="4" rx="1" fill="#FFB547" />
    </>
  )
}
