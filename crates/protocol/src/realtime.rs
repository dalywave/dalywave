//! Сообщения WebSocket `/api/ws`. JSON-объекты с полем `type`; остальные поля — на одном уровне с ним.
//!
//! Этап 3 добавит сюда управление плеером и список устройств — новыми вариантами тех же перечислений.
//! Незнакомый `type` клиент должен пропускать, а не считать ошибкой.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    catalog::AudioStatus,
    jobs::{AcquisitionStatus, JobStatus},
    sync::SyncEntity,
};

/// Сообщение сервера.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(tag = "type")]
pub enum ServerEvent {
    /// Первое сообщение после подключения.
    #[serde(rename = "hello")]
    Hello(Hello),
    /// Ответ на `ping` клиента.
    #[serde(rename = "pong")]
    Pong(Pong),
    /// Сменился шаг скачивания записи (и, возможно, состояние её аудио).
    #[serde(rename = "acquisition.updated")]
    AcquisitionUpdated(AcquisitionUpdate),
    /// Задача очереди сменила состояние. Только администраторам.
    #[serde(rename = "job.updated")]
    JobUpdated(JobUpdate),
    /// Медиатека изменилась (возможно, с другого устройства): пора запросить `/api/sync`.
    #[serde(rename = "library.changed")]
    LibraryChanged(LibraryChange),
    /// Часть событий потеряна: перечитать то, что на экране, и запросить `/api/sync`.
    #[serde(rename = "resync")]
    Resync(Resync),
}

/// Сообщение клиента.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(tag = "type")]
pub enum ClientMessage {
    /// Проверка связи на уровне приложения; сервер ответит `pong` с тем же `id`.
    #[serde(rename = "ping")]
    Ping(Ping),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Hello {
    pub session_id: Uuid,
    pub user_id: Uuid,
    pub server_time: DateTime<Utc>,
    /// Как часто сервер шлёт ping-кадры WebSocket. Без ответа за два интервала соединение закрывается.
    pub heartbeat_ms: u32,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Ping {
    #[serde(default)]
    pub id: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Pong {
    #[schema(required = true)]
    pub id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AcquisitionUpdate {
    pub id: Uuid,
    pub recording_id: Uuid,
    pub track_id: Uuid,
    pub status: AcquisitionStatus,
    pub audio_status: AudioStatus,
    #[schema(required = true)]
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct JobUpdate {
    pub id: Uuid,
    pub kind: String,
    pub lane: String,
    pub status: JobStatus,
    pub attempts: i16,
    pub max_attempts: i16,
    #[schema(required = true)]
    pub last_error: Option<String>,
    pub run_after: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct LibraryChange {
    /// Какие таблицы локальной базы затронуты.
    pub entities: Vec<SyncEntity>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Resync {
    /// Почему: `lagged` — клиент не успевал читать события.
    pub reason: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn messages_are_tagged() {
        let event = ServerEvent::LibraryChanged(LibraryChange {
            entities: vec![SyncEntity::LikedTracks],
        });
        assert_eq!(
            serde_json::to_value(&event).unwrap(),
            serde_json::json!({ "type": "library.changed", "entities": ["likedTracks"] })
        );
        let ping: ClientMessage = serde_json::from_str(r#"{"type":"ping"}"#).unwrap();
        assert_eq!(ping, ClientMessage::Ping(Ping { id: None }));
        let ping: ClientMessage = serde_json::from_str(r#"{"type":"ping","id":"7"}"#).unwrap();
        assert_eq!(
            ping,
            ClientMessage::Ping(Ping {
                id: Some("7".into())
            })
        );
        assert!(serde_json::from_str::<ClientMessage>(r#"{"type":"dance"}"#).is_err());
    }
}
