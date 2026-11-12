// Перемешивание и повтор очереди плеера: bun test (из apps/ui)
import { describe, expect, test } from 'bun:test'
import { afterTrack, mixIn, nextRepeat, previousAction, restoreQueue, roundsNeeded, shuffleQueue, shuffled } from '../app/utils/queue-order'

const e = (uid: string) => ({ uid })
const uids = (list: { uid: string }[]) => list.map(x => x.uid)

/** Предсказуемый «случай»: линейный конгруэнтный генератор */
function seeded(seed: number) {
  let s = seed
  return () => {
    s = (s * 1664525 + 1013904223) % 2 ** 32
    return s / 2 ** 32
  }
}

describe('shuffled', () => {
  test('keeps every entry exactly once and does not touch the input', () => {
    const list = Array.from({ length: 50 }, (_, i) => e(`t${i}`))
    const copy = list.slice()
    const out = shuffled(list, seeded(7))
    expect(list).toEqual(copy)
    expect(out).toHaveLength(50)
    expect(new Set(uids(out)).size).toBe(50)
    expect(uids(out)).not.toEqual(uids(list))
  })

  test('empty and single lists', () => {
    expect(shuffled([])).toEqual([])
    expect(uids(shuffled([e('a')]))).toEqual(['a'])
  })
})

describe('shuffle on, then off', () => {
  const album = ['a', 'b', 'c', 'd', 'e', 'f', 'g'].map(e)

  test('on: the rest of the context is shuffled, the current track and order are remembered', () => {
    const [cur, ...rest] = album
    const { upcoming, original } = shuffleQueue(cur!, rest, seeded(3))
    expect(new Set(uids(upcoming))).toEqual(new Set(uids(rest)))
    expect(uids(original)).toEqual(uids(album))
  })

  test('off right away: the original order after the current track', () => {
    const [cur, ...rest] = album
    const { upcoming, original } = shuffleQueue(cur!, rest, seeded(3))
    expect(uids(restoreQueue(cur!, upcoming, original))).toEqual(['b', 'c', 'd', 'e', 'f', 'g'])
  })

  test('off later: continues from the current track’s place, played ones do not come back', () => {
    const [cur, ...rest] = album
    const { original } = shuffleQueue(cur!, rest, seeded(3))
    // Сыграли вразброс: e, b — сейчас играет d; впереди ещё g, c, f
    const now = album[3]!
    const left = ['g', 'c', 'f'].map(e)
    expect(uids(restoreQueue(now, left, original))).toEqual(['f', 'g'])
  })

  test('off while a manual track plays: unplayed context in the original order', () => {
    const [cur, ...rest] = album
    const { original } = shuffleQueue(cur!, rest, seeded(3))
    const manual = e('m')
    const left = ['g', 'c', 'f'].map(e)
    expect(uids(restoreQueue(manual, left, original))).toEqual(['c', 'f', 'g'])
  })

  test('entries added after shuffling stay after the restored order', () => {
    const [cur, ...rest] = album
    const { upcoming, original } = shuffleQueue(cur!, rest, seeded(3))
    const round2 = ['a2', 'b2'].map(e)
    const out = restoreQueue(cur!, [...upcoming, ...round2], original)
    expect(uids(out)).toEqual(['b', 'c', 'd', 'e', 'f', 'g', 'a2', 'b2'])
  })

  test('without a current track the whole remembered order is the base', () => {
    const { upcoming, original } = shuffleQueue(null, album, seeded(9))
    expect(uids(original)).toEqual(uids(album))
    expect(uids(restoreQueue(null, upcoming, original))).toEqual(uids(album))
  })
})

describe('repeat', () => {
  test('the button cycles off → all → one → off', () => {
    expect(nextRepeat('off')).toBe('all')
    expect(nextRepeat('all')).toBe('one')
    expect(nextRepeat('one')).toBe('off')
  })

  test('after a track ends by itself', () => {
    expect(afterTrack('off', true, true)).toBe('next')
    expect(afterTrack('off', false, true)).toBe('stop')
    expect(afterTrack('all', true, true)).toBe('next')
    expect(afterTrack('all', false, true)).toBe('wrap')
    expect(afterTrack('all', false, false)).toBe('stop')
    expect(afterTrack('one', true, true)).toBe('replay')
    expect(afterTrack('one', false, false)).toBe('replay')
  })

  test('repeat all keeps enough tracks ahead in whole rounds', () => {
    expect(roundsNeeded(15, 12, 10)).toBe(0)
    expect(roundsNeeded(3, 12, 10)).toBe(1)
    expect(roundsNeeded(0, 1, 10)).toBe(10)
    expect(roundsNeeded(0, 0, 10)).toBe(0)
  })

  test('previous restarts after 3 seconds, like Spotify', () => {
    expect(previousAction(3001, true)).toBe('restart')
    expect(previousAction(3000, true)).toBe('previous')
    expect(previousAction(500, false)).toBe('restart')
  })
})

describe('mixIn', () => {
  const s = (uid: string) => ({ uid, suggested: true })
  test('one suggestion after every three tracks of the list', () => {
    const list = Array.from({ length: 7 }, (_, i) => e(`t${i}`))
    expect(uids(mixIn(list, [s('a'), s('b'), s('c')]))).toEqual(['t0', 't1', 't2', 'a', 't3', 't4', 't5', 'b', 't6'])
  })
  test('already mixed-in suggestions restart the count and extras are dropped', () => {
    const list = [e('t0'), s('x'), e('t1'), e('t2'), e('t3'), e('t4')]
    expect(uids(mixIn(list, [s('a'), s('b')]))).toEqual(['t0', 'x', 't1', 't2', 't3', 'a', 't4'])
    expect(uids(mixIn([], [s('a')]))).toEqual([])
  })
})
