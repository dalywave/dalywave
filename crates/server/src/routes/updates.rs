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
const MAX_FILE_NAME: usize = 200;
/// Подпись minisign в base64 — около 400 символов
const MAX_SIGNATURE: usize = 4096;
const MAX_NOTES: usize = 20_000;

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
    /// `darwin`, `windows`, `linux`
    pub target: String,
    /// `aarch64`, `x86_64`, `i686`, `armv7`
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
    /// Содержимое `.sig` из сборки
    pub signature: String,
    pub notes: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AppRelease {
    pub target: String,
    pub arch: String,
    pub version: String,
    pub notes: Option<String>,
    pub size_bytes: i64,
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

/// Платформа и архитектура — как их называет плагин Tauri (`{{target}}`, `{{arch}}`).
fn platform(target: &str, arch: &str) -> AppResult<()> {
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

fn release_key(target: &str, arch: &str, version: &Version, file: &str) -> String {
    format!("releases/{target}/{arch}/{version}/{file}")
}

fn storage_unavailable(e: impl std::fmt::Display) -> AppError {
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
        (status = 409, description = "`not_uploaded` — файла ещё нет в хранилище", body = ErrorBody),
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
    let signature_ok = !signature.is_empty()
        && signature.len() <= MAX_SIGNATURE
        && signature
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '/' | '='));
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
    let meta = st
        .storage
        .head(&key)
        .await
        .map_err(storage_unavailable)?
        .ok_or_else(|| AppError::Conflict {
            code: "not_uploaded",
            message: format!("файла {key} нет в хранилище — сначала загрузите его"),
        })?;
    let size = i64::try_from(meta.size).map_err(|_| bad("файл слишком большой"))?;
    if size == 0 {
        return Err(bad("файл обновления пустой"));
    }
    let version = version.to_string();
    let published_at = sqlx::query_scalar!(
        "INSERT INTO app_releases (target, arch, version, storage_key, size_bytes, signature, notes)
         VALUES ($1, $2, $3, $4, $5, $6, $7)
         ON CONFLICT (target, arch, version) DO UPDATE
         SET storage_key = EXCLUDED.storage_key, size_bytes = EXCLUDED.size_bytes,
             signature = EXCLUDED.signature, notes = EXCLUDED.notes, published_at = now()
         RETURNING published_at",
        req.target,
        req.arch,
        version,
        key,
        size,
        signature,
        notes,
    )
    .fetch_one(&st.db)
    .await?;
    tracing::info!(target = %req.target, arch = %req.arch, %version, size, "app release registered");
    Ok(Json(AppRelease {
        target: req.target,
        arch: req.arch,
        version,
        notes: notes.map(str::to_string),
        size_bytes: size,
        published_at,
    }))
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
        assert!(platform("darwin", "aarch64").is_ok());
        assert!(platform("ios", "aarch64").is_err());
        assert!(platform("windows", "arm64").is_err());
    }
}
