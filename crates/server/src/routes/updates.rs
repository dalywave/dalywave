//! Автообновление приложений (tauri-plugin-updater) и выпуск новых версий.
//!
//! Приложение спрашивает `GET /api/updates/{target}/{arch}/{current_version}` — открыто без входа:
//! проверка идёт и до входа. 204 — версия последняя, 200 — JSON в формате плагина Tauri с
//! подписанной ссылкой на файл в хранилище и подписью из `.sig`.
//!
//! Выпуск регистрирует конвейер сборки (`make release-mac`, CI) токеном `DALYWAVE_RELEASE_TOKEN`:
//! берёт ссылку на загрузку файла прямо в хранилище (`POST /api/releases/uploads`), кладёт файл и
//! регистрирует выпуск (`POST /api/releases`). Ключ R2 конвейеру не нужен, а файл в сотни мегабайт
//! не идёт через сервер и прокси Cloudflare.
//!
//! Вместе с файлом обновления выпуск может принести установщик для людей (`installerFileName`):
//! macOS — `.dmg`, Windows — тот же `-setup.exe` NSIS, что и обновление. Его раздаёт
//! `/api/downloads` ([`super::downloads`]).
//!
//! Плагин Stream Deck — такой же выпуск с платформой `streamdeck` и архитектурой `universal`: один
//! `.streamDeckPlugin` для Mac и Windows, он же установщик. Подписи у него нет — плагин Tauri его не
//! ставит; `GET /api/updates/streamdeck/universal/{версия}` только сообщает о новой версии.

use std::time::Duration;

use axum::{
    Router,
    extract::{FromRequestParts, State},
    http::{StatusCode, header, request::Parts},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use chrono::{DateTime, Utc};
use dalywave_protocol::ErrorBody;
use semver::Version;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{
    auth,
    crypto::secrets_match,
    error::{AppError, AppResult, bad},
    extract::{Json, Path},
    state::AppState,
};

/// Приложение качает обновление сразу после проверки; следующая проверка — через 6 часов и с новой
/// ссылкой, поэтому дольше ссылке жить незачем.
pub const DOWNLOAD_TTL: Duration = Duration::from_secs(6 * 3600);
/// Сборка с загрузчиком весит сотни мегабайт: час — с запасом на медленную отдачу из дома.
pub const UPLOAD_TTL: Duration = Duration::from_secs(60 * 60);

const TARGETS: [&str; 3] = ["darwin", "windows", "linux"];
const ARCHES: [&str; 4] = ["aarch64", "x86_64", "i686", "armv7"];
/// Плагин Stream Deck: своя «платформа» с единственной архитектурой.
pub const STREAMDECK: &str = "streamdeck";
pub const UNIVERSAL: &str = "universal";
const MAX_FILE_NAME: usize = 200;
/// Подпись minisign в base64 — около 400 символов
const MAX_SIGNATURE: usize = 4096;
const MAX_NOTES: usize = 20_000;
/// Установщики, которые сайт отдаёт людям: образ диска macOS, установщики NSIS и MSI
pub const INSTALLER_EXTENSIONS: [&str; 3] = ["dmg", "exe", "msi"];
/// Установщик плагина Stream Deck (сравнение без учёта регистра)
pub const PLUGIN_EXTENSION: &str = "streamdeckplugin";

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/updates/{target}/{arch}/{current_version}", get(check))
        .route("/releases/uploads", post(upload))
        .route("/releases", post(register))
}

