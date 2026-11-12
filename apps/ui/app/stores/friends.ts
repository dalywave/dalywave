import { defineStore } from 'pinia'
import type { Friend, FriendList, PersonRef, Privacy, PrivateSessionMode } from '~/api/types'

// Друзья (сервер — crates/server/src/social.rs): список с тем, что каждый слушает, запросы, приватность.
// Живые события — `friends.changed` (список или запросы) и `friends.activity` (друг сменил трек) по общему
// WebSocket; пока открыт раздел, список ещё и перечитывается раз в минуту — «давно ли» не устаревает
export const useFriendsStore = defineStore('friends', () => {
  const bridge = useBridge()
  const toast = useToast()
  const t = useNuxtApp().$i18n.t

  const list = ref<FriendList | null>(null)
  const privacy = ref<Privacy | null>(null)
  const failed = ref(false)
  /** Что сейчас ждёт ответа сервера: id человека */
  const busy = ref(new Set<string>())
  /** Тикает раз в минуту: подписи «5 мин назад» пересчитываются */
  const now = ref(Date.now())
  let started = false

  const friends = computed<Friend[]>(() => list.value?.friends ?? [])
  const incoming = computed(() => list.value?.incoming ?? [])
  const outgoing = computed(() => list.value?.outgoing ?? [])
  const listening = computed(() => friends.value.filter(f => f.activity?.now))
  /** Друзья с тем, что слушали: сначала играющие, затем по свежести */
  const withActivity = computed(() => friends.value.filter(f => f.activity))
  const privateOn = computed(() => !!privacy.value?.privateSession)

  async function load() {
    try {
      list.value = await bridge.friends.list()
      failed.value = false
    }
    catch {
      failed.value = !list.value
    }
    return list.value
  }

  async function loadPrivacy() {
    try {
      privacy.value = await bridge.friends.privacy()
    }
    catch {
      // Останется прежнее
    }
  }

  function sort(items: Friend[]): Friend[] {
    const key = (f: Friend) => (f.activity ? (f.activity.now ? 2e13 : 0) + new Date(f.activity.at).getTime() : -1)
    return [...items].sort((a, b) => key(b) - key(a))
  }

  /** Подписка на живые события и первая загрузка (оболочка зовёт при запуске) */
  function init() {
    if (started || !import.meta.client) return
    started = true
    void loadPrivacy()
    void load()
    bridge.live.on('friends.changed', (e) => {
      if (e.what === 'privacy') void loadPrivacy()
      else void load()
    })
    bridge.live.on('friends.activity', (e) => {
      if (!list.value) return
      const items = list.value.friends.map(f => (f.person.id === e.userId ? { ...f, activity: e.activity ?? null } : f))
      list.value = { ...list.value, friends: sort(items) }
    })
    bridge.live.onReconnect(() => {
      void load()
      void loadPrivacy()
    })
    window.setInterval(() => {
      now.value = Date.now()
      if (document.visibilityState === 'visible' && list.value?.friends.length) void load()
    }, 60_000)
  }

  async function act(id: string, fn: () => Promise<unknown>, failText = t('friends.failed')) {
    busy.value = new Set(busy.value).add(id)
    try {
      await fn()
      await load()
      return true
    }
    catch {
      toast.error(failText)
      return false
    }
    finally {
      const next = new Set(busy.value)
      next.delete(id)
      busy.value = next
    }
  }

  async function request(p: PersonRef) {
    let friend = false
    const ok = await act(p.id, async () => {
      friend = (await bridge.friends.request(p.id)).relation === 'friend'
    })
    if (ok) toast.show({ text: friend ? t('friends.nowFriends', { name: p.name }) : t('friends.sent'), icon: friend ? 'ph:users-three' : 'ph:paper-plane-tilt' })
    return ok
  }

  async function accept(p: PersonRef) {
    const ok = await act(p.id, () => bridge.friends.accept(p.id))
    if (ok) toast.show({ text: t('friends.nowFriends', { name: p.name }), icon: 'ph:users-three' })
    return ok
  }

  const decline = (p: PersonRef) => act(p.id, () => bridge.friends.decline(p.id))
  const cancel = (p: PersonRef) => act(p.id, () => bridge.friends.cancel(p.id))

  async function remove(p: PersonRef) {
    const ok = await act(p.id, () => bridge.friends.remove(p.id))
    if (ok) toast.show({ text: t('friends.removed', { name: p.name }), icon: 'ph:user-minus' })
    return ok
  }

  async function setShare(on: boolean) {
    const was = privacy.value
    if (was) privacy.value = { ...was, shareListening: on }
    try {
      privacy.value = await bridge.friends.updatePrivacy({ shareListening: on })
    }
    catch {
      privacy.value = was
      toast.error(t('friends.failed'))
    }
  }

  /** Приватная сессия: на 6 часов, до выключения или выключить */
  async function setPrivate(mode: PrivateSessionMode) {
    try {
      privacy.value = await bridge.friends.updatePrivacy({ privateSession: mode })
      if (mode !== 'off') toast.show({ text: t('friends.privateOn'), icon: 'ph:eye-slash' })
      else toast.show({ text: t('friends.privateOff'), icon: 'ph:eye' })
    }
    catch {
      toast.error(t('friends.failed'))
    }
  }

  return {
    list, privacy, failed, busy, now, friends, incoming, outgoing, listening, withActivity, privateOn,
    init, load, loadPrivacy, request, accept, decline, cancel, remove, setShare, setPrivate,
  }
})
