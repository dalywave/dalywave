// Тема оформления: выбор человека (stores/prefs.ts) → тема страницы (data-theme на <html>).
// Скрипт THEME_BOOT ставится в <head> (nuxt.config) и выставляет тему до первой отрисовки —
// без вспышки тёмного на светлой теме; дальше тему ведёт plugins/theme.client.ts.

export type ResolvedTheme = 'dark' | 'light'

/** Цвет основы страницы — для theme-color браузера и фона окна приложения */
export const THEME_BG: Record<ResolvedTheme, string> = { dark: '#08090c', light: '#ffffff' }

export function resolveTheme(pref: string | undefined, systemLight: boolean): ResolvedTheme {
  if (pref === 'light' || pref === 'dark') return pref
  if (pref === 'system') return systemLight ? 'light' : 'dark'
  return 'dark'
}

/** Тема на корне документа: атрибут для токенов, color-scheme для системных частей, theme-color */
export function applyTheme(theme: ResolvedTheme) {
  const root = document.documentElement
  root.dataset.theme = theme
  root.style.colorScheme = theme
  let meta = document.querySelector<HTMLMetaElement>('meta[name="theme-color"]')
  if (!meta) {
    meta = document.createElement('meta')
    meta.name = 'theme-color'
    document.head.appendChild(meta)
  }
  meta.content = THEME_BG[theme]
}

/** Самостоятельный скрипт для <head>: те же правила, что resolveTheme/applyTheme, без импорта */
export const THEME_BOOT = `(function(){var t='dark';try{var p=(JSON.parse(localStorage.getItem('dw.prefs')||'{}')||{}).theme;t=p==='light'||p==='dark'?p:p==='system'&&matchMedia('(prefers-color-scheme: light)').matches?'light':'dark'}catch(e){}var r=document.documentElement;r.dataset.theme=t;r.style.colorScheme=t;var m=document.createElement('meta');m.name='theme-color';m.content=t==='light'?'${THEME_BG.light}':'${THEME_BG.dark}';document.head.appendChild(m)})()`
