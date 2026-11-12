import tailwindcss from '@tailwindcss/vite'

// Данные дизайн-макетов (обложки, превью) — только для моков в разработке
const fixtures = decodeURIComponent(new URL('../../design/fixtures', import.meta.url).pathname)

// Типов Node в проекте нет: переменная окружения — через globalThis
const devApi = (globalThis as { process?: { env: Record<string, string | undefined> } }).process?.env.DALYWAVE_DEV_API ?? 'http://localhost:8400'

// Один интерфейс для сайта и приложений Tauri: SPA, всё нужное для работы без сети — внутри сборки
export default defineNuxtConfig({
  compatibilityDate: '2026-11-01',
  ssr: false,
  devtools: { enabled: false },

  modules: [
    '@pinia/nuxt',
    '@nuxtjs/i18n',
    '@nuxt/icon',
    '@nuxt/fonts',
    '@nuxt/eslint',
    '@vueuse/nuxt',
    // Галерея компонентов нужна только для разработки и проверок скриншотами
    (_options, nuxt) => {
      if (nuxt.options.dev) return
      nuxt.hook('pages:extend', (pages) => {
        const keep = pages.filter(p => !p.path.startsWith('/dev'))
        pages.splice(0, pages.length, ...keep)
      })
    },
  ],

  css: ['~/assets/css/main.css'],

  app: {
    head: {
      htmlAttrs: { 'lang': 'ru', 'data-theme': 'dark', 'data-mood': 'any' },
      title: 'DalyWave',
      meta: [
        { name: 'viewport', content: 'width=device-width, initial-scale=1, viewport-fit=cover' },
        { name: 'color-scheme', content: 'dark' },
        { name: 'theme-color', content: '#08090c' },
      ],
      link: [
        { rel: 'icon', type: 'image/svg+xml', href: '/favicon.svg' },
        { rel: 'icon', type: 'image/png', sizes: '32x32', href: '/favicon-32.png' },
        { rel: 'apple-touch-icon', href: '/apple-touch-icon.png' },
      ],
    },
  },

  devServer: { port: 3400 },

  // Локально API живёт на :8400; cookie сессии ставится для localhost:3400.
  // Второй API (другая ветка, своя база) — DALYWAVE_DEV_API=http://localhost:8410
  nitro: {
    devProxy: {
      '/api': { target: `${devApi}/api`, ws: true },
    },
  },

  $development: {
    nitro: {
      publicAssets: [{ dir: fixtures, baseURL: '/_fixtures', maxAge: 0 }],
    },
  },

  vite: {
    plugins: [tailwindcss()],
    server: { strictPort: true },
  },

  typescript: { strict: true },

  i18n: {
    defaultLocale: 'ru',
    strategy: 'no_prefix',
    detectBrowserLanguage: false,
    locales: [
      { code: 'ru', language: 'ru-RU', name: 'Русский', file: 'ru.json' },
      { code: 'en', language: 'en-US', name: 'English', file: 'en.json' },
    ],
  },

  // Иконки только из сборки: приложения должны работать без сети
  icon: {
    provider: 'none',
    serverBundle: false,
    fallbackToApi: false,
    clientBundle: {
      scan: { globInclude: ['app/**/*.{vue,ts}'] },
      sizeLimitKb: 512,
    },
  },

  // Onest лежит в сборке (@fontsource-variable/onest); модулю шрифтов искать его не нужно
  fonts: {
    families: [{ name: 'Onest Variable', provider: 'none' }],
  },

  eslint: {
    config: { stylistic: false },
  },
})
