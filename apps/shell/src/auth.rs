//! Вход приложения через системный браузер.
//!
//! Браузер открывает `/api/auth/app/start` нашего сервера, тот проводит вход через daly.rest и
//! возвращает одноразовый код на `dalywave://auth?code=…&state=…` (в разработке на macOS —
//! на `http://127.0.0.1:<порт>/callback`: ссылки dalywave:// ведут в собранное приложение,
//! а не в `cargo tauri dev`). Оболочка сверяет `state`, меняет код на токен устройства
//! (`POST /api/auth/app/token`, PKCE S256) и кладёт токен в связку ключей ОС.
//! Verifier PKCE не покидает Rust; интерфейс получает пользователя и устройство.

use std::{
    collections::BTreeMap,
    sync::{
        Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Emitter, Manager, async_runtime::JoinHandle};
use tauri_plugin_opener::OpenerExt;
use tokio::sync::oneshot;
use url::Url;

use crate::{
    MAIN,
    error::{ShellError, ShellResult},
    platform,
    settings::Settings,
    window,
};

pub const EVENT_CHANGED: &str = "auth:changed";
pub const EVENT_DEEP_LINK: &str = "deep-link:open";
const KEYCHAIN_SERVICE: &str = "rest.daly.wave";
const REDIRECT_DEEP_LINK: &str = "dalywave://auth";
/// Сервер держит незаконченный вход 10 минут — дольше ждать бессмысленно
const LOGIN_TIMEOUT: Duration = Duration::from_secs(600);

/// Кто вошёл в приложение на этом сервере.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    pub server_url: String,
    /// `User` из API сервера; `null`, если профиль не сохранился локально
    pub user: Value,
    /// `Device` из API сервера; `null`, если профиль не сохранился локально
    pub device: Value,
}

#[derive(Clone, Serialize)]
struct Changed {
    session: Option<Session>,
}

#[derive(Clone, Serialize)]
struct DeepLink {
    url: String,
}

#[derive(Debug)]
enum Callback {
    Code(String),
    Error {
        code: String,
        description: Option<String>,
    },
}

struct Pending {
    id: u64,
    state: String,
    tx: oneshot::Sender<Callback>,
    /// Приёмник на 127.0.0.1 гасится вместе со входом
    listener: Option<JoinHandle<()>>,
}

impl Pending {
    fn close(self) {
        if let Some(listener) = self.listener {
            listener.abort();
        }
    }
}

pub struct Auth {
    pending: Mutex<Option<Pending>>,
    next_id: AtomicU64,
    http: reqwest::Client,
}

impl Auth {
    pub fn new(app_version: &str) -> Self {
        let http = reqwest::Client::builder()
            .user_agent(format!("DalyWave/{app_version} ({})", platform::OS))
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(30))
            .build()
            .unwrap_or_default();
        Self {
            pending: Mutex::new(None),
            next_id: AtomicU64::new(1),
            http,
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Option<Pending>> {
        self.pending.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Новый вход заменяет незаконченный: тот завершится с `cancelled`.
    fn begin(&self, state: String, tx: oneshot::Sender<Callback>) -> u64 {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let previous = self.lock().replace(Pending {
            id,
            state,
            tx,
            listener: None,
        });
        if let Some(previous) = previous {
            previous.close();
        }
        id
    }

    fn attach_listener(&self, id: u64, listener: JoinHandle<()>) {
        match self.lock().as_mut() {
            Some(p) if p.id == id => p.listener = Some(listener),
            _ => listener.abort(),
        }
    }

    fn finish(&self, id: u64) {
        let mut pending = self.lock();
        if pending.as_ref().is_some_and(|p| p.id == id)
            && let Some(p) = pending.take()
        {
            p.close();
        }
    }

    fn cancel(&self) -> bool {
        let pending = self.lock().take();
        pending.map(Pending::close).is_some()
    }

    /// Возврат из браузера: `?code=…&state=…` или `?error=…&state=…`. Чужой или лишний
    /// возврат пропускается: незаконченный вход продолжает ждать свой.
    fn deliver(&self, url: &Url) {
        let params: BTreeMap<String, String> = url.query_pairs().into_owned().collect();
        let Some(state) = params.get("state") else {
            log::warn!("sign-in callback without state ignored");
            return;
        };
        let mut pending = self.lock();
        let Some(current) = pending.as_ref() else {
            log::warn!("sign-in callback ignored: no sign-in is in progress");
            return;
        };
        if !same_secret(state, &current.state) {
            log::warn!("sign-in callback ignored: state does not match");
            return;
        }
        let callback = match (params.get("code"), params.get("error")) {
            (_, Some(error)) => Callback::Error {
                code: error.clone(),
                description: params.get("error_description").cloned(),
            },
            (Some(code), None) => Callback::Code(code.clone()),
            (None, None) => Callback::Error {
                code: "invalid_request".into(),
                description: None,
            },
        };
        if let Some(Pending { tx, listener, .. }) = pending.take() {
            let _ = tx.send(callback);
            if let Some(listener) = listener {
                listener.abort();
            }
        }
    }
}

/// Ссылки `dalywave://…`: возврат входа или (в будущем) ссылки на треки и альбомы.
pub fn handle_urls(app: &AppHandle, urls: Vec<Url>) {
    for url in urls {
        if url.scheme() != "dalywave" {
            continue;
        }
        if url.host_str() == Some("auth") && matches!(url.path(), "" | "/") {
            app.state::<Auth>().deliver(&url);
        } else {
            log::info!("deep link {url}");
            window::show_main(app);
            let _ = app.emit_to(
                MAIN,
                EVENT_DEEP_LINK,
                DeepLink {
                    url: url.to_string(),
                },
            );
        }
    }
}

fn random_token() -> ShellResult<String> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).map_err(ShellError::internal)?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}

