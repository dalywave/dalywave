import type { MenuAction } from '~/types/ui'

/**
 * Приватная сессия одним нажатием: пункт меню аватара (на 6 часов или до выключения; идёт — «Выключить»)
 * и подпись «до 18:40» для плеера и настроек
 */
export function usePrivateSession() {
  const friends = useFriendsStore()
  const lang = useLang()
  const t = useNuxtApp().$i18n.t

  const on = computed(() => friends.privateOn)
  /** «до 18:40»; до выключения — пусто */
  const until = computed(() => {
    const at = friends.privacy?.privateUntil
    if (!on.value || !at) return ''
    const d = new Date(at)
    const sameDay = d.toDateString() === new Date().toDateString()
    const time = new Intl.DateTimeFormat(intlLocale(lang.value, 'en-GB'), sameDay
      ? { hour: '2-digit', minute: '2-digit' }
      : { weekday: 'short', hour: '2-digit', minute: '2-digit' }).format(d)
    return t('friends.privateUntil', { time })
  })

  function entry(): MenuAction {
    if (on.value) {
      return {
        label: t('friends.private'),
        icon: 'ph:eye-slash',
        hint: until.value || undefined,
        checked: true,
        children: [{ label: t('friends.privateStop'), icon: 'ph:eye', onSelect: () => void friends.setPrivate('off') }],
      }
    }
    return {
      label: t('friends.private'),
      icon: 'ph:eye-slash',
      children: [
        { kind: 'heading', label: t('friends.privateHint') },
        { label: t('friends.private6h'), icon: 'ph:timer', onSelect: () => void friends.setPrivate('hours6') },
        { label: t('friends.privateUntilOff'), icon: 'ph:infinity', onSelect: () => void friends.setPrivate('until_off') },
      ],
    }
  }

  return { on, until, entry }
}
