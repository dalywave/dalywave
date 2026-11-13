//! Журнал админки: входы, смены ролей, блокировки, отзывы сессий, действия с очередью.
//! Читается через `/api/admin/audit`.

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
pub const USER_RENAMED: &str = "user.renamed";
/// Аккаунт удалён со всеми данными
pub const USER_DELETED: &str = "user.deleted";
/// Все сессии человека отозваны («выйти везде»)
pub const SESSIONS_REVOKED: &str = "user.sessions_revoked";
/// Вход через провайдера привязан к уже известному человеку (настройки, подтверждённая почта)
pub const IDENTITY_LINKED: &str = "user.identity_linked";
pub const IDENTITY_UNLINKED: &str = "user.identity_unlinked";
/// Свой аккаунт: пароль задан или сменён (сам человек, по ссылке сброса)
pub const PASSWORD_CHANGED: &str = "user.password_changed";
/// Админ выдал одноразовую ссылку сброса пароля
pub const PASSWORD_RESET_LINK: &str = "user.password_reset_link";
pub const INVITE_CREATED: &str = "invite.created";
pub const INVITE_USED: &str = "invite.used";
pub const INVITE_REVOKED: &str = "invite.revoked";
pub const SESSION_REVOKED: &str = "session.revoked";
pub const JOB_RETRIED: &str = "job.retried";
pub const JOB_CANCELLED: &str = "job.cancelled";
pub const LANE_CHANGED: &str = "lane.changed";
pub const DOWNLOADER_CREATED: &str = "downloader.created";
pub const DOWNLOADER_REVOKED: &str = "downloader.revoked";
/// Решение по спорной записи: выбрано видео, вставлена ссылка, «нет в источниках», повтор
pub const ACQUISITION_RESOLVED: &str = "acquisition.resolved";
/// Настройки разведчика и рекомендаций изменены
pub const RECO_SETTINGS_CHANGED: &str = "reco.settings_changed";
/// Задачи рекомендаций запущены вне расписания: разведчик, пересчёт вкуса, анализ скачанного
pub const RECO_RUN: &str = "reco.run";
/// Настройки сервера изменены: политика подписок по умолчанию, часовой пояс
pub const SETTINGS_CHANGED: &str = "settings.changed";
/// Запись поставлена на скачивание заново из «Каталога»: поиском или по ссылке
pub const RECORDING_REACQUIRE: &str = "recording.reacquire";
/// «Проверить скачивание» в «Источниках»
pub const SOURCE_CHECK: &str = "source.check";
/// Слушатель сбросил свой вкус («Мой вкус» → «Сбросить вкус»)
pub const TASTE_RESET: &str = "taste.reset";
/// Слушатель отметил основной файл записи как «Неверная запись»
pub const RECORDING_REPORTED: &str = "recording.reported";

/// Клип трека выбран администратором: ролик, ссылка, «без клипа», «заново», «оставить»
pub const CLIP_CHOSEN: &str = "clip.chosen";

/// Подхват на другом устройстве: музыка перенесена или предложена (причина — в payload)
pub const HANDOFF: &str = "connect.handoff";
/// Ссылка на оверлей для стрима создана заново (прежняя перестала работать) или выключена
pub const OVERLAY_LINK: &str = "overlay.link";

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
