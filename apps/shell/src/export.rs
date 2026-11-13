//! «Экспорт» на компьютере: выгрузка медиатеки в файл. Окно «Сохранить как» — системное (плагин
//! dialog), файл качается с сервера потоком с токеном устройства прямо на диск: большая медиатека не
//! проходит через WebView. Сначала пишется `.part`, готовый файл встаёт на место переименованием.

use std::{path::PathBuf, time::Duration};

use tauri::{AppHandle, Manager};
use tauri_plugin_dialog::DialogExt;
use tokio::io::AsyncWriteExt;

use crate::{
    auth::Auth,
    error::{ShellError, ShellResult},
    settings::Settings,
};

/// Большая медиатека с историей собирается не мгновенно.
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(30 * 60);

fn filter_name(ext: &str) -> &'static str {
    match ext {
        "zip" => "ZIP",
        "csv" => "CSV",
        "m3u8" => "M3U8",
        "json" => "JSON",
        _ => "",
    }
}

/// Сохранить выгрузку `path` (`/api/export/…`) под именем `name`. `null` — окно закрыли.
#[tauri::command]
pub async fn export_save(
    app: AppHandle,
    path: String,
    name: String,
) -> ShellResult<Option<String>> {
    if !path.starts_with("/api/export/") || path.contains("..") {
        return Err(ShellError::Invalid("не адрес выгрузки".into()));
    }
    let ext = name.rsplit_once('.').map(|(_, e)| e.to_string());
    let (tx, rx) = tokio::sync::oneshot::channel();
    let mut dialog = app.dialog().file().set_file_name(&name);
    if let Some(ext) = ext.as_deref().filter(|e| !filter_name(e).is_empty()) {
        dialog = dialog.add_filter(filter_name(ext), &[ext]);
    }
    if let Ok(dir) = app.path().download_dir() {
        dialog = dialog.set_directory(dir);
    }
    dialog.save_file(move |picked| {
        let _ = tx.send(picked);
    });
    let Some(picked) = rx.await.ok().flatten() else {
        return Ok(None);
    };
    let target: PathBuf = picked
        .into_path()
        .map_err(|e| ShellError::Invalid(format!("путь для сохранения: {e}")))?;

    let server = app.state::<Settings>().server_url();
    let token = crate::auth::auth_token(app.clone())
        .await?
        .ok_or_else(|| ShellError::Denied {
            code: "unauthorized".into(),
            message: "нужно войти".into(),
        })?;
    let mut res = app
        .state::<Auth>()
        .http
        .get(format!("{server}{path}"))
        .bearer_auth(token)
        .timeout(DOWNLOAD_TIMEOUT)
        .send()
        .await
        .map_err(|e| ShellError::Network(e.to_string()))?;
    let status = res.status();
    if !status.is_success() {
        let message = res.text().await.unwrap_or_default();
        return Err(ShellError::Server {
            status: status.as_u16(),
            message,
        });
    }
    let part = target.with_extension(format!(
        "{}.part",
        target.extension().and_then(|e| e.to_str()).unwrap_or("")
    ));
    let write = async {
        let mut file = tokio::fs::File::create(&part).await?;
        while let Some(chunk) = res
            .chunk()
            .await
            .map_err(|e| std::io::Error::other(e.to_string()))?
        {
            file.write_all(&chunk).await?;
        }
        file.flush().await?;
        drop(file);
        tokio::fs::rename(&part, &target).await
    };
    if let Err(e) = write.await {
        let _ = tokio::fs::remove_file(&part).await;
        log::warn!("export download failed: {e}");
        return Err(ShellError::Network(e.to_string()));
    }
    log::info!("export saved to {}", target.display());
    Ok(Some(target.display().to_string()))
}
