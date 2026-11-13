// Обучение сети вкуса на этом компьютере (приложение macOS и Windows): команды `trainer_*` и событие
// `trainer:state` оболочки (apps/shell/src/trainer.rs). Учит Rust; интерфейс только показывает и просит.
import type { Trainer, TrainerSnapshot } from '../types'
import type { Shell } from './shell'

/** Источник обучателя: настоящая оболочка или мок разработки */
export interface TrainerSource {
  state(): Promise<TrainerSnapshot>
  listen(cb: (state: TrainerSnapshot) => void): Promise<() => void> | (() => void)
  setEnabled(enabled: boolean | null): Promise<TrainerSnapshot>
}

/** Обучатель, источник которого грузится лениво: команды ждут загрузки, отписка — синхронная */
export function lazyTrainer(load: () => Promise<TrainerSource>): Trainer {
  let job: Promise<TrainerSource> | null = null
  const source = () => (job ??= load())
  return {
    state: async () => (await source()).state(),
    onState(cb) {
      let off: (() => void) | null = null
      let stopped = false
      void source().then(s => s.listen(cb)).then((unlisten) => {
        if (stopped) unlisten()
        else off = unlisten
      }).catch(() => {})
      return () => {
        stopped = true
        off?.()
      }
    },
    setEnabled: async enabled => (await source()).setEnabled(enabled),
  }
}

export function createTauriTrainer(loadShell: () => Promise<Shell>): Trainer {
  return lazyTrainer(async () => {
    const shell = await loadShell()
    return {
      state: () => shell.trainer.state(),
      listen: cb => shell.on('trainer:state', cb),
      setEnabled: enabled => shell.trainer.setEnabled(enabled),
    }
  })
}

/** Только разработка: `?trainer=off|idle|on-battery|busy|training|active|kept|failed` */
export function createDevTrainer(scenario: string): TrainerSource {
  const hour = 60 * 60 * 1000
  const last = (outcome: 'active' | 'kept' | 'failed') => ({ atMs: Date.now() - 2 * hour, outcome, rows: 8460, tookMs: 41_000, note: '' })
  let snap: TrainerSnapshot = (() => {
    switch (scenario) {
      case 'off': return { enabled: false, phase: 'off', last: null }
      case 'on-battery': return { enabled: true, phase: 'on-battery', last: last('kept') }
      case 'busy': return { enabled: true, phase: 'busy', last: null }
      case 'training': return { enabled: true, phase: 'training', last: null }
      case 'active': return { enabled: true, phase: 'idle', last: last('active') }
      case 'kept': return { enabled: true, phase: 'idle', last: last('kept') }
      case 'failed': return { enabled: true, phase: 'idle', last: last('failed') }
      default: return { enabled: null, phase: 'idle', last: null }
    }
  })()
  const listeners = new Set<(s: TrainerSnapshot) => void>()
  return {
    state: async () => snap,
    listen(cb) {
      listeners.add(cb)
      return () => listeners.delete(cb)
    },
    async setEnabled(enabled) {
      snap = { ...snap, enabled, phase: enabled === false ? 'off' : 'idle' }
      listeners.forEach(cb => cb(snap))
      return snap
    },
  }
}
