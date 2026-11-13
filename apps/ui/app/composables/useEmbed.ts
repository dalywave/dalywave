import type { EmbedTheme } from '~/utils/export'
import type { ShareKind } from './useShare'

export interface EmbedTarget {
  kind: ShareKind
  id: string
  title: string
  /** Свой плейлист: «Можно встраивать» включается прямо в окне */
  owner: boolean
}

/** Адрес плеера для чужого сайта */
export function embedUrl(t: Pick<EmbedTarget, 'kind' | 'id'>, theme: EmbedTheme, lang: string): string {
  const q = new URLSearchParams({ theme })
  if (lang !== 'ru') q.set('lang', lang)
  return `${siteOrigin()}/embed/${t.kind}/${t.id}?${q}`
}

/** «Поделиться → Встроить»: одно окно на приложение (MusicEmbedDialog в app.vue) */
export function useEmbed() {
  const target = useState<EmbedTarget | null>('embed-target', () => null)
  const open = useState('embed-open', () => false)
  function show(t: EmbedTarget) {
    target.value = t
    open.value = true
  }
  return { target, open, show }
}
