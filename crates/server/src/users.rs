//! Люди: профиль из daly.rest, роли и владелец для аварийного входа.
//!
//! Человек ищется по паре (issuer, sub), затем по подтверждённой почте. Сама почта не идентификатор:
//! неподтверждённый адрес никого не связывает и не делает админом.

use chrono::{DateTime, Utc};
use dalywave_protocol::{
    admin::AdminUser,
    auth::{Role, User},
};
use serde_json::json;
use uuid::Uuid;

use crate::{audit, error::AppResult, oidc::Identity, state::AppState};

pub const OWNER_NAME: &str = "Владелец";

pub struct UserRow {
    pub id: Uuid,
    pub issuer: Option<String>,
    pub email: Option<String>,
    pub email_verified: bool,
    pub name: String,
    pub avatar_url: Option<String>,
    pub roles: Vec<String>,
    pub idp_admin: bool,
    pub disabled: bool,
    pub created_at: DateTime<Utc>,
    pub last_login_at: Option<DateTime<Utc>>,
}

impl UserRow {
    pub fn roles(&self) -> Vec<Role> {
        parse_roles(&self.roles)
    }

    pub fn public(&self) -> User {
        User {
            id: self.id,
            name: self.name.clone(),
            email: self.email.clone(),
            avatar_url: self.avatar_url.clone(),
        }
    }

    pub fn admin_view(&self) -> AdminUser {
        AdminUser {
            id: self.id,
            name: self.name.clone(),
            email: self.email.clone(),
            email_verified: self.email_verified,
            avatar_url: self.avatar_url.clone(),
            issuer: self.issuer.clone(),
            roles: self.roles(),
            idp_admin: self.idp_admin,
            disabled: self.disabled,
            created_at: self.created_at,
            last_login_at: self.last_login_at,
        }
    }
}

/// Роли из базы в постоянном порядке. Незнакомые имена пропускаются: их могла оставить будущая версия.
pub fn parse_roles(raw: &[String]) -> Vec<Role> {
    Role::ALL
        .iter()
        .copied()
        .filter(|r| raw.iter().any(|x| x == r.as_str()))
        .collect()
}

pub fn role_names(roles: &[Role]) -> Vec<String> {
    Role::ALL
        .iter()
        .filter(|r| roles.contains(r))
        .map(|r| r.as_str().to_string())
        .collect()
}

/// Роли после входа через daly.rest.
///
/// Админ daly.rest — админ и здесь; флаг `idp_admin` помнит, что роль выдал провайдер. Когда провайдер
/// перестаёт присылать admin, снимается только такая роль: выданная вручную остаётся.
/// Подтверждённые почты из ADMIN_EMAILS получают admin всегда.
pub fn map_roles(
    current: &[Role],
    was_idp_admin: bool,
    upstream_admin: bool,
    env_admin: bool,
) -> (Vec<Role>, bool) {
    let mut roles = current.to_vec();
    let mut idp_admin = was_idp_admin;
    if upstream_admin {
        if !roles.contains(&Role::Admin) {
            roles.push(Role::Admin);
            idp_admin = true;
        }
    } else if was_idp_admin {
        roles.retain(|r| *r != Role::Admin);
        idp_admin = false;
    }
    if env_admin && !roles.contains(&Role::Admin) {
        roles.push(Role::Admin);
        idp_admin = false;
    }
    (parse_roles(&role_names(&roles)), idp_admin)
}

fn clean(value: Option<String>, max: usize) -> Option<String> {
    value
        .map(|v| v.trim().chars().take(max).collect::<String>())
        .filter(|v| !v.is_empty())
}

