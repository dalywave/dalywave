<script setup lang="ts">
import type { ClipSettings, ClipSettingsPatch } from '~/api/types'

// Клипы вместо обложки: качество (с оценкой места), кусок с припевом или клип целиком (в такт
// с треком), сроки хранения, потолок места и автопоиск. У каждой настройки — значение по умолчанию.
// Общая для «Клипов» и вкладки «Клипы» в настройках
const props = defineProps<{ clips: ClipSettings }>()
const emit = defineEmits<{ saved: [ClipSettings] }>()
const bridge = useBridge()
const toast = useToast()
const lang = useLang()
const { t } = useI18n({ useScope: 'local' })

const HEIGHTS = [360, 480, 720, 1080]
const DEFAULTS = { height: 480, whole: false, sync: true, lengthS: 12, keepDays: 7, keepLikedDays: 60, maxGb: 3, autoSearch: true }

async function save(patch: ClipSettingsPatch) {
  try {
    const s = await bridge.admin.updateSettings({ clips: patch })
    emit('saved', s.clips)
    toast.show({ text: t('saved'), icon: 'ph:check' })
  }
  catch {
    toast.error(t('failed'))
  }
}

const GB = 1024 ** 3
const used = computed(() => props.clips.storedBytes / (props.clips.maxGb * GB))
// Сколько клипов при нынешних настройках влезет под потолок
const fits = computed(() => (props.clips.avgBytes > 0 ? Math.floor((props.clips.maxGb * GB) / props.clips.avgBytes) : 0))
const heightOptions = computed(() => HEIGHTS.map(h => ({ value: String(h), label: `${h}p` })))
const height = computed({
  get: () => String(props.clips.height),
  set: v => void save({ height: Number(v) }),
})
const what = computed({
  get: () => (props.clips.whole ? 'whole' : 'piece'),
  set: v => void save({ whole: v === 'whole' }),
})
const lengthOptions = [6, 8, 10, 12, 15, 20, 25, 30].map(s => ({ value: s, label: t('seconds', { n: s }) }))
const keepOptions = [1, 3, 7, 14, 30, 60, 90].map(d => ({ value: d, label: t('days', { n: d }) }))
const keepLikedOptions = [7, 14, 30, 60, 90, 180, 365].map(d => ({ value: d, label: t('days', { n: d }) }))
const maxOptions = computed(() => [...new Set([1, 2, 3, 5, 10, 20, 50, 100, props.clips.maxGb])].sort((a, b) => a - b).map(g => ({ value: g, label: `${g} ${t('gb')}` })))
const def = (v: string) => t('default', { v })
</script>

<template>
  <div class="cs">
    <SettingsGroup :title="t('quality.title')" :footnote="t('quality.note', { size: formatBytes(clips.avgBytes, lang), n: formatNumber(fits, lang) })">
      <SettingsRow :title="t('quality.height')" :description="def('480p')" stack>
        <UiSegmentedControl v-model="height" :options="heightOptions" :label="t('quality.height')" size="sm" />
      </SettingsRow>
      <SettingsRow :title="t('what.title')" :description="def(t('what.piece'))" stack>
        <UiSegmentedControl v-model="what" :options="[{ value: 'piece', label: t('what.piece') }, { value: 'whole', label: t('what.whole') }]" :label="t('what.title')" size="sm" />
      </SettingsRow>
      <SettingsRow v-if="clips.whole" :title="t('what.sync')" :description="t('what.syncHint')">
        <UiSwitch :model-value="clips.sync" :label="t('what.sync')" hide-label @update:model-value="v => save({ sync: v })" />
      </SettingsRow>
      <SettingsRow :title="t('what.length')" :description="clips.whole && clips.sync ? t('what.lengthFallback') : def(t('seconds', { n: DEFAULTS.lengthS }))">
        <AdminSelect :model-value="clips.lengthS" :options="lengthOptions" :label="t('what.length')" align="end" @update:model-value="v => save({ lengthS: v })" />
      </SettingsRow>
      <SettingsRow :title="t('auto.title')" :description="t('auto.hint')">
        <UiSwitch :model-value="clips.autoSearch" :label="t('auto.title')" hide-label @update:model-value="v => save({ autoSearch: v })" />
      </SettingsRow>
    </SettingsGroup>

    <SettingsGroup :title="t('space.title')">
      <div class="cs__meter">
        <div class="cs__bar" role="meter" :aria-valuenow="Math.round(used * 100)" aria-valuemin="0" aria-valuemax="100" :aria-label="t('space.title')">
          <i :style="{ width: `${Math.min(100, used * 100)}%` }" :class="{ 'is-full': used > 0.9 }" />
        </div>
        <span class="cs__meter-text tnum">{{ t('space.used', { used: formatBytes(clips.storedBytes, lang), max: `${clips.maxGb} ${t('gb')}`, n: formatNumber(clips.stored, lang) }) }}</span>
      </div>
      <SettingsRow :title="t('space.max')" :description="clips.maxGbFromEnv ? t('space.env') : def(`${DEFAULTS.maxGb} ${t('gb')}`)">
        <span v-if="clips.maxGbFromEnv" class="cs__env"><Icon name="ph:lock-simple" />{{ clips.maxGb }} {{ t('gb') }}</span>
        <AdminSelect v-else :model-value="clips.maxGb" :options="maxOptions" :label="t('space.max')" align="end" @update:model-value="v => save({ maxGb: v })" />
      </SettingsRow>
      <SettingsRow :title="t('space.keep')" :description="def(t('days', { n: DEFAULTS.keepDays }))">
        <AdminSelect :model-value="clips.keepDays" :options="keepOptions" :label="t('space.keep')" align="end" @update:model-value="v => save({ keepDays: v })" />
      </SettingsRow>
      <SettingsRow :title="t('space.keepLiked')" :description="def(t('days', { n: DEFAULTS.keepLikedDays }))">
        <AdminSelect :model-value="clips.keepLikedDays" :options="keepLikedOptions" :label="t('space.keepLiked')" align="end" @update:model-value="v => save({ keepLikedDays: v })" />
      </SettingsRow>
    </SettingsGroup>
  </div>
