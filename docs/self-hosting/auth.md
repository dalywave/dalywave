# Вход и роли на своём сервере (набросок)

DalyWave пускает людей тремя способами, их можно совмещать:

1. **Свои аккаунты** — логин и пароль (argon2id). Включены по умолчанию: `DALYWAVE_LOCAL_AUTH=true`.
2. **Провайдеры OpenID Connect** — Authentik, Keycloak, Pocket ID, Google, daly.rest и любой другой
   с discovery (`/.well-known/openid-configuration`). Их может быть несколько.
3. **Аварийный пароль** из `.env` (`DALYWAVE_ADMIN_PASSWORD`, не короче 32 символов) — входит
   как первый администратор, когда всё остальное недоступно.

## Первый запуск

Если людей ещё нет и свои аккаунты включены, экран входа предлагает «Создать администратора»:
имя, логин, пароль — и всё. Остальных администратор заводит в **Админка → Доступ**:

- **Новый аккаунт** — логин и роль; пароль человек задаст сам по одноразовой ссылке (48 часов).
- **Пригласить** — ссылка с ролью и сроком (1–30 дней). По ней можно придумать логин и пароль
  или войти через провайдера — новый аккаунт сразу получает роль.
- **⋯ → Ссылка для нового пароля** — сброс пароля: прежние сессии человека закрываются.

Почту сервер не рассылает: ссылки администратор передаёт сам.

> Мастер первого запуска открыт любому, кто первым откроет сайт. Поднимайте сервер и сразу
> создавайте администратора, не оставляя пустую установку в интернете.

## Роли

Роли — свои у DalyWave: `admin` (всё, включая админку) и `listener` (слушать, вести медиатеку).
Человек без ролей видит экран «Ждём доступа», пока администратор не выдаст роль.

Провайдер может делать людей администраторами: если в claim `_ADMIN_CLAIM` есть одно из значений
`_ADMIN_VALUE`, человек получает `admin` при каждом входе. Когда провайдер перестаёт присылать
значение, роль снимается — но только та, что выдал этот же провайдер. Роли, выданные вручную в
«Доступе», провайдеры не трогают. `DALYWAVE_ADMIN_EMAILS` — почты, которые всегда получают `admin`
(только с подтверждённой почтой от доверенного провайдера).

## Провайдеры

Каждый провайдер — набор переменных `DALYWAVE_OIDC_<ID>_*`. `<ID>` — латиница, цифры и `_`; в адресах
он становится строчным с `-` (`POCKET_ID` → `pocket-id`).

| Переменная | Что это | По умолчанию |
|---|---|---|
| `_ISSUER` | issuer провайдера | обязательно |
| `_CLIENT_ID` | id клиента | обязательно |
| `_CLIENT_SECRET` | секрет; без него — публичный клиент (только PKCE) | — |
| `_NAME` | подпись кнопки «Войти через …» | из `<ID>` |
| `_ICON` | картинка на кнопке: `https://…` или путь на сайте | — |
| `_SCOPES` | scope через пробел; `openid` добавляется сам | `openid email profile` |
| `_ADMIN_CLAIM` | claim с ролями/группами, можно вложенный (`realm_access.roles`); `none` — не выдавать админов | `roles` |
| `_ADMIN_VALUE` | значения claim для админа, через запятую | `admin` |
| `_AUTO_REGISTER` | пускать новых людей (без ролей); `false` — только известных и по приглашению | `true` |
| `_TRUST_EMAIL` | доверять подтверждённой почте: по ней вход связывается с уже известным человеком | `false` |
| `_AUTH_METHOD` | `client_secret_basic` или `client_secret_post` | по discovery |

`DALYWAVE_OIDC_PROVIDERS=authentik,google` задаёт порядок кнопок.

**Адрес возврата**, который регистрируется у провайдера: `https://<ваш сайт>/api/auth/oidc/<id>/callback`
(виден в «Доступ → Способы входа», там же кнопка «скопировать»).

Подписи id_token: RS256/384/512, PS256/384/512, ES256, ES384, EdDSA (ключи из JWKS) и HS256/384/512
(секретом клиента). Код меняется на токены только на сервере, PKCE S256, `state` и `nonce` — в
зашифрованной cookie браузера.

