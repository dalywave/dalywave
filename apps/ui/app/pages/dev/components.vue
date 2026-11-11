<script setup lang="ts">
import type { AlbumSummary, ArtistSummary, AudioStatus, PlaylistSummary, ShelfItem, Track } from '~/api/types'
import type { WaveMood, WaveSettings } from '~/bridge/types'
import type { TransitionMode } from '~/stores/player'
import type { MenuEntry } from '~/types/ui'
import { MOOD_ORB } from '~/utils/wave-options'
import { syntheticPeaks } from '~/utils/peaks'

// Галерея компонентов «Полночь» — только разработка (в сборку не попадает).
// Каждый компонент во всех состояниях; данные — настоящие, из медиатеки локального сервера
definePageMeta({ layout: 'bare' })
useHead({ title: 'Компоненты' })

const bridge = useBridge()
const player = usePlayerStore()
const toast = useToast()

const tracks = ref<Track[]>([])
const albums = ref<AlbumSummary[]>([])
const artists = ref<ArtistSummary[]>([])
const playlists = ref<PlaylistSummary[]>([])
const loaded = ref(false)

onMounted(async () => {
  try {
    const [liked, saved, followed, pls] = await Promise.all([
      bridge.library.liked(null, 40),
      bridge.library.savedAlbums(),
      bridge.library.followed(),
      bridge.playlists.list(),
    ])
    tracks.value = liked.items.map(i => i.track).filter(t => t.album?.image)
    albums.value = saved.items.map(s => s.album)
    artists.value = followed.items.map(f => f.artist)
    playlists.value = pls.items
  }
  catch {
    toast.error('Галерея: войдите на /login — образцы берутся из медиатеки сервера')
  }
  loaded.value = true
})

const tr = (i: number) => tracks.value[i % Math.max(1, tracks.value.length)]
const withStatus = (t: Track | undefined, audioStatus: AudioStatus) => (t ? { ...t, audioStatus } : undefined)
const ROW_STATES: { label: string, status: AudioStatus }[] = [
  { label: 'none — скачается при запуске', status: 'none' },
  { label: 'queued — в очереди', status: 'queued' },
  { label: 'acquiring — скачивается', status: 'acquiring' },
  { label: 'disputed — ждёт проверки', status: 'disputed' },
  { label: 'failed — ошибка', status: 'failed' },
  { label: 'unavailable — нет в источниках', status: 'unavailable' },
]

// Состояния для управляемых компонентов
const seg = ref<'catalog' | 'library'>('catalog')
const segMood = ref<WaveMood>('calm')
const sw1 = ref(true)
const sw2 = ref(false)
const vol = ref(0.62)
const slider = ref(0.4)
const tab = ref<'queue' | 'lyrics' | 'devices'>('queue')
const search = ref('')
const searchFilled = ref('Lowborn')
const moods = ref<Record<WaveMood, WaveMood>>({ any: 'any', energetic: 'energetic', happy: 'happy', calm: 'calm', sad: 'sad' })
const moodFull = ref<WaveMood>('any')
const waveSet = ref<WaveSettings>({ mood: 'calm', activity: 'work', character: 'mix', language: 'any' })
const mode = ref<TransitionMode>('smart')
const modeDj = ref<TransitionMode>('dj')
const expr = ref(0.6)
const muted = ref(true)
const menuOpen = ref(false)
const sheetOpen = ref(false)
const popOpen = ref(false)
const devices = computed(() => player.devices)
onMounted(() => void player.loadDevices())

const MOODS: WaveMood[] = ['any', 'energetic', 'happy', 'calm', 'sad']
const ACCENT: Record<WaveMood, string> = { any: '#ff7eb3', energetic: '#ffb26b', happy: '#ff8fd6', calm: '#8ed8f5', sad: '#b6abff' }
const MOOD_NAME: Record<WaveMood, string> = { any: 'Любое — весь спектр «Сумерек»', energetic: 'Бодрое', happy: 'Весёлое', calm: 'Спокойное', sad: 'Грустное' }

const demoMenu: MenuEntry[] = [
  { kind: 'heading', label: 'Doctor Doctor', hint: 'Lowborn' },
  { label: 'Играть следующим', icon: 'ph:arrow-bend-down-right', kbd: ['⌘', '↵'] },
  { label: 'В очередь', icon: 'ph:list-plus' },
  { label: 'В плейлист', icon: 'ph:playlist', children: [{ label: 'Ночной фонк' }, { label: 'Тихо, работаю', checked: true }] },
  { kind: 'separator' },
  { label: 'Скачать на это устройство', icon: 'ph:download-simple', disabled: true, hint: 'В приложении' },
  { label: 'Не рекомендовать', icon: 'ph:prohibit', danger: true },
]

