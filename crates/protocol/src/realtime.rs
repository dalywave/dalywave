//! Сообщения WebSocket `/api/ws`. JSON-объекты с полем `type`; остальные поля — на одном уровне с ним.
//!
//! Управление между устройствами — сообщения `connect.*` (типы в [`crate::connect`]).
//! Незнакомый `type` клиент должен пропускать, а не считать ошибкой.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    catalog::AudioStatus,
    connect::{
        ConnectAck, ConnectActive, ConnectCommand, ConnectDevices, ConnectEmpty, ConnectHello,
        ConnectPublish, ConnectSend, ConnectState, ConnectTransfer,
    },
    jam::{JamChanged, JamNowEvent, JamReaction},
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
    /// Управление между устройствами: список устройств изменился.
    #[serde(rename = "connect.devices")]
    ConnectDevices(ConnectDevices),
    /// Снимок состояния активного устройства.
    #[serde(rename = "connect.state")]
    ConnectState(ConnectState),
    /// Активным стало другое устройство; при переносе — с состоянием, которое надо продолжить.
    #[serde(rename = "connect.active")]
    ConnectActive(ConnectActive),
    /// Команда этому устройству от пульта.
    #[serde(rename = "connect.command")]
    ConnectCommand(ConnectCommand),
    /// Ответ устройства на команду этого пульта.
    #[serde(rename = "connect.ack")]
    ConnectAck(ConnectAck),
    /// «Слушать вместе»: участники, настройки или треки гостей изменились (хозяину и гостям).
    #[serde(rename = "jam.changed")]
    JamChanged(JamChanged),
    /// «Слушать вместе»: что играет у хозяина — только гостям.
    #[serde(rename = "jam.now")]
    JamNow(JamNowEvent),
    /// «Слушать вместе»: ❤ от участника (хозяину и гостям).
    #[serde(rename = "jam.reaction")]
    JamReaction(JamReaction),
}

/// Сообщение клиента.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(tag = "type")]
pub enum ClientMessage {
    /// Проверка связи на уровне приложения; сервер ответит `pong` с тем же `id`.
    #[serde(rename = "ping")]
    Ping(Ping),
    /// Это соединение — устройство (после каждого подключения).
    #[serde(rename = "connect.hello")]
    ConnectHello(ConnectHello),
    /// «Я на связи» — раз в 5 с; без него 15 с устройство считается пропавшим.
    #[serde(rename = "connect.heartbeat")]
    ConnectHeartbeat(ConnectEmpty),
    /// Снимок состояния; принимается только от активного устройства.
    #[serde(rename = "connect.state")]
    ConnectState(ConnectPublish),
    /// Команда другому устройству.
    #[serde(rename = "connect.command")]
    ConnectCommand(ConnectSend),
    /// Ответ на полученную команду.
    #[serde(rename = "connect.ack")]
    ConnectAck(ConnectAck),
    /// Звук начал играть здесь: это устройство становится активным.
    #[serde(rename = "connect.claim")]
    ConnectClaim(ConnectEmpty),
    /// Перенести воспроизведение на устройство `to`.
    #[serde(rename = "connect.transfer")]
    ConnectTransfer(ConnectTransfer),
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
