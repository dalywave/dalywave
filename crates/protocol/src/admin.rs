use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::auth::{AuthVia, Device, Role, SessionKind};

/// Пользователь в разделе «Доступ».
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AdminUser {
    pub id: Uuid,
    pub name: String,
    #[schema(required = true)]
    pub email: Option<String>,
    pub email_verified: bool,
    #[schema(required = true)]
    pub avatar_url: Option<String>,
    /// Провайдер входа; `null` у локального владельца, созданного аварийным входом.
    #[schema(required = true)]
    pub issuer: Option<String>,
    pub roles: Vec<Role>,
    /// Роль admin выдана daly.rest и снимется, когда её снимут там.
    pub idp_admin: bool,
    pub disabled: bool,
    pub created_at: DateTime<Utc>,
    #[schema(required = true)]
    pub last_login_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UserList {
    pub items: Vec<AdminUser>,
    #[schema(required = true)]
    pub next_cursor: Option<String>,
}

/// Изменение пользователя: только переданные поля.
#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UserPatch {
    /// Полный новый список ролей.
    #[serde(default)]
    pub roles: Option<Vec<Role>>,
    #[serde(default)]
    pub disabled: Option<bool>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, IntoParams)]
#[serde(rename_all = "camelCase")]
#[into_params(parameter_in = Query)]
pub struct SessionsQuery {
    /// Только сессии этого пользователя.
    pub user_id: Option<Uuid>,
}

/// Открытая сессия: вкладка сайта или приложение.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SessionInfo {
    pub id: Uuid,
    pub user_id: Uuid,
    pub user_name: String,
    pub kind: SessionKind,
    pub via: AuthVia,
    #[schema(required = true)]
    pub device: Option<Device>,
    pub created_at: DateTime<Utc>,
    pub last_seen_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    #[schema(required = true)]
    pub user_agent: Option<String>,
    #[schema(required = true)]
    pub ip: Option<String>,
    /// Это сессия, из которой сделан запрос.
    pub current: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SessionList {
    pub items: Vec<SessionInfo>,
    #[schema(required = true)]
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, IntoParams)]
#[serde(rename_all = "camelCase")]
#[into_params(parameter_in = Query)]
pub struct AuditQuery {
    /// `nextCursor` из предыдущей страницы.
    pub cursor: Option<String>,
    /// Сколько событий вернуть: 1–200, по умолчанию 50.
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AuditActor {
    pub id: Uuid,
    pub name: String,
}

/// Событие журнала: входы, смены ролей, отзывы сессий.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AuditEvent {
    pub id: Uuid,
    pub at: DateTime<Utc>,
    /// Кто сделал; `null` — сам сервер или вход без сессии.
    #[schema(required = true)]
    pub actor: Option<AuditActor>,
    /// Вид события, например `auth.sign_in` или `user.roles_changed`.
    pub kind: String,
    /// Над чем: id пользователя, сессии или устройства.
    #[schema(required = true)]
    pub subject: Option<String>,
    #[schema(value_type = Object)]
    pub payload: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AuditPage {
    pub items: Vec<AuditEvent>,
    #[schema(required = true)]
    pub next_cursor: Option<String>,
}
