//! Что включено на этом сервере (`DALYWAVE_DEEZER`, `DALYWAVE_DOWNLOADER`) — для фоновых путей, которым
//! настройки не передаются: постановка скачиваний, расписание разведчика. API и воркер ставят флаги при
//! запуске ([`set`]); по умолчанию (тесты) всё включено, как было до флагов. Обработчики, у которых есть
//! `Config`, смотрят `config.features` сами.

use std::sync::atomic::{AtomicBool, Ordering};

use crate::config::Features;

static DEEZER: AtomicBool = AtomicBool::new(true);
static DOWNLOADER: AtomicBool = AtomicBool::new(true);

pub fn set(f: Features) {
    DEEZER.store(f.deezer, Ordering::Relaxed);
    DOWNLOADER.store(f.downloader, Ordering::Relaxed);
}

/// Каталог Deezer включён.
pub fn deezer() -> bool {
    DEEZER.load(Ordering::Relaxed)
}

/// Загрузчик включён: записи можно ставить на скачивание.
pub fn downloader() -> bool {
    DOWNLOADER.load(Ordering::Relaxed)
}
