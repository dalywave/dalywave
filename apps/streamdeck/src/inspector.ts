// Разговор с инспектором свойств (ui/*.html): подключение аккаунта и списки для выбора —
// плейлисты, сохранённые настройки волны, устройства, чарты, поиск альбомов и артистов.
import streamDeck from '@elgato/streamdeck'
import type { JsonValue } from '@elgato/utils'
import { account } from './account'
import { api } from './api'
import { playableDevices, wavePresets } from './catalog'
import { player } from './player'
import type { ChartList, PlaylistList, SearchResults } from './types'

interface Item { label: string, value: string }

async function send(payload: JsonValue): Promise<void> {
  try {
    await streamDeck.ui.sendToPropertyInspector(payload)
  }
  catch {
    // инспектор успели закрыть
  }
}

function accountPayload(): JsonValue {
  return {
    event: 'account',
    connected: account.connected,
    userName: account.settings.userName ?? null,
    server: account.serverUrl,
    status: player.connection,
    pairing: account.pairing as unknown as JsonValue,
  }
}

async function datasource(name: string): Promise<Item[] | null> {
  switch (name) {
    case 'playlists':
    case 'writablePlaylists': {
      const d = await api.get<PlaylistList>('/api/playlists')
      // Добавить трек можно только в свой или совместный плейлист
      const own = name === 'writablePlaylists'
      return d.items
        .filter(p => !own || ((p.kind === 'user') && (p.access ?? 'owner') !== 'viewer'))
        .map(p => ({ label: p.title, value: p.id }))
    }
    case 'presets':
      return (await wavePresets(true)).map(p => ({ label: p.name, value: p.id }))
    case 'devices':
      return playableDevices().map(d => ({ label: d.name, value: d.id }))
    case 'charts': {
      const d = await api.get<ChartList>('/api/charts')
      return d.items.map(c => ({ label: c.title, value: c.id }))
    }
    default:
      return null
  }
}

export function setupInspector(): void {
  account.onChange(() => {
    if (streamDeck.ui.action) void send(accountPayload())
  })
  let lastStatus = player.connection
  player.on('change', () => {
    // Статус связи в инспекторе — только когда он поменялся
    const status = player.connection
    if (status !== lastStatus && streamDeck.ui.action) {
      lastStatus = status
      void send(accountPayload())
    }
  })

  streamDeck.ui.onDidAppear(() => {
    void send(accountPayload())
  })

  streamDeck.ui.onSendToPlugin<JsonValue>(async (ev) => {
    const p = (ev.payload ?? {}) as Record<string, unknown>
    const event = typeof p.event === 'string' ? p.event : ''
    try {
      switch (event) {
        case 'account':
          return send(accountPayload())
        case 'pair':
          return account.start()
        case 'cancelPair':
          return account.cancel()
        case 'disconnect':
          return account.disconnect()
        case 'setServer':
          return account.setServer(String(p.server ?? ''))
        case 'find': {
          const q = String(p.q ?? '').trim()
          const kind = p.kind === 'artist' ? 'artist' : 'album'
          if (!q || !account.connected) return send({ event: 'found', kind, items: [] })
          const r = await api.get<SearchResults>(`/api/search?${new URLSearchParams({ q, types: kind, limit: '12' })}`)
          const items: Item[] = kind === 'album'
            ? r.albums.map(a => ({ label: `${a.title} — ${a.artist.name}`, value: a.id }))
            : r.artists.map(a => ({ label: a.name, value: a.id }))
          return send({ event: 'found', kind, items: items as unknown as JsonValue })
        }
        default: {
          if (!account.connected) return send({ event, items: [] })
          const items = await datasource(event)
          if (items) return send({ event, items: items as unknown as JsonValue })
        }
      }
    }
    catch (e) {
      streamDeck.logger.warn(`inspector ${event}: ${String(e)}`)
      return send({ event, items: [], error: true })
    }
  })
}
