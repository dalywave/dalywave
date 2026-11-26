//! Настройки из переменных `DALYWAVE_*`. Локально их дополняет `.env` в корне репозитория.

use std::{env, net::SocketAddr, path::PathBuf, time::Duration};

use anyhow::{Context, bail};

pub const DEFAULT_DATABASE_URL: &str = "postgres://dalywave:dalywave@localhost:5400/dalywave";
pub const DEFAULT_PUBLIC_URL: &str = "http://localhost:3400";
pub const DEFAULT_LISTEN: &str = "0.0.0.0:8400";
pub const DEFAULT_DEEZER_URL: &str = "https://api.deezer.com";
pub const DEFAULT_FETCHER_URL: &str = "http://localhost:8402";
pub const DEFAULT_ML_URL: &str = "http://localhost:8403";
pub const DEFAULT_LRCLIB_URL: &str = "https://lrclib.net";
/// API «Яндекс Музыки», которым пользуются её приложения (перенос музыки).
pub const DEFAULT_YANDEX_MUSIC_URL: &str = "https://api.music.yandex.net";
/// Сколько держать запрос загрузчика, пока заданий нет: меньше таймаута его HTTP-клиента (30 с).
pub const CLAIM_WAIT: Duration = Duration::from_secs(25);
/// Аренда задания загрузчиком. Обычно трек качается 10–20 с; загрузчик продлевает аренду, пока работает.
pub const LEASE: Duration = Duration::from_secs(5 * 60);
/// Deezer пускает ~50 запросов за 5 секунд с одного адреса; API, воркер и fetcher делят этот лимит.
pub const DEEZER_WINDOW_MAX: u32 = 40;
pub const DEFAULT_DEEZER_RATE_API: u32 = 25;
pub const DEFAULT_DEEZER_RATE_WORKER: u32 = 15;
/// Короче пароль не принимается: аварийный вход не должен подбираться перебором.
pub const MIN_ADMIN_PASSWORD_LEN: usize = 32;
/// Токен выпусков открывает загрузку сборок, которые поставятся всем приложениям: тоже не короче.
pub const MIN_RELEASE_TOKEN_LEN: usize = 32;
/// APNs: боевая среда (TestFlight, App Store) и песочница (отладочные сборки из Xcode).
pub const APNS_PRODUCTION_URL: &str = "https://api.push.apple.com";
pub const APNS_SANDBOX_URL: &str = "https://api.sandbox.push.apple.com";
/// Bundle ID приложения на iPhone и iPad — топик APNs.
pub const DEFAULT_APNS_TOPIC: &str = "rest.daly.wave";
/// Название сервера по умолчанию.
pub const DEFAULT_SERVER_NAME: &str = "DalyWave";
/// Папка хранилища `fs` по умолчанию (том docker).
pub const DEFAULT_STORAGE_PATH: &str = "/data";

#[derive(Clone)]
pub struct Config {
    pub database_url: String,
    /// Где слушать: в docker — 0.0.0.0:8400, за ним Traefik
    pub listen: SocketAddr,
    /// Адрес сайта без `/` в конце. Из него адрес возврата OIDC, разрешённый Origin и флаг Secure у cookie.
    pub public_url: String,
    /// 32 байта из `SECRET_KEY`: из них выводится ключ шифрования cookie.
    pub secret_key: [u8; 32],
    /// Собранный интерфейс; без него API отдаёт только `/api`.
    pub static_dir: Option<PathBuf>,
    /// Провайдеры входа OIDC в порядке кнопок на экране входа; пусто — вход через провайдеров выключен.
    pub oidc: Vec<OidcConfig>,
    /// Свои аккаунты: логин и пароль, мастер первого запуска, приглашения. По умолчанию включены.
    pub local_auth: bool,
    /// Почты (в нижнем регистре), которые получают роль admin при входе, если провайдер их подтвердил.
    pub admin_emails: Vec<String>,
    /// Пароль аварийного входа. `None` — вход паролем выключен.
    pub admin_password: Option<String>,
    /// Логин (в нижнем регистре) общего аккаунта публичного демо: в него входят без пароля, а сменить
    /// пароль, логин, входы, удалить его или сделать админом нельзя. `None` — демо нет.
    pub demo_account: Option<String>,
    /// Хранилище S3. `None` — файлы живут на диске (`storage.kind = fs`) или в памяти процесса.
    pub s3: Option<S3Config>,
    /// Где хранятся файлы сервера: S3, свой диск или память процесса (разработка и тесты).
    pub storage: StorageConfig,
    /// Своя музыка из папок на сервере (только чтение).
    pub library: LibraryConfig,
    /// Что включено на этом сервере: каталог Deezer, загрузчик с YouTube.
    pub features: Features,
    /// Как сервер называет себя приложениям: название и объявление в локальной сети (mDNS).
    pub identity: IdentityConfig,
    pub deezer: DeezerConfig,
    pub fetcher: FetcherConfig,
    pub ml: MlConfig,
    /// Тексты песен
    pub lrclib: LrclibConfig,
    /// Перенос из «Яндекс Музыки»
    pub yandex_music: YandexMusicConfig,
    /// Сведения о музыке: чарт России, MusicBrainz, Wikidata, Википедия
    pub info: InfoConfig,
    /// Токен конвейера выпусков (`make release-mac`, CI): загрузка сборки и регистрация выпуска
    /// приложения. `None` — выпускать нельзя.
    pub release_token: Option<String>,
    /// Каналы push. Без настроек канал выключен, уведомления остаются в центре уведомлений и в окне приложения.
    pub push: PushConfig,
}

/// Где хранятся файлы сервера (обложки, аудио, копии, клипы).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StorageKind {
    /// S3-совместимое хранилище (`DALYWAVE_S3_*`): Cloudflare R2, SeaweedFS, MinIO.
    S3,
    /// Своя папка на диске (`DALYWAVE_STORAGE_PATH`): файлы отдаёт сам API по подписанным ссылкам.
    Fs,
    /// В памяти процесса — только разработка и тесты: всё пропадает при перезапуске.
    Memory,
}

#[derive(Clone, Debug)]
pub struct StorageConfig {
    pub kind: StorageKind,
    /// Папка хранилища `fs`.
    pub path: PathBuf,
    /// Адрес API для ссылок сервису анализа, если он видит API иначе, чем клиенты (`http://api:8400`
    /// в docker). Без него — `DALYWAVE_PUBLIC_URL`.
    pub ml_url: Option<String>,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            kind: StorageKind::Memory,
            path: PathBuf::from(DEFAULT_STORAGE_PATH),
            ml_url: None,
        }
    }
}

/// Папки со своей музыкой: сервер только читает их, файлы не копируются и не меняются.
#[derive(Clone, Debug, Default)]
pub struct LibraryConfig {
    pub dirs: Vec<PathBuf>,
}

/// Возможности сервера, которые включаются отдельно. У новых установок обе выключены: музыка — своя папка.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Features {
    /// Каталог Deezer: поиск по каталогу, страницы артистов и альбомов, чарты, похожие, разведчик.
    pub deezer: bool,
    /// Загрузчик: скачивание записей с YouTube (загрузчик на Mac, серверный fetcher), «Спорные», подписки
    /// с докачкой.
    pub downloader: bool,
}

