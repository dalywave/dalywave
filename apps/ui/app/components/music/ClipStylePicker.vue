<script setup lang="ts">
import { ToggleGroupItem, ToggleGroupRoot } from 'reka-ui'
import { CLIP_STYLES } from '~/stores/prefs'
import type { ClipStyle } from '~/stores/prefs'

// Как клип показан в полном плеере. У каждого варианта — схема плеера: где клип (залито),
// где обложка (контур), где название (полоски)
defineProps<{ label: string }>()

const model = defineModel<ClipStyle>({ required: true })
const { t } = useI18n({ useScope: 'local' })

function update(v: unknown) {
  // Повторное нажатие на выбранный вариант не снимает выбор
  if (CLIP_STYLES.includes(v as ClipStyle)) model.value = v as ClipStyle
}
</script>

<template>
  <ToggleGroupRoot type="single" :model-value="model" :aria-label="label" class="csp" @update:model-value="update">
    <ToggleGroupItem v-for="o in CLIP_STYLES" :key="o" :value="o" class="csp__item">
      <svg class="csp__art" viewBox="0 0 56 40" aria-hidden="true">
        <!-- Рамка плеера -->
        <rect x="0.75" y="0.75" width="54.5" height="38.5" rx="6" class="csp__frame" />
        <template v-if="o === 'square'">
          <rect x="19" y="5" width="18" height="18" rx="2.5" class="csp__clip" />
          <rect x="16" y="27" width="24" height="2.6" rx="1.3" class="csp__ink" />
          <rect x="20" y="31.5" width="16" height="2.2" rx="1.1" class="csp__ink csp__ink--soft" />
        </template>
        <template v-else-if="o === 'wide'">
          <ellipse cx="28" cy="14" rx="22" ry="11" class="csp__glow" />
          <rect x="12" y="7" width="32" height="15" rx="2.5" class="csp__clip" />
          <rect x="16" y="27" width="24" height="2.6" rx="1.3" class="csp__ink" />
          <rect x="20" y="31.5" width="16" height="2.2" rx="1.1" class="csp__ink csp__ink--soft" />
        </template>
        <template v-else-if="o === 'backdrop'">
          <rect x="0.75" y="0.75" width="54.5" height="38.5" rx="6" class="csp__clip csp__clip--bg" />
          <rect x="6" y="26" width="8" height="8" rx="1.5" class="csp__cut" />
          <rect x="17" y="26.5" width="18" height="2.6" rx="1.3" class="csp__cut" />
          <rect x="17" y="31" width="11" height="2.2" rx="1.1" class="csp__cut csp__ink--soft" />
        </template>
        <template v-else-if="o === 'cinema'">
          <rect x="0.75" y="0.75" width="54.5" height="38.5" rx="6" class="csp__bars" />
          <rect x="0.75" y="7" width="54.5" height="26" class="csp__clip" />
        </template>
        <template v-else>
          <rect x="19.5" y="5.5" width="17" height="17" rx="2.5" class="csp__cover" />
          <rect x="16" y="27" width="24" height="2.6" rx="1.3" class="csp__ink" />
          <rect x="20" y="31.5" width="16" height="2.2" rx="1.1" class="csp__ink csp__ink--soft" />
        </template>
      </svg>
      <span class="csp__label"><span class="csp__full">{{ t(o) }}</span><span v-if="o === 'off'" class="csp__short" aria-hidden="true">{{ t('offShort') }}</span></span>
    </ToggleGroupItem>
  </ToggleGroupRoot>
</template>

<style scoped>
.csp {
  display: inline-grid;
  grid-template-columns: repeat(5, 96px);
  gap: 4px;
  max-width: 100%;
  padding: 4px;
  border-radius: var(--r-4);
  background: var(--fill-1);
  box-shadow: inset 0 0 0 1px var(--hair);
}
.csp__item {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 6px;
  min-width: 0;
  padding: 10px 6px 8px;
  border: 0;
  border-radius: var(--r-3);
  background: transparent;
  font-size: var(--fs-small);
  font-weight: 600;
  color: var(--text-2);
  transition: color var(--dur-1), background-color var(--dur-2), box-shadow var(--dur-2);
}
.csp__art { width: 56px; height: 40px; opacity: 0.55; transition: opacity var(--dur-2); }
.csp__frame { fill: none; stroke: currentColor; stroke-width: 1.5; opacity: 0.55; }
.csp__clip { fill: currentColor; opacity: 0.85; }
.csp__clip--bg { opacity: 0.4; }
.csp__glow { fill: currentColor; opacity: 0.16; }
.csp__bars { fill: currentColor; opacity: 0.12; }
.csp__cover { fill: none; stroke: currentColor; stroke-width: 1.5; }
.csp__ink { fill: currentColor; }
.csp__cut { fill: currentColor; }
.csp__ink--soft { opacity: 0.5; }
.csp__label { max-width: 100%; overflow: hidden; text-align: center; text-overflow: ellipsis; white-space: nowrap; }
.csp__short { display: none; }
.csp__item:hover { color: var(--text-1); }
.csp__item:hover .csp__art { opacity: 0.8; }
.csp__item[data-state='on'] { color: var(--text-1); background: var(--ink-4); box-shadow: 0 2px 8px -2px rgba(var(--shade-rgb), calc(0.6 * var(--shade-k))), inset 0 0 0 1px var(--line); }
.csp__item[data-state='on'] .csp__art { opacity: 1; }
.csp__item[data-state='on'] .csp__clip { fill: var(--accent); opacity: 1; }
.csp__item[data-state='on'] .csp__clip--bg { opacity: 0.55; }
.csp__item[data-state='on'] .csp__glow { fill: var(--accent); opacity: 0.22; }

@media (max-width: 699px) {
  .csp { display: grid; grid-template-columns: repeat(5, minmax(0, 1fr)); width: 100%; }
  .csp__item { padding: 10px 2px 8px; font-size: 12px; }
  .csp__art { width: 44px; height: 32px; }
  /* Пять вариантов в ряд на телефоне: «Выключено» не влезает — короче видно, полное читается */
  .csp__short { display: inline; }
  .csp__full:has(+ .csp__short) { position: absolute; width: 1px; height: 1px; overflow: hidden; clip-path: inset(50%); }
}
</style>

<i18n lang="json">
{
  "ru": { "square": "Квадрат", "wide": "Широкий", "backdrop": "Фон", "cinema": "Кино", "off": "Выключено", "offShort": "Выкл." },
  "en": { "square": "Square", "wide": "Wide", "backdrop": "Backdrop", "cinema": "Cinema", "off": "Off", "offShort": "Off" }
}
</i18n>
