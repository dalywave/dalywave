import { ApiError } from '~/api/types'

/** Загрузка данных страницы с состояниями: загрузка, нет такого (404), ошибка, готово */
export function useLoad<T>(fetcher: () => Promise<T>, key?: () => unknown) {
  const data = shallowRef<T | null>(null)
  const state = ref<'loading' | 'error' | 'missing' | 'ready'>('loading')

  async function load() {
    state.value = 'loading'
    try {
      data.value = await fetcher()
      state.value = 'ready'
    }
    catch (e) {
      data.value = null
      state.value = e instanceof ApiError && e.status === 404 ? 'missing' : 'error'
    }
  }

  onMounted(load)
  if (key) watch(key, () => void load())

  return { data, state, load }
}