fn pkce_challenge(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

/// Сравнение за постоянное время: хеши выравнивают длину.
fn same_secret(a: &str, b: &str) -> bool {
    let (x, y) = (Sha256::digest(a.as_bytes()), Sha256::digest(b.as_bytes()));
    x.iter()
        .zip(y.iter())
        .fold(0u8, |acc, (p, q)| acc | (p ^ q))
        == 0
}

fn use_loopback() -> bool {
    match std::env::var("DALYWAVE_AUTH_REDIRECT").as_deref() {
        Ok("loopback") => true,
        Ok("deeplink") => false,
        _ => cfg!(target_os = "macos") && tauri::is_dev(),
    }
}

/// Имя устройства для списка сессий на сервере, например «MacBook Pro — Захар».
fn device_name() -> String {
    #[cfg(target_os = "macos")]
    {
        let name = std::process::Command::new("/usr/sbin/scutil")
            .args(["--get", "ComputerName"])
            .output()
            .ok()
            .filter(|out| out.status.success())
            .map(|out| String::from_utf8_lossy(&out.stdout).trim().to_string())
            .filter(|name| !name.is_empty());
        name.unwrap_or_else(|| "Mac".into())
    }
    #[cfg(windows)]
    {
        std::env::var("COMPUTERNAME")
            .ok()
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| "Windows".into())
    }
    #[cfg(not(any(target_os = "macos", windows)))]
    {
        "Linux".into()
    }
}

fn start_url(
    server: &str,
    redirect_uri: &str,
    challenge: &str,
    state: &str,
    device_name: &str,
) -> ShellResult<Url> {
    let mut url =
        Url::parse(&format!("{server}/api/auth/app/start")).map_err(ShellError::internal)?;
    url.query_pairs_mut()
        .append_pair("redirect_uri", redirect_uri)
        .append_pair("code_challenge", challenge)
        .append_pair("code_challenge_method", "S256")
        .append_pair("state", state)
        .append_pair("device_name", device_name)
        .append_pair("platform", platform::OS);
    Ok(url)
}

#[derive(Deserialize)]
struct AppToken {
    token: String,
    device: Value,
    user: Value,
}

#[derive(Deserialize)]
struct ErrorBody {
    error: ErrorInfo,
}

#[derive(Deserialize)]
struct ErrorInfo {
    code: String,
    message: String,
}

async fn api_error(res: reqwest::Response) -> ShellError {
    let status = res.status();
    match res.json::<ErrorBody>().await {
        Ok(body) if status.is_client_error() => ShellError::Denied {
            code: body.error.code,
            message: body.error.message,
        },
        Ok(body) => ShellError::Server {
            status: status.as_u16(),
            message: body.error.message,
        },
        Err(_) => ShellError::Server {
            status: status.as_u16(),
            message: status.canonical_reason().unwrap_or("error").into(),
        },
    }
}