/// Название сервера (экран «Адрес сервера» в приложениях, `/api/config`) и объявление
/// `_dalywave._tcp` в локальной сети: Apple TV и приложения находят сервер без ввода адреса.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IdentityConfig {
    /// `DALYWAVE_SERVER_NAME`, по умолчанию «DalyWave».
    pub name: String,
    /// `DALYWAVE_MDNS`, по умолчанию включено. В docker без сети хоста объявление в локальную сеть
    /// не выходит — там его лучше выключить.
    pub mdns: bool,
}

impl Default for IdentityConfig {
    fn default() -> Self {
        Self {
            name: DEFAULT_SERVER_NAME.into(),
            mdns: false,
        }
    }
}

/// Каналы push: APNs (iPhone, iPad) и Web Push (сайт). Android (FCM) добавится сюда же.
#[derive(Clone, Debug, Default)]
pub struct PushConfig {
    pub apns: Option<ApnsConfig>,
    pub vapid: Option<VapidConfig>,
}

/// APNs по ключу .p8 (токен JWT, ES256). Ключ — файл на сервере (`~/dalywave/secrets`), в .env — путь и id.
#[derive(Clone, Debug)]
pub struct ApnsConfig {
    pub key_path: PathBuf,
    /// Key ID ключа (10 символов) из портала разработчика.
    pub key_id: String,
    pub team_id: String,
    /// Bundle ID приложения.
    pub topic: String,
    pub production_url: String,
    pub sandbox_url: String,
}

/// Web Push: пара VAPID (P-256) в base64url — открытый ключ 65 байт, закрытый 32 байта.
#[derive(Clone)]
pub struct VapidConfig {
    pub public_key: String,
    pub private_key: [u8; 32],
    /// Контакт для служб push: `mailto:…` или адрес сайта.
    pub subject: String,
}

impl std::fmt::Debug for VapidConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VapidConfig")
            .field("public_key", &self.public_key)
            .field("subject", &self.subject)
            .finish_non_exhaustive()
    }
}

/// Пара VAPID из .env. Открытый ключ необязателен: он выводится из закрытого, а заданный — сверяется.
fn parse_vapid(
    public: Option<String>,
    private: Option<String>,
    subject: Option<String>,
    public_url: &str,
) -> anyhow::Result<Option<VapidConfig>> {
    let Some(private) = private else {
        if public.is_some() {
            tracing::warn!("DALYWAVE_VAPID_PRIVATE is not set: Web Push is off");
        }
        return Ok(None);
    };
    let bytes = crate::crypto::b64_decode(&private)
        .filter(|b| b.len() == 32)
        .context(
            "DALYWAVE_VAPID_PRIVATE must be 32 bytes in base64url (dalywave-api vapid-keys)",
        )?;
    let mut private_key = [0u8; 32];
    private_key.copy_from_slice(&bytes);
    let derived = crate::notify::webpush::public_key(&private_key)
        .context("DALYWAVE_VAPID_PRIVATE is not a valid P-256 key")?;
    if let Some(public) = public
        && public.trim_end_matches('=') != derived
    {
        bail!("DALYWAVE_VAPID_PUBLIC does not match DALYWAVE_VAPID_PRIVATE");
    }
    Ok(Some(VapidConfig {
        public_key: derived,
        private_key,
        subject: subject.unwrap_or_else(|| public_url.to_string()),
    }))
}

/// Сервис анализа звука (services/ml): векторы, темп, тональность, настроение. В базу он не ходит:
/// задания и ссылки на аудио даёт воркер, текстовые запросы для поиска по звучанию — API.
#[derive(Clone, Debug)]
pub struct MlConfig {
    /// Например `http://ml:8403`.
    pub url: String,
}

impl Default for MlConfig {
    fn default() -> Self {
        Self {
            url: DEFAULT_ML_URL.into(),
        }
    }
}

/// LRCLIB — открытая база текстов песен (ключ не нужен).
#[derive(Clone, Debug)]
pub struct LrclibConfig {
    /// Например `https://lrclib.net`.
    pub url: String,
}

impl Default for LrclibConfig {
    fn default() -> Self {
        Self {
            url: DEFAULT_LRCLIB_URL.into(),
        }
    }
}

/// Открытые источники сведений о музыке (ключи не нужны). Адреса меняются только в тестах.
#[derive(Clone, Debug)]
pub struct InfoConfig {
    /// Открытый чарт Apple Music (Россия): `https://rss.marketingtools.apple.com`.
    pub apple_charts_url: String,
    /// `https://musicbrainz.org/ws/2`.
    pub musicbrainz_url: String,
    /// `https://www.wikidata.org`.
    pub wikidata_url: String,
    /// Шаблон адреса Википедии с `{lang}`: `https://{lang}.wikipedia.org`.
    pub wikipedia_url: String,
    /// Apple Music API (живые обложки). `None` — ключа MusicKit нет, живые обложки не ищутся.
    pub musickit: Option<MusicKitConfig>,
}

/// Apple Music API по ключу MusicKit (.p8): токен разработчика (JWT, ES256) подписывает сервер.
#[derive(Clone, Debug)]
pub struct MusicKitConfig {
    pub key_path: PathBuf,
    pub key_id: String,
    pub team_id: String,
    /// Витрина каталога: `us` — там больше всего релизов.
    pub storefront: String,
    /// `https://api.music.apple.com`; меняется в тестах.
    pub url: String,
}

impl Default for InfoConfig {
    fn default() -> Self {
        Self {
            apple_charts_url: "https://rss.marketingtools.apple.com".into(),
            musicbrainz_url: "https://musicbrainz.org/ws/2".into(),
            wikidata_url: "https://www.wikidata.org".into(),
            wikipedia_url: "https://{lang}.wikipedia.org".into(),
            musickit: None,
        }
    }
}

/// API «Яндекс Музыки» для переноса медиатеки (ключа у сервера нет: ключ входа даёт человек).
#[derive(Clone, Debug)]
pub struct YandexMusicConfig {
    /// Например `https://api.music.yandex.net`.
    pub url: String,
}

impl Default for YandexMusicConfig {
    fn default() -> Self {
        Self {
            url: DEFAULT_YANDEX_MUSIC_URL.into(),
        }
    }
}

/// Кто качает аудио. Задачу `acquire` выполняет тот, кто её взял: домашний загрузчик через
/// `/api/fetcher/*` (всегда) и воркер через серверный fetcher — только с `server` в `DALYWAVE_FETCHER_MODE`.
#[derive(Clone, Debug)]
pub struct FetcherConfig {
    /// Серверный fetcher, например `http://fetcher:8402`.
    pub url: String,
    /// Воркер сам качает через серверный fetcher. По умолчанию нет: YouTube не отдаёт аудио адресу
    /// дата-центра (docs/decisions.md), качает загрузчик на Mac.
    pub server: bool,
    /// Long-poll запроса задания.
    pub claim_wait: Duration,
    /// Аренда задания загрузчиком.
    pub lease: Duration,
}

