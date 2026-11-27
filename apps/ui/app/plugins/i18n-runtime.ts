import { composerTranslator, setTranslator } from '~/utils/i18n-runtime'

// Слова утилит (utils/format.ts и соседи) — из тех же словарей, что и у компонентов
export default defineNuxtPlugin((nuxtApp) => {
  setTranslator(composerTranslator(nuxtApp.$i18n))
})
