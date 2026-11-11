#!/usr/bin/env bun
// Выпуск приложения на сервер автообновлений: файл обновления и его подпись из уже собранного
// бандла (cargo tauri build с ключом подписи) → хранилище R2 → регистрация выпуска. После неё
// установленные приложения увидят новую версию при следующей проверке (запуск или раз в 6 часов).
//
//   DALYWAVE_RELEASE_TOKEN=… bun apps/shell/scripts/release.ts [--target <triple>] [--notes "…"]
//
// --target — тройка сборки (`aarch64-apple-darwin`, `x86_64-pc-windows-msvc`), если бандл собран
// с --target (CI); без него — бандл из target/release этой машины. Сервер — --server или
// DALYWAVE_RELEASE_SERVER (по умолчанию https://wave.daly.rest). Что нового — --notes,
// --notes-file или RELEASE_NOTES. Токен — только из окружения и никуда не печатается.
// Ключа R2 скрипту не нужно: сервер выдаёт подписанную ссылку на загрузку (docs/decisions.md).
import { existsSync, readdirSync, readFileSync, statSync } from 'node:fs'
import { dirname, join } from 'node:path'

const SHELL = join(dirname(import.meta.path), '..')

function fail(message: string): never {
  console.error(`release: ${message}`)
  process.exit(1)
}

function arg(name: string): string | undefined {
  const i = process.argv.indexOf(`--${name}`)
  return i >= 0 ? process.argv[i + 1] : undefined
}

/** Версия — из [package] apps/shell/Cargo.toml: единственный источник (scripts/bump-version.sh) */
function appVersion(): string {
  const toml = readFileSync(join(SHELL, 'Cargo.toml'), 'utf8')
  const pkg = toml.split(/^\[/m).find(s => s.startsWith('package]'))
  const version = pkg?.match(/^version\s*=\s*"([^"]+)"/m)?.[1]
  if (!version) fail('не нашёл version в [package] apps/shell/Cargo.toml')
  return version
}

/** Платформа и архитектура — как их называет плагин обновлений Tauri */
function platform(triple: string | undefined): { target: 'darwin' | 'windows', arch: 'aarch64' | 'x86_64' } {
  const os = triple ? (triple.includes('apple-darwin') ? 'darwin' : triple.includes('windows') ? 'windows' : null) : process.platform === 'darwin' ? 'darwin' : process.platform === 'win32' ? 'windows' : null
  if (!os) fail(`выпуски есть только для macOS и Windows, а не для ${triple ?? process.platform}`)
  const cpu = triple ? triple.split('-')[0] : process.arch === 'arm64' ? 'aarch64' : process.arch === 'x64' ? 'x86_64' : process.arch
  if (cpu !== 'aarch64' && cpu !== 'x86_64') fail(`архитектура ${cpu} не поддерживается`)
  return { target: os, arch: cpu }
}

/** Файл обновления текущей версии и его подпись `.sig` */
function artifact(bundle: string, target: 'darwin' | 'windows', version: string): { file: string, signature: string } {
  let file: string | undefined
  if (target === 'darwin') {
    const dir = join(bundle, 'macos')
    file = existsSync(dir) ? readdirSync(dir).filter(f => f.endsWith('.app.tar.gz')).map(f => join(dir, f))[0] : undefined
    // В имени архива macOS версии нет: сверяем её с Info.plist рядом, чтобы не выпустить старую сборку
    const plist = join(dir, 'DalyWave.app', 'Contents', 'Info.plist')
    const built = existsSync(plist) ? readFileSync(plist, 'utf8').match(/<key>CFBundleShortVersionString<\/key>\s*<string>([^<]+)<\/string>/)?.[1] : undefined
    if (file && built !== version) fail(`в ${dir} собрана версия ${built ?? '?'}, а в Cargo.toml — ${version}: пересоберите`)
  }
  else {
    const dir = join(bundle, 'nsis')
    file = existsSync(dir) ? readdirSync(dir).filter(f => f.endsWith('-setup.exe') && f.includes(`_${version}_`)).map(f => join(dir, f))[0] : undefined
  }
  if (!file) fail(`нет файла обновления версии ${version} в ${bundle} — соберите приложение (make app или cargo tauri build)`)
  const sig = `${file}.sig`
  if (!existsSync(sig)) fail(`нет подписи ${sig}: сборка шла без ключа обновлений (TAURI_SIGNING_PRIVATE_KEY)`)
  return { file, signature: readFileSync(sig, 'utf8').trim() }
}

async function api<T>(server: string, token: string, path: string, body: unknown): Promise<T> {
  const res = await fetch(`${server}${path}`, {
    method: 'POST',
    headers: { 'Authorization': `Bearer ${token}`, 'Content-Type': 'application/json' },
    body: JSON.stringify(body),
  })
  const text = await res.text()
  if (!res.ok) {
    let message = text
    try {
      message = JSON.parse(text).error?.message ?? text
    }
    catch {
      // не JSON — показываем как есть
    }
    fail(`${path}: ${res.status} ${message}`)
  }
  return JSON.parse(text) as T
}

const token = process.env.DALYWAVE_RELEASE_TOKEN?.trim()
if (!token) fail('нет DALYWAVE_RELEASE_TOKEN (тот же, что в .env сервера; apps/shell/README.md, «Выпуск»)')
const server = (arg('server') ?? process.env.DALYWAVE_RELEASE_SERVER ?? 'https://wave.daly.rest').replace(/\/$/, '')
const triple = arg('target')
const notesFile = arg('notes-file')
const notes = arg('notes') ?? (notesFile ? readFileSync(notesFile, 'utf8') : process.env.RELEASE_NOTES) ?? null

const version = appVersion()
const { target, arch } = platform(triple)
const bundle = join(SHELL, 'target', ...(triple ? [triple] : []), 'release', 'bundle')
const { file, signature } = artifact(bundle, target, version)
// В хранилище — с версией и архитектурой в имени: по ключу сразу видно, что это
const fileName = target === 'darwin' ? `DalyWave_${version}_${arch}.app.tar.gz` : `DalyWave_${version}_${arch}-setup.exe`
const size = statSync(file).size

console.log(`DalyWave ${version} ${target}/${arch}: ${file} (${(size / 1048576).toFixed(1)} МБ) → ${server}`)
const upload = await api<{ key: string, url: string }>(server, token, '/api/releases/uploads', { target, arch, version, fileName })
const put = await fetch(upload.url, {
  method: 'PUT',
  headers: { 'Content-Type': 'application/octet-stream' },
  body: Bun.file(file),
})
if (!put.ok) fail(`загрузка в хранилище: ${put.status} ${(await put.text()).slice(0, 300)}`)
console.log(`загружено: ${upload.key}`)
const release = await api<{ version: string, publishedAt: string }>(server, token, '/api/releases', { target, arch, version, fileName, signature, notes })
console.log(`выпуск ${release.version} для ${target}/${arch} зарегистрирован (${release.publishedAt})`)
