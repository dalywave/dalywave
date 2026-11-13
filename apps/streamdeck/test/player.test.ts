import { beforeEach, describe, expect, test } from 'bun:test'
import { Player } from '../src/player'
import type { ConnectAck, ConnectDevice, PlaybackState, RemoteCommand } from '../src/types'
import { waveTitle } from '../src/catalog'
import { setLanguage } from '../src/i18n'

function device(id: string, extra: Partial<ConnectDevice> = {}): ConnectDevice {
  return {
    id,
    name: id,
    kind: 'laptop',
    platform: 'macos',
    appVersion: null,
    capabilities: { volume: true, dj: true },
    online: true,
    active: false,
    lastSeenAt: new Date().toISOString(),
    ...extra,
  }
}

const track = { id: 't1', title: 'Трек', liked: false, artists: [], durationMs: 200_000 } as unknown as NonNullable<PlaybackState['current']>['track']

let player: Player
let sent: { to: string, command: RemoteCommand }[]
let transfers: { to: string, play?: boolean }[]

beforeEach(() => {
  player = new Player()
  player.paired = true
  sent = []
  transfers = []
  player.link.command = async (to: string, command: RemoteCommand): Promise<ConnectAck> => {
    sent.push({ to, command })
    return { id: 'x', ok: true }
  }
  player.link.transfer = async (to: string, play?: boolean): Promise<ConnectAck> => {
    transfers.push({ to, play })
    return { id: 'x', ok: true }
  }
})

describe('куда слать команды', () => {
  test('активному устройству, если оно на связи', async () => {
    player.link.devices = [device('mac'), device('web', { kind: 'web' })]
    player.link.activeId = 'web'
    await player.next()
    expect(sent).toEqual([{ to: 'web', command: { action: 'next' } }])
  })

  test('активное пропало — недавнему на связи, но не пульту', async () => {
    const old = new Date(Date.now() - 60_000).toISOString()
    player.link.devices = [
      device('phone', { online: false }),
      device('watch', { kind: 'watch', capabilities: { remote: true } }),
      device('mac', { lastSeenAt: old }),
      device('pc', { kind: 'desktop' }),
    ]
    player.link.activeId = 'phone'
    expect(player.target?.id).toBe('pc')
  })

  test('«играть» без активного, но со снимком — перенос на устройство рядом со звуком', async () => {
    player.link.devices = [device('phone', { online: false }), device('mac')]
    player.link.activeId = 'phone'
    player.link.state = { current: { uid: 'u1', track, source: 'manual' }, playing: false } as PlaybackState
    await player.toggle()
    expect(transfers).toEqual([{ to: 'mac', play: true }])
    expect(sent).toEqual([])
  })

  test('нет ни одного устройства — команда не уходит', async () => {
    player.link.devices = [device('watch', { capabilities: { remote: true } })]
    expect(await player.pause()).toBe(false)
    expect(sent).toEqual([])
  })
})

describe('режимы', () => {
  beforeEach(() => {
    player.link.devices = [device('mac')]
    player.link.activeId = 'mac'
    player.link.state = { repeat: 'off', transition: 'smart', volume: 0.5 } as PlaybackState
  })

  test('повтор по кругу: выкл → все → один', async () => {
    await player.cycleRepeat()
    expect(sent.at(-1)?.command).toEqual({ action: 'setRepeat', mode: 'all' })
    expect(player.repeat).toBe('all')
    await player.cycleRepeat()
    expect(sent.at(-1)?.command).toEqual({ action: 'setRepeat', mode: 'one' })
  })

  test('переходы по кругу, «Диджей» — только где он есть', async () => {
    await player.cycleTransition()
    expect(sent.at(-1)?.command).toEqual({ action: 'setTransition', mode: 'dj' })
  })

  test('без «Диджея» после «Умного» — снова «Без пауз»', async () => {
    player.link.devices = [device('mac', { capabilities: { volume: true, dj: false } })]
    await player.cycleTransition()
    expect(sent.at(-1)?.command).toEqual({ action: 'setTransition', mode: 'gapless' })
  })

  test('громкость в пределах 0…1, сразу видна на кнопках', async () => {
    await player.adjustVolume(0.7)
    expect(sent.at(-1)?.command).toEqual({ action: 'setVolume', volume: 1 })
    expect(player.volume).toBe(1)
    await player.setVolume(-3)
    expect(player.volume).toBe(0)
  })

  test('перемотка не уходит за конец трека', async () => {
    player.link.state = { positionMs: 190_000, durationMs: 200_000, playing: false } as PlaybackState
    await player.seekBy(30_000)
    expect(sent.at(-1)?.command).toEqual({ action: 'seek', positionMs: 199_500 })
    await player.seekBy(-500_000)
    expect(sent.at(-1)?.command).toEqual({ action: 'seek', positionMs: 0 })
  })

  test('длинный список — окно вокруг выбранного трека', async () => {
    const many = Array.from({ length: 500 }, (_, i) => ({ ...track, id: `t${i}` }))
    await player.playTracks(many, 450, { type: 'library', id: 'liked', title: 'Любимые' })
    const c = sent.at(-1)?.command as { trackIds: string[], index: number }
    expect(c.trackIds.length).toBe(300)
    expect(c.trackIds[c.index]).toBe('t450')
  })
})

describe('позиция', () => {
  test('пока играет — идёт по часам сервера, на паузе — стоит', () => {
    player.link.offset = 0
    const at = new Date(Date.now() - 2000).toISOString()
    player.link.state = { positionMs: 10_000, durationMs: 100_000, playing: true, at, current: { uid: 'u', track, source: 'manual' } } as PlaybackState
    expect(player.position()).toBeGreaterThanOrEqual(11_900)
    expect(player.position()).toBeLessThan(12_500)
    player.link.state = { ...player.link.state, playing: false }
    expect(player.position()).toBe(10_000)
  })
})

describe('названия волны', () => {
  test('занятие и настроение, иначе «Моя волна»', () => {
    setLanguage('ru')
    expect(waveTitle({ activity: 'work', mood: 'calm' })).toBe('Работа · Спокойное')
    expect(waveTitle({})).toBe('Моя волна')
    setLanguage('en')
    expect(waveTitle({ mood: 'sad', language: 'none' })).toBe('Sad · Instrumental')
    setLanguage('ru')
  })
})
