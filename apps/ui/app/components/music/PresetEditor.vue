<script setup lang="ts">
import { PopoverAnchor, PopoverContent, PopoverPortal, PopoverRoot } from 'reka-ui'
import { PRESET_ICONS } from '~/utils/wave-options'

// Название и иконка сочетания волны: на телефоне — лист снизу, на компьютере — панель у карточки
const props = defineProps<{
  title: string
  submitLabel: string
  name: string
  icon: string
  /** Карточка, у которой раскрывается панель (компьютер) */
  anchor: HTMLElement | null
  busy?: boolean
}>()
const emit = defineEmits<{ submit: [value: { name: string, icon: string }] }>()
const open = defineModel<boolean>('open', { default: false })

const ui = useUiStore()
const { t } = useI18n({ useScope: 'local' })
const name = ref(props.name)
const icon = ref(props.icon)
const input = ref<HTMLInputElement | null>(null)
const valid = computed(() => !!name.value.trim() && name.value.trim().length <= 40)

watch(open, async (v) => {
  if (!v) return
  name.value = props.name
  icon.value = props.icon
  // Предложенное название выделено: его легко принять или сразу перепечатать
  await nextTick()
  setTimeout(() => input.value?.select(), 60)
})

function submit() {
  if (!valid.value || props.busy) return
  emit('submit', { name: name.value.trim(), icon: icon.value })
}
</script>

<template>
  <UiSheet v-if="ui.isPhone" v-model:open="open" :title="title">
    <form class="pe pe--sheet" @submit.prevent="submit">
      <input ref="input" v-model="name" class="pe__input" :aria-label="t('name')" maxlength="40" enterkeyhint="done">
      <div class="pe__icons" role="radiogroup" :aria-label="t('icon')">
        <button
          v-for="i in PRESET_ICONS"
          :key="i"
          type="button"
          role="radio"
          class="pe__icon"
          :class="{ 'is-on': icon === i }"
          :aria-checked="icon === i"
          :aria-label="i.slice(3)"
          @click="icon = i"
        >
          <Icon :name="i" />
        </button>
      </div>
      <UiButton type="submit" variant="primary" size="lg" :loading="busy" :disabled="!valid" block>{{ submitLabel }}</UiButton>
    </form>
  </UiSheet>
  <PopoverRoot v-else v-model:open="open">
    <PopoverAnchor :reference="anchor ?? undefined" />
    <PopoverPortal>
      <PopoverContent side="bottom" align="start" :side-offset="8" :collision-padding="12" class="pop pe-pop" @open-auto-focus.prevent>
        <header class="pop__head">
          <span class="pop__title">{{ title }}</span>
        </header>
        <form class="pe" @submit.prevent="submit">
          <input ref="input" v-model="name" class="pe__input" :aria-label="t('name')" maxlength="40">
          <div class="pe__icons" role="radiogroup" :aria-label="t('icon')">
            <button
              v-for="i in PRESET_ICONS"
              :key="i"
              type="button"
              role="radio"
              class="pe__icon"
              :class="{ 'is-on': icon === i }"
              :aria-checked="icon === i"
              :aria-label="i.slice(3)"
              @click="icon = i"
            >
              <Icon :name="i" />
            </button>
          </div>
          <UiButton type="submit" variant="primary" :loading="busy" :disabled="!valid" block>{{ submitLabel }}</UiButton>
        </form>
      </PopoverContent>
    </PopoverPortal>
  </PopoverRoot>
</template>

<style scoped>
.pe-pop { width: 312px; }
.pe { display: flex; flex-direction: column; gap: 12px; padding: 0 6px 6px; }
.pe__input {
  height: 40px;
  padding: 0 14px;
  border: 0;
  border-radius: var(--r-pill);
  background: var(--fill-1);
  box-shadow: inset 0 0 0 1px var(--line);
  color: var(--text-1);
  font: inherit;
}
.pe__input:focus-visible { outline: none; box-shadow: inset 0 0 0 1.5px var(--focus); }
.pe__icons { display: grid; grid-template-columns: repeat(7, minmax(0, 1fr)); gap: 4px; }
.pe__icon {
  display: grid;
  place-items: center;
  aspect-ratio: 1;
  border: 0;
  border-radius: var(--r-2);
  background: transparent;
  color: var(--text-2);
  font-size: 18px;
  transition: background-color var(--dur-1), color var(--dur-1), box-shadow var(--dur-1);
}
.pe__icon:hover { background: var(--fill-1); color: var(--text-1); }
.pe__icon:active { background: var(--fill-2); }
.pe__icon:focus-visible { outline: none; box-shadow: inset 0 0 0 1.5px var(--focus); }
.pe__icon.is-on { background: var(--fill-2); color: var(--text-1); box-shadow: inset 0 0 0 1.5px var(--accent-line); }
.pe--sheet { gap: 16px; padding: 4px 16px 16px; }
.pe--sheet .pe__input { height: 48px; padding: 0 16px; font-size: 16px; }
.pe--sheet .pe__icons { gap: 6px; }
.pe--sheet .pe__icon { min-height: 44px; font-size: 20px; }
</style>

<i18n lang="json">
{
  "ru": { "name": "Название", "icon": "Иконка" },
  "en": { "name": "Name", "icon": "Icon" }
}
</i18n>
