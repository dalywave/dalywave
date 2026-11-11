import type { AudioOutput, AudioOutputKind, AudioOutputs } from '~/bridge/types'

const ICON: Record<AudioOutputKind, string> = {
  builtin: 'ph:speaker-high',
  headphones: 'ph:headphones',
  bluetooth: 'ph:bluetooth',
  airplay: 'ph:airplay',
  usb: 'ph:usb',
  hdmi: 'ph:television',
  other: 'ph:speaker-simple-high',
}

/** Вывод звука этого устройства для списка «Где играть»: что умеет платформа, куда звук идёт сейчас */
export function useAudioOutputs() {
  const bridge = useBridge()
  const routes = bridge.outputs
  const state = ref<AudioOutputs>({ outputs: [], followsSystem: true })
  const current = computed<AudioOutput | null>(() => state.value.outputs.find(o => o.current) ?? null)

  // Виртуальные устройства (Steam, BlackHole) — в конце списка, как и в настройках звука macOS
  const apply = (s: AudioOutputs) => {
    state.value = { ...s, outputs: [...s.outputs].sort((a, b) => Number(a.kind === 'other') - Number(b.kind === 'other')) }
  }

  let off: (() => void) | null = null
  onMounted(() => {
    if (!routes) return
    void routes.get().then(apply).catch(() => {})
    off = routes.onChange(apply)
  })
  onBeforeUnmount(() => off?.())

  async function select(id: string | null) {
    if (!routes) return
    await routes.select(id)
    apply(await routes.get().catch(() => state.value))
  }

  return {
    mode: routes?.mode ?? null,
    state,
    current,
    select,
    showPicker: () => routes?.showPicker(),
    iconOf: (kind: AudioOutputKind) => ICON[kind],
  }
}
