//! «Система» админки: здоровье сервера, резервные копии базы, выпуски приложений и настройки из
//! переменных окружения (только чтение). Права admin — слоем в `routes::router`.

use std::{
    io::{BufRead, BufReader, Read},
    path::{Path as FsPath, PathBuf},
    time::{Duration, Instant},
};

use axum::{
    Router,
    extract::State,
    routing::{get, post},
};
use chrono::{DateTime, Utc};
use dalywave_protocol::{
    ErrorBody,
    admin_settings::{EnvSetting, EnvSettings, SettingsSection},
    admin_system::{
        AdminHealth, AppReleases, BackupCheck, BackupFile, Backups, CheckState, HealthCheck,
        HealthFailure, LatestRelease, ReleaseItem, VersionShare,
    },
};
use semver::Version;
use serde::Deserialize;
use serde_json::json;
use utoipa::ToSchema;

use crate::{
    audit,
    auth::Principal,
    error::{AppError, AppResult, bad},
    extract::Json,
    state::AppState,
};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/settings/env", get(env_settings))
        .route("/health", get(health))
        .route("/backups", get(backups))
        .route("/backups/check", post(check_backup))
        .route("/releases", get(releases))
}

use SettingsSection as S;

/// Переменные, которые сервер читает: раздел настроек и секрет ли (значение не показывается).
const KNOWN: &[(&str, SettingsSection, bool)] = &[
    ("PUBLIC_URL", S::General, false),
    ("LISTEN", S::General, false),
    ("STATIC_DIR", S::General, false),
    ("LOG", S::General, false),
    ("LOG_FORMAT", S::General, false),
    ("SECRET_KEY", S::General, true),
    ("DATABASE_URL", S::General, true),
    ("ADMIN_PASSWORD", S::SignIn, true),
    ("ADMIN_EMAILS", S::SignIn, false),
    ("LOCAL_AUTH", S::SignIn, false),
    ("OIDC_ISSUER", S::SignIn, false),
    ("OIDC_CLIENT_ID", S::SignIn, false),
    ("OIDC_CLIENT_SECRET", S::SignIn, true),
    ("OIDC_TITLE", S::SignIn, false),
    ("OIDC_ICON", S::SignIn, false),
    ("OIDC_ADMIN_ROLES", S::SignIn, false),
    ("OIDC_PROVIDERS", S::SignIn, false),
    ("DEEZER", S::Music, false),
    ("DEEZER_URL", S::Music, false),
    ("DOWNLOADER", S::Music, false),
    ("FETCHER_MODE", S::Music, false),
    ("FETCHER_URL", S::Music, false),
    ("MUSIC_DIRS", S::Music, false),
    ("STORAGE", S::Storage, false),
    ("STORAGE_PATH", S::Storage, false),
    ("STORAGE_ML_URL", S::Storage, false),
    ("S3_ENDPOINT", S::Storage, false),
    ("S3_PUBLIC_ENDPOINT", S::Storage, false),
    ("S3_ML_ENDPOINT", S::Storage, false),
    ("S3_BUCKET", S::Storage, false),
    ("S3_REGION", S::Storage, false),
    ("S3_ACCESS_KEY_ID", S::Storage, true),
    ("S3_SECRET_ACCESS_KEY", S::Storage, true),
    ("RENDITIONS_MAX_GB", S::Storage, false),
    ("BACKUP_DIR", S::Storage, false),
    ("CLIPS_MAX_GB", S::Clips, false),
    ("ML_URL", S::Reco, false),
    ("VAPID_PUBLIC", S::Notifications, false),
    ("VAPID_PRIVATE", S::Notifications, true),
    ("VAPID_SUBJECT", S::Notifications, false),
    ("APNS_KEY_PATH", S::Notifications, false),
    ("APNS_KEY_ID", S::Notifications, false),
    ("APNS_TEAM_ID", S::Notifications, false),
    ("APNS_TOPIC", S::Notifications, false),
    ("LRCLIB_URL", S::Integrations, false),
    ("YANDEX_MUSIC_URL", S::Integrations, false),
    ("MUSICKIT_URL", S::Integrations, false),
    ("MUSICKIT_KEY_PATH", S::Integrations, false),
    ("MUSICKIT_KEY_ID", S::Integrations, false),
    ("MUSICKIT_TEAM_ID", S::Integrations, false),
    ("MUSICKIT_STOREFRONT", S::Integrations, false),
    ("RELEASE_TOKEN", S::Integrations, true),
];

