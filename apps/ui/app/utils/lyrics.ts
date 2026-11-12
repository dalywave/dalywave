// Тексты песен: модель для показа «сценой» — строки, проигрыши, подпевки и оценка слов.
// Чистые функции без DOM: проверяются `bun test` (tests/lyrics.test.ts).
//
// Что у нас честно, а что оценка. Сервер (LRCLIB) даёт только время НАЧАЛА каждой строки. Честно:
// какая строка сейчас, перемотка, время до следующей строки, длина проигрыша. Оценка: где кончается
// строка перед долгой паузой (по слогам) и время каждого слова. Правило слов — в `wordMode`.

export interface LyricsSourceLine { tMs: number, text: string }

export interface LyricWord {
  w: string
  /** Начало и конец «загорания» слова, мс от начала трека */
  s: number
  e: number
}

export interface LyricLine {
  type: 'line'
  i: number
  tMs: number
  /** Конец строки: начало следующей или оценка, если дальше проигрыш */
  endMs: number
  text: string
  /** Основной текст без подпевки в скобках */
  main: string
  /** Подпевка «(…)» — отдельной строкой помельче */
  backing: string
  /** Строка целиком — подпевка */
  onlyBacking: boolean
  /** words — слова загораются по очереди; line — вся строка сразу (см. wordMode) */
  mode: 'words' | 'line'
  words: LyricWord[]
  backWords: LyricWord[] | null
}

export interface LyricBreak {
  type: 'break'
  i: number
  tMs: number
  endMs: number
  /** Вступление до первой строки */
  intro: boolean
}

/** После последней строки: ничего не подсвечено, текст «отпускает» */
export interface LyricEnd { type: 'end', i: number, tMs: number, endMs: number }

export type LyricItem = LyricLine | LyricBreak | LyricEnd
type Draft = Omit<LyricLine, 'i'> | Omit<LyricBreak, 'i'> | Omit<LyricEnd, 'i'>

/** Строку подсвечиваем чуть раньше: глаз успевает к началу пения */
export const LYRICS_LEAD_MS = 250
/** Пауза после конца строки длиннее этого — проигрыш */
export const LYRICS_BREAK_MS = 5000
/** Вступление длиннее этого — тоже проигрыш */
export const LYRICS_INTRO_MS = 4000
/** Пустая строка LRCLIB короче этого — просто вдох, не проигрыш */
const MIN_GAP_MS = 3000
/** Ручная прокрутка: столько лента ждёт, потом сама возвращается к текущей строке */
export const LYRICS_RESUME_MS = 6000
/** Слова «успевают» по слогам только в этом темпе (мс на слог); медленнее — строка сильно растянута */
export const WORD_PACE_MAX = 600
const WORD_PACE_MIN = 120

const clamp = (v: number, a = 0, b = 1) => Math.min(b, Math.max(a, v))
const VOWELS = /[аеёиоуыэюяaeiouyäöüéèáíóúàùâêîôûæøå]/gi

/** Слоги слова — по гласным; слово без гласных («в», «к», «—») всё равно занимает долю */
export function syllables(word: string): number {
  return Math.max(1, word.match(VOWELS)?.length ?? 0)
}

function syllablesOf(text: string): number {
  return text.split(/\s+/).filter(Boolean).reduce((a, w) => a + syllables(w), 0)
}

/** Оценка длины пения строки, если после неё долгая пауза: ≈240 мс на слог плюс выдох */
export function estimateSingMs(text: string): number {
  return clamp(syllablesOf(text) * 240 + 500, 1300, 7000)
}

/** Слова строки: время пения делится пропорционально слогам */
export function splitWords(text: string, start: number, sing: number): LyricWord[] {
  const parts = text.split(/\s+/).filter(Boolean)
  const syl = parts.map(syllables)
  const total = syl.reduce((a, b) => a + b, 0) || 1
  let acc = 0
  return parts.map((w, k) => {
    const s = start + (sing * acc) / total
    acc += syl[k]!
    return { w, s, e: start + (sing * acc) / total }
  })
}

/**
 * Правило пословной подсветки. Слова загораются по очереди, только если:
 * - конец строки известен честно (дальше сразу следующая строка, а не проигрыш с оценкой конца);
 * - в строке хотя бы два слога;
 * - темп правдоподобный: от 120 до 600 мс на слог (время до следующей строки / слоги).
 * Иначе — строка растянута (долгие ноты, паузы внутри), слишком плотная или конец неизвестен:
 * пропорция по слогам там убегает вперёд голоса, и честнее зажечь строку целиком.
 */
export function wordMode(windowMs: number, syl: number, endKnown: boolean): 'words' | 'line' {
  if (!endKnown || syl < 2) return 'line'
  const pace = windowMs / syl
  return pace >= WORD_PACE_MIN && pace <= WORD_PACE_MAX ? 'words' : 'line'
}

