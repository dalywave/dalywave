// События прослушивания и реакции копятся и уходят пачкой. clientEventId делает повтор безопасным:
// при обрыве связи пачка просто отправится ещё раз. Очередь переживает перезагрузку вкладки.
import type { EventBatch, EventResult, FeedbackEvent, PlayEvent } from '~/api/types'

const STORAGE_KEY = 'dw.events.v1'
const FLUSH_MS = 10_000
const BATCH_MAX = 500

interface Stored {
  plays: PlayEvent[]
  feedback: FeedbackEvent[]
}

function read(): Stored {
  try {
    const raw = localStorage.getItem(STORAGE_KEY)
    if (raw) {
      const parsed = JSON.parse(raw) as Partial<Stored>
      return { plays: parsed.plays ?? [], feedback: parsed.feedback ?? [] }
    }
  }
  catch {
    // Повреждённая очередь не должна ломать плеер
  }
  return { plays: [], feedback: [] }
}

export class EventQueue {
  private store: Stored = read()
  private timer = 0
  private sending = false
  private failures = 0

  constructor(private readonly send: (batch: EventBatch, keepalive: boolean) => Promise<EventResult>) {
    if (typeof document !== 'undefined') {
      document.addEventListener('visibilitychange', () => {
        if (document.visibilityState === 'hidden') void this.flush(true)
      })
    }
    if (this.size) this.schedule()
  }

  get size(): number {
    return this.store.plays.length + this.store.feedback.length
  }

  play(event: Omit<PlayEvent, 'clientEventId'>) {
    this.store.plays.push({ ...event, clientEventId: crypto.randomUUID() } as PlayEvent)
    this.persist()
    this.schedule()
  }

  feedback(event: Omit<FeedbackEvent, 'clientEventId' | 'at'>) {
    this.store.feedback.push({ ...event, clientEventId: crypto.randomUUID(), at: new Date().toISOString() } as FeedbackEvent)
    this.persist()
    if (this.store.feedback.length >= 20) void this.flush()
    else this.schedule()
  }

  async flush(keepalive = false) {
    if (this.sending || !this.size) return
    this.sending = true
    clearTimeout(this.timer)
    this.timer = 0
    const batch: EventBatch = { plays: this.store.plays.slice(0, BATCH_MAX), feedback: this.store.feedback.slice(0, BATCH_MAX) }
    try {
      await this.send(batch, keepalive)
      // Принятые, повторы и отклонённые больше не нужны: отклонённые повторять бессмысленно
      const sent = new Set([...(batch.plays ?? []), ...(batch.feedback ?? [])].map(e => e.clientEventId))
      this.store.plays = this.store.plays.filter(e => !sent.has(e.clientEventId))
      this.store.feedback = this.store.feedback.filter(e => !sent.has(e.clientEventId))
      this.persist()
      this.failures = 0
    }
    catch {
      this.failures++
    }
    finally {
      this.sending = false
      if (this.size) this.schedule()
    }
  }

  private schedule() {
    if (this.timer) return
    // После неудач отправка реже: 10 с, 20 с, 40 с… до 5 минут
    const delay = Math.min(300_000, FLUSH_MS * Math.pow(2, this.failures))
    this.timer = window.setTimeout(() => {
      this.timer = 0
      void this.flush()
    }, delay)
  }

  private persist() {
    try {
      localStorage.setItem(STORAGE_KEY, JSON.stringify(this.store))
    }
    catch {
      // Переполненное хранилище: события останутся в памяти до отправки
    }
  }
}