</template>

<style scoped>
.cs { display: flex; flex-direction: column; gap: 32px; }
.cs__meter { display: flex; flex-direction: column; gap: 8px; padding: 4px 0 12px; }
.cs__bar { height: 6px; overflow: hidden; border-radius: 3px; background: var(--fill-1); }
.cs__bar i { display: block; height: 100%; border-radius: inherit; background: var(--accent); transition: width var(--dur-2); }
.cs__bar i.is-full { background: var(--warning); }
.cs__meter-text { font-size: var(--fs-small); color: var(--text-2); }
.cs__env { display: inline-flex; align-items: center; gap: 6px; font-size: var(--fs-small); color: var(--text-2); }
</style>

<i18n lang="json">
{
  "ru": {
    "saved": "Сохранено",
    "failed": "Не получилось сохранить — попробуйте ещё раз",
    "default": "По умолчанию — {v}",
    "seconds": "{n} с",
    "days": "{n} дн.",
    "gb": "ГБ",
    "quality": {
      "title": "Показ",
      "note": "Один клип при этих настройках — около {size}: под потолок поместится примерно {n}",
      "height": "Качество"
    },
    "what": {
      "title": "Что показывать",
      "piece": "Кусок с припевом",
      "whole": "Клип целиком",
      "length": "Длина куска",
      "lengthFallback": "Если клип не совпал со звуком трека — кусок с припевом по кругу",
      "sync": "В такт с треком",
      "syncHint": "Сервер сверяет звук клипа с треком: перемотка и пауза ведут и видео"
    },
    "auto": {
      "title": "Искать клипы сами",
      "hint": "Выключено — только клипы, выбранные администратором"
    },
    "space": {
      "title": "Место",
      "used": "Занято {used} из {max} · клипов: {n}",
      "max": "Потолок места",
      "env": "Задано на сервере (DALYWAVE_CLIPS_MAX_GB)",
      "keep": "Хранить после показа",
      "keepLiked": "Хранить для любимых треков"
    }
  },
  "uk": {
    "saved": "Збережено",
    "failed": "Не вдалося зберегти — спробуйте ще раз",
    "default": "За замовчуванням — {v}",
    "seconds": "{n} с",
    "days": "{n} дн.",
    "gb": "ГБ",
    "quality": {
      "title": "Показ",
      "note": "Один кліп із цими налаштуваннями — близько {size}: під ліміт вміститься приблизно {n}",
      "height": "Якість"
    },
    "what": {
      "title": "Що показувати",
      "piece": "Фрагмент із приспівом",
      "whole": "Кліп повністю",
      "length": "Довжина фрагмента",
      "lengthFallback": "Якщо кліп не збігся зі звуком трека — фрагмент із приспівом по колу",
      "sync": "У такт із треком",
      "syncHint": "Сервер звіряє звук кліпу з треком: перемотування і пауза керують і відео"
    },
    "auto": {
      "title": "Шукати кліпи самостійно",
      "hint": "Вимкнено — лише кліпи, вибрані адміністратором"
    },
    "space": {
      "title": "Місце",
      "used": "Зайнято {used} з {max} · кліпів: {n}",
      "max": "Ліміт місця",
      "env": "Задано на сервері (DALYWAVE_CLIPS_MAX_GB)",
      "keep": "Зберігати після показу",
      "keepLiked": "Зберігати для вподобаних треків"
    }
  },
  "en": {
    "saved": "Saved",
    "failed": "Couldn't save — try again",
    "default": "Default — {v}",
    "seconds": "{n} s",
    "days": "{n} d",
    "gb": "GB",
    "quality": {
      "title": "Display",
      "note": "One clip with these settings is about {size}: roughly {n} fit under the limit",
      "height": "Quality"
    },
    "what": {
      "title": "What to show",
      "piece": "Chorus piece",
      "whole": "Whole clip",
      "length": "Piece length",
      "lengthFallback": "If the clip doesn't match the track audio — the chorus piece on a loop",
      "sync": "In sync with the track",
      "syncHint": "The server matches the clip audio to the track: seeking and pausing move the video too"
    },
    "auto": {
      "title": "Find clips automatically",
      "hint": "Off — only clips picked by an admin"
    },
    "space": {
      "title": "Space",
      "used": "{used} of {max} used · clips: {n}",
      "max": "Space limit",
      "env": "Set on the server (DALYWAVE_CLIPS_MAX_GB)",
      "keep": "Keep after showing",
      "keepLiked": "Keep for liked tracks"
    }
  }
}
</i18n>
