// Стенд оболочки без настоящего интерфейса: страница с кнопками всех команд моста (:3499)
// и заглушка сервера DalyWave для входа (:3498). Порядок проверки — apps/shell/README.md.
//   bun apps/shell/harness/server.ts
// Отчёты страницы и журнал заглушки — в $TMPDIR/dalywave-harness/.
import { appendFileSync, mkdirSync, readFileSync } from 'node:fs'
import { createHash, randomBytes } from 'node:crypto'
import { tmpdir } from 'node:os'

const dir = import.meta.dir
const out = `${tmpdir()}/dalywave-harness`
mkdirSync(out, { recursive: true })
const reports = `${out}/reports.jsonl`
const mockLog = `${out}/mock.log`

const built = await Bun.build({ entrypoints: [`${dir}/entry.ts`], target: 'browser' })
if (!built.success) {
  console.error(built.logs)
  process.exit(1)
}
const bundle = await built.outputs[0]!.text()

const queue: string[] = []
const cors = { 'Access-Control-Allow-Origin': '*' }

Bun.serve({
  port: 3499,
  hostname: '::',
  async fetch(req) {
    const url = new URL(req.url)
    if (url.pathname === '/') return new Response(readFileSync(`${dir}/index.html`), { headers: { 'content-type': 'text/html; charset=utf-8' } })
    if (url.pathname === '/harness.js') return new Response(bundle, { headers: { 'content-type': 'text/javascript' } })
    if (url.pathname === '/report' && req.method === 'POST') {
      appendFileSync(reports, JSON.stringify({ t: new Date().toISOString(), ...JSON.parse(await req.text()) }) + '\n')
      return new Response(null, { status: 204, headers: cors })
    }
    // Команда странице из терминала: curl -d login localhost:3499/command
    if (url.pathname === '/command' && req.method === 'POST') {
      queue.push((await req.text()).trim())
      return new Response('queued\n')
    }
    if (url.pathname === '/command') {
      const next = queue.shift()
      return next ? new Response(next, { headers: cors }) : new Response(null, { status: 204, headers: cors })
    }
    return new Response('not found', { status: 404 })
  },
})

// Тестовое аудио (crates/client/audio/tools/serve.py): ссылки на него отдаёт заглушка потока
const audio = process.env.DALYWAVE_TEST_AUDIO ?? 'http://127.0.0.1:8420'

// Заглушка входа приложения: те же адреса и ответы, что у сервера (crates/server/src/routes/auth.rs)
type Grant = { challenge: string, device: string, platform: string, used: boolean }
const grants = new Map<string, Grant>()
const tokens = new Set<string>()
const b64url = (b: Buffer) => b.toString('base64url')
const log = (line: string) => appendFileSync(mockLog, `${new Date().toISOString()} ${line}\n`)

Bun.serve({
  port: 3498,
  hostname: '::',
  async fetch(req) {
    const url = new URL(req.url)
    const q = url.searchParams
    if (url.pathname === '/api/auth/app/start') {
      const redirect = q.get('redirect_uri') ?? ''
      const ok = (redirect === 'dalywave://auth' || /^http:\/\/127\.0\.0\.1:\d+\/callback$/.test(redirect))
        && (q.get('code_challenge') ?? '').length === 43 && q.get('code_challenge_method') === 'S256'
        && !!q.get('state') && !!q.get('platform')
      log(`GET start redirect=${redirect} platform=${q.get('platform')} device=${q.get('device_name')} ok=${ok}`)
      if (!ok) return Response.json({ error: { code: 'invalid_request', message: 'bad params' } }, { status: 400 })
      const code = b64url(randomBytes(16))
      grants.set(code, { challenge: q.get('code_challenge')!, device: q.get('device_name') ?? '', platform: q.get('platform')!, used: false })
      const back = `${redirect}?code=${encodeURIComponent(code)}&state=${encodeURIComponent(q.get('state')!)}`
      return new Response(null, { status: 302, headers: { location: back } })
    }
    if (url.pathname === '/api/auth/app/token' && req.method === 'POST') {
      const body = await req.json() as { code: string, codeVerifier: string, appVersion?: string }
      const grant = grants.get(body.code)
      const pkce = !!grant && b64url(createHash('sha256').update(body.codeVerifier).digest()) === grant.challenge
      log(`POST token known=${!!grant} pkce=${pkce} used=${grant?.used} appVersion=${body.appVersion}`)
      if (!grant || grant.used || !pkce) {
        return Response.json({ error: { code: 'invalid_grant', message: 'код неверный, истёк или уже использован' } }, { status: 400 })
      }
      grant.used = true
      const token = `dwd_mock_${b64url(randomBytes(24))}`
      tokens.add(token)
      return Response.json({
        token,
        device: { id: '0193b0c4-0000-7000-8000-000000000001', name: grant.device, platform: grant.platform, appVersion: body.appVersion ?? null, createdAt: new Date().toISOString(), lastSeenAt: null },
        user: { id: '0193b0c4-0000-7000-8000-0000000000aa', name: 'Проверка оболочки', email: null, avatarUrl: null },
      })
    }
    // Ссылка на файл трека, как `GET /api/tracks/{id}/stream`: id — путь файла у тестового сервера
    const stream = url.pathname.match(/^\/api\/tracks\/([^/]+)\/stream$/)
    if (stream) {
      const file = decodeURIComponent(stream[1]!)
      log(`GET stream ${file} auth=${req.headers.has('authorization')}`)
      const head = await fetch(`${audio}/${file}`, { method: 'HEAD' }).catch(() => null)
      if (!head?.ok) return Response.json({ error: { code: 'not_found', message: 'нет такого трека' } }, { status: 404 })
      return Response.json({
        status: 'ready', url: `${audio}/${file}?sig=${Date.now()}`, expiresAt: new Date(Date.now() + 6 * 3600_000).toISOString(),
        mimeType: 'audio/ogg', loudnessLufs: -20, truePeakDb: -6, durationMs: null, waveform: null,
      })
    }
    if (url.pathname === '/api/auth/logout' && req.method === 'POST') {
      const token = (req.headers.get('authorization') ?? '').replace(/^Bearer /, '')
      log(`POST logout known=${tokens.has(token)}`)
      tokens.delete(token)
      return new Response(null, { status: 204 })
    }
    log(`${req.method} ${url.pathname} 404`)
    return new Response('not found', { status: 404 })
  },
})

console.log(`стенд http://localhost:3499, заглушка сервера http://localhost:3498, отчёты ${out}`)