/// Вход через провайдера: находит или заводит человека, обновляет профиль и роли.
pub async fn sign_in(st: &AppState, ident: Identity) -> AppResult<UserRow> {
    let email = clean(ident.email, 320).filter(|e| e.contains('@'));
    let verified = email.is_some() && ident.email_verified;
    let name = clean(ident.name, 128);
    let avatar = clean(ident.avatar_url, 1000)
        .filter(|u| u.starts_with("https://") || u.starts_with("http://"));
    let upstream_admin = st
        .oidc
        .as_ref()
        .is_some_and(|o| ident.roles.iter().any(|r| o.config.admin_roles.contains(r)));
    let env_admin = verified
        && email
            .as_ref()
            .is_some_and(|e| st.config.admin_emails.contains(&e.to_lowercase()));

    let mut tx = st.db.begin().await?;
    let mut found = sqlx::query_as!(
        UserRow,
        "SELECT id, issuer, email, email_verified, name, avatar_url, roles, idp_admin, disabled, created_at, last_login_at
         FROM users WHERE issuer = $1 AND subject = $2 FOR UPDATE",
        ident.issuer,
        ident.subject,
    )
    .fetch_optional(&mut *tx)
    .await?;
    if found.is_none() && verified {
        // Тот же человек мог войти раньше под другим sub: связываем только по подтверждённой почте
        found = sqlx::query_as!(
            UserRow,
            "SELECT id, issuer, email, email_verified, name, avatar_url, roles, idp_admin, disabled, created_at, last_login_at
             FROM users WHERE lower(email) = lower($1) AND email_verified
             ORDER BY created_at LIMIT 1 FOR UPDATE",
            email,
        )
        .fetch_optional(&mut *tx)
        .await?;
    }
    let found_id = found.as_ref().map(|u| u.id);
    if verified {
        // Чужой неподтверждённый адрес не держит почту: её забирает тот, чей адрес подтверждён
        sqlx::query!(
            "UPDATE users SET email = NULL, email_verified = false, updated_at = now()
             WHERE lower(email) = lower($1) AND NOT email_verified AND id IS DISTINCT FROM $2",
            email,
            found_id,
        )
        .execute(&mut *tx)
        .await?;
    }
    let email_free = match &email {
        Some(e) => sqlx::query_scalar!(
            r#"SELECT NOT EXISTS (SELECT 1 FROM users WHERE lower(email) = lower($1) AND id IS DISTINCT FROM $2) AS "free!""#,
            e,
            found_id,
        )
        .fetch_one(&mut *tx)
        .await?,
        None => false,
    };
    let (email, verified) = match &found {
        // Почта занята другим человеком или провайдер её не прислал: остаётся прежняя
        Some(u) if !email_free => (u.email.clone(), u.email_verified),
        _ if email_free => (email, verified),
        _ => (None, false),
    };

    let (before, was_idp_admin) = found
        .as_ref()
        .map(|u| (u.roles(), u.idp_admin))
        .unwrap_or_default();
    let (roles, idp_admin) = map_roles(&before, was_idp_admin, upstream_admin, env_admin);
    let role_list = role_names(&roles);

    let user = match &found {
        Some(u) => {
            sqlx::query_as!(
                UserRow,
                "UPDATE users SET issuer = $2, subject = $3, email = $4, email_verified = $5,
                    name = COALESCE($6, name), avatar_url = COALESCE($7, avatar_url),
                    roles = $8, idp_admin = $9, last_login_at = now(), updated_at = now()
                 WHERE id = $1
                 RETURNING id, issuer, email, email_verified, name, avatar_url, roles, idp_admin, disabled, created_at, last_login_at",
                u.id,
                ident.issuer,
                ident.subject,
                email,
                verified,
                name,
                avatar,
                &role_list,
                idp_admin,
            )
            .fetch_one(&mut *tx)
            .await?
        }
        None => {
            let name = name.or_else(|| email.as_deref().and_then(|e| e.split('@').next()).map(str::to_string));
            let user = sqlx::query_as!(
                UserRow,
                "INSERT INTO users (issuer, subject, email, email_verified, name, avatar_url, roles, idp_admin, last_login_at)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, now())
                 RETURNING id, issuer, email, email_verified, name, avatar_url, roles, idp_admin, disabled, created_at, last_login_at",
                ident.issuer,
                ident.subject,
                email,
                verified,
                name.unwrap_or_default(),
                avatar,
                &role_list,
                idp_admin,
            )
            .fetch_one(&mut *tx)
            .await?;
            let payload = json!({ "issuer": ident.issuer, "email": user.email, "name": user.name });
            audit::record(&mut *tx, None, audit::USER_CREATED, Some(user.id.to_string()), payload).await?;
            user
        }
    };
    if before != roles {
        let reason = if env_admin && !upstream_admin {
            "admin_emails"
        } else {
            "provider"
        };
        let payload = json!({ "from": role_names(&before), "to": role_list, "reason": reason });
        audit::record(
            &mut *tx,
            None,
            audit::ROLES_CHANGED,
            Some(user.id.to_string()),
            payload,
        )
        .await?;
    }
    tx.commit().await?;
    Ok(user)
}