async fn exchange(
    http: &reqwest::Client,
    server: &str,
    code: &str,
    verifier: &str,
    app_version: &str,
) -> ShellResult<AppToken> {
    let res = http
        .post(format!("{server}/api/auth/app/token"))
        .json(&json!({ "code": code, "codeVerifier": verifier, "appVersion": app_version }))
        .send()
        .await?;
    let status = res.status();
    if !status.is_success() {
        return Err(api_error(res).await);
    }
    res.json::<AppToken>()
        .await
        .map_err(|e| ShellError::Server {
            status: status.as_u16(),
            message: format!("неожиданный ответ: {e}"),
        })
}

/// Токен устройства в связке ключей: отдельная запись на каждый сервер.
mod keychain {
    use super::KEYCHAIN_SERVICE;
    use crate::error::{ShellError, ShellResult};

    fn entry(server: &str) -> ShellResult<keyring::Entry> {
        Ok(keyring::Entry::new(
            KEYCHAIN_SERVICE,
            &format!("device-token {server}"),
        )?)
    }

    // Связка ключей может показать системный запрос доступа — вызывать вне главного потока
    pub async fn run<T: Send + 'static>(
        f: impl FnOnce() -> ShellResult<T> + Send + 'static,
    ) -> ShellResult<T> {
        tauri::async_runtime::spawn_blocking(f)
            .await
            .map_err(ShellError::internal)?
    }

    pub fn store(server: &str, token: &str) -> ShellResult<()> {
        Ok(entry(server)?.set_password(token)?)
    }

    pub fn load(server: &str) -> ShellResult<Option<String>> {
        match entry(server)?.get_password() {
            Ok(token) => Ok(Some(token)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub fn delete(server: &str) -> ShellResult<()> {
        match entry(server)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(e.into()),
        }
    }
}

/// Пользователь и устройство — без секретов, чтобы интерфейс открывался без сети.
mod sessions {
    use std::{collections::BTreeMap, path::PathBuf};

    use tauri::{AppHandle, Manager};

    use super::Session;

    fn path(app: &AppHandle) -> Option<PathBuf> {
        app.path()
            .app_data_dir()
            .ok()
            .map(|d| d.join("sessions.json"))
    }

    fn read(app: &AppHandle) -> BTreeMap<String, Session> {
        path(app)
            .and_then(|p| std::fs::read(p).ok())
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default()
    }

    fn write(app: &AppHandle, all: &BTreeMap<String, Session>) {
        let Some(path) = path(app) else {
            return;
        };
        let saved = path
            .parent()
            .map_or(Ok(()), std::fs::create_dir_all)
            .and_then(|()| {
                std::fs::write(&path, serde_json::to_vec_pretty(all).unwrap_or_default())
            });
        if let Err(e) = saved {
            log::warn!("could not save the session profile: {e}");
        }
    }

    pub fn get(app: &AppHandle, server: &str) -> Option<Session> {
        read(app).remove(server)
    }

    pub fn save(app: &AppHandle, session: &Session) {
        let mut all = read(app);
        all.insert(session.server_url.clone(), session.clone());
        write(app, &all);
    }

    pub fn remove(app: &AppHandle, server: &str) {
        let mut all = read(app);
        if all.remove(server).is_some() {
            write(app, &all);
        }
    }
}

/// Приёмник возврата на 127.0.0.1 для разработки (RFC 8252, 7.3).
mod loopback {
    use std::time::Duration;

    use tauri::{AppHandle, Manager};
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
    };
    use url::Url;

    use super::Auth;
    use crate::error::{ShellError, ShellResult};

    const PAGE: &str = "<!doctype html><html lang=\"ru\"><meta charset=\"utf-8\"><title>DalyWave</title>\
<body style=\"margin:0;height:100vh;display:grid;place-items:center;background:#08090c;color:#f3f5f9;\
font:16px/1.5 system-ui,sans-serif\"><p>Можно вернуться в DalyWave — эту вкладку можно закрыть.</p></body></html>";

    pub async fn bind() -> ShellResult<(String, u16, TcpListener)> {
        let listener = TcpListener::bind(("127.0.0.1", 0))
            .await
            .map_err(ShellError::internal)?;
        let port = listener.local_addr().map_err(ShellError::internal)?.port();
        Ok((format!("http://127.0.0.1:{port}/callback"), port, listener))
    }

    pub async fn serve(listener: TcpListener, port: u16, app: AppHandle) {
        loop {
            let Ok((mut stream, _)) = listener.accept().await else {
                return;
            };
            let mut buf = vec![0u8; 8192];
            let mut len = 0;
            let read = tokio::time::timeout(Duration::from_secs(5), async {
                while len < buf.len() {
                    let n = stream.read(&mut buf[len..]).await?;
                    if n == 0 {
                        break;
                    }
                    len += n;
                    if buf[..len].windows(4).any(|w| w == b"\r\n\r\n") {
                        break;
                    }
                }
                Ok::<_, std::io::Error>(())
            })
            .await;
            if !matches!(read, Ok(Ok(()))) {
                continue;
            }
            let head = String::from_utf8_lossy(&buf[..len]);
            let target = head.lines().next().and_then(|line| {
                let mut parts = line.split(' ');
                (parts.next() == Some("GET"))
                    .then(|| parts.next())
                    .flatten()
            });
            let url = target
                .and_then(|t| Url::parse(&format!("http://127.0.0.1:{port}{t}")).ok())
                .filter(|u| u.path() == "/callback");
            let Some(url) = url else {
                let _ = stream
                    .write_all(
                        b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                    )
                    .await;
                continue;
            };
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\n\
                 Cache-Control: no-store\r\nConnection: close\r\n\r\n{PAGE}",
                PAGE.len()
            );
            let _ = stream.write_all(response.as_bytes()).await;
            let _ = stream.shutdown().await;
            app.state::<Auth>().deliver(&url);
            return;
        }
    }
}

