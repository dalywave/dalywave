import tailwindcss from '@tailwindcss/vite'

// Один интерфейс для сайта и приложений Tauri: SPA, всё нужное для работы без сети — внутри сборки
export default defineNuxtConfig({
  compatibilityDate: '2026-11-01',
  ssr: false,
  devtools: { enabled: false },

  modules: ['@pinia/nuxt', '@nuxtjs/i18n', '@nuxt/icon', '@nuxt/fonts', '@nuxt/eslint', '@vueuse/nuxt'],

  css: ['~/assets/css/main.css'],

  app: {
    head: {
      htmlAttrs: { lang: 'ru' },
      title: 'DalyWave',
      meta: [
        { name: 'viewport', content: 'width=device-width, initial-scale=1, viewport-fit=cover' },
        { name: 'color-scheme', content: 'dark light' },
      ],
    },
  },

  devServer: { port: 3400 },

  // Локально API живёт на :8400; cookie сессии ставится для localhost:3400
  nitro: {
    devProxy: {
      '/api': { target: 'http://localhost:8400/api', ws: true },
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
    clientBundle: { scan: true, sizeLimitKb: 512 },
  },

  eslint: {
    config: { stylistic: false },
  },
})