/// Секрет по имени: ключи, пароли, токены, адрес базы (в нём пароль).
fn secret_name(name: &str) -> bool {
    [
        "SECRET",
        "PASSWORD",
        "TOKEN",
        "PRIVATE",
        "DATABASE_URL",
        "ACCESS_KEY",
    ]
    .iter()
    .any(|w| name.contains(w))
}

/// Список для админки из любого источника переменных (в тестах — словарь).
pub fn collect(vars: &[(String, String)]) -> Vec<EnvSetting> {
    let get = |name: &str| {
        vars.iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.trim().to_string())
            .filter(|v| !v.is_empty())
    };
    let mut items: Vec<EnvSetting> = KNOWN
        .iter()
        .map(|(name, section, secret)| {
            let full = format!("DALYWAVE_{name}");
            let value = get(&full);
            EnvSetting {
                set: value.is_some(),
                value: value.filter(|_| !secret),
                name: full,
                section: *section,
                secret: *secret,
            }
        })
        .collect();
    // Дополнительные провайдеры входа: DALYWAVE_OIDC_<ID>_*
    let mut extra: Vec<&(String, String)> = vars
        .iter()
        .filter(|(k, _)| k.starts_with("DALYWAVE_OIDC_") && !items.iter().any(|i| &i.name == k))
        .collect();
    extra.sort_by(|a, b| a.0.cmp(&b.0));
    for (k, v) in extra {
        let secret = secret_name(k);
        let v = v.trim().to_string();
        items.push(EnvSetting {
            name: k.clone(),
            section: S::SignIn,
            set: !v.is_empty(),
            value: (!secret && !v.is_empty()).then_some(v),
            secret,
        });
    }
    items
}

