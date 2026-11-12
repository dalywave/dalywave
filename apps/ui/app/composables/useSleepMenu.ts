import type { MenuEntry } from '~/types/ui'
import { SLEEP_MINUTES } from '~/stores/sound'

/** Пункты таймера сна: 5…60 минут, «До конца трека», «Выключить» — для меню «⋯» и кнопки-луны */
export function useSleepMenu() {
  const sound = useSoundStore()
  const { t } = useI18n()

  // Остаток — только у таймера на минуты; «до конца трека» понятен и без цифр
  const remaining = computed(() => {
    const ms = sound.sleepRemainingMs
    return ms === null || sound.sleep?.mode !== 'minutes' ? null : formatDuration(ms)
  })
  const hint = computed(() => {
    if (!sound.sleepActive) return undefined
    if (sound.sleep?.mode === 'track') return t('sound.endOfTrack')
    return remaining.value ? t('sound.sleepLeft', { time: remaining.value }) : undefined
  })

  const items = computed<MenuEntry[]>(() => {
    const choice = sound.sleep?.choice ?? null
    const list: MenuEntry[] = [
      { kind: 'heading', label: t('sound.sleep'), hint: hint.value },
      ...SLEEP_MINUTES.map((m): MenuEntry => ({
        label: m === 60 ? t('sound.hour') : t('sound.minutes', { n: m }),
        checked: choice === m,
        onSelect: () => sound.setSleep(m),
      })),
      { label: t('sound.endOfTrack'), checked: choice === 'track', onSelect: () => sound.setSleep('track') },
    ]
    if (sound.sleepActive) {
      list.push({ kind: 'separator' }, { label: t('sound.sleepOff'), onSelect: () => sound.setSleep(null) })
    }
    return list
  })

  return { items, remaining, hint }
}
