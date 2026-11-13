//! Свои аккаунты: логин и пароль (argon2id), приглашения и одноразовые ссылки сброса пароля.
//! Включаются `DALYWAVE_LOCAL_AUTH` (по умолчанию да). Почты сервер не рассылает: ссылки админ
//! передаёт сам.

use std::sync::OnceLock;

use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use chrono::{DateTime, Utc};
use dalywave_protocol::auth::Role;
use sqlx::{PgExecutor, PgPool};
use uuid::Uuid;

use crate::{
    crypto::{random_bytes, sha256},
    error::{AppError, AppResult},
    users::parse_roles,
};

/// Пароль своего аккаунта — не короче.
pub const MIN_PASSWORD: usize = 10;
/// И не длиннее: argon2 от мегабайта текста — способ загрузить сервер.
pub const MAX_PASSWORD: usize = 256;
/// Срок ссылки сброса пароля.
pub const RESET_HOURS: i64 = 48;
pub const INVITE_DEFAULT_DAYS: u32 = 7;
pub const INVITE_MAX_DAYS: u32 = 30;

pub fn check_password(password: &str) -> AppResult<()> {
    let len = password.chars().count();
    if len < MIN_PASSWORD {
        return Err(AppError::Unprocessable {
            code: "weak_password",
            message: format!("пароль — не короче {MIN_PASSWORD} символов"),
        });
    }
    if len > MAX_PASSWORD {
        return Err(AppError::Unprocessable {
            code: "weak_password",
            message: format!("пароль — не длиннее {MAX_PASSWORD} символов"),
        });
    }
    Ok(())
}

/// Логин: 3–32 символа, буквы, цифры, `.`, `_`, `-`; хранится в нижнем регистре.
pub fn normalize_username(raw: &str) -> AppResult<String> {
    let name = raw.trim().to_lowercase();
    let len = name.chars().count();
    let ok = (3..=32).contains(&len)
        && name
            .chars()
            .all(|c| c.is_alphanumeric() || matches!(c, '.' | '_' | '-'))
        && name.chars().next().is_some_and(char::is_alphanumeric);
    if !ok {
        return Err(AppError::Unprocessable {
            code: "bad_username",
            message: "логин — 3–32 символа: буквы, цифры, точка, _ и -".into(),
        });
    }
    Ok(name)
}

/// Имя для показа: пусто — логин.
pub fn display_name(name: Option<&str>, username: &str) -> String {
    name.map(|n| n.trim().chars().take(128).collect::<String>())
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| username.to_string())
}

fn hash_now(password: &str) -> anyhow::Result<String> {
    let salt = random_bytes::<16>()?;
    let hash = Argon2::default()
        .hash_password_with_salt(password.as_bytes(), &salt)
        .map_err(|e| anyhow::anyhow!("argon2: {e}"))?;
    Ok(hash.to_string())
}

/// Хеш argon2id (параметры по умолчанию: 19 МиБ, 2 прохода). Считается вне потоков запросов.
pub async fn hash_password(password: String) -> AppResult<String> {
    let hash = tokio::task::spawn_blocking(move || hash_now(&password))
        .await
        .map_err(anyhow::Error::from)??;
    Ok(hash)
}

/// Хеш для несуществующего логина: проверка занимает столько же времени, сколько настоящая,
/// и по времени ответа не понять, есть ли такой логин.
fn dummy_hash() -> &'static str {
    static DUMMY: OnceLock<String> = OnceLock::new();
    DUMMY.get_or_init(|| {
        hash_now("dalywave-no-such-user").unwrap_or_else(|_| {
            "$argon2id$v=19$m=19456,t=2,p=1$c2FsdHNhbHRzYWx0$AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"
                .into()
        })
    })
}

/// Сверяет пароль с хешем; без хеша — всё равно тратит то же время и отвечает «нет».
pub async fn verify_password(password: String, hash: Option<String>) -> bool {
    let real = hash.is_some();
    let hash = hash.unwrap_or_else(|| dummy_hash().to_string());
    let ok = tokio::task::spawn_blocking(move || {
        let Ok(parsed) = PasswordHash::new(&hash) else {
            return false;
        };
        Argon2::default()
            .verify_password(password.as_bytes(), &parsed)
            .is_ok()
    })
    .await
    .unwrap_or(false);
    ok && real
}

