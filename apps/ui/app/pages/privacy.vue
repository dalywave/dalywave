<script setup lang="ts">
// Политика конфиденциальности — статическая страница интерфейса: открыта без входа (ссылка со страницы
// входа, из «О приложении» и из карточки App Store: https://<сервер>/privacy). Текст один на все
// серверы DalyWave: данные хранит сервер, который выбрал человек, приложение ничего не отправляет
// третьим сторонам. Текст — pages.privacy в словарях. Язык — как у интерфейса; ?lang=<код> — любой
// из языков интерфейса (?lang=en — ссылка для App Store)
import type { Locale } from '~/utils/format'

definePageMeta({ layout: 'bare' })

const route = useRoute()
const i18n = useNuxtApp().$i18n
const host = typeof location !== 'undefined' ? location.host : ''

const wanted = computed<Locale>(() => {
  const q = route.query.lang
  return i18n.localeCodes.value.find(c => c === q) ?? i18n.locale.value
})
// Словарь другого языка подгружается до показа: пока его нет — прежний язык
const lang = ref<Locale>(i18n.locale.value)

const tp = (key: string) => i18n.t(`pages.privacy.${key}`, {}, { locale: lang.value })
// Сколько разделов и абзацев — по словарю языка (нет раздела — по английскому); тексты — через tp
type Shape = { paragraphs: unknown[] }[]
function shape(l: Locale): Shape {
  const page = (code: Locale) => (i18n.getLocaleMessage(code) as { pages?: { privacy?: { sections?: Shape } } }).pages?.privacy?.sections
  return page(l) ?? page('en') ?? []
}
const sections = computed(() => shape(lang.value).map((s, i) => ({
  title: tp(`sections.${i}.title`),
  paragraphs: s.paragraphs.map((_, j) => tp(`sections.${i}.paragraphs.${j}`)),
})))
const locales = computed(() => i18n.locales.value.map(l => ({ code: l.code, name: l.name ?? l.code })))
useHead({ title: () => tp('title'), htmlAttrs: { lang: () => lang.value } })

async function show(l: Locale) {
  await i18n.loadLocaleMessages(l)
  lang.value = l
}
watch(wanted, show)
await show(wanted.value)

function back() {
  if (window.history.length > 1) window.history.back()
  else void navigateTo('/')
}
</script>

<template>
  <main class="pv" :lang="lang">
    <button type="button" class="pv__back" @click="back">
      <Icon name="ph:caret-left" aria-hidden="true" />{{ tp('back') }}
    </button>
    <BrandGlyph class="pv__glyph" />
    <h1 class="pv__title">{{ tp('title') }}</h1>
    <p class="pv__meta">{{ tp('updated') }}<template v-if="host"> · {{ host }}</template></p>
    <p class="pv__lead">{{ tp('lead') }}</p>
    <section v-for="s in sections" :key="s.title" class="pv__sec">
      <h2 class="pv__h2">{{ s.title }}</h2>
      <p v-for="(p, i) in s.paragraphs" :key="i" class="pv__p">{{ p }}</p>
    </section>
    <nav class="pv__langs" :aria-label="tp('languages')">
      <NuxtLink v-for="l in locales" :key="l.code" :to="{ query: { lang: l.code } }" :lang="l.code" :aria-current="lang === l.code ? 'page' : undefined">{{ l.name }}</NuxtLink>
    </nav>
  </main>
</template>

<style scoped>
.pv {
  width: min(680px, 100%);
  min-height: 100dvh;
  margin: 0 auto;
  padding: calc(var(--safe-top) + var(--titlebar-size, 0px) + 24px) 24px calc(var(--safe-bottom) + 48px);
  background: var(--stage);
  color: var(--text-1);
}
.pv__back { display: inline-flex; align-items: center; gap: 4px; min-height: 44px; margin-left: -8px; padding: 0 8px; border: 0; border-radius: var(--r-2); background: none; color: var(--text-2); font: inherit; font-size: var(--fs-small); cursor: pointer; }
.pv__back:hover { color: var(--text-1); }
.pv__back:focus-visible { outline: none; box-shadow: 0 0 0 2px var(--focus); }
.pv__glyph { display: block; width: 36px; height: 36px; margin-top: 24px; color: var(--text-1); }
.pv__title { margin: 16px 0 0; font-size: var(--fs-h1); font-weight: 750; letter-spacing: var(--tr-display); line-height: 1.05; }
.pv__meta { margin: 8px 0 0; color: var(--text-3); font-size: var(--fs-caption); }
.pv__lead { margin: 24px 0 0; color: var(--text-1); font-size: 17px; line-height: 1.55; }
.pv__sec { margin-top: 32px; }
.pv__h2 { margin: 0 0 8px; font-size: 17px; font-weight: 700; }
.pv__p { margin: 0 0 10px; color: var(--text-2); font-size: 15px; line-height: 1.6; }
.pv__langs { display: flex; gap: 4px; margin-top: 40px; }
.pv__langs a { display: inline-flex; align-items: center; min-height: 44px; padding: 0 12px; border-radius: var(--r-pill); color: var(--text-3); font-size: var(--fs-small); text-decoration: none; }
.pv__langs a[aria-current='page'] { background: var(--fill-1); color: var(--text-1); }
.pv__langs a:focus-visible { outline: none; box-shadow: 0 0 0 2px var(--focus); }

@media (max-width: 699px) {
  .pv { padding-left: 20px; padding-right: 20px; }
  .pv__title { font-size: 34px; }
}
</style>
