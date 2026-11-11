/**
 * Общий словарь (i18n/locales) на страницах со своим блоком <i18n>: второй вызов useI18n
 * в компоненте с локальным блоком vue-i18n не поддерживает, поэтому берём глобальный экземпляр
 */
export function useGlobalT() {
  const i18n = useNuxtApp().$i18n
  return { tg: i18n.t, te: i18n.te }
}
