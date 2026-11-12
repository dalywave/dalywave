import type { MenuAction } from '~/types/ui'

export type ShareKind = 'track' | 'album' | 'artist' | 'playlist'

/** Публичный адрес сервиса: ссылки уходят наружу, поэтому не адрес окна приложения (tauri://…) */
const PUBLIC_URL = 'https://wave.daly.rest'

/** Адрес страницы на сайте: на самом сайте — его же origin (в разработке — localhost) */
export function shareUrl(kind: ShareKind, id: string): string {
  const site = typeof location !== 'undefined' && /^https?:$/.test(location.protocol) && !('__TAURI_INTERNALS__' in window)
    ? location.origin
    : PUBLIC_URL
  return `${site}/${kind}/${id}`
}

/** «Поделиться»: на телефоне — системное меню, на компьютере — ссылка в буфер обмена и уведомление */
export function useShare() {
  const bridge = useBridge()
  const toast = useToast()
  const ui = useUiStore()
  const t = useNuxtApp().$i18n.t

  async function copy(text: string) {
    // Приложение для компьютера: буфер обмена из Rust — WebView требует жеста, а пункт меню
    // срабатывает уже после закрытия меню
    if (bridge.kind === 'tauri' && bridge.isDesktopApp) {
      const { invoke } = await import('@tauri-apps/api/core')
      await invoke('plugin:clipboard-manager|write_text', { text })
      return
    }
    await navigator.clipboard.writeText(text)
  }

  /** `text` — цитата перед ссылкой (строка песни); `copyOnly` — сразу в буфер, без системного меню */
  async function share(kind: ShareKind, id: string, title?: string, text?: string, opts: { copyOnly?: boolean } = {}) {
    const url = shareUrl(kind, id)
    const touch = ui.isPhone || bridge.platform === 'ios' || bridge.platform === 'ipados'
    if (touch && !opts.copyOnly && typeof navigator.share === 'function') {
      try {
        await navigator.share(text ? { title, text, url } : { title, url })
        return
      }
      catch (e) {
        // Закрыли системное меню — это не ошибка
        if (e instanceof DOMException && e.name === 'AbortError') return
      }
    }
    try {
      await copy(text ? `${text}\n${url}` : url)
      bridge.haptic('light')
      toast.show(text ? { text: t('share.textCopied'), icon: 'ph:copy' } : { text: t('share.copied'), icon: 'ph:link' })
    }
    catch {
      toast.error(t('share.failed'))
    }
  }

  /** Пункт «Поделиться» для меню ⋯ трека, альбома, плейлиста, артиста */
  function entry(kind: ShareKind, id: string, title?: string): MenuAction {
    return { label: t('share.action'), icon: 'ph:export', onSelect: () => void share(kind, id, title) }
  }

  return { share, entry, url: shareUrl }
}
