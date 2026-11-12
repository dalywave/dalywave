// Только разработка: «Распознать» на сайте для снимков экранов — `?recognize=match|none|permission`.
// Совпадение приходит через 2,5 с, как у проверки оболочки (DALYWAVE_RECOGNIZE_MOCK)
import type { RecognizeMatch, Recognizer } from '../types'

const SAMPLE: RecognizeMatch = { title: 'Midnight City', artist: 'M83', isrc: null, appleMusicId: null, shazamId: null, artworkUrl: null }

export function devRecognizer(): Recognizer | null {
  if (!import.meta.dev || typeof location === 'undefined') return null
  const scenario = new URLSearchParams(location.search).get('recognize')
  if (!scenario) return null
  let cancel: (() => void) | null = null
  return {
    listen: () => new Promise((resolve, reject) => {
      const timer = setTimeout(() => {
        cancel = null
        if (scenario === 'match') resolve(SAMPLE)
        else reject({ code: scenario === 'none' ? 'no_match' : scenario, message: 'mock' })
      }, scenario === 'listening' ? 600_000 : 2500)
      cancel = () => {
        clearTimeout(timer)
        reject({ code: 'cancelled', message: 'mock' })
      }
    }),
    cancel: async () => {
      cancel?.()
      cancel = null
    },
    onOpen: () => () => {},
  }
}
