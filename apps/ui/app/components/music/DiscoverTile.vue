<script setup lang="ts">
// Вход в «Открытия» (лента клипов): строка-плитка на главной и в поиске. Ровная заливка, знак ленты
// слева, подпись и стрелка — как остальные строки-ссылки
const { t } = useScopedI18n('components.music.DiscoverTile')
// Лента ищет новое в каталоге Deezer: без него входа в неё нет
const server = useServerStore()
</script>

<template>
  <NuxtLink v-if="server.deezer" to="/discover" class="dt">
    <span class="dt__art" aria-hidden="true">
      <i class="dt__card dt__card--back" />
      <i class="dt__card"><Icon name="ph:play-fill" /></i>
    </span>
    <span class="dt__text">
      <span class="dt__title">{{ t('title') }}</span>
      <span class="dt__sub">{{ t('sub') }}</span>
    </span>
    <Icon name="ph:caret-right" class="dt__chev" />
  </NuxtLink>
</template>

<style scoped>
.dt {
  display: flex;
  align-items: center;
  gap: 14px;
  min-height: 72px;
  padding: 10px 16px 10px 10px;
  border-radius: var(--r-3);
  background: var(--fill-1);
  color: var(--text-1);
  -webkit-touch-callout: none;
  transition: background-color var(--dur-1), transform var(--dur-1) var(--ease-out);
}
.dt:hover { background: var(--fill-2); }
.dt:active { transform: scale(0.985); }
.dt:focus-visible { outline: 2px solid var(--focus); outline-offset: 2px; }
/* Две карточки ленты одна за другой — знак «листать вверх» */
.dt__art { position: relative; width: 52px; height: 52px; flex: none; }
.dt__card {
  position: absolute;
  left: 12px;
  top: 4px;
  display: grid;
  place-items: center;
  width: 28px;
  height: 44px;
  border-radius: 7px;
  background: var(--text-1);
  color: var(--ink-1);
  font-size: 13px;
}
.dt__card--back { left: 18px; top: 0; height: 40px; opacity: 0.32; transform: rotate(8deg); }
.dt__text { display: flex; flex-direction: column; flex: 1; min-width: 0; gap: 2px; }
.dt__title { font-size: 16px; font-weight: 700; letter-spacing: -0.01em; }
.dt__sub { font-size: var(--fs-small); color: var(--text-2); }
.dt__chev { flex: none; font-size: 18px; color: var(--text-3); }
</style>