/// Кем входит аварийный пароль: самый ранний незаблокированный админ. Если админа нет,
/// роль получает локальный владелец (его заводит первый такой вход).
pub async fn owner(st: &AppState) -> AppResult<UserRow> {
    let admin = sqlx::query_as!(
        UserRow,
        "SELECT id, issuer, email, email_verified, name, avatar_url, roles, idp_admin, disabled, created_at, last_login_at
         FROM users WHERE 'admin' = ANY(roles) AND NOT disabled ORDER BY created_at, id LIMIT 1",
    )
    .fetch_optional(&st.db)
    .await?;
    if let Some(user) = admin {
        return Ok(user);
    }

    let mut tx = st.db.begin().await?;
    let local = sqlx::query_as!(
        UserRow,
        "SELECT id, issuer, email, email_verified, name, avatar_url, roles, idp_admin, disabled, created_at, last_login_at
         FROM users WHERE issuer IS NULL ORDER BY created_at, id LIMIT 1 FOR UPDATE",
    )
    .fetch_optional(&mut *tx)
    .await?;
    let user = match local {
        Some(u) => {
            let before = u.roles();
            let mut roles = before.clone();
            roles.push(Role::Admin);
            let role_list = role_names(&roles);
            let user = sqlx::query_as!(
                UserRow,
                "UPDATE users SET roles = $2, disabled = false, updated_at = now() WHERE id = $1
                 RETURNING id, issuer, email, email_verified, name, avatar_url, roles, idp_admin, disabled, created_at, last_login_at",
                u.id,
                &role_list,
            )
            .fetch_one(&mut *tx)
            .await?;
            let payload =
                json!({ "from": role_names(&before), "to": role_list, "reason": "password" });
            audit::record(
                &mut *tx,
                None,
                audit::ROLES_CHANGED,
                Some(user.id.to_string()),
                payload,
            )
            .await?;
            user
        }
        None => {
            let user = sqlx::query_as!(
                UserRow,
                "INSERT INTO users (name, roles) VALUES ($1, '{admin}')
                 RETURNING id, issuer, email, email_verified, name, avatar_url, roles, idp_admin, disabled, created_at, last_login_at",
                OWNER_NAME,
            )
            .fetch_one(&mut *tx)
            .await?;
            let payload = json!({ "name": user.name, "reason": "password" });
            audit::record(
                &mut *tx,
                None,
                audit::USER_CREATED,
                Some(user.id.to_string()),
                payload,
            )
            .await?;
            user
        }
    };
    tx.commit().await?;
    Ok(user)
}

pub async fn get(st: &AppState, id: Uuid) -> AppResult<Option<UserRow>> {
    Ok(sqlx::query_as!(
        UserRow,
        "SELECT id, issuer, email, email_verified, name, avatar_url, roles, idp_admin, disabled, created_at, last_login_at
         FROM users WHERE id = $1",
        id,
    )
    .fetch_optional(&st.db)
    .await?)
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: Role = Role::Admin;
    const L: Role = Role::Listener;

    #[test]
    fn upstream_admin_comes_and_goes() {
        // daly.rest выдал admin: роль появляется и помечается как выданная провайдером
        assert_eq!(map_roles(&[], false, true, false), (vec![A], true));
        assert_eq!(map_roles(&[L], false, true, false), (vec![A, L], true));
        // Провайдер снял admin: снимается и здесь, остальные роли остаются
        assert_eq!(map_roles(&[A, L], true, false, false), (vec![L], false));
        // Повторный вход админа ничего не меняет
        assert_eq!(map_roles(&[A], true, true, false), (vec![A], true));
    }

    #[test]
    fn manual_admin_stays() {
        // admin выдан вручную: провайдер его не снимает
        assert_eq!(map_roles(&[A], false, false, false), (vec![A], false));
        // ...даже если какое-то время сам тоже присылал admin: флаг провайдера так и не появляется
        let (roles, idp) = map_roles(&[A], false, true, false);
        assert_eq!((roles.clone(), idp), (vec![A], false));
        assert_eq!(map_roles(&roles, idp, false, false), (vec![A], false));
    }

    #[test]
    fn admin_emails_always_win() {
        assert_eq!(map_roles(&[], false, false, true), (vec![A], false));
        // Провайдер снял admin, но почта в ADMIN_EMAILS: роль остаётся, уже не как роль провайдера
        assert_eq!(map_roles(&[A], true, false, true), (vec![A], false));
    }

    #[test]
    fn roles_are_parsed_in_order() {
        let raw = vec![
            "listener".to_string(),
            "root".to_string(),
            "admin".to_string(),
            "admin".to_string(),
        ];
        assert_eq!(parse_roles(&raw), vec![A, L]);
        assert_eq!(role_names(&[L, A]), vec!["admin", "listener"]);
    }
}