impl Default for FetcherConfig {
    fn default() -> Self {
        Self {
            url: DEFAULT_FETCHER_URL.into(),
            server: false,
            claim_wait: CLAIM_WAIT,
            lease: LEASE,
        }
    }
}

/// `remote` (по умолчанию), `server` или оба через запятую: `remote,server`.
fn parse_fetcher_mode(value: Option<&str>) -> anyhow::Result<bool> {
    let mut server = false;
    for part in value
        .unwrap_or("remote")
        .split([',', '+', ' '])
        .map(str::trim)
        .filter(|p| !p.is_empty())
    {
        match part {
            "remote" => {}
            "server" => server = true,
            other => bail!("DALYWAVE_FETCHER_MODE: unknown mode `{other}` (remote, server)"),
        }
    }
    Ok(server)
}

/// S3-совместимое хранилище: SeaweedFS локально, Cloudflare R2 в проде — один и тот же код.
#[derive(Clone)]
pub struct S3Config {
    /// Адрес, по которому ходит сам сервер, например `http://localhost:8401` или `https://<account>.r2.cloudflarestorage.com`.
    pub endpoint: String,
    /// Адрес для подписанных ссылок, если клиентам он виден иначе (сервер ходит по внутреннему адресу docker).
    pub public_endpoint: Option<String>,
    /// Адрес для ссылок сервису анализа, если он видит хранилище иначе: локально ml в docker, а SeaweedFS —
    /// на `localhost` Mac (`http://host.docker.internal:8401`). Без него — тот же, что у клиентов.
    pub ml_endpoint: Option<String>,
    pub region: String,
    pub bucket: String,
    pub access_key_id: String,
    pub secret_access_key: String,
}

#[derive(Clone)]
pub struct DeezerConfig {
    pub url: String,
    /// Сколько запросов за 5 секунд позволено API.
    pub rate_api: u32,
    /// Сколько — воркеру.
    pub rate_worker: u32,
}

impl Default for DeezerConfig {
    fn default() -> Self {
        Self {
            url: DEFAULT_DEEZER_URL.into(),
            rate_api: DEFAULT_DEEZER_RATE_API,
            rate_worker: DEFAULT_DEEZER_RATE_WORKER,
        }
    }
}

/// Провайдер входа OpenID Connect. Провайдеров может быть несколько: daly.rest, Authentik, Keycloak,
/// Pocket ID, Google… Настраиваются переменными `DALYWAVE_OIDC_<ID>_*`; прежние `DALYWAVE_OIDC_ISSUER`,
/// `_CLIENT_ID`, `_CLIENT_SECRET` дают провайдера `dalyrest` со старым адресом возврата.
#[derive(Clone)]
pub struct OidcConfig {
    /// Имя в адресах `/api/auth/oidc/{id}/…`: строчные латинские буквы, цифры и `-`.
    pub id: String,
    pub issuer: String,
    pub client_id: String,
    /// `None` — публичный клиент: только PKCE, без секрета.
    pub client_secret: Option<String>,
    /// Подпись кнопки входа.
    pub title: String,
    /// Картинка на кнопке: адрес `https://…` или путь на этом сайте.
    pub icon: Option<String>,
    /// Запрашиваемые scope; `openid` есть всегда.
    pub scopes: String,
    /// Claim со списком ролей или групп: `roles`, `groups`, `realm_access.roles`.
    /// `None` — провайдер не выдаёт админов.
    pub admin_claim: Option<String>,
    /// Значения claim, которые делают человека администратором DalyWave.
    pub admin_values: Vec<String>,
    /// Пускать новых людей (без ролей, «ждём доступа»). Нет — только уже известных и по приглашению.
    pub auto_register: bool,
    /// Подтверждённой почте провайдера можно верить: по ней вход связывается с уже известным человеком.
    pub trust_email: bool,
    /// Как предъявлять секрет; `None` — по discovery (basic, если провайдер его умеет).
    pub client_auth: Option<ClientAuth>,
    /// Адрес возврата — прежний `/api/auth/oidc/callback` (так зарегистрирован клиент в daly.rest).
    pub legacy_callback: bool,
}

/// Способ предъявить секрет клиента в token endpoint (RFC 6749 2.3.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClientAuth {
    Basic,
    Post,
}

pub const DEFAULT_SCOPES: &str = "openid email profile";
/// Провайдер из прежних переменных `DALYWAVE_OIDC_ISSUER`…
pub const LEGACY_PROVIDER_ID: &str = "dalyrest";

impl OidcConfig {
    /// Провайдер с настройками по умолчанию: claim `roles`, значение `admin`, почте не верим.
    pub fn new(id: &str, issuer: &str, client_id: &str, client_secret: Option<&str>) -> Self {
        Self {
            id: id.into(),
            issuer: issuer.trim_end_matches('/').into(),
            client_id: client_id.into(),
            client_secret: client_secret.map(str::to_string),
            title: id.into(),
            icon: None,
            scopes: DEFAULT_SCOPES.into(),
            admin_claim: Some("roles".into()),
            admin_values: vec!["admin".into()],
            auto_register: true,
            trust_email: false,
            client_auth: None,
            legacy_callback: false,
        }
    }
}

fn flag(name: &str, value: Option<String>, default: bool) -> anyhow::Result<bool> {
    match value.map(|v| v.to_ascii_lowercase()).as_deref() {
        None => Ok(default),
        Some("1" | "true" | "yes" | "on") => Ok(true),
        Some("0" | "false" | "no" | "off") => Ok(false),
        Some(other) => bail!("DALYWAVE_{name}: `{other}` is neither true nor false"),
    }
}