const peaks = syntheticPeaks('gallery', 120)
const resumeItems = computed<ShelfItem[]>(() => {
  const out: ShelfItem[] = []
  if (albums.value[0]) out.push({ type: 'album', ...albums.value[0] })
  if (playlists.value[0]) out.push({ type: 'playlist', ...playlists.value[0] })
  if (artists.value[0]) out.push({ type: 'artist', ...artists.value[0] })
  if (tracks.value[0]) out.push({ type: 'track', ...tracks.value[0] })
  return out
})
const genPlaylist = computed<PlaylistSummary | null>(() => (playlists.value[0] ? { ...playlists.value[0], id: 'mix-demo', title: 'Микс дня 1', kind: 'mix', covers: [] } : null))

function showToast(kind: 'plain' | 'undo' | 'error') {
  if (kind === 'plain') toast.show({ text: '«Doctor Doctor» — в любимых', icon: 'ph:heart-fill' })
  else if (kind === 'undo') toast.show({ text: '«Doctor Doctor» убран из любимых', icon: 'ph:heart-break', undo: () => {} })
  else toast.error('Не получилось сохранить — проверьте связь и попробуйте ещё раз')
}
</script>

<template>
  <div class="gal">
    <header class="gal__head">
      <BrandLogo />
      <div>
        <h1 class="gal__title">Компоненты «Полночь»</h1>
        <p class="gal__lead">Каждый компонент во всех состояниях. Только для разработки — в сборку страница не попадает.</p>
      </div>
      <UiButton to="/" variant="ghost" icon="ph:arrow-left">В приложение</UiButton>
    </header>

    <!-- Бренд и цвет -->
    <section id="brand" class="gal__sec">
      <h2 class="gal__h2">Бренд и цвет</h2>
      <div class="grid grid--3">
        <DevSpec label="Знак «Волна‑W» — currentColor">
          <BrandGlyph class="g48" />
          <BrandGlyph class="g28 accent" />
          <BrandGlyph class="g16" />
          <BrandLogo />
        </DevSpec>
        <DevSpec label="Значок «Сумерки»">
          <img src="/favicon.svg" alt="" width="64" height="64">
          <img src="/favicon.svg" alt="" width="40" height="40">
          <img src="/favicon.svg" alt="" width="24" height="24">
        </DevSpec>
        <DevSpec label="Спектр «Сумерек» — «Любое настроение»">
          <div class="brandbar" />
        </DevSpec>
      </div>
      <div class="grid grid--5">
        <DevSpec v-for="m in MOODS" :key="m" :label="MOOD_NAME[m]" col>
          <div class="moodsw" :class="{ 'moodsw--any': m === 'any' }" :style="{ '--o1': MOOD_ORB[m][0], '--o2': MOOD_ORB[m][1], '--o3': MOOD_ORB[m][2] }" />
          <div class="accentrow"><i :style="{ background: m === 'any' ? 'var(--brand-gradient-x)' : ACCENT[m] }" /><code>{{ ACCENT[m] }}</code></div>
        </DevSpec>
      </div>
      <div class="grid grid--2">
        <DevSpec label="Поверхности: ink-0 … ink-4, fill-1 … fill-3">
          <span v-for="n in 5" :key="n" class="chipcolor" :style="{ background: `var(--ink-${n - 1})` }">ink-{{ n - 1 }}</span>
          <span v-for="n in 3" :key="`f${n}`" class="chipcolor" :style="{ background: `var(--fill-${n})` }">fill-{{ n }}</span>
        </DevSpec>
        <DevSpec label="Текст и состояния">
          <span style="color: var(--text-1)">Основной</span>
          <span style="color: var(--text-2)">Второй</span>
          <span style="color: var(--text-3)">Третий</span>
          <span style="color: var(--text-4)">Четвёртый</span>
          <span style="color: var(--success)">Готово</span>
          <span style="color: var(--warning)">Внимание</span>
          <span style="color: var(--danger)">Ошибка</span>
        </DevSpec>
      </div>
    </section>

    <!-- Типографика -->
    <section id="type" class="gal__sec">
      <h2 class="gal__h2">Типографика — Onest</h2>
      <DevSpec label="56 · 40 · 28 · 22 · 17 · 14 · 13 · 12 · 11, табличные цифры в длительностях" col wide>
        <div class="t-display">Моя волна</div>
        <div class="t-h1">Belaya Polosa Remixes</div>
        <div class="t-h2">Медиатека</div>
        <div class="t-h3">Новинки подписок</div>
        <div class="t-h4">Спокойная волна для работы — знакомое и новое</div>
        <div>Основной текст: сервер скачивает новинки сам, как только они выходят.</div>
        <div class="t-small">Мелкий: Lowborn · Похоже на Mytch</div>
        <div class="t-caption tnum">Подпись: 1:15 — 2:49 · 412 треков</div>
        <div class="caps">Заголовок группы</div>
      </DevSpec>
    </section>

    <!-- Кнопки -->
    <section id="buttons" class="gal__sec">
      <h2 class="gal__h2">Кнопки</h2>
      <div class="grid grid--1">
        <DevSpec v-for="v in (['primary', 'secondary', 'ghost', 'accent', 'danger'] as const)" :key="v" :label="`Button ${v}: обычная · наведение · нажата · фокус · выключена · загрузка`">
          <UiButton :variant="v" icon="ph:play-fill">Слушать</UiButton>
          <UiButton :variant="v" icon="ph:play-fill" force-state="hover">Слушать</UiButton>
          <UiButton :variant="v" icon="ph:play-fill" force-state="pressed">Слушать</UiButton>
          <UiButton :variant="v" icon="ph:play-fill" force-state="focus">Слушать</UiButton>
          <UiButton :variant="v" icon="ph:play-fill" disabled>Слушать</UiButton>
          <UiButton :variant="v" loading>Слушать</UiButton>
        </DevSpec>
      </div>
      <div class="grid grid--2">
        <DevSpec label="Размеры sm · md · lg, значок справа, на всю ширину" col>
          <div class="row"><UiButton size="sm" icon="ph:shuffle">Перемешать</UiButton><UiButton icon="ph:shuffle">Перемешать</UiButton><UiButton size="lg" icon="ph:shuffle">Перемешать</UiButton></div>
          <div class="row"><UiButton variant="ghost" size="sm" icon-right="ph:caret-right">Все любимые</UiButton></div>
          <UiButton variant="primary" block icon="ph:sign-in">Войти через daly.rest</UiButton>
        </DevSpec>
        <DevSpec label="IconButton: xs · sm · md · lg · xl, тона, включена, нажата, выключена">
          <UiIconButton icon="ph:heart" label="xs" size="xs" />
          <UiIconButton icon="ph:heart" label="sm" size="sm" />
          <UiIconButton icon="ph:heart" label="md" />
          <UiIconButton icon="ph:heart" label="lg" size="lg" />
          <UiIconButton icon="ph:heart" label="xl" size="xl" />
          <UiIconButton icon="ph:heart-fill" label="liked" tone="liked" pressed />
          <UiIconButton icon="ph:queue" label="active" active />
          <UiIconButton icon="ph:caret-down" label="glass" tone="glass" />
          <UiIconButton icon="ph:skip-forward-fill" label="hover" force-state="hover" />
          <UiIconButton icon="ph:skip-forward-fill" label="pressed" force-state="pressed" />
          <UiIconButton icon="ph:skip-forward-fill" label="focus" force-state="focus" />
          <UiIconButton icon="ph:skip-forward-fill" label="disabled" disabled />
        </DevSpec>
        <DevSpec label="PlayButton: пауза · играет · наведение · нажата · фокус · загрузка · выключена">
          <MusicPlayButton />
          <MusicPlayButton playing />
          <MusicPlayButton force-state="hover" />
          <MusicPlayButton force-state="pressed" />
          <MusicPlayButton force-state="focus" />
          <MusicPlayButton loading />
          <MusicPlayButton disabled />
          <MusicPlayButton :size="56" />
        </DevSpec>
        <DevSpec label="Кнопка волны: ждёт · играет · загрузка · наведение · фокус">
          <MusicWavePlayButton :beat="false" :size="64" />
          <MusicWavePlayButton :beat="false" :size="64" playing />
          <MusicWavePlayButton :beat="false" :size="64" loading />
          <MusicWavePlayButton :beat="false" :size="64" force-state="hover" />
          <MusicWavePlayButton :beat="false" :size="64" force-state="focus" />
        </DevSpec>
      </div>
    </section>

    <!-- Выбор и ввод -->
    <section id="inputs" class="gal__sec">
      <h2 class="gal__h2">Выбор и ввод</h2>
      <div class="grid grid--2">
        <DevSpec label="Chip: обычный · выбран · наведение · выключен · убираемый">
          <UiChip icon="ph:clock-counter-clockwise">lowborn</UiChip>
          <UiChip selected>Знакомое и новое</UiChip>
          <UiChip force-state="hover">Редкое</UiChip>
          <UiChip disabled>Без слов</UiChip>
          <UiChip removable icon="ph:clock-counter-clockwise">mzlff</UiChip>
        </DevSpec>
        <DevSpec label="SegmentedControl: md · sm · со значками · с точками · выключенный пункт" col>
          <UiSegmentedControl v-model="seg" :options="[{ value: 'catalog', label: 'Каталог', icon: 'ph:globe-hemisphere-east' }, { value: 'library', label: 'Моя медиатека', icon: 'ph:books' }]" label="Где искать" />
          <UiSegmentedControl v-model="segMood" size="sm" :options="[{ value: 'energetic', label: 'Бодрое', dot: ACCENT.energetic }, { value: 'calm', label: 'Спокойное', dot: ACCENT.calm }, { value: 'sad', label: 'Грустное', dot: ACCENT.sad, disabled: true }]" label="Настроение" />
        </DevSpec>
        <DevSpec label="Switch: включён · выключен · выключенный" col>
          <UiSwitch v-model="sw1" label="Выравнивать громкость" hint="По замеру громкости на сервере" />
          <UiSwitch v-model="sw2" label="Показывать ненормативное" />
          <UiSwitch :model-value="true" label="Недоступно" disabled />
        </DevSpec>
        <DevSpec label="Slider: обычный · наведение · выключен; VolumeSlider: звук · наведение · выкл." col>
          <UiSlider v-model="slider" label="Выразительность" />
          <UiSlider v-model="slider" label="Выразительность" force-state="hover" />
          <UiSlider :model-value="0.3" label="Выключен" disabled />
          <div class="row"><MusicVolumeSlider v-model:volume="vol" /><MusicVolumeSlider v-model:volume="vol" force-hover /><MusicVolumeSlider v-model:volume="vol" v-model:muted="muted" /></div>
        </DevSpec>
        <DevSpec label="SearchField: пусто · наведение · фокус · с текстом · большой" col>
          <UiSearchField v-model="search" shortcut />
          <UiSearchField v-model="search" force-state="hover" />
          <UiSearchField v-model="search" force-state="focus" />
          <UiSearchField v-model="searchFilled" />
          <UiSearchField v-model="search" size="lg" />
        </DevSpec>
        <DevSpec label="Tabs и Kbd" col>
          <UiTabs v-model="tab" :items="[{ value: 'queue', label: 'Очередь', count: 7 }, { value: 'lyrics', label: 'Текст' }, { value: 'devices', label: 'Устройства' }]" label="Панель" />
          <div class="row"><UiKbd :keys="['⌘', 'K']" /><UiKbd :keys="['⇧', '→']" /><UiKbd :keys="['L']" /><UiKbd :keys="['Space']" /></div>
        </DevSpec>
      </div>
    </section>

    <!-- Настроение и волна -->
    <section id="wave" class="gal__sec">
      <h2 class="gal__h2">Настроение и волна</h2>
      <div class="grid grid--2">
        <DevSpec label="MoodOrbs компактно: выбран каждый из пяти" col>
          <MusicMoodOrbs v-for="m in MOODS" :key="m" v-model="moods[m]" />
        </DevSpec>
        <DevSpec label="MoodOrbs плитками (телефон), наведение на «Бодрое»" col>
          <MusicMoodOrbs v-model="moodFull" variant="full" hover="energetic" />
        </DevSpec>
        <DevSpec label="WaveSettingsBar: док (большой экран)" wide>
          <MusicWaveSettingsBar v-model="waveSet" />
        </DevSpec>
        <DevSpec label="WaveSettingsBar: токены (телефон)" wide>
          <MusicWaveSettingsBar v-model="waveSet" variant="tokens" />
        </DevSpec>
        <DevSpec label="SavedCombos: «Работа» сейчас" wide>
          <MusicSavedCombos active="work" />
        </DevSpec>
        <DevSpec label="WaveHero — живая волна; настроение меняет весь акцент интерфейса" wide :pad="false">
          <div class="herobox"><MusicWaveHero greeting="Добрый вечер, Захар" /></div>
        </DevSpec>
      </div>
    </section>

    <!-- Значки и состояния -->
    <section id="states" class="gal__sec">
      <h2 class="gal__h2">Значки и состояния</h2>
      <div class="grid grid--2">
        <DevSpec label="Badge: качество · explicit · новое · скоро · мета · ошибка · готово">
          <UiBadge variant="quality" icon="ph:waveform">Opus 160</UiBadge>
          <UiBadge variant="explicit" />
          <UiBadge variant="new">Новое</UiBadge>
          <UiBadge variant="soon">в приложении</UiBadge>
          <UiBadge variant="meta">88 BPM</UiBadge>
          <UiBadge variant="danger">Отключён</UiBadge>
          <UiBadge variant="success">эта сессия</UiBadge>
        </DevSpec>
        <DevSpec label="ProgressRing: в очереди · 30 % · 75 % · готово · ошибка · размеры">
          <UiProgressRing state="queued" />
          <UiProgressRing :value="0.3" />
          <UiProgressRing :value="0.75" />
          <UiProgressRing state="done" />
          <UiProgressRing state="error" />
          <UiProgressRing :value="0.5" :size="16" />
          <UiProgressRing :value="0.5" :size="36" />
        </DevSpec>
        <DevSpec label="Ждём звук: скачивается с местом в очереди · скачивается · ждём загрузчик на Mac" col>
          <MusicPendingNote :pending="{ status: 'acquiring', queuePosition: 3 }" />
          <MusicPendingNote :pending="{ status: 'acquiring', queuePosition: null }" />
          <MusicPendingNote :pending="{ status: 'waiting_for_downloader', queuePosition: null }" />
          <MusicPendingNote :pending="{ status: 'waiting_for_downloader', queuePosition: null }" long />
        </DevSpec>
        <DevSpec label="Skeleton: блок · строка · круг" col>
          <div class="row"><UiSkeleton width="56px" height="56px" /><div class="col"><UiSkeleton variant="line" width="180px" /><UiSkeleton variant="line" width="120px" /></div><UiSkeleton variant="round" width="40px" height="40px" /></div>
        </DevSpec>
        <DevSpec label="Toast (неподвижно): обычный · с «Отменить» · ошибка · с действием; живые — кнопками" col>
          <UiToast text="«Doctor Doctor» — в любимых" icon="ph:heart-fill" />
          <UiToast text="«Doctor Doctor» убран из любимых" icon="ph:heart-break" action="Отменить" />
          <UiToast text="Не получилось сохранить — проверьте связь" icon="ph:warning-circle-fill" tone="error" action="Повторить" />
          <UiToast text="Доступна новая версия DalyWave — 0.2.0" icon="ph:arrow-circle-up" action="Обновить" />
          <div class="row"><UiButton size="sm" @click="showToast('plain')">Показать</UiButton><UiButton size="sm" @click="showToast('undo')">С отменой</UiButton><UiButton size="sm" variant="danger" @click="showToast('error')">Ошибка</UiButton></div>
        </DevSpec>
        <DevSpec label="EmptyState: подсказка · ошибка · компактный" col>
          <UiEmptyState icon="ph:heart" title="Здесь появится то, что вам нравится" text="Отмечайте треки сердцем — в плеере, в списках или клавишей L" compact>
            <UiButton variant="primary" icon="ph:play-fill">Слушать волну</UiButton>
          </UiEmptyState>
          <UiEmptyState icon="ph:cloud-warning" title="Не загрузилось" text="Сервер не ответил. Проверьте связь" tone="error" compact>
            <UiButton icon="ph:arrow-clockwise">Повторить</UiButton>
          </UiEmptyState>
        </DevSpec>
      </div>
    </section>

    <!-- Всплывающие слои -->
    <section id="layers" class="gal__sec">
      <h2 class="gal__h2">Всплывающие слои</h2>
      <div class="grid grid--3">
        <DevSpec label="Подсказка (неподвижно) и с клавишами" col>
          <div class="tooltip tooltip--static">Нравится <UiKbd :keys="['L']" /></div>
          <div class="tooltip tooltip--static">Нет на сервере — скачается при запуске</div>
          <UiTooltip text="Подсказка при наведении" :kbd="['⌘', 'K']"><UiButton size="sm">Наведите</UiButton></UiTooltip>
        </DevSpec>
        <DevSpec label="Переходы между треками — панель (сайт: «Диджей» в приложении)" :pad="false">
          <MusicTransitionModePicker v-model:mode="modeDj" v-model:expressiveness="expr" inline next-at="2:26" />
        </DevSpec>
        <DevSpec label="Чип режима: обычный и узкий; панели — по нажатию" col>
          <div class="row">
            <MusicTransitionModePicker v-model:mode="mode" v-model:expressiveness="expr" side="bottom" />
            <MusicTransitionModePicker v-model:mode="mode" v-model:expressiveness="expr" compact side="bottom" />
            <MusicDevicePicker :devices="devices" side="bottom" />
          </div>
          <div class="row">
            <UiMenu v-model:open="menuOpen" :items="demoMenu" align="start"><UiButton size="sm" icon="ph:dots-three">Меню трека</UiButton></UiMenu>
            <UiPopover v-model:open="popOpen" title="Панель" hint="подсказка" :width="260">
              <template #trigger><UiButton size="sm">Popover</UiButton></template>
              <p class="poptext">Стеклянная панель у элемента: режимы переходов, устройства, новый плейлист.</p>
            </UiPopover>
            <UiButton size="sm" @click="sheetOpen = true">Шторка</UiButton>
          </div>
          <UiContextMenu :items="demoMenu">
            <div class="ctxarea">Правый клик — контекстное меню</div>
          </UiContextMenu>
        </DevSpec>
        <DevSpec label="Где играть — список устройств" :pad="false">
          <div class="pop pop--static" style="width: 320px">
            <header class="pop__head"><span class="pop__title">Где играть</span><span class="pop__hint">2 в сети</span></header>
            <MusicDeviceList :devices="devices" :playing="true" />
            <div class="pop__foot">Управление идёт через сервер: можно переключать с телефона.</div>
          </div>
        </DevSpec>
        <DevSpec label="Заголовок окна приложения: macOS (место под «светофор») и Windows" col>
          <ShellTitleBar controls="mac-inset" preview />
          <ShellTitleBar controls="windows" preview />
          <ShellTitleBar controls="windows" preview :focused="false" maximized />
        </DevSpec>
      </div>
    </section>

    <!-- Музыка -->
    <section v-if="loaded && tr(0)" id="music" class="gal__sec">
      <h2 class="gal__h2">Музыка</h2>
      <div class="grid grid--5">
        <DevSpec label="CoverArt маленькая"><MusicCoverArt :image="tr(0)!.album?.image" size="small" class="cv56" /></DevSpec>
        <DevSpec label="CoverArt со свечением"><MusicCoverArt :image="tr(1)!.album?.image" size="medium" ambient class="cv96" /></DevSpec>
        <DevSpec label="Круглая (артист)"><MusicCoverArt :image="artists[0]?.image" size="medium" round class="cv96" fallback-icon="ph:user" /></DevSpec>
        <DevSpec label="Мозаика плейлиста"><MusicPlaylistCover v-if="playlists[0]" :id="playlists[0].id" :title="playlists[0].title" :covers="playlists[0].covers" class="cv96" /></DevSpec>
        <DevSpec label="Нет обложки · собранная"><MusicCoverArt :image="null" class="cv56" /><MusicPlaylistCover id="gen-demo" title="Ночной фонк" generated class="cv96" /></DevSpec>
      </div>

      <div class="grid grid--2">
        <DevSpec label="TrackRow: обычная · играет · наведение · в любимых, версия, explicit" col :pad="false">
          <div class="rows">
            <MusicTrackRow :track="tr(2)!" :index="1" reason="Похоже на Mytch" :playing="false" />
            <MusicTrackRow :track="tr(3)!" :index="2" :playing="true" reason="Сейчас играет" />
            <MusicTrackRow :track="tr(4)!" :index="3" force-hover :playing="false" />
            <MusicTrackRow :track="{ ...tr(5)!, liked: true, explicit: true, version: 'Slowed' }" :index="4" :playing="false" />
          </div>
        </DevSpec>
        <DevSpec label="TrackRow: состояние на сервере" col :pad="false">
          <div class="rows">
            <template v-for="(s, i) in ROW_STATES" :key="s.status">
              <MusicTrackRow v-if="withStatus(tr(6 + i), s.status)" :track="withStatus(tr(6 + i), s.status)!" :index="i + 1" :reason="s.label" :playing="false" />
            </template>
          </div>
        </DevSpec>
        <DevSpec label="TrackRow: компактная · очередь · альбом · широкая" col :pad="false">
          <div class="rows">
            <MusicTrackRow :track="tr(12)!" variant="compact" :playing="false" />
            <MusicTrackRow :track="tr(13)!" variant="queue" :playing="true" reason="Похоже на Lowborn" />
            <MusicTrackRow :track="tr(14)!" variant="album" :index="7" :playing="false" />
            <MusicTrackRow :track="tr(15)!" variant="wide" :index="12" :playing="false" />
          </div>
        </DevSpec>
        <DevSpec label="TrackList — длинные списки рисуются окном" col :pad="false">
          <div class="rows"><MusicTrackList :tracks="tracks.slice(0, 5)" :context="{ type: 'library', id: 'gallery', title: 'Галерея' }" /></div>
        </DevSpec>
      </div>

      <div class="grid grid--6">
        <DevSpec v-if="albums[0]" label="AlbumCard" col><MusicAlbumCard :album="albums[0]" /></DevSpec>
        <DevSpec v-if="albums[1]" label="наведение" col><MusicAlbumCard :album="albums[1]" force-hover /></DevSpec>
        <DevSpec v-if="albums[2]" label="новинка" col><MusicAlbumCard :album="albums[2]" badge="new" meta="Mytch · 6 нояб." /></DevSpec>
        <DevSpec v-if="artists[0]" label="ArtistCard" col><MusicArtistCard :artist="artists[0]" /></DevSpec>
        <DevSpec v-if="artists[1]" label="наведение" col><MusicArtistCard :artist="artists[1]" force-hover /></DevSpec>
        <DevSpec v-if="playlists[1]" label="MixCard · микс" col><MusicMixCard :playlist="playlists[1]" /><MusicMixCard v-if="genPlaylist" :playlist="genPlaylist" /></DevSpec>
      </div>

      <div class="grid grid--2">
        <DevSpec label="ResumeCard: альбом · плейлист · радио артиста · трек; наведение" col>
          <MusicResumeCard v-for="(it, i) in resumeItems" :key="it.type" :item="it" :progress="[0.32, 0.6, null, 0.12][i]" :force-hover="i === 1" />
        </DevSpec>
        <DevSpec label="Scrubber: панель (наведение у маркера) · телефон · выключен" col>
          <MusicScrubber :peaks="peaks" :position-ms="75000" :duration-ms="169000" :transition-ms="146000" transition-label="Переход в 2:26 · Умный" force-hover />
          <MusicScrubber :peaks="peaks.slice(0, 56)" :position-ms="74000" :duration-ms="169000" :transition-ms="146000" size="lg" times="below" />
          <MusicScrubber :peaks="peaks" :position-ms="0" :duration-ms="169000" disabled />
        </DevSpec>
      </div>

      <DevSpec label="Shelf — полка с прокруткой и стрелками" wide col>
        <MusicShelf title="Новинки подписок" hint="Сервер скачивает новинки сам, как только они выходят" :cols="5">
          <MusicAlbumCard v-for="a in albums" :key="a.id" :album="a" />
        </MusicShelf>
      </DevSpec>

      <div class="grid grid--2">
        <DevSpec label="NowPlayingBar — живая; «Слушать волну» запускает её" wide :pad="false">
          <div class="barbox"><MusicNowPlayingBar /></div>
          <div class="row" style="padding: 12px 16px"><UiButton size="sm" variant="primary" icon="ph:play-fill" @click="player.startWave()">Слушать волну</UiButton><UiButton size="sm" @click="tracks[6] && player.playTracks([tracks[6]], 0, { type: 'library', id: null, title: 'Галерея' })">Трек без файла</UiButton></div>
        </DevSpec>
        <DevSpec label="MiniPlayer (телефон) — когда что-то играет" col>
          <MusicMiniPlayer inline />
          <span v-if="!player.current" class="t-caption">Ничего не играет — запустите волну кнопкой выше</span>
        </DevSpec>
        <DevSpec label="О треке" :pad="false">
          <MusicAboutTrack :track="tr(2) ?? null" />
        </DevSpec>
        <DevSpec label="Очередь — живая" :pad="false" wide>
          <div class="queuebox"><MusicQueuePanel /></div>
        </DevSpec>
      </div>
    </section>

    <UiSheet v-model:open="sheetOpen" title="Очередь">
      <div style="padding: 0 8px 16px"><MusicQueuePanel /></div>
    </UiSheet>
  </div>