/** Модель синхронного текста: строки, проигрыши (пустые строки, паузы, вступление) и хвост */
export function buildLyrics(src: LyricsSourceLine[], durationMs: number): LyricItem[] {
  const lines = src.filter(l => Number.isFinite(l.tMs)).map(l => ({ tMs: Math.max(0, l.tMs), text: l.text.trim() })).sort((a, b) => a.tMs - b.tMs)
  const dur = Math.max(durationMs || 0, (lines.at(-1)?.tMs ?? 0) + 1)
  const items: Draft[] = []
  const firstText = lines.find(l => l.text)
  if (!firstText) return []
  if (firstText.tMs >= LYRICS_INTRO_MS) items.push({ type: 'break', tMs: 0, endMs: firstText.tMs, intro: true })

  for (let k = 0; k < lines.length; k++) {
    const l = lines[k]!
    const next = lines[k + 1]
    const nextStart = next ? next.tMs : dur
    if (!l.text) {
      // Пустая строка — проигрыш до следующей строки с текстом; короткие паузы не показываем
      if (l.tMs < firstText.tMs) continue
      const after = lines.slice(k + 1).find(x => x.text)
      const end = after ? after.tMs : dur
      const prev = items.at(-1)
      if (end - l.tMs >= MIN_GAP_MS && prev?.type === 'line') items.push(after ? { type: 'break', tMs: l.tMs, endMs: end, intro: false } : { type: 'end', tMs: l.tMs, endMs: end })
      continue
    }
    const backing = [...l.text.matchAll(/\(([^)]*)\)/g)].map(m => m[1]!.trim()).filter(Boolean).join(' ')
    let main = l.text.replace(/\([^)]*\)/g, '').replace(/\s+/g, ' ').trim()
    const onlyBacking = !main
    if (onlyBacking) main = backing
    if (!main) continue
    const syl = syllablesOf(main)
    const est = estimateSingMs(main)
    const slot = nextStart - l.tMs
    let endMs = nextStart
    let tail: Draft | null = null
    if (next?.text && slot > est + LYRICS_BREAK_MS) {
      endMs = l.tMs + est
      tail = { type: 'break', tMs: endMs, endMs: nextStart, intro: false }
    }
    else if (!next && slot > est + MIN_GAP_MS) {
      endMs = l.tMs + est
      tail = { type: 'end', tMs: endMs, endMs: dur }
    }
    // Следующая — пустая строка (проигрыш): конец пения тоже неизвестен
    const endKnown = !tail && !!next?.text
    const span = endMs - l.tMs
    const mode = wordMode(span, syl, endKnown)
    // Слова успевают к 90 % времени строки: последнее загорается до смены строки, а не после
    const sing = span * 0.9
    const mainSing = onlyBacking || !backing ? sing : sing * 0.78
    items.push({
      type: 'line',
      tMs: l.tMs,
      endMs,
      text: l.text,
      main,
      backing: onlyBacking ? '' : backing,
      onlyBacking,
      mode,
      words: splitWords(main, l.tMs, mainSing),
      backWords: backing && !onlyBacking ? splitWords(backing, l.tMs + mainSing, sing - mainSing) : null,
    })
    if (tail) items.push(tail)
  }
  return items.map((it, i) => ({ ...it, i }) as LyricItem)
}

/** Индекс текущего пункта: последний, что начался к `ms` (с опережением); -1 — до первого */
export function lyricAt(items: readonly { tMs: number }[], ms: number, lead = LYRICS_LEAD_MS): number {
  let lo = 0
  let hi = items.length - 1
  let found = -1
  while (lo <= hi) {
    const mid = (lo + hi) >> 1
    if (items[mid]!.tMs <= ms + lead) {
      found = mid
      lo = mid + 1
    }
    else hi = mid - 1
  }
  return found
}

/** Доля пройденного пункта (проигрыша, строки) к моменту `ms`: 0…1 */
export function lyricProgress(item: { tMs: number, endMs: number }, ms: number): number {
  return clamp((ms - item.tMs) / Math.max(1, item.endMs - item.tMs))
}

/** Сколько слова «загорелось» к моменту `ms`: 0…1. Короткие слова загораются не быстрее 120 мс */
export function wordProgress(word: LyricWord, ms: number): number {
  return clamp((ms - word.s) / Math.max(120, word.e - word.s))
}

// ---------- Текст без времени ----------

export interface Stanza {
  lines: string[]
  /** Строфа встречается в песне не раз — это припев */
  chorus: boolean
  /** Повтор уже показанной строфы — сворачиваем до подписи */
  repeat: boolean
}

/** Строфы простого текста; повторы строф (припев) отмечены, чтобы свернуть их */
export function plainStanzas(text: string | null | undefined): Stanza[] {
  if (!text) return []
  const raw = text.split(/\n\s*\n/).map(s => s.split('\n').map(l => l.trim()).filter(Boolean)).filter(s => s.length)
  const key = (s: string[]) => s.join('\n').toLowerCase().replace(/[^\p{L}\p{N}\n ]+/gu, '').replace(/ +/g, ' ')
  const counts = new Map<string, number>()
  for (const s of raw) counts.set(key(s), (counts.get(key(s)) ?? 0) + 1)
  const seen = new Set<string>()
  return raw.map((lines) => {
    const k = key(lines)
    // Повтор из одной строки не сворачиваем: подпись заняла бы столько же места
    const chorus = (counts.get(k) ?? 0) > 1 && lines.length > 1
    const repeat = chorus && seen.has(k)
    seen.add(k)
    return { lines, chorus, repeat }
  })
}
