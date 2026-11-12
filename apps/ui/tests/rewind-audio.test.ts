// Музыка историй «Итогов года»: расписание треков, наложение, приглушение — без звука: bun test (из apps/ui)
import { describe, expect, test } from 'bun:test'
import type { Track } from '../app/api/types'
import type { SourceResult } from '../app/bridge/types'
import {
  REWIND_DUCK, REWIND_FADE_IN_MS, REWIND_FADE_MS, REWIND_STEP_MS, RewindAudio, type RewindAudioPort, type RewindCueLike,
  gainRamp, rewindDip, rewindGain, rewindNearestReady, rewindPreload, rewindStep, rewindTimeline,
} from '../app/utils/rewind-audio'

const track = (id: string, audioStatus = 'ready') => ({ id, title: id, durationMs: 200_000, audioStatus }) as Track
const cue = (slide: string, id: string, startMs = 60_000, status = 'ready'): RewindCueLike => ({ slide, track: track(id, status), startMs })
const SLIDES = ['intro', 'minutes', 'days', 'tracks', 'toy', 'summary']

describe('rewindTimeline', () => {
  test('slides without their own track keep the previous one; the first slides take the first track', () => {
    const tl = rewindTimeline(SLIDES, [cue('minutes', 'a'), cue('toy', 'b'), cue('summary', 'b')])
    expect(tl.map(c => c?.track.id ?? null)).toEqual(['a', 'a', 'a', 'a', 'b', 'b'])
    expect(rewindTimeline(SLIDES, []).every(c => c === null)).toBe(true)
  })
})

describe('rewindStep and rewindPreload', () => {
  test('same track keeps playing, a new one switches, silence starts', () => {
    expect(rewindStep('a', cue('x', 'a'))).toBe('keep')
    expect(rewindStep('a', cue('x', 'b'))).toBe('switch')
    expect(rewindStep(null, cue('x', 'b'))).toBe('start')
    expect(rewindStep('a', null)).toBe('keep')
  })
  test('preloads the nearest later slide with a different track', () => {
    const tl = rewindTimeline(SLIDES, [cue('intro', 'a'), cue('toy', 'b')])
    expect(rewindPreload(tl, 0, 'a')?.track.id).toBe('b')
    expect(rewindPreload(tl, 4, 'b')).toBeNull()
  })
})

describe('rewindNearestReady', () => {
  test('looks forward first, then back', () => {
    const tl = rewindTimeline(SLIDES, [cue('intro', 'a'), cue('days', 'b'), cue('toy', 'c')])
    const ready = (ids: string[]) => (id: string) => ids.includes(id)
    expect(rewindNearestReady(tl, 0, ready(['c']))?.track.id).toBe('c')
    expect(rewindNearestReady(tl, 4, ready(['a']))?.track.id).toBe('a')
    expect(rewindNearestReady(tl, 0, ready([]))).toBeNull()
  })
})

describe('gain', () => {
  test('mute wins, pause ducks', () => {
    expect(rewindGain({ muted: false, ducked: false })).toBe(1)
    expect(rewindGain({ muted: false, ducked: true })).toBe(REWIND_DUCK)
    expect(rewindGain({ muted: true, ducked: true })).toBe(0)
  })
  test('ramps end exactly on target, monotonic, one value per step', () => {
    const up = gainRamp(0, 1, 900)
    expect(up.length).toBe(900 / REWIND_STEP_MS)
    expect(up.at(-1)).toBe(1)
    expect(up.every((v, i) => i === 0 || v >= up[i - 1]!)).toBe(true)
    const down = gainRamp(1, 0.3, 250)
    expect(down.at(-1)).toBeCloseTo(0.3, 10)
    expect(down.every((v, i) => i === 0 || v <= down[i - 1]!)).toBe(true)
  })
  test('the app dip takes the whole crossfade time', () => {
    const { outMs, inMs } = rewindDip()
    expect(outMs + inMs).toBe(REWIND_FADE_MS)
    expect(outMs).toBeLessThan(inMs)
  })
})

// ---------- Контроллер на заглушках: часы, сервер и плеер ----------

interface Call { op: string, id?: string, startMs?: number, fadeMs?: number }

