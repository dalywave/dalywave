// «Слушать вместе»: управление сессией хозяина (его сессия — cookie или токен приложения) и клиент
// гостя со своим токеном `dwj_…`, который действует только на /api/jam/guest/* этой сессии.
import type { JamBridge, JamGuestClient } from '../types'
import { createHttp, done, unwrap, type Http } from './http'
import { createLive } from './live'

export function createJam(http: Http): JamBridge {
  const { api } = http

  function guest(token: string): JamGuestClient {
    // Отдельный клиент: токен гостя не должен уходить на другие пути, а сессия сайта — сюда
    const g = createHttp({ baseUrl: http.baseUrl || undefined, token: async () => token })
    const live = createLive({
      url: async () => {
        const base = http.baseUrl ? http.baseUrl.replace(/^http/, 'ws') : location.origin.replace(/^http/, 'ws')
        return `${base}/api/jam/guest/ws?token=${encodeURIComponent(token)}`
      },
    })
    return {
      view: () => unwrap(g.api.GET('/api/jam/guest')),
      search: async q => (await unwrap(g.api.GET('/api/jam/guest/search', { params: { query: { q } } }))).items,
      add: trackId => unwrap(g.api.POST('/api/jam/guest/queue', { body: { trackId } })),
      react: () => done(g.api.POST('/api/jam/guest/react', { body: { kind: 'heart' } })),
      skip: () => unwrap(g.api.POST('/api/jam/guest/skip')),
      stream: trackId => unwrap(g.api.GET('/api/jam/guest/stream/{trackId}', { params: { path: { trackId } } })),
      leave: () => done(g.api.POST('/api/jam/guest/leave')),
      live,
    }
  }

  return {
    get: async () => (await unwrap(api.GET('/api/jam'))).jam,
    start: () => unwrap(api.POST('/api/jam')),
    update: patch => unwrap(api.PATCH('/api/jam', { body: patch })),
    end: () => done(api.DELETE('/api/jam')),
    removeGuest: id => done(api.DELETE('/api/jam/guests/{id}', { params: { path: { id } } })),
    approve: id => unwrap(api.POST('/api/jam/requests/{id}/approve', { params: { path: { id } } })),
    decline: id => unwrap(api.POST('/api/jam/requests/{id}/decline', { params: { path: { id } } })),
    invite: code => unwrap(api.GET('/api/jam/join/{code}', { params: { path: { code } } })),
    join: (code, name) => unwrap(api.POST('/api/jam/join/{code}', { params: { path: { code } }, body: { name } })),
    guest,
  }
}
