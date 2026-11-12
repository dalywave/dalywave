# DalyWave на iPhone и iPad

Приложение ставится прямо из Xcode, без TestFlight. Первый раз — минут 15, дальше обновление —
одна кнопка. Нужны Mac с Xcode 27.2, Rust, bun, xcodegen и iPhone с кабелем.

## 1. Проект

```sh
cd ~/Documents/Develop/SideMusic
make ios        # пересобирает Xcode-проект и открывает его в Xcode
```

Проект — `apps/shell/gen/apple/dalywave-shell.xcodeproj`. Интерфейс и Rust Xcode собирает сам в
фазе «Build Rust Code» (`apps/shell/scripts/build-ios.sh`): интерфейс — если его исходники новее
сборки, Rust — всегда release. Запущенный `cargo tauri` для этого не нужен.

## 2. iPhone

1. Подключите кабелем, на iPhone — «Доверять этому компьютеру».
2. Режим разработчика: «Настройки» → «Конфиденциальность и безопасность» → «Режим разработчика»
   → вкл., перезагрузка, подтвердить. Пункт появляется после первого подключения к Xcode — если его
   нет, сделайте шаг 3 и вернитесь.

## 3. Xcode

1. Слева проект `dalywave-shell` → цель `dalywave-shell_iOS` → «Signing & Capabilities»:
   «Automatically manage signing» включено, Team — **Zakhar Stupak (Y5KR5JPCRG)**, Bundle
   Identifier — `rest.daly.wave`. Если команды нет: Xcode → Settings → Accounts → «+» → Apple ID.
2. Сверху: схема **dalywave-shell_iOS**, устройство — ваш iPhone (или iPad).
3. ⌘R (Product → Run). Первая сборка — 3–5 минут, потом — меньше минуты.
4. Если iPhone пишет «Ненадёжный разработчик»: «Настройки» → «Основные» → «VPN и управление
   устройством» → ваш профиль → «Доверять». Запустите снова.

Без кабеля: после первой установки Xcode → Window → Devices and Simulators → iPhone → «Connect via
network»; дальше ⌘R работает по Wi‑Fi. Приложение из Xcode живёт, пока действует профиль
разработки (год), — потом снова ⌘R. Обновление — тоже ⌘R: вход и настройки сохраняются.

## 4. Вход

1. «Войти через daly.rest». iOS спросит: «“DalyWave” хочет использовать “wave.daly.rest” для
   входа» → «Продолжить».
2. Поверх приложения откроется страница daly.rest: войдите как обычно (Google, Discord, Telegram,
   Twitch, ключ доступа). Если в Safari на iPhone вы уже входили в daly.rest, вход пройдёт сразу.
3. Окно закроется само, откроется главная. Токен устройства — в связке ключей iPhone; устройство
   видно в админке («Доступ» → сессии), выйти — в настройках приложения.

Если окно входа не открылось, приложение откроет Safari; после входа Safari спросит «Открыть в
DalyWave?» → «Открыть».

## 5. Что проверить

| # | Действие | Ожидание |
|---|---|---|
| 1 | Включить волну или альбом | Звук меньше чем через секунду, треки идут подряд без пауз |
| 2 | Заблокировать телефон | Музыка играет. На экране блокировки — название, исполнитель, обложка, прогресс |
| 3 | Кнопки на экране блокировки: пауза, играть, следующий, предыдущий, ползунок | Работают, интерфейс после разблокировки показывает тот же трек |
| 4 | Оставить заблокированным на 10 минут | Играет всё время, треки меняются сами |
| 5 | Пункт управления, AirPods (двойное касание — следующий) | То же, что на экране блокировки |
| 6 | Наушники снять или выдернуть | Пауза. Надеть и нажать «играть» — продолжает |
| 7 | Звонок, Siri, будильник | Пауза; после — продолжает, если iOS разрешит |
| 8 | Настройки → «Переходы между треками» → «Плавный» | Треки накладываются 6 с, громкость ровная |
| 9 | Свернуть приложение и открыть другое | Играет |
| 10 | Поставить на паузу на ночь, утром — «играть» | Продолжает: истёкшую ссылку на файл движок получит заново сам |
| 11 | Альбом → «Скачать», свернуть приложение на пару минут | Загрузка шла в фоне: кольца на строках, в «Медиатеке» → «Скачанное» — альбом |
| 12 | Авиарежим → «Моя волна» | Играет из скачанного, пропуски и настроения учитываются; альбом из «Скачанного» играет |
| 13 | Лайк, «играть», «следующий», шторка по ступеням, долгое нажатие | Лёгкий тактильный отклик |