/// Ответ в формате tauri-plugin-updater. Имена полей задаёт плагин (`pub_date`), поэтому здесь
/// не camelCase.
#[derive(Debug, Serialize, ToSchema)]
pub struct UpdateManifest {
    /// SemVer новой версии
    pub version: String,
    /// Что нового
    pub notes: Option<String>,
    pub pub_date: DateTime<Utc>,
    /// Подписанная ссылка на файл обновления, живёт 6 часов
    pub url: String,
    /// Содержимое `.sig` из сборки: плагин проверяет файл ключом из `tauri.conf.json`
    pub signature: String,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseUploadRequest {
    /// `darwin`, `windows`, `linux`; `streamdeck` — плагин Stream Deck
    pub target: String,
    /// `aarch64`, `x86_64`, `i686`, `armv7`; у `streamdeck` — только `universal`
    pub arch: String,
    /// SemVer, например `0.2.0`
    pub version: String,
    /// Имя файла обновления, например `DalyWave.app.tar.gz` или `DalyWave_0.2.0_x64-setup.exe`
    pub file_name: String,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseUpload {
    /// Ключ файла в хранилище
    pub key: String,
    /// Подписанная ссылка: `PUT` с телом файла
    pub url: String,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseRequest {
    pub target: String,
    pub arch: String,
    pub version: String,
    /// Тот же файл, что в `POST /api/releases/uploads`; он уже должен лежать в хранилище
    pub file_name: String,
    /// Содержимое `.sig` из сборки; у `streamdeck` не нужно
    #[serde(default)]
    pub signature: String,
    pub notes: Option<String>,
    /// Установщик для скачивания с сайта (`.dmg`, `.exe`, `.msi`, у плагина — `.streamDeckPlugin`),
    /// загруженный так же, как файл обновления; на Windows и у плагина это может быть тот же файл.
    /// Без него выпуск только обновляет установленные приложения.
    #[serde(default)]
    pub installer_file_name: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AppRelease {
    pub target: String,
    pub arch: String,
    pub version: String,
    pub notes: Option<String>,
    pub size_bytes: i64,
    /// Размер установщика, если он есть в выпуске
    pub installer_size_bytes: Option<i64>,
    pub published_at: DateTime<Utc>,
}

/// Конвейер выпусков: `Authorization: Bearer <DALYWAVE_RELEASE_TOKEN>`. Без токена в настройках
/// сервера выпускать нельзя никому.
pub struct ReleaseAuth;

impl FromRequestParts<AppState> for ReleaseAuth {
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, st: &AppState) -> Result<Self, Self::Rejection> {
        let Some(expected) = st.config.release_token.as_deref() else {
            tracing::warn!("app release attempt, but DALYWAVE_RELEASE_TOKEN is not set");
            return Err(AppError::Unauthorized);
        };
        match auth::bearer(&parts.headers) {
            Some(token) if secrets_match(token, expected) => Ok(Self),
            _ => Err(AppError::Unauthorized),
        }
    }
}

fn parse_version(value: &str) -> AppResult<Version> {
    Version::parse(value.trim().trim_start_matches('v'))
        .map_err(|_| bad(format!("версия «{value}» — не SemVer, нужно вроде 0.2.0")))
}

/// Платформа и архитектура — как их называет плагин Tauri (`{{target}}`, `{{arch}}`), или плагин
/// Stream Deck (`streamdeck`/`universal`); смешивать нельзя.
fn platform(target: &str, arch: &str) -> AppResult<()> {
    if target == STREAMDECK || arch == UNIVERSAL {
        return if target == STREAMDECK && arch == UNIVERSAL {
            Ok(())
        } else {
            Err(bad("плагин Stream Deck — только streamdeck/universal"))
        };
    }
    if !TARGETS.contains(&target) {
        return Err(bad(format!(
            "платформа «{target}» неизвестна: {}",
            TARGETS.join(", ")
        )));
    }
    if !ARCHES.contains(&arch) {
        return Err(bad(format!(
            "архитектура «{arch}» неизвестна: {}",
            ARCHES.join(", ")
        )));
    }
    Ok(())
}

/// Имя файла становится частью ключа в хранилище: без папок и странных символов.
fn file_name(name: &str) -> AppResult<&str> {
    let ok = !name.is_empty()
        && name.len() <= MAX_FILE_NAME
        && !name.starts_with('.')
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'));
    if ok {
        Ok(name)
    } else {
        Err(bad(
            "имя файла — латиница, цифры, «.», «_» и «-», не длиннее 200 символов",
        ))
    }
}

/// Установщик — по расширению: файл обновления `.app.tar.gz` людям не нужен. У плагина Stream Deck —
/// только `.streamDeckPlugin`, у приложений — `.dmg`, `.exe`, `.msi`.
fn installer_name<'a>(target: &str, name: &'a str) -> AppResult<&'a str> {
    let name = file_name(name)?;
    let ext = name
        .rsplit_once('.')
        .map(|(_, e)| e.to_ascii_lowercase())
        .unwrap_or_default();
    match (target == STREAMDECK, ext.as_str()) {
        (true, PLUGIN_EXTENSION) => Ok(name),
        (true, _) => Err(bad("установщик плагина — файл .streamDeckPlugin")),
        (false, e) if INSTALLER_EXTENSIONS.contains(&e) => Ok(name),
        (false, _) => Err(bad("установщик — файл .dmg, .exe или .msi")),
    }
}

pub(crate) fn release_key(target: &str, arch: &str, version: &Version, file: &str) -> String {
    format!("releases/{target}/{arch}/{version}/{file}")
}

pub(crate) fn storage_unavailable(e: impl std::fmt::Display) -> AppError {
    AppError::Unavailable(format!("хранилище не ответило: {e}"))
}

#[utoipa::path(
    get,
    path = "/api/updates/{target}/{arch}/{current_version}",
    tag = "system",
    params(
        ("target" = String, Path, description = "Платформа: `darwin`, `windows`, `linux`"),
        ("arch" = String, Path, description = "Архитектура: `aarch64`, `x86_64`"),
        ("current_version" = String, Path, description = "Установленная версия, например `0.1.0`"),
    ),
    responses(
        (status = 200, description = "Есть версия новее: формат tauri-plugin-updater", body = UpdateManifest),
        (status = 204, description = "Обновлений нет"),
        (status = 400, description = "Версия — не SemVer", body = ErrorBody),
    ),
)]
pub async fn check(
    State(st): State<AppState>,
    Path((target, arch, current)): Path<(String, String, String)>,
) -> AppResult<Response> {
    let current = parse_version(&current)?;
    let rows = sqlx::query!(
        "SELECT version, notes, storage_key, signature, published_at
         FROM app_releases WHERE target = $1 AND arch = $2",
        target,
        arch,
    )
    .fetch_all(&st.db)
    .await?;
    // Выпусков у платформы — десятки, а порядок SemVer строками в SQL не выразить
    let latest = rows
        .into_iter()
        .filter_map(|r| Version::parse(&r.version).ok().map(|v| (v, r)))
        .max_by(|a, b| a.0.cmp(&b.0));
    let Some((version, release)) = latest.filter(|(v, _)| *v > current) else {
        return Ok(StatusCode::NO_CONTENT.into_response());
    };
    let url = st
        .storage
        .presign_get(&release.storage_key, DOWNLOAD_TTL)
        .await
        .map_err(storage_unavailable)?;
    let body = UpdateManifest {
        version: version.to_string(),
        notes: release.notes,
        pub_date: release.published_at,
        url: url.to_string(),
        signature: release.signature,
    };
    // В ответе подписанная ссылка со сроком: кешировать его нельзя
    Ok(([(header::CACHE_CONTROL, "no-store")], Json(body)).into_response())
}

