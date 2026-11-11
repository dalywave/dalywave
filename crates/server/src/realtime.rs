//! События в реальном времени. API и воркер — разные процессы, поэтому событие уходит в Postgres
//! `NOTIFY dalywave_events`, а API слушает канал и раздаёт его открытым WebSocket через broadcast.
//! В транзакции NOTIFY доставляется при коммите: клиент не увидит событие раньше данных.

use std::{sync::Arc, time::Duration};

use dalywave_protocol::realtime::{Resync, ServerEvent};
use serde::{Deserialize, Serialize};
use sqlx::{PgExecutor, PgPool, postgres::PgListener};
use tokio::{
    sync::{broadcast, watch},
    task::JoinHandle,
};
use uuid::Uuid;

pub const CHANNEL: &str = "dalywave_events";
/// Postgres ограничивает полезную нагрузку NOTIFY 8000 байтами
const MAX_PAYLOAD: usize = 7900;
/// Сколько событий держит broadcast, пока медленный клиент их не прочёл
const BUFFER: usize = 1024;

/// Кому событие.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Audience {
    User(Uuid),
    /// Людям с правом admin.
    Admins,
    All,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Envelope {
    pub to: Audience,
    pub event: ServerEvent,
}

/// Отправляет событие через Postgres. Не мешает основной работе: ошибка только пишется в лог.
pub async fn publish(db: impl PgExecutor<'_>, to: Audience, event: ServerEvent) {
    let envelope = Envelope { to, event };
    let payload = match serde_json::to_string(&envelope) {
        Ok(p) if p.len() <= MAX_PAYLOAD => p,
        Ok(p) => {
            tracing::warn!(len = p.len(), "realtime event is too large, dropped");
            return;
        }
        Err(e) => {
            tracing::error!(error = %e, "could not serialize a realtime event");
            return;
        }
    };
    // pg_notify возвращает void: макросы sqlx такой тип не описывают, поэтому запрос без проверки
    if let Err(e) = sqlx::query("SELECT pg_notify($1, $2)")
        .bind(CHANNEL)
        .bind(payload)
        .execute(db)
        .await
    {
        tracing::warn!(error = %e, "could not publish a realtime event");
    }
}

/// Раздаёт события открытым соединениям этого процесса.
pub struct Hub {
    tx: broadcast::Sender<Arc<Envelope>>,
    stop: watch::Sender<bool>,
    /// Слушатель Postgres подписан на канал
    listening: watch::Sender<bool>,
}

impl Default for Hub {
    fn default() -> Self {
        let (tx, _) = broadcast::channel(BUFFER);
        let (stop, _) = watch::channel(false);
        let (listening, _) = watch::channel(false);
        Self {
            tx,
            stop,
            listening,
        }
    }
}

impl Hub {
    pub fn subscribe(&self) -> broadcast::Receiver<Arc<Envelope>> {
        self.tx.subscribe()
    }

    pub fn send(&self, envelope: Envelope) {
        // Нет слушателей — некому и доставлять
        let _ = self.tx.send(Arc::new(envelope));
    }

    /// Сигнал остановки сервера: соединения закрываются с кодом «перезапуск».
    pub fn stopping(&self) -> watch::Receiver<bool> {
        self.stop.subscribe()
    }

    pub fn shutdown(&self) {
        let _ = self.stop.send(true);
    }

    pub fn connections(&self) -> usize {
        self.tx.receiver_count()
    }

    pub fn is_listening(&self) -> bool {
        *self.listening.borrow()
    }

    /// Ждёт, пока слушатель Postgres подпишется на канал (события раньше этого не доходят).
    pub async fn wait_listening(&self) {
        let mut rx = self.listening.subscribe();
        let _ = rx.wait_for(|on| *on).await;
    }
}

/// Слушает канал Postgres и передаёт события в [`Hub`]. После потери связи с базой события за это время
/// пропали: всем уходит `resync`.
pub fn spawn_listener(db: PgPool, hub: Arc<Hub>) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut stopping = hub.stopping();
        loop {
            let mut listener = match connect(&db).await {
                Ok(l) => {
                    hub.listening.send_replace(true);
                    l
                }
                Err(e) if is_closed(&e) => return,
                Err(e) => {
                    tracing::warn!(error = %e, "realtime listener could not connect, retrying");
                    tokio::select! {
                        _ = tokio::time::sleep(Duration::from_secs(2)) => continue,
                        _ = stopping.changed() => return,
                    }
                }
            };
            loop {
                let next = tokio::select! {
                    next = listener.try_recv() => next,
                    _ = stopping.changed() => return,
                };
                match next {
                    Ok(Some(n)) => match serde_json::from_str::<Envelope>(n.payload()) {
                        Ok(envelope) => hub.send(envelope),
                        Err(e) => tracing::warn!(error = %e, "unreadable realtime event"),
                    },
                    // Связь потеряна и восстановлена: что пришло в промежутке — неизвестно
                    Ok(None) => hub.send(Envelope {
                        to: Audience::All,
                        event: ServerEvent::Resync(Resync {
                            reason: "reconnected".into(),
                        }),
                    }),
                    Err(e) if is_closed(&e) => return,
                    Err(e) => {
                        tracing::warn!(error = %e, "realtime listener failed, reconnecting");
                        hub.listening.send_replace(false);
                        break;
                    }
                }
            }
        }
    })
}

async fn connect(db: &PgPool) -> Result<PgListener, sqlx::Error> {
    let mut listener = PgListener::connect_with(db).await?;
    listener.listen(CHANNEL).await?;
    Ok(listener)
}

fn is_closed(e: &sqlx::Error) -> bool {
    matches!(e, sqlx::Error::PoolClosed)
}
