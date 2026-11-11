import { getBridge } from '~/bridge'
import type { Bridge } from '~/bridge'

/** Экраны ходят к данным и звуку только через мост */
export function useBridge(): Bridge {
  return getBridge()
}
