<script setup lang="ts">
import type { Notification } from '~/api/types'

// Строка центра уведомлений: обложка или значок вида, заголовок, текст, когда пришло; непрочитанное —
// точка акцента. Собранное из нескольких событий («3 новых релиза») раскрывается строками с обложками
const props = defineProps<{ n: Notification }>()
const emit = defineEmits<{ open: [link: string | null] }>()

const bridge = useBridge()
const lang = useLang()
const { tg } = useGlobalT()

const expanded = ref(false)
const group = computed(() => (props.n.items.length > 1 ? props.n.items : []))
const time = computed(() => notificationTime(props.n.createdAt, new Date(), lang.value))
const src = (url: string | null | undefined) => (url ? bridge.resolveUrl(url) : null)
const image = computed(() => src(props.n.image))
</script>

<template>
  <div class="nr" :class="{ 'is-unread': !n.readAt }">
    <button type="button" class="nr__main" @click="emit('open', n.link)">
      <span class="nr__art" :class="{ 'nr__art--icon': !image }">
        <img v-if="image" :src="image" alt="" loading="lazy">
        <Icon v-else :name="notificationIcon(n.kind)" />
      </span>
      <span class="nr__text">
        <span class="nr__title">{{ n.title }}</span>
        <span v-if="n.body" class="nr__body">{{ n.body }}</span>
      </span>
      <span class="nr__meta">
        <span class="nr__time tnum">{{ time }}</span>
        <span v-if="!n.readAt" class="nr__dot" aria-hidden="true" />
      </span>
    </button>
    <template v-if="group.length">
      <div v-if="expanded" class="nr__items">
        <button v-for="(it, i) in group" :key="i" type="button" class="nr__item" @click="emit('open', it.link ?? n.link)">
          <span class="nr__thumb">
            <img v-if="it.image" :src="src(it.image)!" alt="" loading="lazy">
            <Icon v-else name="ph:vinyl-record" />
          </span>
          <span class="nr__item-text">
            <span class="nr__item-title ellipsis">{{ it.title }}</span>
            <span v-if="it.subtitle" class="nr__item-sub ellipsis">{{ it.subtitle }}</span>
          </span>
        </button>
      </div>
      <button type="button" class="nr__toggle" :aria-expanded="expanded" @click="expanded = !expanded">
        {{ expanded ? tg('notify.collapse') : tg('notify.showAll', { n: group.length }) }}
        <Icon :name="expanded ? 'ph:caret-up' : 'ph:caret-down'" />
      </button>
    </template>
  </div>
</template>

<style scoped>
.nr { position: relative; display: flex; flex-direction: column; }
.nr + .nr::before { content: ''; position: absolute; top: 0; left: 68px; right: 12px; height: 1px; background: var(--hair); }
.nr__main {
  display: flex;
  align-items: flex-start;
  gap: 12px;
  width: 100%;
  padding: 12px;
  border: 0;
  border-radius: var(--r-3);
  background: transparent;
  color: inherit;
  font: inherit;
  text-align: left;
  cursor: pointer;
  -webkit-tap-highlight-color: transparent;
  transition: background-color var(--dur-1);
}
.nr__main:hover { background: var(--fill-1); }
.nr__main:active { background: var(--fill-2); }
.nr__main:focus-visible { outline: none; box-shadow: inset 0 0 0 2px var(--focus); }
.nr__art { display: grid; place-items: center; width: 44px; height: 44px; flex: none; overflow: hidden; border-radius: var(--r-2); background: var(--fill-2); color: var(--text-2); font-size: 20px; }
.nr__art img { width: 100%; height: 100%; object-fit: cover; }
.nr__art--icon { border-radius: 50%; }
.nr__text { display: flex; flex: 1; flex-direction: column; gap: 2px; min-width: 0; padding-top: 2px; }
.nr__title { font-size: var(--fs-body); font-weight: 600; line-height: 1.3; color: var(--text-1); overflow-wrap: anywhere; }
.nr__body { display: -webkit-box; overflow: hidden; font-size: var(--fs-small); line-height: 1.4; color: var(--text-2); -webkit-line-clamp: 2; -webkit-box-orient: vertical; overflow-wrap: anywhere; }
.nr:not(.is-unread) .nr__title { font-weight: 500; color: var(--text-2); }
.nr:not(.is-unread) .nr__body { color: var(--text-3); }
.nr__meta { display: flex; flex: none; flex-direction: column; align-items: flex-end; gap: 8px; padding-top: 3px; }
.nr__time { font-size: var(--fs-caption); color: var(--text-3); white-space: nowrap; }
.nr__dot { width: 8px; height: 8px; border-radius: 50%; background: var(--accent); }

.nr__items { display: flex; flex-direction: column; gap: 2px; margin: -4px 0 0 56px; }
.nr__item { display: flex; align-items: center; gap: 10px; min-height: 44px; padding: 4px 12px 4px 12px; border: 0; border-radius: var(--r-2); background: transparent; color: inherit; font: inherit; text-align: left; cursor: pointer; }
.nr__item:hover { background: var(--fill-1); }
.nr__item:focus-visible { outline: none; box-shadow: inset 0 0 0 2px var(--focus); }
.nr__thumb { display: grid; place-items: center; width: 32px; height: 32px; flex: none; overflow: hidden; border-radius: var(--r-1); background: var(--fill-2); color: var(--text-3); font-size: 16px; }
.nr__thumb img { width: 100%; height: 100%; object-fit: cover; }
.nr__item-text { display: flex; flex-direction: column; min-width: 0; }
.nr__item-title { font-size: var(--fs-small); font-weight: 550; color: var(--text-1); }
.nr__item-sub { font-size: var(--fs-caption); color: var(--text-3); }
.nr__toggle {
  display: inline-flex;
  align-items: center;
  align-self: flex-start;
  gap: 4px;
  min-height: 32px;
  margin: -6px 0 6px 56px;
  padding: 0 12px;
  border: 0;
  border-radius: var(--r-pill);
  background: transparent;
  color: var(--text-3);
  font: inherit;
  font-size: var(--fs-caption);
  font-weight: 600;
  cursor: pointer;
}
.nr__toggle:hover { color: var(--text-1); background: var(--fill-1); }
.nr__toggle:focus-visible { outline: none; box-shadow: inset 0 0 0 2px var(--focus); }

@media (pointer: coarse) {
  .nr__toggle { min-height: 44px; margin-top: -10px; margin-bottom: 0; }
  .nr__main:hover { background: transparent; }
  .nr__main:active { background: var(--fill-2); }
}
</style>
