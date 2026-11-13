// Выгрузка медиатеки и встраиваемый плеер: чистые помощники (имя файла, код <iframe>)

export type EmbedSize = 'compact' | 'large'
export type EmbedTheme = 'dark' | 'light'

/** Имя файла из названия — как на сервере: без запрещённых в Windows и macOS символов */
export function exportFileName(title: string, fallback: string): string {
  const cleaned = title
    .replace(/[/\\:*?"<>|]/g, '_')
    // eslint-disable-next-line no-control-regex
    .replace(/[\u0000-\u001F\u007F]/g, ' ')
    .replace(/\s+/g, ' ')
    .replace(/^[.\s]+|[.\s]+$/g, '')
    .slice(0, 80)
    .trim()
  return cleaned || fallback
}

/** Высота плеера: карточка или со списком треков */
export const EMBED_HEIGHT: Record<EmbedSize, number> = { compact: 152, large: 352 }

const escapeAttr = (s: string) => s.replace(/&/g, '&amp;').replace(/"/g, '&quot;').replace(/</g, '&lt;').replace(/>/g, '&gt;')

/** Код для вставки: ширина — по колонке сайта, высота — одна из двух */
export function embedCode(src: string, size: EmbedSize, title: string): string {
  return `<iframe src="${escapeAttr(src)}" width="100%" height="${EMBED_HEIGHT[size]}" frameborder="0" loading="lazy" allow="autoplay; encrypted-media" style="border:0;border-radius:12px" title="${escapeAttr(title ? `${title} — DalyWave` : 'DalyWave')}"></iframe>`
}

