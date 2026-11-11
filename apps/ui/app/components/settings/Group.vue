<script setup lang="ts">
// Группа настроек — общий кирпич страницы настроек и разделов, которые в неё встраиваются
// (например, <DownloaderSettings />). Как собирать:
//
//   <SettingsGroup :title="t('playback')">
//     <SettingsRow :title="t('normalize')">
//       <UiSwitch v-model="…" :label="t('normalize')" hide-label />   ← элемент управления справа
//     </SettingsRow>
//     <SettingsRow :title="…" stack>…широкий выбор: на телефоне уходит под подпись…</SettingsRow>
//     <SettingsRow indent>…подстрока, относится к строке выше…</SettingsRow>
//     <SettingsRow as="button" radio :checked="…" :title="…" @click="…" />  ← вариант с кружком
//   </SettingsGroup>
//
// Простой список, как в Spotify и Apple Music: заголовок, строки «подпись слева — управление справа»,
// между строками волоски. Без карточек, значков и поясняющих абзацев; description у строки —
// только если без него не понять, и в одну строку
withDefaults(defineProps<{
  title?: string
  /** Короткая сноска под группой — только если без неё не обойтись */
  footnote?: string
}>(), {})
</script>

<template>
  <section class="sg" :aria-label="title">
    <h2 v-if="title || $slots.title" class="sg__title"><slot name="title">{{ title }}</slot></h2>
    <div class="sg__list">
      <slot />
    </div>
    <p v-if="footnote || $slots.footnote" class="sg__note"><slot name="footnote">{{ footnote }}</slot></p>
  </section>
</template>

<style scoped>
.sg { display: flex; flex-direction: column; min-width: 0; }
.sg__title { display: flex; align-items: center; gap: 8px; margin: 0 0 4px; font-size: var(--fs-h4); font-weight: 700; letter-spacing: -0.01em; color: var(--text-1); }
.sg__list { display: flex; flex-direction: column; min-width: 0; }
.sg__note { margin: 6px 0 0; font-size: var(--fs-caption); line-height: 1.45; color: var(--text-3); }
</style>