/// Войти: открыть браузер, дождаться возврата, получить токен устройства.
#[tauri::command]
pub async fn auth_login(app: AppHandle) -> ShellResult<Session> {
    let auth = app.state::<Auth>();
    let server = app.state::<Settings>().server_url();
    let verifier = random_token()?;
    let state = random_token()?;

    let loopback = if use_loopback() {
        Some(loopback::bind().await?)
    } else {
        None
    };
    let redirect_uri = loopback
        .as_ref()
        .map_or_else(|| REDIRECT_DEEP_LINK.to_string(), |(uri, ..)| uri.clone());
    let start = start_url(
        &server,
        &redirect_uri,
        &pkce_challenge(&verifier),
        &state,
        &device_name(),
    )?;

    let (tx, rx) = oneshot::channel();
    let id = auth.begin(state, tx);
    if let Some((_, port, listener)) = loopback {
        let task = tauri::async_runtime::spawn(loopback::serve(listener, port, app.clone()));
        auth.attach_listener(id, task);
    }
    // Проверки без браузера (только отладочная сборка): адрес уходит в журнал, браузер
    // изображает сценарий проверки
    if cfg!(debug_assertions) && std::env::var("DALYWAVE_AUTH_OPEN").as_deref() == Ok("print") {
        log::info!("sign-in start URL (not opened): {start}");
    } else if let Err(e) = app.opener().open_url(start.as_str(), None::<&str>) {
        auth.finish(id);
        return Err(ShellError::internal(format!("не открылся браузер: {e}")));
    }
    log::info!("sign-in started in the system browser, callback {redirect_uri}");

    let outcome = tokio::time::timeout(LOGIN_TIMEOUT, rx).await;
    auth.finish(id);
    let code = match outcome {
        Err(_) => return Err(ShellError::Timeout),
        Ok(Err(_)) => return Err(ShellError::Cancelled),
        Ok(Ok(Callback::Code(code))) => code,
        Ok(Ok(Callback::Error { code, description })) => {
            log::warn!("sign-in failed: {code}");
            return Err(match code.as_str() {
                "cancelled" | "access_denied" => ShellError::Cancelled,
                _ => ShellError::Denied {
                    message: description.unwrap_or_else(|| format!("вход не удался ({code})")),
                    code,
                },
            });
        }
    };
    // Браузер остался впереди — возвращаем окно приложения
    window::show_main(&app);

    let version = app.package_info().version.to_string();
    let token = exchange(&auth.http, &server, &code, &verifier, &version).await?;
    let (key_server, key_token) = (server.clone(), token.token);
    keychain::run(move || keychain::store(&key_server, &key_token)).await?;

    let session = Session {
        server_url: server,
        user: token.user,
        device: token.device,
    };
    sessions::save(&app, &session);
    let _ = app.emit_to(
        MAIN,
        EVENT_CHANGED,
        Changed {
            session: Some(session.clone()),
        },
    );
    log::info!("signed in, device token stored in the keychain");
    Ok(session)
}