/// Провайдеры из переменных. `keys` — имена всех переменных окружения: по ним находятся
/// `DALYWAVE_OIDC_<ID>_ISSUER`.
fn parse_providers(
    keys: &[String],
    var: &dyn Fn(&str) -> Option<String>,
) -> anyhow::Result<Vec<OidcConfig>> {
    let mut out: Vec<OidcConfig> = Vec::new();
    match (
        var("OIDC_ISSUER"),
        var("OIDC_CLIENT_ID"),
        var("OIDC_CLIENT_SECRET"),
    ) {
        (Some(issuer), Some(client_id), Some(secret)) => {
            let mut p = OidcConfig::new(LEGACY_PROVIDER_ID, &issuer, &client_id, Some(&secret));
            p.title = var("OIDC_TITLE").unwrap_or_else(|| "daly.rest".into());
            p.icon = var("OIDC_ICON");
            let roles = list(var("OIDC_ADMIN_ROLES"));
            if !roles.is_empty() {
                p.admin_values = roles;
            }
            // daly.rest подтверждает почту сам: так было и до нескольких провайдеров
            p.trust_email = true;
            p.legacy_callback = true;
            out.push(p);
        }
        (None, None, None) => {}
        _ => bail!(
            "DALYWAVE_OIDC_ISSUER, DALYWAVE_OIDC_CLIENT_ID and DALYWAVE_OIDC_CLIENT_SECRET go together"
        ),
    }

    let mut ids: Vec<String> = keys
        .iter()
        .filter_map(|k| k.strip_prefix("DALYWAVE_OIDC_")?.strip_suffix("_ISSUER"))
        .filter(|id| !id.is_empty())
        .map(str::to_string)
        .collect();
    ids.sort();
    ids.dedup();
    for env_id in ids {
        let v = |field: &str| var(&format!("OIDC_{env_id}_{field}"));
        let id = env_id.to_ascii_lowercase().replace('_', "-");
        if !id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        {
            bail!("DALYWAVE_OIDC_{env_id}_ISSUER: provider id must be latin letters, digits and _");
        }
        let (Some(issuer), Some(client_id)) = (v("ISSUER"), v("CLIENT_ID")) else {
            bail!("DALYWAVE_OIDC_{env_id}_ISSUER and DALYWAVE_OIDC_{env_id}_CLIENT_ID go together");
        };
        let parsed = url::Url::parse(&issuer)
            .with_context(|| format!("DALYWAVE_OIDC_{env_id}_ISSUER is not a valid URL"))?;
        if !matches!(parsed.scheme(), "http" | "https") {
            bail!("DALYWAVE_OIDC_{env_id}_ISSUER must be an http(s) address");
        }
        let mut p = OidcConfig::new(&id, &issuer, &client_id, v("CLIENT_SECRET").as_deref());
        // Без имени — из id: POCKET_ID → «Pocket Id»
        p.title = v("NAME").unwrap_or_else(|| {
            env_id
                .split('_')
                .filter(|w| !w.is_empty())
                .map(|w| {
                    let lower = w.to_ascii_lowercase();
                    let mut chars = lower.chars();
                    chars.next().map_or_else(String::new, |c| {
                        c.to_ascii_uppercase().to_string() + chars.as_str()
                    })
                })
                .collect::<Vec<_>>()
                .join(" ")
        });
        p.icon = v("ICON");
        if let Some(scopes) = v("SCOPES") {
            let mut parts: Vec<&str> = scopes.split([' ', ',']).filter(|s| !s.is_empty()).collect();
            if !parts.contains(&"openid") {
                parts.insert(0, "openid");
            }
            p.scopes = parts.join(" ");
        }
        match v("ADMIN_CLAIM").as_deref() {
            Some("none" | "off" | "-") => p.admin_claim = None,
            Some(claim) => p.admin_claim = Some(claim.to_string()),
            None => {}
        }
        let values = list(v("ADMIN_VALUE").or_else(|| v("ADMIN_VALUES")));
        if !values.is_empty() {
            p.admin_values = values;
        }
        let name = |field: &str| format!("OIDC_{env_id}_{field}");
        p.auto_register = flag(&name("AUTO_REGISTER"), v("AUTO_REGISTER"), true)?;
        p.trust_email = flag(&name("TRUST_EMAIL"), v("TRUST_EMAIL"), false)?;
        p.client_auth = match v("AUTH_METHOD").as_deref() {
            None => None,
            Some("client_secret_basic" | "basic") => Some(ClientAuth::Basic),
            Some("client_secret_post" | "post") => Some(ClientAuth::Post),
            Some(other) => bail!(
                "DALYWAVE_OIDC_{env_id}_AUTH_METHOD: `{other}` (client_secret_basic, client_secret_post)"
            ),
        };
        if out.iter().any(|o| o.id == p.id) {
            bail!(
                "DALYWAVE_OIDC_{env_id}_*: provider `{}` is configured twice",
                p.id
            );
        }
        out.push(p);
    }

    // Порядок кнопок: DALYWAVE_OIDC_PROVIDERS=authentik,google; не названные — после, как были
    let order = list(var("OIDC_PROVIDERS"));
    if !order.is_empty() {
        let pos = |id: &str| {
            order
                .iter()
                .position(|o| o.eq_ignore_ascii_case(id))
                .unwrap_or(usize::MAX)
        };
        out.sort_by_key(|p| pos(&p.id));
    }
    Ok(out)
}

