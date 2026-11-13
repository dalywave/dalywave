//! Приложения для компьютера с сайта: меню аккаунта → «Приложения для компьютера».
//!
//! Установщики кладёт конвейер выпусков вместе с файлами автообновления ([`super::updates`],
//! `installerFileName`). Скачивать могут только свои — любой человек с ролью (слой
//! `require_access`), как и весь каталог. `GET /api/downloads/{platform}/latest` отвечает
//! редиректом на подписанную ссылку хранилища: файл в сотни мегабайт идёт мимо сервера, а имя
//! файла у человека — `DalyWave-0.2.0.dmg`, а не ключ из хранилища.

use std::{collections::HashMap, time::Duration};

use axum::{
    Router,
    extract::State,
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};
use chrono::{DateTime, Utc};
use dalywave_protocol::ErrorBody;
use semver::Version;
use serde::Serialize;
use utoipa::ToSchema;

use super::updates::storage_unavailable;
use crate::{
    error::{AppError, AppResult},
    extract::{Json, Path},
    state::AppState,
};

/// Ссылку открывают сразу после редиректа; хранилище проверяет срок только в начале скачивания,
/// поэтому медленная загрузка не обрывается.
pub const LINK_TTL: Duration = Duration::from_secs(15 * 60);

/// Платформа на сайте: имя в адресе, платформа и архитектура выпуска (как у плагина обновлений),
/// система и её минимальная версия.
struct Platform {
    slug: &'static str,
    target: &'static str,
    arch: &'static str,
    os: &'static str,
    /// macOS — `bundle.macOS.minimumSystemVersion` в apps/shell/tauri.macos.conf.json; Windows —
    /// WebView2 и Tauri 2 работают с Windows 10
    min_os: &'static str,
    /// Добавка к имени файла: Intel-сборку не спутать с Apple silicon
    suffix: &'static str,
}

const MAC_MIN_OS: &str = "13";

const PLATFORMS: [Platform; 3] = [
    Platform {
        slug: "mac",
        target: "darwin",
        arch: "aarch64",
        os: "macos",
        min_os: MAC_MIN_OS,
        suffix: "",
    },
    Platform {
        slug: "mac-intel",
        target: "darwin",
        arch: "x86_64",
        os: "macos",
        min_os: MAC_MIN_OS,
        suffix: "-Intel",
    },
    Platform {
        slug: "windows",
        target: "windows",
        arch: "x86_64",
        os: "windows",
        min_os: "10",
        suffix: "",
    },
];

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/downloads", get(desktop_apps))
        .route("/downloads/{platform}/latest", get(desktop_app_latest))
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AppDownload {
    /// `mac` (Apple silicon), `mac-intel`, `windows`
    pub platform: String,
    /// `macos` или `windows`
    pub os: String,
    /// `aarch64` или `x86_64`
    pub arch: String,
    pub version: String,
    pub published_at: DateTime<Utc>,
    /// Размер установщика
    pub size_bytes: i64,
    /// Под каким именем файл сохранится, например `DalyWave-0.2.0.dmg`
    pub file_name: String,
    /// Минимальная версия системы: `13` — macOS 13, `10` — Windows 10
    pub min_os_version: String,
    /// Скачать: редирект на файл
    pub url: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct Downloads {
    /// Платформы, для которых есть установщик; порядок — Mac, Mac Intel, Windows
    pub apps: Vec<AppDownload>,
}

struct Installer {
    version: Version,
    key: String,
    size: i64,
    published_at: DateTime<Utc>,
}

/// Самый новый выпуск с установщиком у каждой пары (платформа, архитектура).
async fn installers(st: &AppState) -> AppResult<HashMap<(String, String), Installer>> {
    let rows = sqlx::query!(
        r#"SELECT target, arch, version, installer_key AS "key!", installer_size AS "size!", published_at
           FROM app_releases WHERE installer_key IS NOT NULL"#,
    )
    .fetch_all(&st.db)
    .await?;
    let mut out: HashMap<(String, String), Installer> = HashMap::new();
    for r in rows {
        // Порядок SemVer строками в SQL не выразить; кривую версию сервер принять не мог
        let Ok(version) = Version::parse(&r.version) else {
            continue;
        };
        let slot = (r.target, r.arch);
        if out.get(&slot).is_some_and(|cur| cur.version >= version) {
            continue;
        }
        out.insert(
            slot,
            Installer {
                version,
                key: r.key,
                size: r.size,
                published_at: r.published_at,
            },
        );
    }
    Ok(out)
}

/// Имя файла у человека: `DalyWave-0.2.0.dmg`, `DalyWave-0.2.0-Intel.dmg`, `DalyWave-0.2.0-Setup.exe`.
fn download_name(p: &Platform, version: &Version, key: &str) -> String {
    let ext = key
        .rsplit_once('.')
        .map(|(_, e)| e.to_ascii_lowercase())
        .unwrap_or_default();
    let setup = if ext == "exe" { "-Setup" } else { "" };
    format!("DalyWave-{version}{}{setup}.{ext}", p.suffix)
}

#[utoipa::path(
    get,
    path = "/api/downloads",
    tag = "system",
    responses(
        (status = 200, description = "Последние установщики приложений для компьютера", body = Downloads),
        (status = 401, description = "Нужен вход", body = ErrorBody),
        (status = 403, description = "`no_access` — ролей ещё нет", body = ErrorBody),
    ),
)]
pub async fn desktop_apps(State(st): State<AppState>) -> AppResult<Json<Downloads>> {
    let mut found = installers(&st).await?;
    let apps = PLATFORMS
        .iter()
        .filter_map(|p| {
            let i = found.remove(&(p.target.to_string(), p.arch.to_string()))?;
            Some(AppDownload {
                platform: p.slug.into(),
                os: p.os.into(),
                arch: p.arch.into(),
                file_name: download_name(p, &i.version, &i.key),
                version: i.version.to_string(),
                published_at: i.published_at,
                size_bytes: i.size,
                min_os_version: p.min_os.into(),
                url: format!("/api/downloads/{}/latest", p.slug),
            })
        })
        .collect();
    Ok(Json(Downloads { apps }))
}

