//! Журнал админки: входы, смены ролей, блокировки, отзывы сессий. Читается через `/api/admin/audit`.

use serde_json::Value;
use sqlx::PgExecutor;
use uuid::Uuid;

pub const SIGN_IN: &str = "auth.sign_in";
pub const SIGN_IN_FAILED: &str = "auth.sign_in_failed";
/// Вход прошёл у провайдера, но аккаунт заблокирован здесь
pub const SIGN_IN_DENIED: &str = "auth.sign_in_denied";
pub const LOGOUT: &str = "auth.logout";
/// Код приложения предъявили второй раз: выданная по нему сессия отозвана
pub const APP_CODE_REUSED: &str = "auth.app_code_reused";
pub const USER_CREATED: &str = "user.created";
pub const ROLES_CHANGED: &str = "user.roles_changed";
pub const USER_DISABLED: &str = "user.disabled";
pub const USER_ENABLED: &str = "user.enabled";
pub const SESSION_REVOKED: &str = "session.revoked";

pub async fn record(
    db: impl PgExecutor<'_>,
    actor: Option<Uuid>,
    kind: &str,
    subject: Option<String>,
    payload: Value,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "INSERT INTO audit_events (actor_user_id, kind, subject, payload) VALUES ($1, $2, $3, $4)",
        actor,
        kind,
        subject,
        payload,
    )
    .execute(db)
    .await?;
    Ok(())
}
