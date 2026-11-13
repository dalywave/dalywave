// Вход плагина: браузер → код на свой порт → обмен с PKCE. Сервер — поддельный, Stream Deck — заглушка.
import { afterAll, describe, expect, mock, test } from 'bun:test'
import { createHash } from 'node:crypto'

let opened: string[] = []
let stored: Record<string, unknown> = {}

mock.module('@elgato/streamdeck', () => ({
  default: {
    settings: {
      getGlobalSettings: async () => stored,
      setGlobalSettings: async (s: Record<string, unknown>) => {
        stored = { ...s }
      },
    },
    system: { openUrl: async (url: string) => void opened.push(url) },
    logger: { warn: () => {}, info: () => {}, error: () => {} },
  },
}))

const { account } = await import('../src/account')
const { player } = await import('../src/player')

let exchanged: { code: string, codeVerifier: string } | null = null
let challenge = ''
const server = Bun.serve({
  port: 0,
  async fetch(req) {
    const url = new URL(req.url)
    if (url.pathname === '/api/auth/app/token') {
      exchanged = await req.json() as { code: string, codeVerifier: string }
      const ok = exchanged.code === 'good-code'
        && createHash('sha256').update(exchanged.codeVerifier).digest('base64url') === challenge
      if (!ok) return Response.json({ error: { code: 'invalid_grant', message: 'нет' } }, { status: 400 })
      return Response.json({ token: 'dwd_test', device: { id: 'd1' }, user: { name: 'Захар' } })
    }
    if (url.pathname === '/api/auth/logout') return new Response(null, { status: 204 })
    return new Response('нет', { status: 404 })
  },
})

afterAll(() => {
  player.stop()
  server.stop(true)
})

describe('вход через браузер', () => {
  test('код возвращается на свой порт и меняется на токен', async () => {
    stored = { server: `http://127.0.0.1:${server.port}` }
    await account.load()
    expect(String(stored.deviceId)).toStartWith('streamdeck-')
    opened = []
    await account.start()
    expect(opened.length).toBe(1)
    const start = new URL(opened[0]!)
    expect(start.pathname).toBe('/api/auth/app/start')
    expect(start.searchParams.get('platform')).toBe('streamdeck')
    expect(start.searchParams.get('code_challenge_method')).toBe('S256')
    challenge = start.searchParams.get('code_challenge')!
    const redirect = start.searchParams.get('redirect_uri')!
    expect(redirect).toMatch(/^http:\/\/127\.0\.0\.1:\d+\/callback$/)
    expect(account.pairing.phase).toBe('waiting')

    // Чужой state не принимается
    const bad = await fetch(`${redirect}?code=good-code&state=wrong`)
    expect(bad.status).toBe(400)
    expect(account.pairing.phase).toBe('error')
    expect(stored.token ?? null).toBeNull()

    // Заново — и браузер вернулся с верным кодом
    opened = []
    await account.start()
    const again = new URL(opened[0]!)
    challenge = again.searchParams.get('code_challenge')!
    const back = await fetch(`${again.searchParams.get('redirect_uri')}?code=good-code&state=${again.searchParams.get('state')}`)
    expect(back.status).toBe(200)
    expect(await back.text()).toContain('Stream Deck')
    expect(stored.token).toBe('dwd_test')
    expect(stored.userName).toBe('Захар')
    expect(account.pairing.phase).toBe('idle')
    expect(player.paired).toBe(true)
  })

  test('«Отключить» закрывает сессию и забывает токен', async () => {
    await account.disconnect()
    expect(stored.token).toBeNull()
    expect(player.paired).toBe(false)
  })
})
