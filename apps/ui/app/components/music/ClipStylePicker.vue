<script setup lang="ts">
import { ToggleGroupItem, ToggleGroupRoot } from 'reka-ui'
import { CLIP_STYLES } from '~/stores/prefs'
import type { ClipStyle } from '~/stores/prefs'

// Как клип показан в полном плеере. У каждого варианта — схема экрана плеера в одном стиле:
// клип — ровная серая заливка, обложка — контур, название — полоски. Выбранный — обводка акцентом.
// Места хватает — карточки в ряд (на узком — 3 + 2); совсем узко (телефон) — список со схемой слева
defineProps<{ label: string }>()

const model = defineModel<ClipStyle>({ required: true })
const { t } = useI18n({ useScope: 'local' })

function update(v: unknown) {
  // Повторное нажатие на выбранный вариант не снимает выбор
  if (CLIP_STYLES.includes(v as ClipStyle)) model.value = v as ClipStyle
}
</script>

<template>
  <div class="csp-wrap">
    <ToggleGroupRoot type="single" :model-value="model" :aria-label="label" class="csp" @update:model-value="update">
      <ToggleGroupItem v-for="o in CLIP_STYLES" :key="o" :value="o" class="csp__item">
        <span class="csp__pv">
          <svg viewBox="0 0 64 44" aria-hidden="true">
            <template v-if="o === 'square'">
              <rect x="23" y="6" width="18" height="18" rx="2.5" class="csp__clip" />
              <rect x="20" y="29" width="24" height="2.5" rx="1.25" class="csp__ink" />
              <rect x="24" y="34" width="16" height="2" rx="1" class="csp__ink csp__ink--soft" />
            </template>
            <template v-else-if="o === 'wide'">
              <rect x="11" y="7" width="42" height="17" rx="2.5" class="csp__clip" />
              <rect x="20" y="29" width="24" height="2.5" rx="1.25" class="csp__ink" />
              <rect x="24" y="34" width="16" height="2" rx="1" class="csp__ink csp__ink--soft" />
            </template>
            <template v-else-if="o === 'backdrop'">
              <rect width="64" height="44" class="csp__clip" />
              <rect x="7" y="28.5" width="8" height="8" rx="1.5" class="csp__on" />
              <rect x="18" y="29" width="20" height="2.5" rx="1.25" class="csp__on" />
              <rect x="18" y="34" width="12" height="2" rx="1" class="csp__on csp__ink--soft" />
            </template>
            <template v-else-if="o === 'cinema'">
              <rect width="64" height="44" class="csp__clip csp__clip--soft" />
              <rect y="9" width="64" height="26" class="csp__clip" />
            </template>
            <template v-else>
              <rect x="23.75" y="6.75" width="16.5" height="16.5" rx="2.25" class="csp__cover" />
              <rect x="20" y="29" width="24" height="2.5" rx="1.25" class="csp__ink" />
              <rect x="24" y="34" width="16" height="2" rx="1" class="csp__ink csp__ink--soft" />
            </template>
          </svg>
        </span>
        <span class="csp__label">{{ t(o) }}</span>
        <span class="csp__radio" aria-hidden="true" />
      </ToggleGroupItem>
    </ToggleGroupRoot>
  </div>
</template>

<style scoped>
.csp-wrap { container-type: inline-size; width: 100%; }
.csp {
  --pv-clip: color-mix(in oklab, var(--text-1) 24%, transparent);
  display: grid;
  grid-template-columns: repeat(5, minmax(0, 1fr));
  gap: 12px;
  width: 100%;
}
.csp__item {
  display: flex;
  flex-direction: column;
  align-items: stretch;
  gap: 8px;
  min-width: 0;
  padding: 0;
  border: 0;
  background: none;
  color: var(--text-2);
  font: inherit;
  font-size: var(--fs-small);
  font-weight: 500;
  text-align: center;
  cursor: pointer;
  -webkit-tap-highlight-color: transparent;
}
.csp__pv {
  display: block;
  overflow: hidden;
  aspect-ratio: 64 / 44;
  border-radius: var(--r-3);
  background: var(--fill-1);
  box-shadow: inset 0 0 0 1px var(--line);
  transition: box-shadow var(--dur-2) var(--ease-out), background-color var(--dur-2);
}
.csp__pv svg { display: block; width: 100%; height: 100%; }
.csp__clip { fill: var(--pv-clip); }
.csp__clip--soft { opacity: 0.35; }
.csp__cover { fill: none; stroke: var(--text-3); stroke-width: 1.5; }
.csp__ink { fill: var(--text-3); }
.csp__on { fill: var(--text-1); opacity: 0.85; }
.csp__ink--soft { opacity: 0.55; }
.csp__label { overflow-wrap: anywhere; line-height: 1.3; transition: color var(--dur-1); }
.csp__radio { display: none; }

.csp__item:hover .csp__pv { background: var(--fill-2); }
.csp__item:hover { color: var(--text-1); }
.csp__item:focus-visible { outline: none; }
.csp__item:focus-visible .csp__pv { box-shadow: inset 0 0 0 1px var(--line), 0 0 0 2px var(--focus); }
.csp__item[data-state='on'] { color: var(--text-1); font-weight: 600; }
.csp__item[data-state='on'] .csp__pv { box-shadow: inset 0 0 0 1px var(--line), 0 0 0 2px var(--accent); }

/* Узко: 3 + 2 */
@container (max-width: 520px) {
  .csp { grid-template-columns: repeat(6, minmax(0, 1fr)); row-gap: 14px; }
  .csp__item { grid-column: span 2; }
  .csp__item:nth-child(4) { grid-column: 2 / span 2; }
}

/* Телефон: список — схема слева, подпись, кружок выбора справа; зона касания ≥ 56 pt */
@container (max-width: 420px) {
  .csp { grid-template-columns: 1fr; gap: 0; }
  .csp__item, .csp__item:nth-child(4) { grid-column: auto; }
  .csp__item {
    position: relative;
    flex-direction: row;
    align-items: center;
    gap: 14px;
    min-height: 60px;
    padding: 8px 0;
    font-size: var(--fs-body);
    text-align: left;
  }
  .csp__item + .csp__item::before { content: ''; position: absolute; top: 0; left: 70px; right: 0; height: 1px; background: var(--hair); }
  .csp__pv { width: 56px; flex: none; border-radius: var(--r-2); }
  .csp__label { flex: 1; }
  .csp__item[data-state='on'] { font-weight: 500; }
  .csp__item[data-state='on'] .csp__pv { box-shadow: inset 0 0 0 1px var(--line); }
  .csp__radio { display: block; width: 20px; height: 20px; flex: none; border-radius: 50%; box-shadow: inset 0 0 0 1.5px var(--line-strong); transition: box-shadow var(--dur-2) var(--ease-out); }
  .csp__item[data-state='on'] .csp__radio { box-shadow: inset 0 0 0 6px var(--accent); }
}
</style>

<i18n lang="json">
{
  "ru": { "square": "Квадрат", "wide": "Широкий", "backdrop": "Фон", "cinema": "Кино", "off": "Выключено" },
  "uk": { "square": "Квадрат", "wide": "Широкий", "backdrop": "Тло", "cinema": "Кіно", "off": "Вимкнено" },
  "en": { "square": "Square", "wide": "Wide", "backdrop": "Backdrop", "cinema": "Cinema", "off": "Off" }
}
</i18n>
