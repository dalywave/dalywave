import { ApiError } from '~/api/types'
import type { MenuAction } from '~/types/ui'

export type ListFormat = 'csv' | 'm3u8' | 'json'

/** «Экспорт»: медиатека, любимые или плейлист — в файл. Сохраняет мост: сайт — загрузкой браузера,
 *  приложение для компьютера — окном «Сохранить как», iPhone и iPad — системным «Поделиться» */
export function useExport() {
  const bridge = useBridge()
  const toast = useToast()
  const t = useNuxtApp().$i18n.t

  async function save(path: string, name: string): Promise<boolean> {
    try {
      const result = await bridge.exports.save(path, name)
      if (result === 'saved') {
        bridge.haptic('success')
        // Сайт отдаёт файл браузеру: дальше его загрузку видно в самом браузере
        toast.show({ text: bridge.kind === 'web' ? t('export.started') : t('export.saved'), icon: 'ph:check-circle' })
      }
      return result !== 'cancelled'
    }
    catch (e) {
      toast.error(e instanceof ApiError && e.status === 429 ? t('export.busy') : t('export.failed'))
      return false
    }
  }

  function playlist(id: string, title: string, format: ListFormat) {
    return save(`/api/export/playlists/${id}?format=${format}`, `${exportFileName(title, 'Плейлист')}.${format}`)
  }

  function liked(format: ListFormat) {
    return save(`/api/export/liked?format=${format}`, `${exportFileName(t('nav.liked'), 'liked')}.${format}`)
  }

  function library(format: 'zip' | 'json', history: boolean) {
    const date = new Date().toISOString().slice(0, 10)
    return save(`/api/export/library?format=${format}${history ? '&history=true' : ''}`, `DalyWave ${date}.${format}`)
  }

  /** Пункт меню «⋯» со списком форматов */
  function entry(run: (format: ListFormat) => unknown): MenuAction {
    const formats: ListFormat[] = ['csv', 'm3u8', 'json']
    return {
      label: t('export.action'),
      icon: 'ph:file-arrow-down',
      children: formats.map(f => ({ label: t(`export.${f}`), hint: t(`export.${f}Hint`), onSelect: () => void run(f) })),
    }
  }

  return { save, playlist, liked, library, entry }
}
