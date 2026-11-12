// Список «Где играть» без старых записей: переустановленное приложение регистрируется заново, и
// прежнее «iPhone · Не в сети» висит рядом с новым. Из записей с тем же именем и видом остаётся одна —
// это устройство, иначе то, что в сети, иначе последнее увиденное. Активное не прячется никогда.
import type { PlaybackDevice } from '~/bridge/types'

type DeviceLike = Pick<PlaybackDevice, 'id' | 'name' | 'kind' | 'current' | 'online' | 'lastSeenAt' | 'active'>

const seen = (d: DeviceLike) => (d.lastSeenAt ? Date.parse(d.lastSeenAt) : 0)

function better(a: DeviceLike, b: DeviceLike): boolean {
  if (a.current !== b.current) return a.current
  if (a.online !== b.online) return a.online
  return seen(a) > seen(b)
}

export function withoutStale<D extends DeviceLike>(devices: D[]): D[] {
  const keep = new Map<string, D>()
  for (const d of devices) {
    const key = `${d.kind}\u0000${d.name.trim().toLowerCase()}`
    const prev = keep.get(key)
    if (!prev || better(d, prev)) keep.set(key, d)
  }
  const kept = new Set(keep.values())
  return devices.filter(d => kept.has(d) || d.active)
}
