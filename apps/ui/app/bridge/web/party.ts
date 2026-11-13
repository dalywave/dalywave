// Большой экран: режим вечеринки хозяина и вход на телевизоре кодом с экрана (как device flow).
// Телевизор просит код без входа, вошедший человек подтверждает его на /tv, браузер телевизора
// получает свою сессию сайта (cookie ставит сервер в ответе на опрос).
import type { PartyBridge } from '../types'
import { done, unwrap, type Http } from './http'

export function createParty(http: Http): PartyBridge {
  const { api } = http
  return {
    get: () => unwrap(api.GET('/api/party')),
    update: patch => unwrap(api.PATCH('/api/party', { body: patch })),
    code: () => unwrap(api.POST('/api/auth/tv/code', { body: { platform: 'web' } })),
    token: deviceCode => unwrap(api.POST('/api/auth/tv/token', { body: { deviceCode } })),
    lookup: code => unwrap(api.GET('/api/auth/tv/{code}', { params: { path: { code } } })),
    approve: userCode => done(api.POST('/api/auth/tv/approve', { body: { userCode } })),
  }
}