#[utoipa::path(
    get,
    path = "/api/downloads/{platform}/latest",
    tag = "system",
    params(("platform" = String, Path, description = "`mac`, `mac-intel` или `windows`")),
    responses(
        (status = 302, description = "Редирект на установщик в хранилище: ссылка живёт 15 минут, файл сохранится как `DalyWave-<версия>.dmg`"),
        (status = 401, description = "Нужен вход", body = ErrorBody),
        (status = 403, description = "`no_access` — ролей ещё нет", body = ErrorBody),
        (status = 404, description = "Неизвестная платформа или установщика ещё нет", body = ErrorBody),
    ),
)]
pub async fn desktop_app_latest(
    State(st): State<AppState>,
    Path(platform): Path<String>,
) -> AppResult<Response> {
    let p = PLATFORMS
        .iter()
        .find(|p| p.slug == platform)
        .ok_or(AppError::NotFound)?;
    let i = installers(&st)
        .await?
        .remove(&(p.target.to_string(), p.arch.to_string()))
        .ok_or(AppError::NotFound)?;
    let name = download_name(p, &i.version, &i.key);
    let url = st
        .storage
        .presign_download(&i.key, LINK_TTL, &name)
        .await
        .map_err(storage_unavailable)?;
    // Подписанная ссылка со сроком: кешировать редирект нельзя
    Ok((
        StatusCode::FOUND,
        [
            (header::LOCATION, url.to_string()),
            (header::CACHE_CONTROL, "no-store".to_string()),
        ],
    )
        .into_response())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names() {
        let v = Version::parse("0.2.0").unwrap();
        let [mac, intel, win] = &PLATFORMS;
        let key = "releases/darwin/aarch64/0.2.0/DalyWave_0.2.0_aarch64.dmg";
        assert_eq!(download_name(mac, &v, key), "DalyWave-0.2.0.dmg");
        assert_eq!(download_name(intel, &v, key), "DalyWave-0.2.0-Intel.dmg");
        let key = "releases/windows/x86_64/0.2.0/DalyWave_0.2.0_x64-setup.exe";
        assert_eq!(download_name(win, &v, key), "DalyWave-0.2.0-Setup.exe");
        assert_eq!(download_name(win, &v, "a/b.MSI"), "DalyWave-0.2.0.msi");
    }

    /// Минимальная macOS на сайте — та же, что в сборке
    #[test]
    fn mac_minimum_matches_the_bundle() {
        let conf = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../apps/shell/tauri.macos.conf.json"
        );
        let conf: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(conf).unwrap()).unwrap();
        let min = conf["bundle"]["macOS"]["minimumSystemVersion"]
            .as_str()
            .unwrap();
        assert_eq!(min.split('.').next(), Some(MAC_MIN_OS), "{min}");
    }
}
