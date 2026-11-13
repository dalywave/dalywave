// Проверка на живом сервере: пульт плагина и поддельное «играющее» устройство на одном аккаунте.
// Плагин шлёт команды, устройство их получает, отвечает и публикует снимок — плагин видит новое состояние.
//   DW_API=http://localhost:8475 DW_TOKEN_FILE=<файл с dwd_…> bun test/live/connect.live.ts
import { readFileSync } from 'node:fs'
import WebSocket from 'ws'
import { api } from '../../src/api'
import { Player } from '../../src/player'
import type { PlaybackState, RemoteCommand, Track } from '../../src/types'

const server = process.env.DW_API ?? 'http://localhost:8475'
const token = readFileSync(process.env.DW_TOKEN_FILE ?? '', 'utf8').trim()
api.configure(server, token)

const liked = await api.get<{ items: { track: Track }[] }>('/api/library/tracks?limit=3')
const tracks = liked.items.map(i => i.track)
if (!tracks.length) throw new Error('в «Любимых» пусто')

// ---------- поддельное устройство, где «играет» музыка ----------
const got: RemoteCommand[] = []
let state: PlaybackState = {
  current: { uid: 'u0', track: tracks[0]!, source: 'context' },
  upcoming: tracks.slice(1).map((t, i) => ({ uid: `u${i + 1}`, track: t, source: 'context' as const })),
  playing: true, positionMs: 30_000, durationMs: tracks[0]!.durationMs, volume: 0.5, muted: false,
  transition: 'smart', shuffle: false, repeat: 'off',
}
const dev = new WebSocket(`${server.replace(/^http/, 'ws')}/api/ws`, { headers: { Authorization: `Bearer ${token}` } })
const publish = () => dev.send(JSON.stringify({ type: 'connect.state', state }))
await new Promise<void>(r => dev.on('open', () => r()))
dev.send(JSON.stringify({ type: 'connect.hello', deviceId: 'fake-mac', name: 'Проверочный Mac', kind: 'laptop', platform: 'macos', capabilities: { volume: true, dj: true } }))
dev.send(JSON.stringify({ type: 'connect.claim' }))
setTimeout(publish, 300)
dev.on('message', (data) => {
  const m = JSON.parse(String(data)) as { type: string, id: string, from: string, command: RemoteCommand }
  if (m.type !== 'connect.command') return
  got.push(m.command)
  const c = m.command
  if (c.action === 'next') state = { ...state, current: state.upcoming![0]!, upcoming: state.upcoming!.slice(1), positionMs: 0 }
  if (c.action === 'toggle' || c.action === 'pause') state = { ...state, playing: false }
  if (c.action === 'play') state = { ...state, playing: true }
  if (c.action === 'setVolume') state = { ...state, volume: c.volume }
  if (c.action === 'setRepeat') state = { ...state, repeat: c.mode }
  publish()
  dev.send(JSON.stringify({ type: 'connect.ack', id: m.id, to: m.from, ok: true }))
})

// ---------- плагин ----------
const player = new Player()
player.paired = true
player.link.configure({ server, token, deviceId: 'streamdeck-live-test', name: 'Stream Deck · проверка', version: '0.0.0' })

async function until(what: string, ok: () => boolean, ms = 8000): Promise<void> {
  const end = Date.now() + ms
  while (!ok()) {
    if (Date.now() > end) throw new Error(`не дождались: ${what}`)
    await new Promise(r => setTimeout(r, 100))
  }
  console.log(`✓ ${what}`)
}

await until('плагин на связи', () => player.connection === 'online')
await until('активное — проверочный Mac', () => player.activeDevice?.id === 'fake-mac')
await until('сейчас играет — из снимка', () => player.track?.id === tracks[0]!.id && player.playing)
const self = player.devices.find(d => d.id === 'streamdeck-live-test')
console.log(`  пульт в «Где играть»: ${self?.name} · ${self?.platform} · remote=${self?.capabilities.remote}`)
if (self?.platform !== 'streamdeck' || !self.capabilities.remote) throw new Error('пульт не той платформы')

console.log('  следующий:', await player.next())
await until('трек сменился', () => player.track?.id === tracks[1]!.id)
console.log('  громкость 30 %:', await player.setVolume(0.3))
await until('громкость 0,3 в снимке', () => player.state?.volume === 0.3)
console.log('  повтор:', await player.cycleRepeat())
await until('повтор «все»', () => player.state?.repeat === 'all')
console.log('  пауза:', await player.toggle())
await until('на паузе', () => !player.playing)
console.log(`  команды устройству: ${got.map(c => c.action).join(', ')}`)
console.log(`  «нравится» текущего: ${player.liked()}`)

player.stop()
dev.close()
console.log('готово')
process.exit(0)
