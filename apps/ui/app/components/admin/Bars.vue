<script setup lang="ts">
// Столбики по дням: доля успеха загрузок, прирост хранилища. Подпись дня — под первым,
// средним и последним столбиком; точные цифры — в подсказке и для экранного диктора
export interface BarSeries { key: string, label: string, color: string }
export interface BarDay { day: string, values: Record<string, number>, title?: string }
const props = withDefaults(defineProps<{
  days: BarDay[]
  series: BarSeries[]
  label: string
  height?: number
}>(), { height: 120 })
const lang = useLang()
const max = computed(() => Math.max(1, ...props.days.map(d => props.series.reduce((s, x) => s + (d.values[x.key] ?? 0), 0))))
const dayLabel = (iso: string) => new Intl.DateTimeFormat(intlLocale(lang.value, 'en-GB'), { day: 'numeric', month: 'short', timeZone: 'UTC' }).format(new Date(`${iso}T00:00:00Z`))
const marks = computed(() => new Set([0, Math.floor((props.days.length - 1) / 2), props.days.length - 1]))
</script>

<template>
  <figure class="bars" :aria-label="label">
    <div class="bars__plot" :style="{ height: `${height}px` }">
      <div v-for="(d, i) in days" :key="d.day" class="bars__col" :title="d.title ?? `${dayLabel(d.day)}: ${series.map(s => `${s.label} ${d.values[s.key] ?? 0}`).join(', ')}`">
        <div class="bars__stack">
          <i v-for="s in series" :key="s.key" :style="{ height: `${((d.values[s.key] ?? 0) / max) * 100}%`, background: s.color }" />
        </div>
        <span class="bars__day" :class="{ 'is-shown': marks.has(i) }">{{ dayLabel(d.day) }}</span>
      </div>
    </div>
    <figcaption class="bars__legend">
      <span v-for="s in series" :key="s.key"><i :style="{ background: s.color }" />{{ s.label }}</span>
    </figcaption>
  </figure>
</template>

<style scoped>
.bars { display: flex; flex-direction: column; gap: 10px; margin: 0; min-width: 0; }
.bars__plot { display: flex; align-items: stretch; gap: 4px; padding-bottom: 20px; border-bottom: 1px solid var(--hair); }
.bars__col { position: relative; display: flex; flex: 1; flex-direction: column; justify-content: flex-end; min-width: 0; }
.bars__stack { display: flex; flex-direction: column-reverse; height: 100%; justify-content: flex-start; overflow: hidden; border-radius: 3px 3px 0 0; background: var(--fill-1); }
.bars__stack i { display: block; flex: none; min-height: 0; }
.bars__day { position: absolute; bottom: -20px; left: 50%; display: none; font-size: 11px; color: var(--text-3); transform: translateX(-50%); white-space: nowrap; }
.bars__day.is-shown { display: block; }
.bars__col:first-child .bars__day { left: 0; transform: none; }
.bars__col:last-child .bars__day { right: 0; left: auto; transform: none; }
.bars__legend { display: flex; flex-wrap: wrap; gap: 4px 16px; padding-top: 8px; font-size: var(--fs-caption); color: var(--text-2); }
.bars__legend span { display: inline-flex; align-items: center; gap: 6px; }
.bars__legend i { width: 8px; height: 8px; border-radius: 2px; }
</style>