#[utoipa::path(
    post,
    path = "/api/releases/uploads",
    tag = "releases",
    security(("release" = [])),
    request_body = ReleaseUploadRequest,
    responses(
        (status = 200, description = "Ссылка на загрузку файла обновления прямо в хранилище (`PUT`, час)", body = ReleaseUpload),
        (status = 400, description = "Неизвестная платформа, не SemVer или плохое имя файла", body = ErrorBody),
        (status = 401, description = "Токен выпусков не принят", body = ErrorBody),
    ),
)]
pub async fn upload(
    State(st): State<AppState>,
    _: ReleaseAuth,
    Json(req): Json<ReleaseUploadRequest>,
) -> AppResult<Json<ReleaseUpload>> {
    platform(&req.target, &req.arch)?;
    let version = parse_version(&req.version)?;
    let key = release_key(&req.target, &req.arch, &version, file_name(&req.file_name)?);
    let url = st
        .storage
        .presign_put(&key, UPLOAD_TTL)
        .await
        .map_err(storage_unavailable)?;
    Ok(Json(ReleaseUpload {
        key,
        url: url.to_string(),
        expires_at: Utc::now() + UPLOAD_TTL,
    }))
}

#[utoipa::path(
    post,
    path = "/api/releases",
    tag = "releases",
    security(("release" = [])),
    request_body = ReleaseRequest,
    responses(
        (status = 200, description = "Выпуск зарегистрирован (та же версия — заменён): приложения получат его при следующей проверке", body = AppRelease),
        (status = 400, description = "Неверные поля", body = ErrorBody),
        (status = 401, description = "Токен выпусков не принят", body = ErrorBody),
        (status = 409, description = "`not_uploaded` — файла обновления или установщика ещё нет в хранилище", body = ErrorBody),
    ),
)]
pub async fn register(
    State(st): State<AppState>,
    _: ReleaseAuth,
    Json(req): Json<ReleaseRequest>,
) -> AppResult<Json<AppRelease>> {
    platform(&req.target, &req.arch)?;
    let version = parse_version(&req.version)?;
    let key = release_key(&req.target, &req.arch, &version, file_name(&req.file_name)?);
    let signature = req.signature.trim();
    // Плагин Stream Deck ставит сам Stream Deck: подписи minisign у него нет
    let plugin = req.target == STREAMDECK;
    let signature_ok = if signature.is_empty() {
        plugin
    } else {
        signature.len() <= MAX_SIGNATURE
            && signature
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '/' | '='))
    };
    if !signature_ok {
        return Err(bad("подпись — содержимое файла .sig из сборки (base64)"));
    }
    let notes = req
        .notes
        .as_deref()
        .map(str::trim)
        .filter(|n| !n.is_empty());
    if notes.is_some_and(|n| n.chars().count() > MAX_NOTES) {
        return Err(bad("список изменений длиннее 20 000 символов"));
    }
    let installer_key = req
        .installer_file_name
        .as_deref()
        .map(|name| {
            installer_name(&req.target, name)
                .map(|n| release_key(&req.target, &req.arch, &version, n))
        })
        .transpose()?;
    if plugin && installer_key.as_deref() != Some(key.as_str()) {
        return Err(bad(
            "у плагина Stream Deck установщик — тот же файл .streamDeckPlugin, что и обновление",
        ));
    }
    let size = uploaded_size(&st, &key, "файл обновления").await?;
    let installer_size = match &installer_key {
        Some(k) if *k == key => Some(size),
        Some(k) => Some(uploaded_size(&st, k, "установщик").await?),
        None => None,
    };
    let version = version.to_string();
    // Повторная сборка той же версии заменяет выпуск целиком, вместе с установщиком
    let published_at = sqlx::query_scalar!(
        "INSERT INTO app_releases
             (target, arch, version, storage_key, size_bytes, signature, notes, installer_key, installer_size)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
         ON CONFLICT (target, arch, version) DO UPDATE
         SET storage_key = EXCLUDED.storage_key, size_bytes = EXCLUDED.size_bytes,
             signature = EXCLUDED.signature, notes = EXCLUDED.notes,
             installer_key = EXCLUDED.installer_key, installer_size = EXCLUDED.installer_size,
             published_at = now()
         RETURNING published_at",
        req.target,
        req.arch,
        version,
        key,
        size,
        signature,
        notes,
        installer_key,
        installer_size,
    )
    .fetch_one(&st.db)
    .await?;
    tracing::info!(target = %req.target, arch = %req.arch, %version, size, ?installer_size, "app release registered");
    Ok(Json(AppRelease {
        target: req.target,
        arch: req.arch,
        version,
        notes: notes.map(str::to_string),
        size_bytes: size,
        installer_size_bytes: installer_size,
        published_at,
    }))
}