fn list(value: Option<String>) -> Vec<String> {
    value
        .map(|v| {
            v.split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        let keys: Vec<String> = env::vars_os()
            .filter_map(|(k, _)| k.into_string().ok())
            .collect();
        Self::from_lookup(&keys, |name| env::var(name).ok())
    }

    /// Разбор настроек из любого источника: переменные окружения или словарь в тестах.
    /// `keys` — имена всех переменных: по ним находятся провайдеры `DALYWAVE_OIDC_<ID>_*`.
    pub fn from_lookup(
        keys: &[String],
        lookup: impl Fn(&str) -> Option<String>,
    ) -> anyhow::Result<Self> {
        let var = |name: &str| {
            lookup(&format!("DALYWAVE_{name}"))
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty())
        };
        let public_url = var("PUBLIC_URL").unwrap_or_else(|| DEFAULT_PUBLIC_URL.into());
        let parsed =
            url::Url::parse(&public_url).context("DALYWAVE_PUBLIC_URL is not a valid URL")?;
        if !matches!(parsed.scheme(), "http" | "https") || parsed.host_str().is_none() {
            bail!("DALYWAVE_PUBLIC_URL must be an http(s) address, e.g. https://wave.daly.rest");
        }
        let public_url = public_url.trim_end_matches('/').to_string();

        let secret_key = match var("SECRET_KEY") {
            Some(hex) => parse_secret(&hex)?,
            // Локально можно без ключа: cookie просто не переживут перезапуск
            None if public_url.starts_with("http://") => {
                tracing::warn!(
                    "DALYWAVE_SECRET_KEY is not set: using a random key, sign-in state resets on restart"
                );
                crate::crypto::random_bytes::<32>()?
            }
            None => {
                bail!("DALYWAVE_SECRET_KEY is required: 64 hex characters (openssl rand -hex 32)")
            }
        };

        let oidc = parse_providers(keys, &var)?;
        let local_auth = flag("LOCAL_AUTH", var("LOCAL_AUTH"), true)?;

        let admin_password = var("ADMIN_PASSWORD").filter(|p| {
            let ok = p.chars().count() >= MIN_ADMIN_PASSWORD_LEN;
            if !ok {
                tracing::warn!("DALYWAVE_ADMIN_PASSWORD is shorter than {MIN_ADMIN_PASSWORD_LEN} characters: password sign-in is off");
            }
            ok
        });

        let demo_account = var("DEMO_ACCOUNT").map(|name| name.to_lowercase());

        let s3 = match (
            var("S3_ENDPOINT"),
            var("S3_ACCESS_KEY_ID"),
            var("S3_SECRET_ACCESS_KEY"),
        ) {
            (Some(endpoint), Some(access_key_id), Some(secret_access_key)) => {
                url::Url::parse(&endpoint).context("DALYWAVE_S3_ENDPOINT is not a valid URL")?;
                let public_endpoint = var("S3_PUBLIC_ENDPOINT");
                if let Some(public) = &public_endpoint {
                    url::Url::parse(public)
                        .context("DALYWAVE_S3_PUBLIC_ENDPOINT is not a valid URL")?;
                }
                let ml_endpoint = var("S3_ML_ENDPOINT");
                if let Some(ml) = &ml_endpoint {
                    url::Url::parse(ml).context("DALYWAVE_S3_ML_ENDPOINT is not a valid URL")?;
                }
                Some(S3Config {
                    endpoint: endpoint.trim_end_matches('/').to_string(),
                    public_endpoint: public_endpoint.map(|p| p.trim_end_matches('/').to_string()),
                    ml_endpoint: ml_endpoint.map(|p| p.trim_end_matches('/').to_string()),
                    region: var("S3_REGION").unwrap_or_else(|| "auto".into()),
                    bucket: var("S3_BUCKET").unwrap_or_else(|| "dalywave".into()),
                    access_key_id,
                    secret_access_key,
                })
            }
            (None, None, None) => None,
            _ => bail!(
                "DALYWAVE_S3_ENDPOINT, DALYWAVE_S3_ACCESS_KEY_ID and DALYWAVE_S3_SECRET_ACCESS_KEY go together"
            ),
        };

        let storage_kind = match var("STORAGE").map(|v| v.to_ascii_lowercase()).as_deref() {
            Some("s3") if s3.is_none() => {
                bail!("DALYWAVE_STORAGE=s3 needs DALYWAVE_S3_ENDPOINT and both S3 keys")
            }
            Some("s3") => StorageKind::S3,
            Some("fs") => StorageKind::Fs,
            Some("memory") => StorageKind::Memory,
            Some(other) => bail!("DALYWAVE_STORAGE: unknown storage `{other}` (s3, fs)"),
            None if s3.is_some() => StorageKind::S3,
            None if var("STORAGE_PATH").is_some() => StorageKind::Fs,
            None => StorageKind::Memory,
        };
        let storage_ml_url = var("STORAGE_ML_URL");
        if let Some(url) = &storage_ml_url {
            url::Url::parse(url).context("DALYWAVE_STORAGE_ML_URL is not a valid URL")?;
        }
        let storage = StorageConfig {
            kind: storage_kind,
            path: PathBuf::from(var("STORAGE_PATH").unwrap_or_else(|| DEFAULT_STORAGE_PATH.into())),
            ml_url: storage_ml_url.map(|u| u.trim_end_matches('/').to_string()),
        };
        if storage.kind == StorageKind::Fs && !storage.path.is_absolute() {
            bail!("DALYWAVE_STORAGE_PATH must be an absolute path, e.g. /data");
        }
        let library = LibraryConfig {
            dirs: match var("MUSIC_DIRS") {
                Some(v) => {
                    let dirs: Vec<PathBuf> = env::split_paths(&v)
                        .filter(|p| !p.as_os_str().is_empty())
                        .collect();
                    if let Some(bad) = dirs.iter().find(|d| !d.is_absolute()) {
                        bail!(
                            "DALYWAVE_MUSIC_DIRS: {} is not an absolute path",
                            bad.display()
                        );
                    }
                    dirs
                }
                None => Vec::new(),
            },
        };
        let features = Features {
            deezer: flag("DEEZER", var("DEEZER"), false)?,
            downloader: flag("DOWNLOADER", var("DOWNLOADER"), false)?,
        };

        let identity = IdentityConfig {
            name: var("SERVER_NAME")
                .map(|n| n.chars().take(63).collect())
                .unwrap_or_else(|| DEFAULT_SERVER_NAME.into()),
            mdns: flag("MDNS", var("MDNS"), true)?,
        };

        let rate = |name: &str, default: u32| -> anyhow::Result<u32> {
            match var(name) {
                None => Ok(default),
                Some(v) => match v.parse::<u32>() {
                    Ok(n) if (1..=DEEZER_WINDOW_MAX).contains(&n) => Ok(n),
                    _ => bail!(
                        "DALYWAVE_{name} must be 1..={DEEZER_WINDOW_MAX} requests per 5 seconds"
                    ),
                },
            }
        };
        let deezer = DeezerConfig {
            url: var("DEEZER_URL")
                .unwrap_or_else(|| DEFAULT_DEEZER_URL.into())
                .trim_end_matches('/')
                .to_string(),
            rate_api: rate("DEEZER_RATE_API", DEFAULT_DEEZER_RATE_API)?,
            rate_worker: rate("DEEZER_RATE_WORKER", DEFAULT_DEEZER_RATE_WORKER)?,
        };

        let fetcher_url = var("FETCHER_URL").unwrap_or_else(|| DEFAULT_FETCHER_URL.into());
        url::Url::parse(&fetcher_url).context("DALYWAVE_FETCHER_URL is not a valid URL")?;
        let fetcher = FetcherConfig {
            url: fetcher_url.trim_end_matches('/').to_string(),
            // Серверный fetcher качает только при включённом загрузчике
            server: parse_fetcher_mode(var("FETCHER_MODE").as_deref())? && features.downloader,
            ..Default::default()
        };
        let ml_url = var("ML_URL").unwrap_or_else(|| DEFAULT_ML_URL.into());
        url::Url::parse(&ml_url).context("DALYWAVE_ML_URL is not a valid URL")?;
        let ml = MlConfig {
            url: ml_url.trim_end_matches('/').to_string(),
        };
        let lrclib_url = var("LRCLIB_URL").unwrap_or_else(|| DEFAULT_LRCLIB_URL.into());
        url::Url::parse(&lrclib_url).context("DALYWAVE_LRCLIB_URL is not a valid URL")?;
        let lrclib = LrclibConfig {
            url: lrclib_url.trim_end_matches('/').to_string(),
        };
        let yandex_url = var("YANDEX_MUSIC_URL").unwrap_or_else(|| DEFAULT_YANDEX_MUSIC_URL.into());
        url::Url::parse(&yandex_url).context("DALYWAVE_YANDEX_MUSIC_URL is not a valid URL")?;
        let yandex_music = YandexMusicConfig {
            url: yandex_url.trim_end_matches('/').to_string(),
        };
        let release_token = var("RELEASE_TOKEN").filter(|t| {
            let ok = t.chars().count() >= MIN_RELEASE_TOKEN_LEN;
            if !ok {
                tracing::warn!("DALYWAVE_RELEASE_TOKEN is shorter than {MIN_RELEASE_TOKEN_LEN} characters: app releases are off");
            }
            ok
        });

        let apns = match (
            var("APNS_KEY_PATH"),
            var("APNS_KEY_ID"),
            var("APNS_TEAM_ID"),
        ) {
            (Some(key_path), Some(key_id), Some(team_id)) => Some(ApnsConfig {
                key_path: PathBuf::from(key_path),
                key_id,
                team_id,
                topic: var("APNS_TOPIC").unwrap_or_else(|| DEFAULT_APNS_TOPIC.into()),
                production_url: APNS_PRODUCTION_URL.into(),
                sandbox_url: APNS_SANDBOX_URL.into(),
            }),
            (None, None, None) => None,
            // Ключ ещё не положили на сервер: остальное работает, push на iPhone выключен
            _ => {
                tracing::warn!(
                    "DALYWAVE_APNS_KEY_PATH, DALYWAVE_APNS_KEY_ID and DALYWAVE_APNS_TEAM_ID go together: APNs is off"
                );
                None
            }
        };
        let push = PushConfig {
            apns,
            vapid: parse_vapid(
                var("VAPID_PUBLIC"),
                var("VAPID_PRIVATE"),
                var("VAPID_SUBJECT"),
                &public_url,
            )?,
        };

        Ok(Self {
            database_url: var("DATABASE_URL").unwrap_or_else(|| DEFAULT_DATABASE_URL.into()),
            listen: var("LISTEN")
                .unwrap_or_else(|| DEFAULT_LISTEN.into())
                .parse()
                .context("DALYWAVE_LISTEN must look like 0.0.0.0:8400")?,
            public_url,
            secret_key,
            static_dir: var("STATIC_DIR").map(PathBuf::from),
            oidc,
            local_auth,
            admin_emails: list(var("ADMIN_EMAILS"))
                .into_iter()
                .map(|e| e.to_lowercase())
                .collect(),
            admin_password,
            demo_account,
            s3,
            storage,
            library,
            features,
            identity,
            deezer,
            fetcher,
            ml,
            lrclib,
            yandex_music,
            info: InfoConfig {
                musickit: parse_musickit(&var),
                ..InfoConfig::default()
            },
            release_token,
            push,
        })
    }

    /// Общий аккаунт демо: `username` — логин из базы.
    pub fn is_demo(&self, username: Option<&str>) -> bool {
        matches!((self.demo_account.as_deref(), username), (Some(demo), Some(name)) if demo == name)
    }

    /// Cookie с флагом Secure браузер не вернёт по http, поэтому флаг ставится только для https.
    pub fn cookie_secure(&self) -> bool {
        self.public_url.starts_with("https://")
    }

    /// Origin сайта: схема, хост и порт.
    pub fn public_origin(&self) -> String {
        url::Url::parse(&self.public_url)
            .map(|u| u.origin().ascii_serialization())
            .unwrap_or_else(|_| self.public_url.clone())
    }

    /// Адрес возврата провайдера: `/api/auth/oidc/{id}/callback`; у прежнего daly.rest —
    /// `/api/auth/oidc/callback`, он так зарегистрирован.
    pub fn oidc_redirect_uri(&self, provider: &OidcConfig) -> String {
        if provider.legacy_callback {
            format!("{}/api/auth/oidc/callback", self.public_url)
        } else {
            format!("{}/api/auth/oidc/{}/callback", self.public_url, provider.id)
        }
    }

    /// Абсолютный адрес страницы интерфейса по пути вида `/library?tab=albums`.
    pub fn page(&self, path: &str) -> String {
        url::Url::parse(&self.public_url)
            .and_then(|base| base.join(path))
            .map(String::from)
            .unwrap_or_else(|_| format!("{}/", self.public_url))
    }

    /// Страница входа с причиной ошибки: `cancelled`, `expired`, `state`, `disabled`, `provider`.
    pub fn login_error(&self, code: &str, next: &str) -> String {
        let mut url = self.page("/login");
        url.push_str("?error=");
        url.push_str(code);
        if next != "/" {
            url.push_str("&next=");
            url.extend(url::form_urlencoded::byte_serialize(next.as_bytes()));
        }
        url
    }
}

