//! Кто делает запрос: [`Principal`] из cookie `dw_session` (сайт) или `Authorization: Bearer dwd_…`
//! (приложения). Права — от ролей: admin может всё, listener — слушать и вести медиатеку.
//! Человек без ролей входит, но кроме `/api/auth/*` получает 403 `no_access` («ждём доступа»).

use std::{future::Future, pin::Pin};

use axum::{
    extract::{FromRequestParts, Request},
    http::{HeaderMap, header, request::Parts},
    middleware::Next,
    response::{IntoResponse, Response},
};
use axum_extra::extract::cookie::CookieJar;
use dalywave_protocol::auth::{AuthVia, Permission, Role, SessionKind, User};
use uuid::Uuid;

use crate::{
    error::{AppError, AppResult},
    session::{self, DEVICE_PREFIX, SESSION_COOKIE},
    state::AppState,
    users::parse_roles,
};

/// Права ролей. Роли совмещаются, права складываются. `downloader` роли не дают: он только у токена
/// домашнего загрузчика ([`crate::downloaders`]).
pub fn permissions(roles: &[Role]) -> Vec<Permission> {
    let mut out: Vec<Permission> = roles
        .iter()
        .flat_map(|role| match role {
            Role::Admin => Permission::PEOPLE,
            Role::Listener => &[Permission::Play, Permission::Library],
        })
        .copied()
        .collect();
    out.sort();
    out.dedup();
    out
}

#[derive(Debug, Clone)]
pub struct Principal {
    pub user: User,
    pub roles: Vec<Role>,
    pub session_id: Uuid,
    pub kind: SessionKind,
    pub via: AuthVia,
    pub device_id: Option<Uuid>,
}

impl Principal {
    pub fn permissions(&self) -> Vec<Permission> {
        permissions(&self.roles)
    }

    pub fn can(&self, perm: Permission) -> bool {
        self.permissions().contains(&perm)
    }

    /// 403 `no_access`, пока ролей нет; 403 `forbidden`, если нет именно этого права.
    pub fn need(&self, perm: Permission) -> AppResult<()> {
        if self.roles.is_empty() {
            return Err(AppError::NoAccess);
        }
        if !self.can(perm) {
            return Err(AppError::Forbidden(format!(
                "нужно право «{}»",
                perm.as_str()
            )));
        }
        Ok(())
    }
}

/// Токен из `Authorization`. Схема Bearer нечувствительна к регистру (RFC 6750).
pub fn bearer(headers: &HeaderMap) -> Option<&str> {
    let value = headers.get(header::AUTHORIZATION)?.to_str().ok()?;
    let (scheme, token) = value.split_once(' ')?;
    scheme
        .eq_ignore_ascii_case("bearer")
        .then(|| token.trim())
        .filter(|t| !t.is_empty())
}

/// Сессия по токену из адреса (`?token=` у WebSocket: не все клиенты умеют ставить заголовки).
pub async fn authenticate_token(st: &AppState, token: &str) -> AppResult<Option<Principal>> {
    let kind = if token.starts_with(DEVICE_PREFIX) {
        SessionKind::Device
    } else if token.starts_with(session::WEB_PREFIX) {
        SessionKind::Web
    } else {
        return Err(AppError::Unauthorized);
    };
    let found = session::find(&st.db, token, kind).await?;
    Ok(principal_from(kind, found))
}

/// Сессия запроса. Заблокированный человек — всё равно что без сессии.
pub async fn authenticate(st: &AppState, headers: &HeaderMap) -> AppResult<Option<Principal>> {
    let (kind, found) = if headers.contains_key(header::AUTHORIZATION) {
        // Заголовок есть, но не наш токен: это ошибка клиента, а не повод смотреть в cookie
        let token = bearer(headers)
            .filter(|t| t.starts_with(DEVICE_PREFIX))
            .ok_or(AppError::Unauthorized)?;
        (
            SessionKind::Device,
            session::find(&st.db, token, SessionKind::Device).await?,
        )
    } else {
        let jar = CookieJar::from_headers(headers);
        let found = match jar.get(SESSION_COOKIE) {
            Some(cookie) => session::find(&st.db, cookie.value(), SessionKind::Web).await?,
            None => None,
        };
        (SessionKind::Web, found)
    };
    Ok(principal_from(kind, found))
}

fn principal_from(kind: SessionKind, found: Option<session::Found>) -> Option<Principal> {
    found.filter(|s| !s.disabled).map(|s| {
        let via = match (kind, s.auth_method.as_str()) {
            (SessionKind::Device, _) => AuthVia::Device,
            (_, "password") => AuthVia::Password,
            (_, "local") => AuthVia::Local,
            _ => AuthVia::Oidc,
        };
        Principal {
            user: User {
                id: s.user_id,
                name: s.user_name,
                email: s.email,
                avatar_url: s.avatar_url,
            },
            roles: parse_roles(&s.roles),
            session_id: s.id,
            kind,
            via,
            device_id: s.device_id,
        }
    })
}

impl FromRequestParts<AppState> for Principal {
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, st: &AppState) -> Result<Self, Self::Rejection> {
        // Защитный слой уже нашёл сессию: второй раз в базу не ходим
        if let Some(principal) = parts.extensions.get::<Principal>() {
            return Ok(principal.clone());
        }
        let principal = authenticate(st, &parts.headers)
            .await?
            .ok_or(AppError::Unauthorized)?;
        parts.extensions.insert(principal.clone());
        Ok(principal)
    }
}

/// Слой для разделов приложения: нужен вход и хотя бы одна роль.
pub async fn require_access(principal: Principal, req: Request, next: Next) -> Response {
    if principal.roles.is_empty() {
        return AppError::NoAccess.into_response();
    }
    next.run(req).await
}

type Guarded = Pin<Box<dyn Future<Output = Response> + Send>>;

/// Слой с проверкой права: `.route_layer(from_fn_with_state(state, need(Permission::Admin)))`.
pub fn need(
    perm: Permission,
) -> impl Fn(Principal, Request, Next) -> Guarded + Clone + Send + Sync + 'static {
    move |principal: Principal, req: Request, next: Next| -> Guarded {
        Box::pin(async move {
            match principal.need(perm) {
                Ok(()) => next.run(req).await,
                Err(e) => e.into_response(),
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use axum::http::HeaderValue;

    use super::*;

    #[test]
    fn role_permissions() {
        assert_eq!(permissions(&[]), vec![]);
        assert_eq!(
            permissions(&[Role::Listener]),
            vec![Permission::Play, Permission::Library]
        );
        assert_eq!(
            permissions(&[Role::Listener, Role::Admin]),
            Permission::PEOPLE.to_vec()
        );
        assert!(!permissions(&[Role::Admin]).contains(&Permission::Downloader));
    }

    #[test]
    fn bearer_scheme() {
        let mut h = HeaderMap::new();
        h.insert(
            header::AUTHORIZATION,
            HeaderValue::from_static("bearer dwd_abc"),
        );
        assert_eq!(bearer(&h), Some("dwd_abc"));
        h.insert(header::AUTHORIZATION, HeaderValue::from_static("Basic abc"));
        assert_eq!(bearer(&h), None);
        h.insert(header::AUTHORIZATION, HeaderValue::from_static("Bearer "));
        assert_eq!(bearer(&h), None);
    }
}