## «Поделиться → DalyWave»

Ссылка из YouTube, Safari, Telegram, Spotify, Apple Music: «Поделиться» → **DalyWave** (если его нет в
ряду — «Ещё» → включить). Приложение открывается и предлагает «Слушать», «В любимые», «В плейлист».
Это расширение `dalywave-share` (`gen/apple/ShareExtension`, Bundle ID `rest.daly.wave.share`) —
Xcode подписывает его той же командой сам. Проверка на симуляторе — `ShareExtensionTests` в
`make ios-test`, снимки — `design/shots/share`.

## Виджеты

Домашний экран → долгое нажатие → «Изменить» → «Добавить виджет» → DalyWave:

| Виджет | Размеры | Что внутри |
|---|---|---|
| Сейчас играет | маленький, средний, строка экрана блокировки | обложка, название, исполнитель, ⏯ ⏭; фон — цвет обложки |
| Моя волна | маленький, средний, круглый значок экрана блокировки | запуск волны; в среднем — три сохранённых сочетания (пока своих нет — «Работа», «Дорога», «Ночь») |
| Недавнее | средний, большой | полка «Продолжить» главной: альбомы, плейлисты, артисты; нажатие открывает экран |

Маленькие виджеты работают и в StandBy. Тёмная и светлая тема — по системе; тонированные и
прозрачные значки iOS 18+ — обложка обесцвечивается и подкрашивается. Кнопки (iOS 17+) — App Intents
(`gen/apple/WidgetsShared/WidgetIntents.swift`): система выполняет их в процессе приложения и
запускает его в фоне, если оно закрыто; дальше — тот же путь, что у Siri (`src/siri.rs`). На iOS 15–16
кнопки — ссылки `dalywave://remote/toggle`, `dalywave://remote/next`, `dalywave://wave?…`
(открывают приложение).

Данные: расширение `dalywave-widgets` (`gen/apple/Widgets`, Bundle ID `rest.daly.wave.widgets`) в сеть
не ходит — читает снимок из App Group **`group.rest.daly.wave`**. Пишет его приложение
(`Sources/dalywave-shell/DalyWaveWidgets.swift`, данные даёт `src/widgets.rs`): что играет — при смене
трека и паузе; недавнее и сочетания — с сервера при запуске, после входа и при уходе в фон (не чаще
раза в 5 минут). После выхода виджеты показывают кнопку волны.

### Что включить в Apple Developer (один раз)

Автоподпись с ключом API регистрирует профили, но **группу App Group по ключу API Xcode создать не
может** («Authentication failed … bearer token» на шаге App Groups) — её создают руками:

1. developer.apple.com → Certificates, Identifiers & Profiles → **Identifiers** → «+» → **App Groups** →
   Description «DalyWave», Identifier `group.rest.daly.wave` → Continue → Register.
2. Identifiers → «+» → **App IDs** → App → Description «DalyWave Widgets», Bundle ID Explicit
   `rest.daly.wave.widgets` → в Capabilities галочка **App Groups** → Continue → Register.
3. Identifiers → **rest.daly.wave** → галочка **App Groups** → Edit (Configure) → отметить
   `group.rest.daly.wave` → Continue → **Save** → Confirm. То же для `rest.daly.wave.widgets`.
4. `make testflight` — профили с группой автоподпись выпустит сама.

Проверки без телефона:

```sh
xcodebuild test -project gen/apple/dalywave-shell.xcodeproj -scheme dalywave-widgets \
  -destination 'platform=iOS Simulator,name=iPhone 16 (iOS 27)'   # PNG каждого виджета → design/shots/widgets
```

Обложки для снимков — `WIDGET_ART=<папка>` (через `TEST_RUNNER_WIDGET_ART`), иначе цветные квадраты.
`WidgetsHomeScreenTests` (`make ios-test`, нужен `make test-audio-server`) добавляет виджеты на
домашний экран симулятора и нажимает ⏯ — снимки в `design/shots/widgets/sim`.

## «Распознать» (Shazam)

Один раз, до первой проверки, — в Apple Developer (developer.apple.com → Account → **Certificates,
Identifiers & Profiles** → **Identifiers**):

