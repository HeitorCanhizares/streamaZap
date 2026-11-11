// Tarefa telas-13: ConfigVideo só informa o plano exigido no Online, sem travar.
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { act, cleanup, fireEvent, render, screen, within } from '@testing-library/react'

vi.mock('../../video/player', () => ({
  VideoPlayer: class { push() {} close() {} },
}))

import { ConfigVideo } from './ConfigVideo'
import { mockSettings, mockSources } from '../../bridge/mock'
import { resetOnlineEnabled, setOnlineEnabled } from '../../bridge/onlineStore'
import { getSnapshot, resetSnapshotForTests, setSnapshot } from '../../bridge/store'
import type { QualityLocks } from '../../bridge/types'

const HD: QualityLocks = {
  can_publish: true,
  screen: [{ preset: '1080p30', needs: 'fullhd' }, { preset: '1080p60', needs: 'fullhd' }],
  camera: [{ preset: '1080p30', needs: 'fullhd' }, { preset: '1080p60', needs: 'fullhd' }],
}

const tela = () => within(screen.getByRole('radiogroup', { name: 'Qualidade padrão das suas salas' }))
const FRASE = 'No Online, algumas qualidades da câmera dependem do seu plano.'

function aplicar(quality_locks: QualityLocks | undefined, online: boolean) {
  setOnlineEnabled(online)
  act(() => {
    setSnapshot({ ...getSnapshot(), version: getSnapshot().version + 1, quality_locks })
  })
}

beforeEach(() => { resetSnapshotForTests(); resetOnlineEnabled() })
afterEach(() => { cleanup(); resetSnapshotForTests(); resetOnlineEnabled() })

describe('ConfigVideo com planos', () => {
  it('mostra o plano nas opções travadas e continua escolhível', () => {
    aplicar(HD, true)
    const onChange = vi.fn()
    const s = mockSettings()
    render(<ConfigVideo settings={s} onChange={onChange} sources={mockSources()} />)
    const r = tela().getAllByRole('radio')
    expect(within(r[0]).queryByText(/Online: /)).toBeNull()
    expect(within(r[1]).queryByText(/Online: /)).toBeNull()
    expect(within(r[2]).getByText('Online: Full HD')).toBeTruthy()
    expect(within(r[3]).getByText('Online: Full HD')).toBeTruthy()
    fireEvent.click(r[3])
    expect(onChange).toHaveBeenCalledWith({ video: { ...s.video, default_preset: '1080p60' } })
    expect(screen.getByText(FRASE)).toBeTruthy()
  })

  it('sem quality_locks ou com Online desligado nada aparece', () => {
    aplicar(undefined, true)
    const { unmount } = render(<ConfigVideo settings={mockSettings()} onChange={vi.fn()} sources={mockSources()} />)
    expect(screen.queryByText(/Online: /)).toBeNull()
    expect(screen.queryByText(FRASE)).toBeNull()
    unmount()
    aplicar(HD, false)
    render(<ConfigVideo settings={mockSettings()} onChange={vi.fn()} sources={mockSources()} />)
    expect(screen.queryByText(/Online: /)).toBeNull()
    expect(screen.queryByText(FRASE)).toBeNull()
  })
})
