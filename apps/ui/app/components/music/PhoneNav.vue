<script setup lang="ts">
// Верхняя строка вложенной страницы на телефоне (альбом, артист, плейлист, «Любимые», «Мой вкус»):
// «назад» слева, второстепенные действия справа. Прозрачная над шапкой; когда название страницы
// уезжает под неё — стекло и название по центру, как в Apple Music. На планшете под пальцем — та же
// строка на всю ширину колонки: верхней строки компьютера там нет
const props = withDefaults(defineProps<{
  title?: string
  /** Название шапки ушло под строку: показать его здесь */
  condensed?: boolean
  /** Куда вернуться, если страницу открыли по ссылке и истории нет */
  fallback?: string
}>(), { title: '', condensed: undefined, fallback: '/' })

const router = useRouter()
const ui = useUiStore()
const { t } = useI18n()

// Без шапки (загрузка, ошибка) стекло включается от прокрутки
const scrolled = ref(false)
function onScroll() {
  scrolled.value = window.scrollY > 8
}
onMounted(() => {
  if (props.condensed !== undefined) return
  window.addEventListener('scroll', onScroll, { passive: true })
  onScroll()
})
onBeforeUnmount(() => window.removeEventListener('scroll', onScroll))
const solid = computed(() => props.condensed ?? scrolled.value)

function back() {
  const state = window.history.state as { back?: string | null } | null
  if (state?.back) router.back()
  else void navigateTo(props.fallback, { replace: true })
}
</script>

<template>
  <div class="pn" :class="{ 'is-solid': solid, 'pn--touch': ui.isTouch }">
    <UiIconButton class="pn__back" icon="ph:caret-left" :label="t('nav.back')" size="lg" :tooltip="false" @click="back" />
    <div class="pn__title ellipsis" :aria-hidden="!solid">{{ title }}</div>
    <div class="pn__tools"><slot /></div>
  </div>
</template>

<style scoped>
.pn {
  position: sticky;
  top: 0;
  z-index: 30;
  display: grid;
  /* Края равны, пока справа хватает места: название остаётся по центру экрана */
  grid-template-columns: minmax(max-content, 1fr) minmax(0, 2fr) minmax(max-content, 1fr);
  align-items: center;
  gap: 8px;
  height: calc(var(--safe-top) + 52px);
  margin-top: calc(var(--safe-top) * -1);
  padding: var(--safe-top) 8px 0;
  transition: background-color var(--dur-3), box-shadow var(--dur-3);
}
.pn.is-solid {
  background: var(--glass-bar);
  -webkit-backdrop-filter: blur(30px) saturate(1.4);
  backdrop-filter: blur(30px) saturate(1.4);
  box-shadow: 0 1px 0 var(--hair);
}
.pn__back { justify-self: start; color: var(--text-1); }
.pn__title {
  max-width: 100%;
  font-size: var(--fs-body);
  font-weight: 650;
  text-align: center;
  opacity: 0;
  transform: translateY(4px);
  transition: opacity var(--dur-2), transform var(--dur-2) var(--ease-out);
}
.pn.is-solid .pn__title { opacity: 1; transform: none; }
.pn__tools { display: flex; align-items: center; justify-content: flex-end; justify-self: end; gap: 0; }
.pn__tools :deep(.ib) { color: var(--text-1); }
@media (min-width: 700px) {
  .pn:not(.pn--touch) { display: none; }
}
/* Планшет: строка выходит за поля колонки (32 px), под ней — сама шапка страницы */
.pn--touch { margin: calc(-20px - var(--safe-top)) -32px 0; padding: var(--safe-top) 20px 0; height: calc(var(--safe-top) + 60px); }
.pn--touch .pn__title { font-size: var(--fs-h4); }
</style>