#[utoipa::path(
    get,
    path = "/api/admin/settings/env",
    tag = "admin",
    security(("session" = []), ("device" = [])),
    responses(
        (status = 200, description = "Настройки из переменных окружения: только чтение, у секретов — без значения", body = EnvSettings),
        (status = 403, description = "Нет права admin", body = ErrorBody),
    ),
)]
pub async fn env_settings() -> AppResult<Json<EnvSettings>> {
    let vars: Vec<(String, String)> = std::env::vars()
        .filter(|(k, _)| k.starts_with("DALYWAVE_"))
        .collect();
    Ok(Json(EnvSettings {
        items: collect(&vars),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secrets_are_hidden() {
        let vars = vec![
            (
                "DALYWAVE_PUBLIC_URL".to_string(),
                "https://wave.example".to_string(),
            ),
            ("DALYWAVE_SECRET_KEY".to_string(), "abc".to_string()),
            (
                "DALYWAVE_OIDC_GOOGLE_ISSUER".to_string(),
                "https://accounts.google.com".to_string(),
            ),
            (
                "DALYWAVE_OIDC_GOOGLE_CLIENT_SECRET".to_string(),
                "xyz".to_string(),
            ),
        ];
        let items = collect(&vars);
        let find = |n: &str| items.iter().find(|i| i.name == n).unwrap();
        assert_eq!(
            find("DALYWAVE_PUBLIC_URL").value.as_deref(),
            Some("https://wave.example")
        );
        let key = find("DALYWAVE_SECRET_KEY");
        assert!(key.set && key.value.is_none() && key.secret);
        assert!(!find("DALYWAVE_STORAGE").set);
        assert_eq!(
            find("DALYWAVE_OIDC_GOOGLE_ISSUER").section,
            SettingsSection::SignIn
        );
        let s = find("DALYWAVE_OIDC_GOOGLE_CLIENT_SECRET");
        assert!(s.secret && s.value.is_none());
    }
}

// ───────────── Здоровье ─────────────

fn check(
    key: &str,
    state: CheckState,
    started: Option<Instant>,
    detail: Option<String>,
) -> HealthCheck {
    HealthCheck {
        key: key.into(),
        state,
        latency_ms: started.map(|t| t.elapsed().as_millis() as i64),
        detail,
    }
}

#[utoipa::path(
    get,
    path = "/api/admin/health",
    tag = "admin",
    security(("session" = []), ("device" = [])),
    responses(
        (status = 200, description = "Проверки частей сервера, очередь и упавшие за сутки задачи", body = AdminHealth),
        (status = 403, description = "Нет права admin", body = ErrorBody),
    ),
)]
pub async fn health(State(st): State<AppState>) -> AppResult<Json<AdminHealth>> {
    let mut checks = Vec::new();

    let t = Instant::now();
    let db = tokio::time::timeout(
        Duration::from_secs(3),
        sqlx::query_scalar!(r#"SELECT pg_database_size(current_database()) AS "size!""#)
            .fetch_one(&st.db),
    )
    .await;
    let db_bytes = match db {
        Ok(Ok(size)) => {
            checks.push(check("database", CheckState::Ok, Some(t), None));
            size
        }
        Ok(Err(e)) => {
            checks.push(check(
                "database",
                CheckState::Down,
                Some(t),
                Some(e.to_string()),
            ));
            0
        }
        Err(_) => {
            let detail = Some("нет ответа 3 с".into());
            checks.push(check("database", CheckState::Down, Some(t), detail));
            0
        }
    };

    let t = Instant::now();
    let probe = st.storage.head("healthcheck/probe");
    checks.push(
        match tokio::time::timeout(Duration::from_secs(5), probe).await {
            Ok(Ok(_)) => check("storage", CheckState::Ok, Some(t), None),
            Ok(Err(e)) => check("storage", CheckState::Down, Some(t), Some(e.to_string())),
            Err(_) => check(
                "storage",
                CheckState::Down,
                Some(t),
                Some("нет ответа 5 с".into()),
            ),
        },
    );

    let listening = st.hub.is_listening();
    checks.push(check(
        "realtime",
        if listening {
            CheckState::Ok
        } else {
            CheckState::Down
        },
        None,
        (!listening).then(|| "нет подписки на события базы".into()),
    ));

    let q = sqlx::query!(
        r#"SELECT count(*) FILTER (WHERE status = 'queued' AND run_after <= now()) AS "queued!",
                  count(*) FILTER (WHERE status = 'running') AS "running!",
                  count(*) FILTER (WHERE status = 'failed' AND updated_at > now() - interval '1 day') AS "failed!",
                  max(finished_at) AS last_done
           FROM jobs"#
    )
    .fetch_one(&st.db)
    .await?;
    // Очередь есть, а последняя задача закрыта давно и ничего не идёт — воркер, похоже, стоит
    let stalled = q.queued > 0
        && q.running == 0
        && q.last_done
            .is_none_or(|d| d < Utc::now() - chrono::Duration::minutes(30));
    checks.push(check(
        "worker",
        if stalled {
            CheckState::Warn
        } else {
            CheckState::Ok
        },
        None,
        q.last_done.map(|d| d.to_rfc3339()),
    ));

    let t = Instant::now();
    checks.push(
        match tokio::time::timeout(Duration::from_secs(5), st.ml.health()).await {
            Ok(Ok(_)) => check("ml", CheckState::Ok, Some(t), None),
            Ok(Err(e)) => check("ml", CheckState::Down, Some(t), Some(e.to_string())),
            Err(_) => check(
                "ml",
                CheckState::Down,
                Some(t),
                Some("нет ответа 5 с".into()),
            ),
        },
    );

    let fetcher = &st.config.fetcher;
    if fetcher.server {
        let t = Instant::now();
        checks.push(
            match crate::acquire::server::health(&st.http, &fetcher.url).await {
                Ok(_) => check("fetcher", CheckState::Ok, Some(t), None),
                Err(e) => check("fetcher", CheckState::Down, Some(t), Some(e)),
            },
        );
    } else {
        checks.push(check("fetcher", CheckState::Off, None, None));
    }

    checks.push(match backup_dir() {
        None => check("backups", CheckState::Off, None, None),
        Some(dir) => match tokio::task::spawn_blocking(move || list_backups(&dir)).await {
            Ok(Ok(files)) => match files.first() {
                Some(f) if f.created_at > Utc::now() - chrono::Duration::hours(36) => check(
                    "backups",
                    CheckState::Ok,
                    None,
                    Some(f.created_at.to_rfc3339()),
                ),
                Some(f) => check(
                    "backups",
                    CheckState::Warn,
                    None,
                    Some(f.created_at.to_rfc3339()),
                ),
                None => check("backups", CheckState::Warn, None, None),
            },
            Ok(Err(e)) => check("backups", CheckState::Down, None, Some(e)),
            Err(e) => check("backups", CheckState::Down, None, Some(e.to_string())),
        },
    });

    let failures = sqlx::query!(
        "SELECT id, kind, lane, last_error, updated_at FROM jobs
         WHERE status = 'failed' AND updated_at > now() - interval '1 day'
         ORDER BY updated_at DESC LIMIT 20"
    )
    .fetch_all(&st.db)
    .await?
    .into_iter()
    .map(|r| HealthFailure {
        job_id: r.id,
        kind: r.kind,
        lane: r.lane,
        error: r.last_error,
        at: r.updated_at,
    })
    .collect();

    Ok(Json(AdminHealth {
        version: env!("CARGO_PKG_VERSION").to_string(),
        uptime_s: st.started.elapsed().as_secs(),
        checks,
        db_bytes,
        connections: st.hub.connections() as u64,
        queued: q.queued,
        running: q.running,
        failed_24h: q.failed,
        failures,
    }))
}

// ───────────── Резервные копии ─────────────

/// Папка с дампами базы (`DALYWAVE_BACKUP_DIR`; в проде — том контейнера backup).
fn backup_dir() -> Option<PathBuf> {
    std::env::var("DALYWAVE_BACKUP_DIR")
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
}

fn is_dump(name: &str) -> bool {
    name.ends_with(".sql.gz") || name.ends_with(".sql")
}

/// Копии в папке, свежие первыми.
pub fn list_backups(dir: &FsPath) -> Result<Vec<BackupFile>, String> {
    let entries = std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let mut files: Vec<BackupFile> = entries
        .filter_map(Result::ok)
        .filter_map(|e| {
            let name = e.file_name().into_string().ok()?;
            let meta = e.metadata().ok()?;
            (meta.is_file() && is_dump(&name)).then(|| BackupFile {
                name,
                size_bytes: meta.len() as i64,
                created_at: meta
                    .modified()
                    .map(DateTime::<Utc>::from)
                    .unwrap_or_else(|_| Utc::now()),
            })
        })
        .collect();
    files.sort_by(|a, b| b.created_at.cmp(&a.created_at).then(b.name.cmp(&a.name)));
    Ok(files)
}

/// Что нашлось в дампе.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct DumpStats {
    pub header: bool,
    pub complete: bool,
    pub tables: i64,
    pub data: i64,
    pub objects: i64,
}

/// Читает дамп `pg_dump` (простой SQL) целиком: у сжатого заодно сверяется контрольная сумма gzip.
pub fn scan_dump(reader: impl Read) -> std::io::Result<DumpStats> {
    let mut stats = DumpStats::default();
    let mut lines = BufReader::new(reader);
    let mut line = Vec::new();
    loop {
        line.clear();
        if lines.read_until(b'\n', &mut line)? == 0 {
            break;
        }
        let text = String::from_utf8_lossy(&line);
        let text = text.trim_end();
        if text.starts_with("-- PostgreSQL database dump complete") {
            stats.complete = true;
        } else if text.starts_with("-- PostgreSQL database dump") {
            stats.header = true;
        } else if text.starts_with("CREATE TABLE ") {
            stats.tables += 1;
            stats.objects += 1;
        } else if text.starts_with("CREATE ") {
            stats.objects += 1;
        } else if text.starts_with("COPY ") && text.ends_with("FROM stdin;") {
            stats.data += 1;
        }
    }
    Ok(stats)
}

fn check_file(path: &FsPath) -> Result<DumpStats, String> {
    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default();
    let stats = if name.ends_with(".gz") {
        scan_dump(flate2::read::MultiGzDecoder::new(file))
    } else {
        scan_dump(file)
    }
    .map_err(|e| format!("копия повреждена: {e}"))?;
    if !stats.header {
        return Err("это не дамп pg_dump".into());
    }
    if !stats.complete {
        return Err("дамп оборван: нет строки о завершении".into());
    }
    if stats.tables == 0 {
        return Err("в дампе нет таблиц".into());
    }
    Ok(stats)
}

async fn last_check(st: &AppState) -> AppResult<Option<BackupCheck>> {
    Ok(sqlx::query!(
        "SELECT name, ok, method, entries, tables, error, checked_at FROM backup_checks
         ORDER BY checked_at DESC LIMIT 1"
    )
    .fetch_optional(&st.db)
    .await?
    .map(|r| BackupCheck {
        name: r.name,
        ok: r.ok,
        method: r.method,
        entries: r.entries,
        tables: r.tables,
        error: r.error,
        checked_at: r.checked_at,
    }))
}

async fn listed(dir: PathBuf) -> AppResult<Result<Vec<BackupFile>, String>> {
    tokio::task::spawn_blocking(move || list_backups(&dir))
        .await
        .map_err(|e| AppError::Internal(e.into()))
}

#[utoipa::path(
    get,
    path = "/api/admin/backups",
    tag = "admin",
    security(("session" = []), ("device" = [])),
    responses(
        (status = 200, description = "Резервные копии базы в папке `DALYWAVE_BACKUP_DIR` и последняя проверка", body = Backups),
        (status = 403, description = "Нет права admin", body = ErrorBody),
    ),
)]
pub async fn backups(State(st): State<AppState>) -> AppResult<Json<Backups>> {
    let dir = backup_dir();
    let files = match &dir {
        Some(d) => listed(d.clone()).await?.ok(),
        None => None,
    };
    let configured = files.is_some();
    let files = files.unwrap_or_default();
    Ok(Json(Backups {
        configured,
        dir: dir.map(|d| d.display().to_string()),
        total_bytes: files.iter().map(|f| f.size_bytes).sum(),
        files,
        last_check: last_check(&st).await?,
    }))
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BackupCheckRequest {
    /// Имя файла копии; без него — самая свежая.
    #[serde(default)]
    pub name: Option<String>,
}

#[utoipa::path(
    post,
    path = "/api/admin/backups/check",
    tag = "admin",
    security(("session" = []), ("device" = [])),
    request_body = BackupCheckRequest,
    responses(
        (status = 200, description = "Копия прочитана целиком: распакована, начало и конец дампа на месте, таблицы посчитаны", body = BackupCheck),
        (status = 400, description = "Нет такой копии", body = ErrorBody),
        (status = 409, description = "Папка с копиями не задана (`no_backups`)", body = ErrorBody),
    ),
)]
pub async fn check_backup(
    State(st): State<AppState>,
    principal: Principal,
    Json(req): Json<BackupCheckRequest>,
) -> AppResult<Json<BackupCheck>> {
    let Some(dir) = backup_dir() else {
        return Err(AppError::Conflict {
            code: "no_backups",
            message: "папка с копиями не задана (DALYWAVE_BACKUP_DIR)".into(),
        });
    };
    let files = listed(dir.clone()).await?.map_err(bad)?;
    // Только имя из списка: путь из запроса в файловую систему не попадает
    let file = match &req.name {
        Some(name) => files.iter().find(|f| &f.name == name),
        None => files.first(),
    }
    .ok_or_else(|| bad("нет такой копии"))?;
    let path = dir.join(&file.name);
    let result = tokio::task::spawn_blocking(move || check_file(&path))
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    let (ok, entries, tables, error) = match &result {
        Ok(s) => (true, Some(s.objects), Some(s.tables), None),
        Err(e) => (false, None, None, Some(e.clone())),
    };
    let mut tx = st.db.begin().await?;
    let checked_at = sqlx::query_scalar!(
        "INSERT INTO backup_checks (name, ok, method, entries, tables, error, checked_by)
         VALUES ($1, $2, 'sql', $3, $4, $5, $6) RETURNING checked_at",
        file.name,
        ok,
        entries,
        tables,
        error,
        principal.user.id,
    )
    .fetch_one(&mut *tx)
    .await?;
    audit::record(
        &mut *tx,
        Some(principal.user.id),
        audit::BACKUP_CHECKED,
        Some(file.name.clone()),
        json!({ "ok": ok, "tables": tables, "error": error }),
    )
    .await?;
    tx.commit().await?;
    Ok(Json(BackupCheck {
        name: file.name.clone(),
        ok,
        method: "sql".into(),
        entries,
        tables,
        error,
        checked_at,
    }))
}