function harness(opts: { native?: boolean, pending?: string[] } = {}) {
  const pending = new Set(opts.pending ?? [])
  const calls: Call[] = []
  const gains: number[] = []
  let now = 0
  let timers: { at: number, fn: () => void, id: number }[] = []
  let nextId = 1
  const resolves: string[] = []
  const port: RewindAudioPort = {
    native: !!opts.native,
    async resolve(t): Promise<SourceResult> {
      resolves.push(t.id)
      if (pending.has(t.id)) return { kind: 'pending', status: 'acquiring', acquisitionId: null, queuePosition: 1 }
      return { kind: 'ready', source: { url: `https://s3/${t.id}.opus`, durationMs: t.durationMs }, peaks: null }
    },
    async start(t, startMs, o) {
      calls.push({ op: 'start', id: t.id, startMs, fadeMs: o.fadeMs })
      return true
    },
    pause: () => calls.push({ op: 'pause' }),
    resume: () => calls.push({ op: 'resume' }),
    setGain: g => gains.push(g),
    end: () => calls.push({ op: 'end' }),
    warm: url => calls.push({ op: 'warm', id: url }),
    schedule(fn, ms) {
      const id = nextId++
      timers.push({ at: now + ms, fn, id })
      return id
    },
    cancel(h) {
      timers = timers.filter(t => t.id !== h)
    },
  }
  /** Время вперёд: срабатывают таймеры и промисы между ними */
  async function advance(ms: number) {
    const end = now + ms
    for (;;) {
      await flush()
      timers.sort((a, b) => a.at - b.at)
      const t = timers[0]
      if (!t || t.at > end) break
      timers.shift()
      now = t.at
      t.fn()
    }
    now = end
    await flush()
  }
  return { port, calls, gains, resolves, pending, advance, gain: () => gains.at(-1) }
}

async function flush() {
  for (let i = 0; i < 10; i++) await Promise.resolve()
}

const cues = [cue('intro', 'a', 50_000), cue('minutes', 'b', 54_000), cue('toy', 'c', 61_000), cue('summary', 'c', 61_000)]

