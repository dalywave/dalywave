/** Общий словарь (i18n/locales) без раздела компонента: `tg('common.save')` рядом с useScopedI18n */
export function useGlobalT() {
  const i18n = useNuxtApp().$i18n
  return { tg: i18n.t, te: i18n.te }
}