</template>

<style scoped>
.gal { display: flex; flex-direction: column; gap: 56px; max-width: 1320px; margin: 0 auto; padding: 40px 32px 120px; }
.gal__head { display: flex; flex-wrap: wrap; align-items: center; justify-content: space-between; gap: 24px; }
.gal__title { margin: 0; font-size: var(--fs-h1); font-weight: 750; letter-spacing: var(--tr-display); }
.gal__lead { margin: 4px 0 0; color: var(--text-2); }
.gal__sec { display: flex; flex-direction: column; gap: 16px; scroll-margin-top: 24px; }
.gal__h2 { margin: 0; font-size: var(--fs-h3); font-weight: 700; letter-spacing: var(--tr-title); }
.grid { display: grid; gap: 16px; }
.grid--1 { grid-template-columns: minmax(0, 1fr); }
.grid--2 { grid-template-columns: repeat(2, minmax(0, 1fr)); }
.grid--3 { grid-template-columns: repeat(3, minmax(0, 1fr)); }
.grid--5 { grid-template-columns: repeat(5, minmax(0, 1fr)); }
.grid--6 { grid-template-columns: repeat(6, minmax(0, 1fr)); }
.row { display: flex; flex-wrap: wrap; align-items: center; gap: 12px; }
.col { display: flex; flex-direction: column; gap: 8px; }
.g48 { width: 48px; height: 48px; }
.g28 { width: 28px; height: 28px; }
.g16 { width: 16px; height: 16px; }
.accent { color: var(--accent); }
.brandbar { width: 100%; height: 48px; border-radius: var(--r-3); background: var(--brand-gradient-x); }
.moodsw { height: 72px; border-radius: var(--r-3); background: radial-gradient(120% 90% at 20% 110%, var(--o2), transparent 60%), radial-gradient(100% 80% at 90% 0%, var(--o3), transparent 55%), var(--o1); }
.moodsw--any { background: var(--brand-gradient); }
.accentrow { display: flex; align-items: center; gap: 8px; font-size: var(--fs-caption); color: var(--text-2); }
.accentrow i { width: 18px; height: 18px; border-radius: 50%; }
.chipcolor { display: inline-grid; place-items: center; width: 64px; height: 40px; border-radius: var(--r-2); box-shadow: inset 0 0 0 1px var(--line); font-size: var(--fs-micro); color: var(--text-2); }
.t-display { font-size: var(--fs-display); font-weight: 750; letter-spacing: var(--tr-display); line-height: 1; }
.t-h1 { font-size: var(--fs-h1); font-weight: 750; letter-spacing: var(--tr-display); }
.t-h2 { font-size: var(--fs-h2); font-weight: 750; letter-spacing: var(--tr-title); }
.t-h3 { font-size: var(--fs-h3); font-weight: 700; letter-spacing: var(--tr-title); }
.t-h4 { font-size: var(--fs-h4); font-weight: 600; }
.t-small { font-size: var(--fs-small); color: var(--text-2); }
.t-caption { font-size: var(--fs-caption); color: var(--text-3); }
.herobox { width: 100%; padding: 16px; }
.tooltip--static { position: relative; align-self: flex-start; }
.poptext { margin: 0; padding: 0 10px 10px; font-size: var(--fs-small); color: var(--text-2); }
.ctxarea { display: grid; place-items: center; height: 64px; border-radius: var(--r-3); box-shadow: inset 0 0 0 1px dashed var(--line); background: var(--fill-1); font-size: var(--fs-small); color: var(--text-3); }
.cv56 { width: 56px; }
.cv96 { width: 96px; }
.rows { width: 100%; padding: 8px; }
.barbox { position: relative; width: 100%; overflow: hidden; border-radius: var(--r-4) var(--r-4) 0 0; }
.queuebox { width: 100%; max-width: 420px; padding: 8px 12px 16px; }
@media (max-width: 1099px) {
  .grid--5, .grid--6 { grid-template-columns: repeat(3, minmax(0, 1fr)); }
  .grid--3 { grid-template-columns: repeat(2, minmax(0, 1fr)); }
}
@media (max-width: 699px) {
  .gal { gap: 40px; padding: 24px 16px 80px; }
  .gal__title { font-size: 28px; }
  .grid--2, .grid--3 { grid-template-columns: minmax(0, 1fr); }
  .grid--5, .grid--6 { grid-template-columns: repeat(2, minmax(0, 1fr)); }
  .t-display { font-size: 34px; }
  .t-h1 { font-size: 26px; }
}
</style>