/// Первый запуск: людей ещё нет.
pub async fn first_run(db: &PgPool) -> AppResult<bool> {
    Ok(
        sqlx::query_scalar!(r#"SELECT NOT EXISTS (SELECT 1 FROM users) AS "empty!""#)
            .fetch_one(db)
            .await?,
    )
}

/// Свой аккаунт для входа.
pub struct LocalUser {
    pub id: Uuid,
    pub password_hash: Option<String>,
    pub disabled: bool,
}

pub async fn find_by_username(db: &PgPool, username: &str) -> AppResult<Option<LocalUser>> {
    Ok(sqlx::query_as!(
        LocalUser,
        "SELECT id, password_hash, disabled FROM users WHERE username = $1",
        username,
    )
    .fetch_optional(db)
    .await?)
}

pub async fn username_taken(
    db: impl PgExecutor<'_>,
    username: &str,
    except: Option<Uuid>,
) -> AppResult<bool> {
    Ok(sqlx::query_scalar!(
        r#"SELECT EXISTS (SELECT 1 FROM users WHERE username = $1 AND id IS DISTINCT FROM $2) AS "taken!""#,
        username,
        except,
    )
    .fetch_one(db)
    .await?)
}

pub fn username_conflict() -> AppError {
    AppError::Conflict {
        code: "username_taken",
        message: "такой логин уже есть".into(),
    }
}

/// Действующее приглашение.
pub struct InviteRow {
    pub id: Uuid,
    pub roles: Vec<Role>,
    pub expires_at: DateTime<Utc>,
}

pub async fn find_invite(db: impl PgExecutor<'_>, token: &str) -> AppResult<Option<InviteRow>> {
    let row = sqlx::query!(
        "SELECT id, roles, expires_at FROM invites
         WHERE token_hash = $1 AND used_at IS NULL AND revoked_at IS NULL AND expires_at > now()",
        &sha256(token)[..],
    )
    .fetch_optional(db)
    .await?;
    Ok(row.map(|r| InviteRow {
        id: r.id,
        roles: parse_roles(&r.roles),
        expires_at: r.expires_at,
    }))
}

/// Гасит приглашение; `false` — его уже использовали.
pub async fn use_invite(db: impl PgExecutor<'_>, id: Uuid, user: Uuid) -> AppResult<bool> {
    let done = sqlx::query!(
        "UPDATE invites SET used_at = now(), used_by = $2 WHERE id = $1 AND used_at IS NULL",
        id,
        user,
    )
    .execute(db)
    .await?;
    Ok(done.rows_affected() > 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usernames() {
        assert_eq!(normalize_username("  Anna.K ").unwrap(), "anna.k");
        assert_eq!(normalize_username("Вася_99").unwrap(), "вася_99");
        assert!(normalize_username("ab").is_err());
        assert!(normalize_username("a b c").is_err());
        assert!(normalize_username("-anna").is_err());
        assert!(normalize_username("anna@example.com").is_err());
        assert!(normalize_username(&"a".repeat(33)).is_err());
    }

    #[test]
    fn password_policy() {
        assert!(check_password("123456789").is_err());
        assert!(check_password("1234567890").is_ok());
        assert!(check_password("пароль-из-10").is_ok());
        assert!(check_password(&"x".repeat(MAX_PASSWORD + 1)).is_err());
    }

    #[tokio::test]
    async fn argon2id_round_trip() {
        let hash = hash_password("correct horse".into()).await.unwrap();
        assert!(hash.starts_with("$argon2id$"), "{hash}");
        assert!(verify_password("correct horse".into(), Some(hash.clone())).await);
        assert!(!verify_password("wrong horse".into(), Some(hash)).await);
        assert!(!verify_password("anything".into(), None).await);
        assert!(!verify_password("x".into(), Some("not a hash".into())).await);
    }
}