/// Размер файла, уже загруженного конвейером по ссылке; 409 `not_uploaded`, если его нет.
async fn uploaded_size(st: &AppState, key: &str, what: &str) -> AppResult<i64> {
    let meta = st
        .storage
        .head(key)
        .await
        .map_err(storage_unavailable)?
        .ok_or_else(|| AppError::Conflict {
            code: "not_uploaded",
            message: format!("файла {key} нет в хранилище — сначала загрузите его"),
        })?;
    let size = i64::try_from(meta.size).map_err(|_| bad("файл слишком большой"))?;
    if size == 0 {
        return Err(bad(format!("{what} пустой")));
    }
    Ok(size)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_and_names() {
        assert_eq!(parse_version("v0.2.0").unwrap().to_string(), "0.2.0");
        assert!(parse_version("0.2").is_err());
        assert!(parse_version("0.10.0").unwrap() > parse_version("0.9.3").unwrap());
        assert!(parse_version("1.0.0-beta.1").unwrap() < parse_version("1.0.0").unwrap());
        assert!(file_name("DalyWave_0.2.0_x64-setup.exe").is_ok());
        assert!(file_name("DalyWave.app.tar.gz").is_ok());
        for bad in ["", "../x", "a/b", ".hidden", "a b.exe", "ы.exe"] {
            assert!(file_name(bad).is_err(), "{bad}");
        }
        assert!(installer_name("darwin", "DalyWave_0.2.0_aarch64.dmg").is_ok());
        assert!(installer_name("windows", "DalyWave_0.2.0_x64-setup.EXE").is_ok());
        assert!(installer_name("windows", "DalyWave_0.2.0_x64_ru-RU.msi").is_ok());
        assert!(installer_name("darwin", "DalyWave.app.tar.gz").is_err());
        assert!(installer_name("darwin", "../x.dmg").is_err());
        assert!(installer_name("darwin", "DalyWave_0.2.0.streamDeckPlugin").is_err());
        assert!(installer_name("streamdeck", "DalyWave_0.2.0.streamDeckPlugin").is_ok());
        assert!(installer_name("streamdeck", "DalyWave_0.2.0.STREAMDECKPLUGIN").is_ok());
        assert!(installer_name("streamdeck", "DalyWave_0.2.0.dmg").is_err());
        assert!(platform("darwin", "aarch64").is_ok());
        assert!(platform("ios", "aarch64").is_err());
        assert!(platform("windows", "arm64").is_err());
        assert!(platform("streamdeck", "universal").is_ok());
        assert!(platform("streamdeck", "x86_64").is_err());
        assert!(platform("darwin", "universal").is_err());
    }
}
