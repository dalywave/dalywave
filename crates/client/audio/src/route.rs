//! Какой вывод нужен на текущем маршруте звука: обычный (cpal, RemoteIO/CoreAudio — малая
//! задержка) или буферизованный (Apple: AVSampleBufferAudioRenderer — AirPlay 2 с большим
//! буфером, несколькими колонками сразу и верной задержкой).
//!
//! Решение — чистая функция маршрута, поэтому проверяется подставным маршрутом без устройств.

use std::fmt;
use std::sync::Arc;

use crate::devices::OutputKind;

/// Когда выводить через буферизованный вывод.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BufferedOutput {
    /// По маршруту: есть AirPlay — буферизованный, иначе обычный.
    #[default]
    Auto,
    /// Всегда (проверки на Mac без колонок AirPlay: тот же путь, что у AirPlay).
    Always,
    Never,
}

impl BufferedOutput {
    /// `DALYWAVE_OUTPUT=buffered|realtime` — для проверок; остальное — `Auto`.
    pub fn from_env_value(v: &str) -> Self {
        match v {
            "buffered" => Self::Always,
            "realtime" => Self::Never,
            _ => Self::Auto,
        }
    }
}

type RouteFn = dyn Fn() -> Vec<OutputKind> + Send + Sync;

/// Текущий маршрут звука: выходы, куда сейчас идёт звук (на iOS — AVAudioSession, в проверках —
/// подставной). Вызывается в потоке движка.
#[derive(Clone)]
pub struct RouteProbe(Arc<RouteFn>);

impl RouteProbe {
    pub fn new(f: impl Fn() -> Vec<OutputKind> + Send + Sync + 'static) -> Self {
        Self(Arc::new(f))
    }

    pub fn kinds(&self) -> Vec<OutputKind> {
        (self.0)()
    }
}

impl fmt::Debug for RouteProbe {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("RouteProbe")
    }
}

/// Нужен ли буферизованный вывод на маршруте. `None` — маршрут пуст: система как раз его меняет
/// (между старым и новым выходом), решать рано.
///
/// Несколько колонок AirPlay 2 сразу («мультирум») — несколько выходов AirPlay в маршруте: это тот
/// же буферизованный вывод, синхронность колонок обеспечивает система.
pub fn wants_buffered(mode: BufferedOutput, route: &[OutputKind]) -> Option<bool> {
    match mode {
        BufferedOutput::Always => Some(true),
        BufferedOutput::Never => Some(false),
        BufferedOutput::Auto if route.is_empty() => None,
        BufferedOutput::Auto => Some(route.contains(&OutputKind::AirPlay)),
    }
}

/// Сменился маршрут: на какой вывод переходить (`Some(true)` — на буферизованный), `None` —
/// оставить текущий.
pub fn switch_to(mode: BufferedOutput, buffered_now: bool, route: &[OutputKind]) -> Option<bool> {
    let want = wants_buffered(mode, route)?;
    (want != buffered_now).then_some(want)
}

/// Маршрут для журнала: `airplay+airplay`, `builtin`.
pub fn describe(route: &[OutputKind]) -> String {
    if route.is_empty() {
        return "none".into();
    }
    route
        .iter()
        .map(|k| k.as_str())
        .collect::<Vec<_>>()
        .join("+")
}

#[cfg(test)]
mod tests {
    use super::*;
    use OutputKind::*;

    #[test]
    fn airplay_routes_go_buffered() {
        let auto = BufferedOutput::Auto;
        assert_eq!(wants_buffered(auto, &[BuiltIn]), Some(false));
        assert_eq!(wants_buffered(auto, &[Headphones]), Some(false));
        assert_eq!(wants_buffered(auto, &[Bluetooth]), Some(false));
        assert_eq!(wants_buffered(auto, &[AirPlay]), Some(true));
        // Несколько HomePod в окне выбора — несколько выходов AirPlay
        assert_eq!(
            wants_buffered(auto, &[AirPlay, AirPlay, AirPlay]),
            Some(true)
        );
        // Пока система переключает маршрут, выходов нет — решать рано
        assert_eq!(wants_buffered(auto, &[]), None);
        assert_eq!(wants_buffered(BufferedOutput::Always, &[]), Some(true));
        assert_eq!(
            wants_buffered(BufferedOutput::Never, &[AirPlay]),
            Some(false)
        );
    }

    #[test]
    fn switching_follows_the_route_once() {
        let auto = BufferedOutput::Auto;
        // Динамик → AirPlay: переход на буферизованный
        assert_eq!(switch_to(auto, false, &[AirPlay]), Some(true));
        // Добавили ещё колонку — уже буферизованный, вывод не трогаем
        assert_eq!(switch_to(auto, true, &[AirPlay, AirPlay]), None);
        // Промежуточный пустой маршрут ничего не меняет
        assert_eq!(switch_to(auto, true, &[]), None);
        assert_eq!(switch_to(auto, false, &[]), None);
        // AirPlay → наушники: обратно на обычный
        assert_eq!(switch_to(auto, true, &[Headphones]), Some(false));
        // Bluetooth и динамик между собой — обычный остаётся обычным
        assert_eq!(switch_to(auto, false, &[Bluetooth]), None);
        assert_eq!(switch_to(BufferedOutput::Never, false, &[AirPlay]), None);
        assert_eq!(describe(&[AirPlay, AirPlay]), "airplay+airplay");
        assert_eq!(describe(&[]), "none");
        assert_eq!(
            BufferedOutput::from_env_value("buffered"),
            BufferedOutput::Always
        );
    }
}