describe('RewindAudio', () => {
  test('starts the first slide from its chorus and fades in from silence', async () => {
    const h = harness()
    const audio = new RewindAudio(h.port)
    audio.setCues(SLIDES, cues)
    void audio.show(0)
    await h.advance(0)
    expect(h.calls.find(c => c.op === 'start')).toEqual({ op: 'start', id: 'a', startMs: 50_000, fadeMs: 0 })
    expect(h.gains[0]).toBe(0)
    await h.advance(REWIND_FADE_IN_MS)
    expect(h.gain()).toBe(1)
    // Следующий трек готовится заранее
    expect(h.resolves).toContain('b')
    expect(h.calls).toContainEqual({ op: 'warm', id: 'https://s3/b.opus' })
  })

  test('web: switching slides crossfades 1.5 s into the next chorus; same track keeps playing', async () => {
    const h = harness()
    const audio = new RewindAudio(h.port)
    audio.setCues(SLIDES, cues)
    void audio.show(0)
    await h.advance(1000)
    void audio.show(1)
    await h.advance(0)
    expect(h.calls.filter(c => c.op === 'start').at(-1)).toEqual({ op: 'start', id: 'b', startMs: 54_000, fadeMs: REWIND_FADE_MS })
    const starts = h.calls.filter(c => c.op === 'start').length
    // «Дни» без своего трека — звучит «Минуты»; «Треки» — тоже
    void audio.show(2)
    void audio.show(3)
    await h.advance(100)
    expect(h.calls.filter(c => c.op === 'start').length).toBe(starts)
    expect(h.gain()).toBe(1)
  })

  test('app: no overlap — volume dips out, track changes, volume comes back within the crossfade time', async () => {
    const h = harness({ native: true })
    const audio = new RewindAudio(h.port)
    audio.setCues(SLIDES, cues)
    void audio.show(0)
    await h.advance(REWIND_FADE_IN_MS)
    const mark = h.gains.length
    void audio.show(1)
    const { outMs, inMs } = rewindDip()
    await h.advance(outMs)
    expect(h.gain()).toBe(0)
    expect(h.calls.filter(c => c.op === 'start').at(-1)).toEqual({ op: 'start', id: 'b', startMs: 54_000, fadeMs: 0 })
    await h.advance(inMs)
    expect(h.gain()).toBe(1)
    // Громкость шла вниз, потом вверх — без скачков больше шага
    const path = h.gains.slice(mark)
    expect(Math.max(...path.slice(1).map((g, i) => Math.abs(g - path[i]!)))).toBeLessThan(0.2)
  })

  test('rapid taps: only the last slide plays', async () => {
    const h = harness()
    const audio = new RewindAudio(h.port)
    audio.setCues(SLIDES, cues)
    void audio.show(0)
    await h.advance(1000)
    void audio.show(1)
    void audio.show(4)
    await h.advance(0)
    const started = h.calls.filter(c => c.op === 'start').map(c => c.id)
    expect(started).toEqual(['a', 'c'])
    expect(audio.current?.id).toBe('c')
  })

  test('a track still downloading: the previous one keeps playing, the new one comes in when ready', async () => {
    const h = harness()
    const audio = new RewindAudio(h.port)
    audio.setCues(SLIDES, [cue('intro', 'a', 50_000), cue('minutes', 'b', 54_000, 'queued'), cue('toy', 'c', 61_000)])
    void audio.show(0)
    await h.advance(1000)
    void audio.show(1)
    await h.advance(0)
    expect(audio.current?.id).toBe('a')
    // Не готовое на сервере не спрашивается — сервер скачивает его сам
    expect(h.resolves).not.toContain('b')
    audio.trackReady('b')
    await h.advance(0)
    expect(audio.current?.id).toBe('b')
    // Готовность трека не своего слайда ничего не меняет
    audio.trackReady('zzz')
    await h.advance(0)
    expect(audio.current?.id).toBe('b')
  })

  test('own track not on the server yet and nothing playing: the nearest ready one starts right away', async () => {
    const h = harness()
    const audio = new RewindAudio(h.port)
    audio.setCues(SLIDES, [cue('intro', 'a', 50_000, 'queued'), cue('toy', 'c', 61_000)])
    void audio.show(0)
    await h.advance(0)
    expect(h.calls.find(c => c.op === 'start')).toEqual({ op: 'start', id: 'c', startMs: 61_000, fadeMs: 0 })
    audio.trackReady('a')
    await h.advance(0)
    expect(audio.current?.id).toBe('a')
  })

  test('the server says ready but the file is gone: keeps what plays', async () => {
    const h = harness({ pending: ['b'] })
    const audio = new RewindAudio(h.port)
    audio.setCues(SLIDES, cues)
    void audio.show(0)
    await h.advance(1000)
    void audio.show(1)
    await h.advance(0)
    expect(audio.current?.id).toBe('a')
  })

  test('hold ducks, mute silences, hidden page pauses, closing hands the player back once', async () => {
    const h = harness()
    const audio = new RewindAudio(h.port)
    audio.setCues(SLIDES, cues)
    void audio.show(0)
    await h.advance(REWIND_FADE_IN_MS)
    audio.setDucked(true)
    await h.advance(300)
    expect(h.gain()).toBeCloseTo(REWIND_DUCK, 10)
    audio.setDucked(false)
    await h.advance(300)
    expect(h.gain()).toBe(1)
    audio.setMuted(true)
    await h.advance(300)
    expect(h.gain()).toBe(0)
    expect(audio.isMuted).toBe(true)
    audio.setHidden(true)
    audio.setHidden(false)
    expect(h.calls.filter(c => c.op === 'pause' || c.op === 'resume').map(c => c.op)).toEqual(['pause', 'resume'])
    audio.close()
    audio.close()
    void audio.show(2)
    await h.advance(1000)
    expect(h.calls.filter(c => c.op === 'end').length).toBe(1)
    expect(h.calls.at(-1)?.op).toBe('end')
  })

  test('muted from the start stays silent through the fade-in', async () => {
    const h = harness()
    const audio = new RewindAudio(h.port)
    audio.setCues(SLIDES, cues)
    audio.setMuted(true)
    void audio.show(0)
    await h.advance(2000)
    expect(Math.max(...h.gains)).toBe(0)
  })

  test('no soundtrack: nothing starts and closing does not touch the player', async () => {
    const h = harness()
    const audio = new RewindAudio(h.port)
    audio.setCues(SLIDES, [])
    void audio.show(0)
    await h.advance(1000)
    audio.close()
    expect(h.calls).toEqual([])
  })
})
