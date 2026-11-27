<script setup lang="ts">
// Календарь «Перейти к дате» в истории: месяц сеткой с понедельника, дни с прослушиваниями — точкой,
// без них — неактивны. Листается между первым месяцем с прослушиваниями и текущим
const props = defineProps<{
  /** Дни с прослушиваниями, `YYYY-MM-DD` по местному времени */
  days: Set<string>
  /** Выбранный день */
  selected: string | null
}>()
const emit = defineEmits<{ pick: [day: string] }>()

const lang = useLang()
const { t } = useScopedI18n('components.history.Calendar')

const pad = (n: number) => String(n).padStart(2, '0')
const keyOf = (d: Date) => `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`
const today = keyOf(new Date())

const first = computed(() => [...props.days].sort()[0] ?? today)
const monthOf = (key: string) => key.slice(0, 7)
// Открывается на выбранном месяце, иначе — на текущем
const month = ref(monthOf(props.selected ?? today))
watch(() => props.selected, (s) => {
  if (s) month.value = monthOf(s)
})

const canPrev = computed(() => month.value > monthOf(first.value))
const canNext = computed(() => month.value < monthOf(today))
function shift(by: number) {
  const [y, m] = month.value.split('-').map(Number) as [number, number]
  const d = new Date(y, m - 1 + by, 1)
  month.value = `${d.getFullYear()}-${pad(d.getMonth() + 1)}`
}

const locale = computed(() => intlLocale(lang.value, 'en-GB'))
const title = computed(() => {
  const [y, m] = month.value.split('-').map(Number) as [number, number]
  const s = new Intl.DateTimeFormat(locale.value, { month: 'long', year: 'numeric' }).format(new Date(y, m - 1, 1))
  // «ноябрь 2026 г.» / «листопад 2026 р.» — без «г.» и «р.»
  return s.charAt(0).toUpperCase() + s.slice(1).replace(/\s*[гр]\.$/, '')
})
const weekdays = computed(() => {
  const f = new Intl.DateTimeFormat(locale.value, { weekday: 'short' })
  // 5 января 2026 — понедельник
  return Array.from({ length: 7 }, (_, i) => f.format(new Date(2026, 0, 5 + i)).replace('.', ''))
})

interface Cell { key: string, day: number, has: boolean, future: boolean }
const cells = computed<(Cell | null)[]>(() => {
  const [y, m] = month.value.split('-').map(Number) as [number, number]
  const start = new Date(y, m - 1, 1)
  const lead = (start.getDay() + 6) % 7
  const count = new Date(y, m, 0).getDate()
  const out: (Cell | null)[] = Array.from({ length: lead }, () => null)
  for (let d = 1; d <= count; d++) {
    const key = `${y}-${pad(m)}-${pad(d)}`
    out.push({ key, day: d, has: props.days.has(key), future: key > today })
  }
  return out
})

const fullDate = (key: string) => {
  const [y, m, d] = key.split('-').map(Number) as [number, number, number]
  return new Intl.DateTimeFormat(locale.value, { day: 'numeric', month: 'long', year: 'numeric' }).format(new Date(y, m - 1, d))
}
</script>

<template>
  <div class="cal">
    <header class="cal__head">
      <UiIconButton icon="ph:caret-left" :label="t('prev')" size="sm" :disabled="!canPrev" :tooltip="false" @click="shift(-1)" />
      <span class="cal__title">{{ title }}</span>
      <UiIconButton icon="ph:caret-right" :label="t('next')" size="sm" :disabled="!canNext" :tooltip="false" @click="shift(1)" />
    </header>
    <div class="cal__grid" role="grid" :aria-label="title">
      <span v-for="w in weekdays" :key="w" class="cal__wd" aria-hidden="true">{{ w }}</span>
      <template v-for="(c, i) in cells" :key="c?.key ?? `pad${i}`">
        <span v-if="!c" class="cal__pad" />
        <button
          v-else
          type="button"
          class="cal__day tnum"
          :class="{ 'is-today': c.key === today, 'is-on': c.key === selected, 'has-plays': c.has }"
          :disabled="!c.has || c.future"
          :aria-pressed="c.key === selected"
          :aria-label="fullDate(c.key)"
          @click="emit('pick', c.key)"
        >
          {{ c.day }}
        </button>
      </template>
    </div>
  </div>
</template>

<style scoped>
.cal { display: flex; flex-direction: column; gap: 8px; }
.cal__head { display: flex; align-items: center; justify-content: space-between; gap: 8px; }
.cal__title { font-size: var(--fs-body); font-weight: 650; }
.cal__grid { display: grid; grid-template-columns: repeat(7, minmax(0, 1fr)); gap: 2px; }
.cal__wd { display: grid; place-items: center; height: 28px; font-size: var(--fs-caption); font-weight: 600; color: var(--text-3); text-transform: lowercase; }
.cal__day {
  position: relative;
  justify-self: center;
  width: 100%;
  max-width: 52px;
  display: grid;
  place-items: center;
  aspect-ratio: 1;
  min-height: 36px;
  padding: 0;
  border: 0;
  border-radius: 50%;
  background: none;
  font: inherit;
  font-size: var(--fs-small);
  font-weight: 550;
  color: var(--text-1);
  transition: background-color var(--dur-1), color var(--dur-1);
}
.cal__day:disabled { color: var(--text-3); opacity: 0.55; }
.cal__day.has-plays:not(:disabled)::after { content: ''; position: absolute; bottom: 5px; left: 50%; width: 4px; height: 4px; margin-left: -2px; border-radius: 50%; background: var(--accent); }
.cal__day:not(:disabled):hover { background: var(--fill-2); }
.cal__day:not(:disabled):active { background: var(--fill-3); }
.cal__day:focus-visible { outline: 2px solid var(--focus); outline-offset: 1px; }
.cal__day.is-today { box-shadow: inset 0 0 0 1px var(--line); }
.cal__day.is-on { background: var(--accent); color: var(--ink-1); }
.cal__day.is-on::after { background: var(--ink-1); }
.tnum { font-variant-numeric: tabular-nums; }
@media (max-width: 699px) {
  .cal__day { aspect-ratio: auto; height: 48px; min-height: 44px; font-size: var(--fs-body); }
}
</style>
