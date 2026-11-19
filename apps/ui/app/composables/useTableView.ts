import type { Ref } from 'vue'
import type { ViewState } from '~/components/admin/SavedViews.vue'

/**
 * Вид таблицы админки на стороне интерфейса: поиск, сортировка по колонке и фильтры — одним
 * состоянием, которое сохраняется в виды (`<AdminSavedViews>`). Строки фильтруются и сортируются
 * здесь же; таблицы с фильтрами на сервере (задачи) берут только `state`/`apply`.
 *
 *   const view = useTableView(rows, {
 *     search: r => [r.name, r.workerId],
 *     sorts: { name: r => r.name.toLowerCase(), seen: r => r.lastSeenAt ?? '' },
 *     filters: { state: (r, v) => v === 'online' ? r.online : true },
 *     initial: { sort: 'seen', desc: true },
 *   })
 */
export interface TableViewOptions<T> {
  search?: (row: T) => (string | null | undefined)[]
  sorts?: Record<string, (row: T) => string | number>
  filters?: Record<string, (row: T, value: string) => boolean>
  initial?: { sort?: string, desc?: boolean, filters?: Record<string, string> }
}

export function useTableView<T>(rows: Ref<T[] | null | undefined>, opts: TableViewOptions<T> = {}) {
  const q = ref('')
  const sort = ref(opts.initial?.sort ?? '')
  const desc = ref(opts.initial?.desc ?? false)
  const filters = reactive<Record<string, string>>({ ...opts.initial?.filters })
  const norm = (s: string | null | undefined) => (s ?? '').toLowerCase().replace(/ё/g, 'е')

  const shown = computed<T[]>(() => {
    const needle = norm(q.value.trim())
    let list = (rows.value ?? []).filter((r) => {
      for (const [key, test] of Object.entries(opts.filters ?? {})) {
        const v = filters[key]
        if (v && !test(r, v)) return false
      }
      return !needle || !opts.search || opts.search(r).some(x => norm(x).includes(needle))
    })
    const key = opts.sorts?.[sort.value]
    if (key) {
      const dir = desc.value ? -1 : 1
      list = [...list].sort((a, b) => {
        const x = key(a)
        const y = key(b)
        return (x < y ? -1 : x > y ? 1 : 0) * dir
      })
    }
    return list
  })

  /** Щелчок по заголовку: та же колонка — обратный порядок, другая — с неё (числа — сначала большие) */
  function sortBy(k: string, numericFirstDesc = true) {
    if (sort.value === k) desc.value = !desc.value
    else {
      sort.value = k
      desc.value = numericFirstDesc
    }
  }
  const ariaSort = (k: string) => (sort.value === k ? (desc.value ? 'descending' : 'ascending') : 'none')
  const sortIcon = (k: string) => (sort.value === k ? (desc.value ? 'ph:caret-down' : 'ph:caret-up') : undefined)

  const state = computed<ViewState>(() => ({ q: q.value.trim(), sort: sort.value, desc: desc.value, ...filters }))
  function apply(s: ViewState) {
    q.value = typeof s.q === 'string' ? s.q : ''
    sort.value = typeof s.sort === 'string' ? s.sort : (opts.initial?.sort ?? '')
    desc.value = typeof s.desc === 'boolean' ? s.desc : (opts.initial?.desc ?? false)
    for (const key of Object.keys(opts.filters ?? {})) {
      const v = s[key]
      filters[key] = typeof v === 'string' ? v : ''
    }
  }
  return reactive({ q, sort, desc, filters, shown, sortBy, ariaSort, sortIcon, state, apply })
}