// ───────────── Выпуски ─────────────

/// Цель выпуска для платформы устройства (у iOS, часов и ТВ выпусков на сервере нет — TestFlight).
fn target_of(platform: &str) -> Option<&'static str> {
    match platform {
        "macos" => Some("darwin"),
        "windows" => Some("windows"),
        "linux" => Some("linux"),
        "streamdeck" => Some("streamdeck"),
        _ => None,
    }
}

fn parse_version(v: &str) -> Option<Version> {
    Version::parse(v.trim().trim_start_matches('v')).ok()
}

#[utoipa::path(
    get,
    path = "/api/admin/releases",
    tag = "admin",
    security(("session" = []), ("device" = [])),
    responses(
        (status = 200, description = "Выпуски приложений и плагина и кто на какой версии", body = AppReleases),
        (status = 403, description = "Нет права admin", body = ErrorBody),
    ),
)]
pub async fn releases(State(st): State<AppState>) -> AppResult<Json<AppReleases>> {
    let rows = sqlx::query!(
        r#"SELECT id, target, arch, version, size_bytes, installer_key IS NOT NULL AS "installer!",
                  notes, published_at
           FROM app_releases ORDER BY published_at DESC LIMIT 200"#
    )
    .fetch_all(&st.db)
    .await?;
    let mut latest: Vec<(Version, LatestRelease)> = Vec::new();
    for r in &rows {
        let Some(v) = parse_version(&r.version) else {
            continue;
        };
        match latest.iter_mut().find(|(_, l)| l.target == r.target) {
            Some((best, l)) => {
                if v > *best {
                    *best = v;
                    l.version = r.version.clone();
                    l.published_at = r.published_at;
                }
            }
            None => latest.push((
                v,
                LatestRelease {
                    target: r.target.clone(),
                    version: r.version.clone(),
                    published_at: r.published_at,
                },
            )),
        }
    }
    let items = rows
        .into_iter()
        .take(40)
        .map(|r| ReleaseItem {
            id: r.id,
            target: r.target,
            arch: r.arch,
            version: r.version,
            size_bytes: r.size_bytes,
            installer: r.installer,
            notes: r.notes,
            published_at: r.published_at,
        })
        .collect();
    let installs = sqlx::query!(
        r#"SELECT platform, app_version, count(*) AS "devices!", count(DISTINCT user_id) AS "people!"
           FROM devices
           WHERE revoked_at IS NULL AND COALESCE(last_seen_at, created_at) > now() - interval '60 days'
           GROUP BY platform, app_version
           ORDER BY platform, app_version DESC NULLS LAST"#
    )
    .fetch_all(&st.db)
    .await?
    .into_iter()
    .map(|r| {
        let newest = target_of(&r.platform)
            .and_then(|t| latest.iter().find(|(_, l)| l.target == t))
            .map(|(v, _)| v);
        let outdated = match (newest, r.app_version.as_deref().map(parse_version)) {
            (Some(n), Some(Some(v))) => v < *n,
            (Some(_), _) => true,
            _ => false,
        };
        VersionShare {
            platform: r.platform,
            version: r.app_version,
            devices: r.devices,
            people: r.people,
            outdated,
        }
    })
    .collect();
    Ok(Json(AppReleases {
        server_version: env!("CARGO_PKG_VERSION").to_string(),
        items,
        latest: latest.into_iter().map(|(_, l)| l).collect(),
        installs,
    }))
}