/// Ключ MusicKit: путь к .p8, Key ID и Team ID вместе; без них живые обложки выключены.
fn parse_musickit(var: &dyn Fn(&str) -> Option<String>) -> Option<MusicKitConfig> {
    match (
        var("MUSICKIT_KEY_PATH"),
        var("MUSICKIT_KEY_ID"),
        var("MUSICKIT_TEAM_ID"),
    ) {
        (Some(key_path), Some(key_id), Some(team_id)) => Some(MusicKitConfig {
            key_path: PathBuf::from(key_path),
            key_id,
            team_id,
            storefront: var("MUSICKIT_STOREFRONT")
                .map(|s| s.to_ascii_lowercase())
                .filter(|s| s.len() == 2 && s.chars().all(|c| c.is_ascii_alphabetic()))
                .unwrap_or_else(|| "us".into()),
            url: var("MUSICKIT_URL")
                .unwrap_or_else(|| "https://api.music.apple.com".into())
                .trim_end_matches('/')
                .to_string(),
        }),
        (None, None, None) => None,
        _ => {
            tracing::warn!(
                "DALYWAVE_MUSICKIT_KEY_PATH, DALYWAVE_MUSICKIT_KEY_ID and DALYWAVE_MUSICKIT_TEAM_ID go together: animated covers are off"
            );
            None
        }
    }
}

