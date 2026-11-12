/** Окно мини-плеера приложения (apps/shell/src/mini.rs): там только вид плеера — без сторов плеера,
 *  сокета и загрузок, звук и очередь живут в главном окне и движке. В разработке — и `/mini` в браузере */
export function isMiniWindow(): boolean {
  if (typeof window === 'undefined') return false
  return '__DALYWAVE_MINI__' in window || (import.meta.dev && window.location.pathname === '/mini')
}