**Связка аккаунтов.** Человек ищется по паре (issuer, `sub`). По подтверждённой почте вход связывается
с уже известным человеком только у провайдеров с `_TRUST_EMAIL=true` — включайте его лишь там, где
провайдер сам проверяет почту и где почту нельзя поменять на чужую. Человек может сам привязать
провайдера в **Настройки → Вход** (на сайте) и отвязать, если остаётся другой способ войти.

### Authentik

Приложение → провайдер OAuth2/OpenID, тип клиента Confidential, Redirect URI — адрес возврата выше.
Подпись — выберите ключ (RS256); без ключа Authentik подписывает HS256 секретом клиента, это тоже работает.

```env
DALYWAVE_OIDC_AUTHENTIK_ISSUER=https://auth.example.com/application/o/dalywave/
DALYWAVE_OIDC_AUTHENTIK_CLIENT_ID=…
DALYWAVE_OIDC_AUTHENTIK_CLIENT_SECRET=…
DALYWAVE_OIDC_AUTHENTIK_NAME=Authentik
DALYWAVE_OIDC_AUTHENTIK_ADMIN_CLAIM=groups
DALYWAVE_OIDC_AUTHENTIK_ADMIN_VALUE=DalyWave Admins
```

### Keycloak

Клиент OpenID Connect, Client authentication — On, Valid redirect URIs — адрес возврата.
Роли realm лежат в `realm_access.roles` (или добавьте mapper в claim `groups`).

```env
DALYWAVE_OIDC_KEYCLOAK_ISSUER=https://sso.example.com/realms/home
DALYWAVE_OIDC_KEYCLOAK_CLIENT_ID=dalywave
DALYWAVE_OIDC_KEYCLOAK_CLIENT_SECRET=…
DALYWAVE_OIDC_KEYCLOAK_ADMIN_CLAIM=realm_access.roles
DALYWAVE_OIDC_KEYCLOAK_ADMIN_VALUE=dalywave-admin
```

### Pocket ID

Вход ключами доступа. Клиент можно сделать публичным (без секрета).

```env
DALYWAVE_OIDC_POCKET_ID_ISSUER=https://id.example.com
DALYWAVE_OIDC_POCKET_ID_CLIENT_ID=…
DALYWAVE_OIDC_POCKET_ID_NAME=Pocket ID
DALYWAVE_OIDC_POCKET_ID_SCOPES=openid email profile groups
DALYWAVE_OIDC_POCKET_ID_ADMIN_CLAIM=groups
DALYWAVE_OIDC_POCKET_ID_ADMIN_VALUE=admins
```

### Google

Google Cloud → APIs & Services → Credentials → OAuth client ID (Web application),
Authorized redirect URI — адрес возврата. Ролей Google не присылает.

```env
DALYWAVE_OIDC_GOOGLE_ISSUER=https://accounts.google.com
DALYWAVE_OIDC_GOOGLE_CLIENT_ID=….apps.googleusercontent.com
DALYWAVE_OIDC_GOOGLE_CLIENT_SECRET=…
DALYWAVE_OIDC_GOOGLE_NAME=Google
DALYWAVE_OIDC_GOOGLE_ADMIN_CLAIM=none
DALYWAVE_OIDC_GOOGLE_TRUST_EMAIL=true
```

### daly.rest (прежние переменные)

`DALYWAVE_OIDC_ISSUER`, `DALYWAVE_OIDC_CLIENT_ID`, `DALYWAVE_OIDC_CLIENT_SECRET` (и `_TITLE`,
`_ADMIN_ROLES`) по-прежнему работают: это провайдер `dalyrest` с адресом возврата
`/api/auth/oidc/callback`, claim `roles` = `admin`, почте доверяет.

## Приложения

Приложения (Mac, Windows, iPhone, Stream Deck) входят через системный браузер: он открывает этот же
экран входа сайта со всеми способами, после входа браузер возвращает приложению одноразовый код.
Если способ входа один — провайдер без своих аккаунтов, — браузер сразу уходит к провайдеру.
Телевизор входит кодом с экрана, часы — от iPhone.

## Ограничения

- Вход своим аккаунтом: 5 попыток в минуту на пару «адрес + логин», 30 — на адрес.
- Пароль — не короче 10 символов. Логин — 3–32 символа: буквы, цифры, `.`, `_`, `-`.