#[cfg(test)]
mod system_tests {
    use super::*;

    const DUMP: &str = "--\n-- PostgreSQL database dump\n--\nCREATE TABLE public.users (id uuid);\nCREATE INDEX x ON public.users (id);\nCOPY public.users (id) FROM stdin;\n\\.\n--\n-- PostgreSQL database dump complete\n--\n";

    #[test]
    fn plain_dump_is_counted() {
        let s = scan_dump(DUMP.as_bytes()).unwrap();
        assert_eq!(
            s,
            DumpStats {
                header: true,
                complete: true,
                tables: 1,
                data: 1,
                objects: 2
            }
        );
    }

    #[test]
    fn gzip_dump_and_broken_one() {
        use std::io::Write;
        let mut enc = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        enc.write_all(DUMP.as_bytes()).unwrap();
        let gz = enc.finish().unwrap();
        let s = scan_dump(flate2::read::MultiGzDecoder::new(&gz[..])).unwrap();
        assert!(s.complete && s.tables == 1);
        // Оборванный архив — ошибка чтения, а не «пустая» копия
        let cut = &gz[..gz.len() - 12];
        assert!(scan_dump(flate2::read::MultiGzDecoder::new(cut)).is_err());
    }

    #[test]
    fn targets_and_versions() {
        assert_eq!(target_of("macos"), Some("darwin"));
        assert_eq!(target_of("ios"), None);
        assert!(parse_version("v0.1.16").unwrap() > parse_version("0.1.9").unwrap());
    }
}