/// Прервать незаконченный вход: `auth_login` завершится с `cancelled`.
#[tauri::command]
pub fn auth_cancel(app: AppHandle) -> bool {
    app.state::<Auth>().cancel()
}

/// Токен устройства для `Authorization: Bearer` или `null`, если вход не выполнен.
#[tauri::command]
pub async fn auth_token(app: AppHandle) -> ShellResult<Option<String>> {
    let server = app.state::<Settings>().server_url();
    keychain::run(move || keychain::load(&server)).await
}

/// Сохранённый вход на текущем сервере — без сети. `null`, если токена нет.
#[tauri::command]
pub async fn auth_session(app: AppHandle) -> ShellResult<Option<Session>> {
    let server = app.state::<Settings>().server_url();
    let key_server = server.clone();
    let token = keychain::run(move || keychain::load(&key_server)).await?;
    Ok(token.map(|_| {
        sessions::get(&app, &server).unwrap_or(Session {
            server_url: server,
            user: Value::Null,
            device: Value::Null,
        })
    }))
}

/// Выйти: отозвать сессию устройства на сервере (если он доступен) и стереть токен.
#[tauri::command]
pub async fn auth_logout(app: AppHandle) -> ShellResult<()> {
    let server = app.state::<Settings>().server_url();
    let key_server = server.clone();
    let token = keychain::run(move || keychain::load(&key_server)).await?;
    if let Some(token) = token {
        let revoked = app
            .state::<Auth>()
            .http
            .post(format!("{server}/api/auth/logout"))
            .bearer_auth(token)
            .timeout(Duration::from_secs(5))
            .send()
            .await;
        match revoked {
            Ok(res) if res.status().is_success() => log::info!("device session revoked"),
            Ok(res) => log::warn!("server refused to revoke the session: {}", res.status()),
            Err(e) => log::warn!("could not reach the server to revoke the session: {e}"),
        }
    }
    let key_server = server.clone();
    keychain::run(move || keychain::delete(&key_server)).await?;
    sessions::remove(&app, &server);
    let _ = app.emit_to(MAIN, EVENT_CHANGED, Changed { session: None });
    log::info!("signed out, device token removed from the keychain");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pkce_matches_rfc_7636_example() {
        // Приложение B к RFC 7636
        assert_eq!(
            pkce_challenge("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
    }

    #[test]
    fn tokens_fit_pkce_rules() {
        let token = random_token().unwrap();
        assert_eq!(token.len(), 43);
        assert!(
            token
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        );
        assert_ne!(token, random_token().unwrap());
    }

    #[test]
    fn start_url_has_all_parameters() {
        let url = start_url(
            "https://wave.daly.rest",
            REDIRECT_DEEP_LINK,
            "challenge",
            "st",
            "MacBook Захара",
        )
        .unwrap();
        let q: BTreeMap<String, String> = url.query_pairs().into_owned().collect();
        assert_eq!(url.path(), "/api/auth/app/start");
        assert_eq!(q["redirect_uri"], "dalywave://auth");
        assert_eq!(q["code_challenge_method"], "S256");
        assert_eq!(q["state"], "st");
        assert_eq!(q["device_name"], "MacBook Захара");
        assert_eq!(q["platform"], platform::OS);
    }

    #[test]
    fn callback_with_matching_state_is_delivered() {
        let auth = Auth::new("0.0.0");
        let (tx, mut rx) = oneshot::channel();
        auth.begin("good".into(), tx);
        auth.deliver(&Url::parse("dalywave://auth?code=c1&state=bad").unwrap());
        assert!(rx.try_recv().is_err(), "чужой state не принимается");
        auth.deliver(&Url::parse("dalywave://auth?code=c1&state=good").unwrap());
        assert!(matches!(rx.try_recv(), Ok(Callback::Code(c)) if c == "c1"));
        assert!(auth.lock().is_none());
    }

    #[test]
    fn new_login_cancels_the_previous_one() {
        let auth = Auth::new("0.0.0");
        let (tx1, mut rx1) = oneshot::channel();
        auth.begin("a".into(), tx1);
        let (tx2, _rx2) = oneshot::channel();
        auth.begin("b".into(), tx2);
        assert!(matches!(
            rx1.try_recv(),
            Err(oneshot::error::TryRecvError::Closed)
        ));
    }
}