fn parse_secret(hex: &str) -> anyhow::Result<[u8; 32]> {
    let bytes = hex::decode(hex).context("DALYWAVE_SECRET_KEY must be hex")?;
    bytes
        .try_into()
        .map_err(|_| anyhow::anyhow!("DALYWAVE_SECRET_KEY must be 64 hex characters (32 bytes)"))
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    fn parse(pairs: &[(&str, &str)]) -> anyhow::Result<Config> {
        let map: HashMap<String, String> = pairs
            .iter()
            .map(|(k, v)| (format!("DALYWAVE_{k}"), v.to_string()))
            .collect();
        let keys: Vec<String> = map.keys().cloned().collect();
        Config::from_lookup(&keys, |name| map.get(name).cloned())
    }

    #[test]
    fn local_defaults() {
        let c = parse(&[]).unwrap();
        assert_eq!(c.public_url, DEFAULT_PUBLIC_URL);
        assert_eq!(c.listen, DEFAULT_LISTEN.parse().unwrap());
        assert_eq!(c.database_url, DEFAULT_DATABASE_URL);
        assert!(c.oidc.is_empty() && c.admin_password.is_none() && c.static_dir.is_none());
        assert!(c.local_auth, "свои аккаунты по умолчанию включены");
        assert!(!c.cookie_secure());
    }

    #[test]
    fn production_needs_a_secret() {
        assert!(parse(&[("PUBLIC_URL", "https://wave.daly.rest")]).is_err());
        let key = "ab".repeat(32);
        let c = parse(&[
            ("PUBLIC_URL", "https://wave.daly.rest/"),
            ("SECRET_KEY", &key),
        ])
        .unwrap();
        assert_eq!(c.public_url, "https://wave.daly.rest");
        assert!(c.cookie_secure());
        assert_eq!(c.public_origin(), "https://wave.daly.rest");
        assert_eq!(c.secret_key, [0xab; 32]);
        assert!(parse(&[("PUBLIC_URL", "ftp://wave.daly.rest")]).is_err());
    }

    #[test]
    fn demo_account_is_off_by_default() {
        let c = parse(&[]).unwrap();
        assert!(c.demo_account.is_none() && !c.is_demo(Some("demo")));
        let c = parse(&[("DEMO_ACCOUNT", " Demo ")]).unwrap();
        assert_eq!(c.demo_account.as_deref(), Some("demo"));
        assert!(c.is_demo(Some("demo")) && !c.is_demo(Some("owner")) && !c.is_demo(None));
    }

    #[test]
    fn short_password_is_off() {
        assert!(
            parse(&[("ADMIN_PASSWORD", "too short")])
                .unwrap()
                .admin_password
                .is_none()
        );
        let long = "x".repeat(MIN_ADMIN_PASSWORD_LEN);
        assert_eq!(
            parse(&[("ADMIN_PASSWORD", &long)]).unwrap().admin_password,
            Some(long)
        );
    }

    #[test]
    fn oidc_goes_together() {
        assert!(parse(&[("OIDC_ISSUER", "https://daly.rest")]).is_err());
        let c = parse(&[
            ("OIDC_ISSUER", "https://daly.rest/"),
            ("OIDC_CLIENT_ID", "dalywave-37a20828"),
            ("OIDC_CLIENT_SECRET", "s"),
            ("ADMIN_EMAILS", "Owner@Example.com, other@example.com"),
        ])
        .unwrap();
        // Прежние переменные — провайдер dalyrest со старым адресом возврата
        let [oidc] = &c.oidc[..] else {
            panic!("один провайдер")
        };
        assert_eq!(oidc.id, "dalyrest");
        assert_eq!(oidc.issuer, "https://daly.rest");
        assert_eq!(oidc.title, "daly.rest");
        assert_eq!(oidc.admin_claim.as_deref(), Some("roles"));
        assert_eq!(oidc.admin_values, vec!["admin"]);
        assert!(oidc.trust_email && oidc.auto_register && oidc.legacy_callback);
        assert_eq!(
            c.oidc_redirect_uri(oidc),
            "http://localhost:3400/api/auth/oidc/callback"
        );
        assert_eq!(
            c.admin_emails,
            vec!["owner@example.com", "other@example.com"]
        );
    }

    #[test]
    fn several_providers() {
        let c = parse(&[
            ("OIDC_ISSUER", "https://daly.rest"),
            ("OIDC_CLIENT_ID", "dw"),
            ("OIDC_CLIENT_SECRET", "s"),
            ("OIDC_POCKET_ID_ISSUER", "https://id.example.com/"),
            ("OIDC_POCKET_ID_CLIENT_ID", "pocket"),
            (
                "OIDC_AUTHENTIK_ISSUER",
                "https://auth.example.com/application/o/dalywave/",
            ),
            ("OIDC_AUTHENTIK_CLIENT_ID", "ak"),
            ("OIDC_AUTHENTIK_CLIENT_SECRET", "ak-secret"),
            ("OIDC_AUTHENTIK_NAME", "Authentik"),
            ("OIDC_AUTHENTIK_ICON", "https://auth.example.com/icon.svg"),
            ("OIDC_AUTHENTIK_ADMIN_CLAIM", "groups"),
            (
                "OIDC_AUTHENTIK_ADMIN_VALUE",
                "authentik Admins, wave-admins",
            ),
            ("OIDC_AUTHENTIK_AUTO_REGISTER", "false"),
            ("OIDC_AUTHENTIK_TRUST_EMAIL", "yes"),
            ("OIDC_AUTHENTIK_AUTH_METHOD", "client_secret_post"),
            ("OIDC_AUTHENTIK_SCOPES", "email profile groups"),
            ("LOCAL_AUTH", "false"),
        ])
        .unwrap();
        assert!(!c.local_auth);
        let ids: Vec<&str> = c.oidc.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(ids, ["dalyrest", "authentik", "pocket-id"]);
        let ak = &c.oidc[1];
        assert_eq!(ak.issuer, "https://auth.example.com/application/o/dalywave");
        assert_eq!(ak.title, "Authentik");
        assert_eq!(ak.admin_claim.as_deref(), Some("groups"));
        assert_eq!(ak.admin_values, vec!["authentik Admins", "wave-admins"]);
        assert!(!ak.auto_register && ak.trust_email && !ak.legacy_callback);
        assert_eq!(ak.client_auth, Some(ClientAuth::Post));
        assert_eq!(ak.scopes, "openid email profile groups");
        assert_eq!(
            c.oidc_redirect_uri(ak),
            "http://localhost:3400/api/auth/oidc/authentik/callback"
        );
        let pocket = &c.oidc[2];
        assert!(pocket.client_secret.is_none(), "публичный клиент");
        assert!(
            !pocket.trust_email,
            "почте нового провайдера по умолчанию не верим"
        );
        assert_eq!(pocket.title, "Pocket Id");

        // Порядок кнопок задаётся явно
        let c = parse(&[
            ("OIDC_A_ISSUER", "https://a.example.com"),
            ("OIDC_A_CLIENT_ID", "a"),
            ("OIDC_B_ISSUER", "https://b.example.com"),
            ("OIDC_B_CLIENT_ID", "b"),
            ("OIDC_PROVIDERS", "b,a"),
        ])
        .unwrap();
        assert_eq!(c.oidc[0].id, "b");

        assert!(parse(&[("OIDC_X_ISSUER", "https://x.example.com")]).is_err());
        assert!(parse(&[("OIDC_X_ISSUER", "ftp://x"), ("OIDC_X_CLIENT_ID", "x")]).is_err());
        assert!(
            parse(&[
                ("OIDC_X_ISSUER", "https://x.example.com"),
                ("OIDC_X_CLIENT_ID", "x"),
                ("OIDC_X_AUTO_REGISTER", "maybe"),
            ])
            .is_err()
        );
        assert!(
            parse(&[
                ("OIDC_ISSUER", "https://daly.rest"),
                ("OIDC_CLIENT_ID", "dw"),
                ("OIDC_CLIENT_SECRET", "s"),
                ("OIDC_DALYREST_ISSUER", "https://daly.rest"),
                ("OIDC_DALYREST_CLIENT_ID", "dw"),
            ])
            .is_err(),
            "один id дважды"
        );
    }

    #[test]
    fn pages_stay_on_site() {
        let c = parse(&[]).unwrap();
        assert_eq!(
            c.page("/library?tab=albums"),
            "http://localhost:3400/library?tab=albums"
        );
        assert_eq!(
            c.page("/поиск"),
            "http://localhost:3400/%D0%BF%D0%BE%D0%B8%D1%81%D0%BA"
        );
        assert_eq!(
            c.login_error("state", "/"),
            "http://localhost:3400/login?error=state"
        );
        assert_eq!(
            c.login_error("expired", "/a?b=1"),
            "http://localhost:3400/login?error=expired&next=%2Fa%3Fb%3D1"
        );
    }

    #[test]
    fn storage_and_deezer() {
        let c = parse(&[]).unwrap();
        assert!(c.s3.is_none());
        assert_eq!(c.deezer.url, DEFAULT_DEEZER_URL);
        assert!(c.deezer.rate_api + c.deezer.rate_worker <= 45);
        assert!(parse(&[("S3_ENDPOINT", "http://localhost:8401")]).is_err());
        let c = parse(&[
            ("S3_ENDPOINT", "http://localhost:8401/"),
            ("S3_ACCESS_KEY_ID", "k"),
            ("S3_SECRET_ACCESS_KEY", "s"),
            ("S3_PUBLIC_ENDPOINT", "https://files.example.com"),
            ("DEEZER_RATE_WORKER", "10"),
        ])
        .unwrap();
        let s3 = c.s3.unwrap();
        assert_eq!(s3.endpoint, "http://localhost:8401");
        assert_eq!(s3.bucket, "dalywave");
        assert_eq!(s3.region, "auto");
        assert_eq!(
            s3.public_endpoint.as_deref(),
            Some("https://files.example.com")
        );
        assert_eq!(c.deezer.rate_worker, 10);
        assert!(parse(&[("DEEZER_RATE_API", "100")]).is_err());
    }

    #[test]
    fn server_name_and_mdns() {
        let c = parse(&[]).unwrap();
        assert_eq!(c.identity.name, "DalyWave");
        assert!(
            c.identity.mdns,
            "в локальной сети сервер объявляет себя по умолчанию"
        );
        let c = parse(&[("SERVER_NAME", " Дом "), ("MDNS", "false")]).unwrap();
        assert_eq!(c.identity.name, "Дом");
        assert!(!c.identity.mdns);
        assert!(parse(&[("MDNS", "maybe")]).is_err());
    }

    #[test]
    fn storage_kinds_and_features() {
        let c = parse(&[]).unwrap();
        assert_eq!(c.storage.kind, StorageKind::Memory);
        assert!(c.library.dirs.is_empty());
        assert_eq!(
            c.features,
            Features::default(),
            "новая установка — без Deezer и загрузчика"
        );
        let s3 = [
            ("S3_ENDPOINT", "http://localhost:8401"),
            ("S3_ACCESS_KEY_ID", "k"),
            ("S3_SECRET_ACCESS_KEY", "s"),
        ];
        assert_eq!(parse(&s3).unwrap().storage.kind, StorageKind::S3);
        assert!(parse(&[("STORAGE", "s3")]).is_err());
        let c = parse(&[
            ("STORAGE", "fs"),
            ("STORAGE_PATH", "/srv/dalywave"),
            ("MUSIC_DIRS", "/music:/mnt/Музыка"),
            ("DEEZER", "true"),
            ("DOWNLOADER", "0"),
            ("STORAGE_ML_URL", "http://api:8400/"),
        ])
        .unwrap();
        assert_eq!(c.storage.kind, StorageKind::Fs);
        assert_eq!(c.storage.path, PathBuf::from("/srv/dalywave"));
        assert_eq!(c.storage.ml_url.as_deref(), Some("http://api:8400"));
        assert_eq!(
            c.library.dirs,
            vec![PathBuf::from("/music"), PathBuf::from("/mnt/Музыка")]
        );
        assert!(c.features.deezer && !c.features.downloader);
        assert_eq!(
            parse(&[("STORAGE", "fs")]).unwrap().storage.path,
            PathBuf::from("/data")
        );
        assert_eq!(
            parse(&[("STORAGE_PATH", "/data")]).unwrap().storage.kind,
            StorageKind::Fs
        );
        assert!(parse(&[("STORAGE", "fs"), ("STORAGE_PATH", "data")]).is_err());
        assert!(parse(&[("MUSIC_DIRS", "music")]).is_err());
        assert!(parse(&[("DEEZER", "maybe")]).is_err());
        assert!(parse(&[("STORAGE", "ftp")]).is_err());
    }

    #[test]
    fn fetcher_mode() {
        let c = parse(&[]).unwrap();
        assert!(!c.fetcher.server, "по умолчанию качает загрузчик на Mac");
        assert_eq!(c.fetcher.url, DEFAULT_FETCHER_URL);
        assert!(c.fetcher.claim_wait < Duration::from_secs(30));
        let c = parse(&[
            ("FETCHER_MODE", "remote,server"),
            ("FETCHER_URL", "http://fetcher:8402/"),
            ("DOWNLOADER", "true"),
        ])
        .unwrap();
        assert!(c.fetcher.server);
        assert_eq!(c.fetcher.url, "http://fetcher:8402");
        assert!(
            parse(&[("FETCHER_MODE", "server"), ("DOWNLOADER", "true")])
                .unwrap()
                .fetcher
                .server
        );
        // Без загрузчика серверный fetcher не качает
        assert!(!parse(&[("FETCHER_MODE", "server")]).unwrap().fetcher.server);
        assert!(!parse(&[("FETCHER_MODE", "remote")]).unwrap().fetcher.server);
        assert!(parse(&[("FETCHER_MODE", "cloud")]).is_err());
        assert!(parse(&[("FETCHER_URL", "not a url")]).is_err());
    }

    #[test]
    fn ml_service() {
        let c = parse(&[]).unwrap();
        assert_eq!(c.ml.url, DEFAULT_ML_URL);
        let c = parse(&[
            ("ML_URL", "http://ml:8403/"),
            ("S3_ENDPOINT", "http://localhost:8401"),
            ("S3_ACCESS_KEY_ID", "k"),
            ("S3_SECRET_ACCESS_KEY", "s"),
            ("S3_ML_ENDPOINT", "http://host.docker.internal:8401/"),
        ])
        .unwrap();
        assert_eq!(c.ml.url, "http://ml:8403");
        assert_eq!(
            c.s3.unwrap().ml_endpoint.as_deref(),
            Some("http://host.docker.internal:8401")
        );
        assert!(parse(&[("ML_URL", "ml")]).is_err());
    }

    #[test]
    fn secret_must_be_32_bytes() {
        assert!(parse_secret(&"ab".repeat(32)).is_ok());
        assert!(parse_secret(&"ab".repeat(16)).is_err());
        assert!(parse_secret("not hex").is_err());
    }

    #[test]
    fn push_channels() {
        let c = parse(&[]).unwrap();
        assert!(c.push.apns.is_none() && c.push.vapid.is_none());
        // Часть настроек APNs — канал выключен, сервер работает
        assert!(
            parse(&[("APNS_KEY_ID", "ABC123DEFG")])
                .unwrap()
                .push
                .apns
                .is_none()
        );
        let c = parse(&[
            ("APNS_KEY_PATH", "/secrets/AuthKey_ABC123DEFG.p8"),
            ("APNS_KEY_ID", "ABC123DEFG"),
            ("APNS_TEAM_ID", "Y5KR5JPCRG"),
        ])
        .unwrap();
        let apns = c.push.apns.unwrap();
        assert_eq!(apns.topic, DEFAULT_APNS_TOPIC);
        assert_eq!(apns.production_url, APNS_PRODUCTION_URL);

        let (public, private) = crate::notify::webpush::generate_keys().unwrap();
        let c = parse(&[("VAPID_PRIVATE", &private)]).unwrap();
        let vapid = c.push.vapid.unwrap();
        assert_eq!(vapid.public_key, public);
        assert_eq!(vapid.subject, DEFAULT_PUBLIC_URL);
        assert!(
            parse(&[
                ("VAPID_PRIVATE", &private),
                ("VAPID_PUBLIC", &public),
                ("VAPID_SUBJECT", "mailto:owner@example.com"),
            ])
            .is_ok()
        );
        let (other, _) = crate::notify::webpush::generate_keys().unwrap();
        assert!(parse(&[("VAPID_PRIVATE", &private), ("VAPID_PUBLIC", &other)]).is_err());
        assert!(parse(&[("VAPID_PRIVATE", "short")]).is_err());
        assert!(
            parse(&[("VAPID_PUBLIC", &public)])
                .unwrap()
                .push
                .vapid
                .is_none()
        );
    }

    #[test]
    fn lists_are_trimmed() {
        assert_eq!(
            list(Some(" a@x.ru , ,b@y.ru".into())),
            vec!["a@x.ru", "b@y.ru"]
        );
        assert!(list(None).is_empty());
    }
}