1. Открыть App ID **rest.daly.wave** (если его нет — Xcode создаёт его при первом ⌘R).
2. Вкладка **App Services** → поставить галочку **ShazamKit** → **Save** (подтвердить «Confirm»).
3. В Xcode: Settings → Accounts → команда Y5KR5JPCRG → **Download Manual Profiles**, затем ⌘R. Отдельного
   права (entitlement) в проекте для ShazamKit нет — служба включается только у App ID.

Тот же App ID — у приложения macOS (`make app`, подпись Developer ID той же командой): на Mac
распознавание заработает после того же шага. Без службы Shazam отвечает ошибкой — экран покажет
«Shazam не ответил», в журнале — `recognize: ShazamKit error 202`.

Проверка на iPhone: «Поиск» → значок микрофона справа от поля (или долгое нажатие на значок DalyWave →
«Распознать»). Первый раз iOS спросит доступ к микрофону — «Разрешить». Включите песню на другом
устройстве: за 3–8 с откроется лист с обложкой и «Слушать», «В любимые», «В плейлист», «Волна по треку»;
своя музыка на это время на паузе. Узнанное — в «Поиске», блок «Распознанное». Симулятор без
микрофона: `DALYWAVE_RECOGNIZE_MOCK=1 DALYWAVE_RECOGNIZE_OPEN=1` (BRIDGE.md, «Распознать»), снимки —
`design/shots/shazam`.

## AirPlay 2 и HomePod

Ничего включать не нужно. Аудиосессия — «длинное аудио» (`longFormAudio`): в окне AirPlay (Пункт
управления или «Устройства» в плеере) видны колонки AirPlay 2, можно отметить несколько HomePod сразу.
Как только в маршруте AirPlay, движок переходит на буферизованный вывод (AVSampleBufferAudioRenderer):
буфер на стороне колонки, мультирум, позиция и текст песни — по тому, что слышно. В журнале:
`output reopened (route airplay: buffered output)`, обратно — `route builtin: realtime output`.

| # | Действие | Ожидание |
|---|---|---|
| 1 | Играет на телефоне → AirPlay → HomePod | Пауза около секунды, дальше с того же места на колонке |
| 2 | Отметить вторую колонку | Звучат обе синхронно; переключения вывода в журнале нет |
| 3 | Пауза, перемотка, следующий на экране блокировки | Сразу, без «хвоста» в пару секунд |
| 4 | Строка прогресса и текст песни | Идут вровень со звуком колонки, а не на 2 с впереди |
| 5 | Громкость в приложении | Меняется сразу |
| 6 | AirPlay → динамик телефона | Продолжает с того места, что звучало на колонке |

Симулятор AirPlay не умеет: тот же путь без колонок — `SIMCTL_CHILD_DALYWAVE_OUTPUT=buffered` к
`make ios-sim` (на Mac — `DALYWAVE_OUTPUT=buffered`), логика переключения — `tests/buffered.rs`
(подставной маршрут). Эквалайзер и затухание таймера сна в AirPlay слышны с задержкой буфера.

## Siri и «Команды»

Фразы (iOS 17+, работают сразу после установки, без настроек): «Включи мою волну в DalyWave»,
«Включи бодрую (весёлую, спокойную, грустную) волну в DalyWave», «Включи волну для дороги (работы,
тренировки, сна…) в DalyWave», «Включи <плейлист, альбом или артист из медиатеки> в DalyWave»,
«Пауза в DalyWave», «Дальше в DalyWave», «Предыдущий трек в DalyWave», «Продолжи DalyWave»,
«Нравится в DalyWave»; по-английски — «Play my wave in DalyWave» и т. д. Все они — плитки в
приложении «Команды» (раздел DalyWave), их можно собирать в свои команды. Произносить название
можно и как «Дейли Вейв».

Запрос музыки SiriKit — «Включи Lowborn в DalyWave», «Включи мою музыку в DalyWave», и он же на
HomePod — идёт через расширение `dalywave-siri` (`gen/apple/SiriExtension`, Bundle ID
`rest.daly.wave.siri`): приложение просыпается в фоне, ищет в каталоге и играет, звук на HomePod — по
AirPlay 2. При первом запуске после входа iOS один раз спросит «Разрешить Siri» — «Разрешить».

### Что включить в Apple Developer (один раз)

developer.apple.com → Account → **Certificates, Identifiers & Profiles** → **Identifiers**:

1. **rest.daly.wave** → вкладка Capabilities (App Services) → галочка **SiriKit** → **Save** → Confirm.
2. **rest.daly.wave.siri** — если его нет: «+» → App IDs → App → Description «DalyWave Siri», Bundle
   ID Explicit `rest.daly.wave.siri` → Continue → Register (служб не нужно). Xcode с вошедшим
   аккаунтом создаёт его сам.
3. **App Groups для Siri не нужны**: расширение не ходит в сеть и не читает токен (группа
   `group.rest.daly.wave` нужна виджетам — см. «Виджеты»).
4. Xcode → Settings → Accounts: аккаунт Apple ID команды Y5KR5JPCRG должен быть добавлен (без него
   `make ios-build` пишет «No Accounts» и «profile doesn't include the Siri capability»), затем
   **Download Manual Profiles** и ⌘R / `make ios-build`. Автоподпись сама выпустит профили с Siri для
   приложения и расширения.

### Что заработает только из TestFlight / на телефоне

- **HomePod.** «Привет, Siri, включи мою волну» без «в DalyWave» — после выбора сервиса по
  умолчанию: «Дом» → Дом → Настройки дома → ваш профиль → **Медиасервисы** (Media Services) →
  DalyWave → включить, затем там же **Сервис по умолчанию** → DalyWave. DalyWave появится в списке,
  только когда приложение установлено через TestFlight или App Store (сборки из Xcode Siri на HomePod
  не предлагает), а «Распознавать мой голос» на HomePod включён. HomePod отдаёт запрос iPhone рядом
  (тот же Apple ID, iPhone разблокирован хотя бы раз после включения).
- Если интерфейс спит (телефон заблокирован), волну включает сама оболочка: музыка играет сразу, а
  интерфейс увидит её очередь, только когда приложение откроют.
- Проверка без голоса: `make ios-test` — тест `SiriShortcutsTests` открывает «Команды», раздел DalyWave
  и выполняет «Пауза» (снимки — `design/shots/siri`; нужен `make test-audio-server`).

## Apple Watch

Приложение часов (`gen/apple/WatchApp`, SwiftUI, watchOS 11+, Bundle ID `rest.daly.wave.watchkitapp`)
ставится вместе с iPhone-приложением: iOS сама предложит его на часах (или «Watch» → DalyWave →
«Установить»). Это пульт «Где играть»: «Сейчас играет» (⏮ ⏯ ⏭, «Нравится», громкость колёсиком
Digital Crown, выбор устройства), «Моя волна» (одно нажатие, настроения, сохранённые сочетания),
медиатека («Любимые треки», плейлисты, недавнее). Звук играет на iPhone, компьютере или в браузере.
Осложнения и Smart Stack (`gen/apple/WatchWidgets`, `rest.daly.wave.watchkitapp.widgets`): кнопка
«Моя волна» и мини «Сейчас играет».

**Вход.** Своего входа у часов нет. Часы просят ключ у iPhone (WatchConnectivity,
`Sources/dalywave-shell/WatchLink.swift`), iPhone просит у сервера дочернюю сессию
`POST /api/auth/app/pair` (`src/watch.rs`, токен iPhone не покидает Rust) и передаёт её часам; часы
хранят её в своей связке ключей. В разделе «Доступ» это отдельная строка «Apple Watch»: её можно
отозвать, а выход или отзыв iPhone отзывает и часы. Повторная выдача заменяет прежние часы этого
iPhone. Ключ отозван — часы сами просят новый, когда iPhone рядом.

**Связь.** watchOS даёт WebSocket только приложениям со своим звуком, поэтому на часах — пульт по
HTTP: раз в 2 с `POST /api/devices/remote` (отметиться и забрать состояние без очереди), команды —
`POST /api/devices/remote/command` с подтверждением устройства в ответе, перенос —
`/api/devices/remote/transfer`. На симуляторе WebSocket работает, и часы ходят по нему
(`DALYWAVE_WATCH_TRANSPORT=http` — проверить путь настоящих часов).

### Что включить в Apple Developer (один раз)

Часы и осложнения делят снимок «Сейчас играет» через App Group `group.rest.daly.wave` (ту же, что у
виджетов iPhone). Ключ App Store Connect API группы не создаёт (у API нет такой операции), поэтому один
раз — в портале или Xcode с вошедшим аккаунтом:

1. developer.apple.com → **Certificates, Identifiers & Profiles** → **Identifiers** → «+» →
   **App Groups** → Description «DalyWave», Identifier `group.rest.daly.wave` → Register (если её ещё
   нет).
2. Там же «+» → **App IDs** → App → Bundle ID Explicit `rest.daly.wave.watchkitapp` (Description
   «DalyWave Watch») → Capabilities: **App Groups** → Register. Затем откройте его → App Groups →
   **Edit** → отметить `group.rest.daly.wave` → Save.
3. То же для `rest.daly.wave.watchkitapp.widgets` («DalyWave Watch Widgets»).
4. `make testflight` (или архив в Xcode): автоподпись с ключом API сама выпустит профили.

Проще всего: Xcode → Settings → Accounts (Apple ID команды Y5KR5JPCRG) → открыть проект, цель
`dalywave-watch` → Signing & Capabilities: Xcode сам зарегистрирует App ID и группу; то же для
`dalywave-watch-widgets`.

### Симулятор

Пара часов и iPhone: `xcrun simctl pair <часы> <iPhone>`, обе загрузить, поставить приложение iPhone
(`DALYWAVE_DEV_TOKEN`, `DALYWAVE_SERVER_URL`) и приложение часов из
`Products/debug-watchsimulator/DalyWaveWatch.app`. Без iPhone: часы берут ключ из
`DALYWAVE_DEV_TOKEN` / `DALYWAVE_DEV_SERVER` (только симулятор). Проверки без касаний (только
симулятор): `DALYWAVE_WATCH_PAGE` (0 — «Сейчас играет», 1 — «Моя волна», 2 — медиатека),
`DALYWAVE_WATCH_SHEET=devices`, `DALYWAVE_WATCH_ACTIONS=toggle,next,like,calm,liked`. Переменные
передаются как `SIMCTL_CHILD_…` при `xcrun simctl launch`.

## Уведомления (push)

Сервер присылает уведомления через APNs: новые релизы подписок, перенос музыки завершён, «Итоги года»
открыты, администратору — спорные треки и проблемы источников. Разрешение приложение просит не при
первом запуске, а после входа — мягким экраном или в «Настройки» → «Уведомления». Токен устройства
интерфейс отправляет серверу при каждом запуске (`push_register`, `src/push.rs`). Нажатие открывает
нужный экран (`dalywave://…`, `Sources/dalywave-shell/PushLink.swift`); пока приложение на экране,
уведомление показывает сам интерфейс, а в системе остаётся только запись в центре уведомлений. На
значке — число непрочитанных.

Без ключа APNs на сервере всё остальное работает: центр уведомлений и тосты в приложении приходят по
WebSocket, просто без push на заблокированный телефон.

### Разовая настройка

1. **Ключ APNs.** developer.apple.com → **Certificates, Identifiers & Profiles** → **Keys** → «+» →
   Key Name «DalyWave Push» → галочка **Apple Push Notifications service (APNs)** → **Configure** →
   Environment: **Sandbox & Production** (Key Restriction — Team Scoped) → Save → Continue → Register
   → **Download** — файл `AuthKey_XXXXXXXXXX.p8` скачивается **один раз**. Запомните **Key ID**
   (10 символов, он же в имени файла). Ключ в репозиторий не передаётся.
2. **Push у App ID.** Там же → **Identifiers** → `rest.daly.wave` → Capabilities → галочка
   **Push Notifications** → **Save** → Confirm. Профили разработки и App Store Xcode пересоздаст сам
   при автоматической подписи (⌘R или `make testflight`). Если профиль разработки
   `rest.daly.wave` уже содержит `aps-environment`, возможность включена — достаточно проверить
   галочку. Без этого шага сборка падает с ошибкой
   «Provisioning profile … doesn't include the aps-environment entitlement».
3. **Ключ на сервер** (с Mac, из папки с файлом):
   ```sh
   ssh ubuntu@129.159.27.88 'mkdir -p ~/dalywave/secrets && chmod 700 ~/dalywave/secrets'
   scp AuthKey_XXXXXXXXXX.p8 ubuntu@129.159.27.88:~/dalywave/secrets/
   ssh ubuntu@129.159.27.88 'chmod 600 ~/dalywave/secrets/AuthKey_*.p8'
   ```
4. **Настройки сервера** — в `~/dalywave/.env` (папку `~/dalywave/secrets` воркер видит как `/secrets`):
   ```sh
   DALYWAVE_APNS_KEY_PATH=/secrets/AuthKey_XXXXXXXXXX.p8
   DALYWAVE_APNS_KEY_ID=XXXXXXXXXX
   DALYWAVE_APNS_TEAM_ID=Y5KR5JPCRG
   ```
   Топик по умолчанию — `rest.daly.wave`. Затем перезапустить воркер и API:
   `docker compose --env-file .env -f infra/docker-compose.prod.yml up -d`.

### Среда APNs

В проекте у приложения `aps-environment = development`. Отладочная сборка (⌘R из Xcode, профиль
разработчика) получает токены **песочницы**; симулятор — тоже. При экспорте для App Store
(`make testflight`) Xcode переподписывает архив профилем дистрибуции, и право становится
`production` (архив подписан с `development`, локальный экспорт для App Store —
`production`): TestFlight и App Store получают **боевые** токены. Приложение определяет среду само —
по `embedded.mobileprovision` в бандле (у TestFlight там `production`, у App Store профиля нет) — и
сообщает её серверу вместе с токеном; сервер шлёт на `api.sandbox.push.apple.com` или
`api.push.apple.com` по среде подписки. Один ключ `.p8` работает для обеих сред.

### Проверка

1. Войти, «Настройки» → «Уведомления» → включить на этом устройстве → «Разрешить».
2. Там же «Прислать пробное» — заблокировать телефон: пробное уведомление придёт за пару секунд.
3. Нажать на уведомление — откроется нужный экран (у пробного — центр уведомлений).

Симулятор (iOS 16+ на Mac с Apple silicon) тоже получает push песочницы; без сервера проверить
показ и нажатие можно файлом: `xcrun simctl push booted rest.daly.wave push.apns` с
`{"aps":{"alert":{"title":"Проба","body":"Текст"}},"link":"dalywave://import"}`.

На компьютерах (macOS, Windows) push с сервера нет: пока приложение открыто, уведомление приходит
по WebSocket и, если окно не в фокусе, показывается системным (`src/notify.rs`). На macOS щелчок
открывает нужный экран; на Windows — только окно приложения.

## Журнал

Xcode → Window → Devices and Simulators → iPhone → Installed Apps → DalyWave → «…» → Download
Container → внутри `AppData/Library/Application Support/rest.daly.wave/logs/DalyWave.log`. По строкам
`player: …`, `remote …`, `audio session: …`, `app in background` видно, что дошло до движка.

## Симулятор

```sh
make test-audio-server            # тестовое аудио на :8420 — в отдельном терминале
make ios-sim                      # сборка, установка и запуск: очередь играет сама, громкость 0
make ios-test                     # окно входа: снимки в design/shots/ios
```

`make ios-sim` запускает приложение с `DW_VOLUME=0` (без звука) и `DALYWAVE_SMOKE` — очередь из
тестового сервера, в журнал — смены треков, «Сейчас играет», нагрузка. Экран блокировки
симулятор не рисует — это проверяется только на телефоне.

## Что пока не так

- **Прослушивания в фоне.** Историю и сигналы для рекомендаций пишет интерфейс, а в фоне iOS
  усыпляет WebView: что играло с заблокированным экраном, учтётся не полностью. Учёт в ядре на
  Rust — следующий шаг.
- **Волна в фоне.** Движок знает до 20 треков вперёд и доиграет их сам; новые треки волны
  добавляет интерфейс — когда приложение снова откроют.
- **Трек ещё скачивается на сервер** (ответ 202): из очереди движок его пропускает; ждёт такой
  трек только интерфейс, если вы включили именно его.
- **Перезапуск WebView.** Если iOS выгрузит страницу в фоне, музыка доиграет очередь движка, а
  интерфейс откроется с пустой очередью.
- **«Диджей»** пока звучит как «Умный» — кроссфейд 4 с; сведение по долям — следующий этап движка.
- **Загрузки на телефон** (умный офлайн): закреплённое и умный запас качает фоновая сессия
  URLSession — и когда приложение свёрнуто или выгружено; по умолчанию только по Wi-Fi, предел — 4 ГБ
  («Настройки» → «Загрузки»). Файлы — в `Library/Application Support/rest.daly.wave/offline`, без резервной
  копии iCloud. Без сети играет скачанное и волна из запаса.
- **iPhone — только вертикально**, iPad — в любой ориентации.
- Аварийный вход паролем — только на сайте.
- `cargo tauri ios init` и `cargo tauri ios dev` не запускать: первый перезапишет проект, второму
  нужен dev-сервер интерфейса в локальной сети. Проект пересобирает `make ios`.
