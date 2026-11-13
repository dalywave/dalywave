<script setup lang="ts">
// Экран без оболочки для входа, «ждём доступа» и ошибки входа: знак, заголовок, строка текста и
// действия — без карточки. Фон — живая волна бренда, приглушённая; под текстом она гаснет до ровного
// фона. На телефоне всё у нижнего края, под палец, волна — над текстом; на iPad и компьютере —
// колонкой по центру
withDefaults(defineProps<{
  title: string
  text?: string
}>(), { text: '' })

const ui = useUiStore()
</script>

<template>
  <div class="as">
    <div class="as__wave"><MusicWaveCanvas :layout="ui.isPhone ? 'full-phone' : 'full'" /></div>
    <main class="as__col">
      <BrandGlyph class="as__glyph" />
      <h1 class="as__title">{{ title }}</h1>
      <p v-if="text" class="as__text" aria-live="polite">{{ text }}</p>
      <slot name="extra" />
      <div class="as__actions"><slot /></div>
      <div v-if="$slots.foot" class="as__foot"><slot name="foot" /></div>
    </main>
  </div>
</template>

<style scoped>
.as {
  position: relative;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  min-height: 100vh;
  min-height: 100dvh;
  padding: calc(var(--safe-top) + var(--titlebar-size, 0px) + 32px) 24px calc(var(--safe-bottom) + 32px);
  overflow: hidden;
  background: var(--stage);
}
.as__wave { position: absolute; inset: 0; opacity: 0.6; }
/* Волна под текстом гаснет: снизу и по центру — ровный фон страницы, чтобы читалось */
.as__wave::after { content: ''; position: absolute; inset: 0; pointer-events: none; background: radial-gradient(48% 42% at 50% 50%, var(--stage), color-mix(in srgb, var(--stage) 0%, transparent)); }
.as__col { position: relative; display: flex; flex-direction: column; align-items: center; width: min(380px, 100%); text-align: center; }
.as__glyph { width: 44px; height: 44px; color: var(--text-1); }
.as__title { margin: 20px 0 0; font-size: var(--fs-h1); font-weight: 750; letter-spacing: var(--tr-display); line-height: 1.05; text-wrap: balance; }
.as__text { margin: 10px 0 0; color: var(--text-2); font-size: 15px; line-height: 1.45; text-wrap: balance; }
.as__actions { display: flex; flex-direction: column; align-items: stretch; gap: 10px; width: 100%; margin-top: 32px; }
.as__foot { display: flex; justify-content: center; width: 100%; margin-top: 12px; }

/* iPad под пальцем: экран большой, смотрят на него издали — крупнее */
@media (min-width: 700px) and (pointer: coarse) {
  .as__col { width: min(440px, 100%); }
  .as__glyph { width: 52px; height: 52px; }
  .as__title { font-size: 48px; }
  .as__text { font-size: 17px; }
}

/* Телефон: колонка у нижнего края — главная кнопка под большим пальцем; волна сверху */
@media (max-width: 699px) {
  .as { justify-content: flex-end; padding: calc(var(--safe-top) + 24px) 24px calc(var(--safe-bottom) + 20px); }
  .as__wave { opacity: 0.8; }
  .as__wave::after { background: linear-gradient(to bottom, color-mix(in srgb, var(--stage) 0%, transparent) 30%, var(--stage) 68%); }
  .as__col { align-items: flex-start; width: 100%; text-align: left; }
  .as__glyph { width: 40px; height: 40px; }
  .as__title { margin-top: 16px; font-size: 36px; }
  .as__actions { margin-top: 28px; }
}
</style>
