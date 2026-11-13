import type { TransitionMode } from '~/stores/player'

// Значки переходов между треками. Кнопка в плеере — всегда один значок «переходы» (наложение двух
// треков), режим — маленькой меткой у него: значок, меняющийся с режимом, было не найти

export const TRANSITION_ICON = 'ph:intersect'

export const TRANSITION_MODES: TransitionMode[] = ['gapless', 'smooth', 'smart', 'dj']

export const TRANSITION_ICONS: Record<TransitionMode, string> = {
  gapless: 'ph:link-simple-horizontal',
  smooth: 'ph:wave-sine',
  smart: 'ph:sparkle',
  dj: 'ph:vinyl-record',
}
