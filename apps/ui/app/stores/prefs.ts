import { defineStore } from 'pinia'

/** Полоса прокрутки: «Волна» — форма волны с пиками, «Линия» — классическая тонкая полоса */
export type ScrubberStyle = 'wave' | 'line'

/** Тема оформления: тёмная «Полночь», светлая или как в системе */
export type ThemePref = 'dark' | 'light' | 'system'
const THEMES: ThemePref[] = ['dark', 'light', 'system']

/**
 * Настройки интерфейса «под себя». Плоский объект с отметкой времени: так же целиком уйдёт
 * на сервер, когда появится синхронизация между устройствами (побеждает более поздняя запись)
 */
export interface Preferences {
  scrubber: ScrubberStyle
  /** Планшет горизонтально: «Сейчас играет» — колонкой справа от содержимого, как у Spotify на iPad */
  sidePlayer: boolean
  /** Настройка устройства: у телефона и компьютера тема может быть своя */
  theme: ThemePref
  updatedAt: string | null
}

export type PreferencesPatch = Partial<Omit<Preferences, 'updatedAt'>>

const STORAGE_KEY = 'dw.prefs'
const DEFAULTS: Preferences = { scrubber: 'wave', sidePlayer: true, theme: 'dark', updatedAt: null }

// По умолчанию — тёмная: «Полночь» задумана тёмной; светлую и «как в системе»
// выбирают в настройках. Ключ и значения читает и скрипт в <head> (utils/theme.ts) — до отрисовки
// Из хранилища (а потом и с сервера) берём только понятные значения: старая версия или чужая
// запись не должны ломать интерфейс
function sanitize(raw: unknown): Preferences {
  const r = (raw && typeof raw === 'object' ? raw : {}) as Record<string, unknown>
  return {
    scrubber: r.scrubber === 'line' ? 'line' : DEFAULTS.scrubber,
    sidePlayer: typeof r.sidePlayer === 'boolean' ? r.sidePlayer : DEFAULTS.sidePlayer,
    theme: THEMES.includes(r.theme as ThemePref) ? r.theme as ThemePref : DEFAULTS.theme,
    updatedAt: typeof r.updatedAt === 'string' ? r.updatedAt : null,
  }
}

function load(): Preferences {
  try {
    const saved = localStorage.getItem(STORAGE_KEY)
    return saved ? sanitize(JSON.parse(saved)) : { ...DEFAULTS }
  }
  catch {
    return { ...DEFAULTS }
  }
}

function save(p: Preferences) {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(p))
  }
  catch {
    // Без хранилища (приватный режим, переполнение) выбор живёт до перезагрузки
  }
}

export const usePrefsStore = defineStore('prefs', () => {
  const prefs = ref<Preferences>(import.meta.client ? load() : { ...DEFAULTS })

  /** Изменить и запомнить. Сюда же придут значения с сервера — со своим updatedAt */
  function update(patch: PreferencesPatch, updatedAt = new Date().toISOString()) {
    prefs.value = sanitize({ ...prefs.value, ...patch, updatedAt })
    save(prefs.value)
  }

  const scrubber = computed<ScrubberStyle>({
    get: () => prefs.value.scrubber,
    set: v => update({ scrubber: v }),
  })

  const sidePlayer = computed<boolean>({
    get: () => prefs.value.sidePlayer,
    set: v => update({ sidePlayer: v }),
  })

  const theme = computed<ThemePref>({
    get: () => prefs.value.theme,
    set: v => update({ theme: v }),
  })

  // Выбор в другой вкладке подхватывается сразу (key = null — там очистили хранилище)
  if (import.meta.client) {
    window.addEventListener('storage', (e) => {
      if (e.key === STORAGE_KEY || e.key === null) prefs.value = load()
    })
  }

  return { prefs, scrubber, sidePlayer, theme, update }
})
