//! Описание API для `/api/openapi.json`. Из него `make api-types` собирает типы интерфейса,
//! поэтому новый обработчик добавляется и сюда.

use utoipa::{
    Modify, OpenApi,
    openapi::security::{ApiKey, ApiKeyValue, HttpAuthScheme, HttpBuilder, SecurityScheme},
};

use crate::routes::{admin, auth, system};

#[derive(OpenApi)]
#[openapi(
    info(title = "DalyWave API", description = "Личный музыкальный сервис DalyWave"),
    paths(
        system::healthz,
        system::openapi,
        auth::providers,
        auth::oidc_start,
        auth::oidc_callback,
        auth::password_login,
        auth::logout,
        auth::me,
        auth::app_start,
        auth::app_token,
        admin::list_users,
        admin::update_user,
        admin::list_sessions,
        admin::revoke_session,
        admin::audit_log,
    ),
    components(schemas(dalywave_protocol::ErrorBody)),
    modifiers(&Security),
    tags(
        (name = "auth", description = "Вход и сессии"),
        (name = "admin", description = "Админка: доступ и журнал"),
        (name = "system", description = "Служебное"),
    ),
)]
pub struct ApiDoc;

struct Security;

impl Modify for Security {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        // Лицензии у личного проекта нет: пустое поле из Cargo.toml только сбивает генераторы
        openapi.info.license = None;
        let components = openapi.components.get_or_insert_with(Default::default);
        components.add_security_scheme(
            "session",
            SecurityScheme::ApiKey(ApiKey::Cookie(ApiKeyValue::with_description(
                crate::session::SESSION_COOKIE,
                "Сессия сайта (HttpOnly cookie)",
            ))),
        );
        components.add_security_scheme(
            "device",
            SecurityScheme::Http(
                HttpBuilder::new()
                    .scheme(HttpAuthScheme::Bearer)
                    .description(Some("Токен приложения `dwd_…`"))
                    .build(),
            ),
        );
    }
}
