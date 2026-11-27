// Проверка словарей: bun run i18n-check. Эталон — i18n/locales/ru.json: у каждого другого языка те же
// ключи, те же плейсхолдеры {name}, у славянских языков столько же форм «a | b | c», нет пустых строк.
// Тексты живут только в словарях: блок <i18n> в компоненте — ошибка.
import { readdirSync, readFileSync } from 'node:fs'
import { join } from 'node:path'

const dir = new URL('../i18n/locales/', import.meta.url).pathname
const app = new URL('../app/', import.meta.url).pathname
const SLAVIC = new Set(['ru', 'uk'])
const PH = /\{[A-Za-z_][A-Za-z0-9_]*\}/g

const load = f => JSON.parse(readFileSync(join(dir, f), 'utf8'))
const ref = load('ru.json')
const errors = []

function walk(a, b, path, lang) {
  if (typeof a === 'object' && a !== null) {
    if (typeof b !== 'object' || b === null) return errors.push(`${lang}: ${path} — не объект`)
    for (const k of Object.keys(a)) {
      if (!(k in b)) errors.push(`${lang}: нет ключа ${path}${k}`)
      else walk(a[k], b[k], `${path}${k}.`, lang)
    }
    for (const k of Object.keys(b)) if (!(k in a)) errors.push(`${lang}: лишний ключ ${path}${k}`)
    return
  }
  const key = path.slice(0, -1)
  if (typeof b !== 'string') return errors.push(`${lang}: ${key} — не строка`)
  if (a.trim() && !b.trim()) errors.push(`${lang}: ${key} — пустая строка`)
  const pa = (a.match(PH) ?? []).sort().join(' ')
  const pb = (b.match(PH) ?? []).sort().join(' ')
  if (pa !== pb) errors.push(`${lang}: ${key} — плейсхолдеры «${pa}» → «${pb}»`)
  if (SLAVIC.has(lang) && a.split(' | ').length !== b.split(' | ').length) errors.push(`${lang}: ${key} — другое число форм`)
}

const langs = readdirSync(dir).filter(f => f.endsWith('.json') && f !== 'ru.json')
for (const f of langs) walk(ref, load(f), '', f.slice(0, -5))

function vueFiles(d) {
  return readdirSync(d, { withFileTypes: true }).flatMap(e =>
    e.isDirectory() ? vueFiles(join(d, e.name)) : e.name.endsWith('.vue') ? [join(d, e.name)] : [])
}
for (const f of vueFiles(app)) {
  if (readFileSync(f, 'utf8').includes('<i18n')) errors.push(`${f.slice(app.length)}: блок <i18n> — тексты переносятся в i18n/locales`)
}

if (errors.length) {
  console.error(errors.join('\n'))
  console.error(`\nошибок: ${errors.length}`)
  process.exit(1)
}
console.log(`словари в порядке: ru и ${langs.map(f => f.slice(0, -5)).join(', ')}`)
