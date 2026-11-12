//! «Итоги года»: год слушателя в цифрах для историй — время, топы с обложками, жанры и настроение,
//! характер слушателя, первый трек, самый громкий день, серия дней, открытия года и кривая по месяцам.
//! Импортированная история Spotify входит в тот год, когда трек играл.

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::catalog::{AlbumSummary, ArtistSummary, Track};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RewindTrack {
    pub track: Track,
    /// Прослушиваний (от 30 секунд или до конца).
    pub plays: i64,
    pub minutes: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RewindArtist {
    pub artist: ArtistSummary,
    pub plays: i64,
    pub minutes: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RewindAlbum {
    pub album: AlbumSummary,
    pub plays: i64,
    pub minutes: i64,
}

/// Жанр года: стиль Discogs из анализа звука (по-русски), а без анализа — жанр Deezer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RewindGenre {
    /// Как есть: «Hip Hop---Trap» или «Rap/Hip Hop».
    pub id: String,
    /// «Трэп».
    pub name: String,
    /// Доля времени прослушивания, 0..1.
    pub share: f32,
}

/// Настроение года по анализу звука: `happy`, `sad`, `aggressive`, `relaxed`, `party`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RewindMood {
    pub id: String,
    /// Средняя вероятность по времени прослушивания, 0..1.
    pub score: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RewindMonth {
    /// 1..12
    pub month: u32,
    pub minutes: i64,
}

/// Характер слушателя: `night_owl` (ночной слушатель), `early_bird` (утро), `explorer` (много нового),
/// `loyal` (переслушивает любимое), `wave_rider` (слушает волну), `balanced` (всего понемногу).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RewindPersonality {
    pub id: String,
    /// Доля времени ночью (23–5).
    pub night_share: f32,
    /// Доля времени утром (5–12).
    pub morning_share: f32,
    /// Доля треков года, которые до этого года не играли.
    pub novelty_share: f32,
    /// Доля прослушиваний-повторов: 1 − треков / прослушиваний.
    pub repeat_share: f32,
    /// Доля времени в «Моей волне» и радио.
    pub wave_share: f32,
    /// Время суток с наибольшей долей: `morning`, `day`, `evening`, `night`.
    pub peak_part: String,
    /// Доля времени по часам суток (24 значения, 0 — с полуночи до часа), по времени слушателя.
    pub hours: Vec<f32>,
    /// Час, когда слушали больше всего, 0..23.
    pub peak_hour: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RewindFirst {
    pub track: Track,
    pub at: DateTime<Utc>,
}

/// День, когда слушали больше всего.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RewindDay {
    /// Дата по времени слушателя.
    pub date: NaiveDate,
    pub minutes: i64,
    pub plays: i64,
    /// Что играло чаще всего в тот день.
    #[schema(required = true)]
    pub track: Option<Track>,
    /// Сколько раз он играл в тот день.
    pub track_plays: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RewindStreak {
    /// Дней подряд.
    pub days: i64,
    pub from: NaiveDate,
    pub to: NaiveDate,
}

/// Открытие года: артист, которого впервые услышали в этом году, и он стал любимым.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RewindDiscovery {
    pub artist: ArtistSummary,
    /// Первое прослушивание (дата по времени слушателя).
    pub first_heard: NaiveDate,
    pub plays: i64,
    /// Лайкнутых треков артиста.
    pub likes: i64,
}

/// Трек года: самый слушаемый, с историей — когда впервые и в каком месяце его было больше всего.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RewindTrackOfYear {
    pub track: Track,
    pub plays: i64,
    pub minutes: i64,
    /// Первое прослушивание в году.
    pub first_played: NaiveDate,
    /// 1..12
    pub peak_month: u32,
}

/// `GET /api/rewind/{year}`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Rewind {
    pub year: i32,
    /// Год закончился: итоги окончательные. Иначе — «по сей день».
    pub complete: bool,
    /// Сезон итогов: с 1 декабря и после конца года — карточка на главной.
    pub season: bool,
    pub computed_at: DateTime<Utc>,
    pub minutes: i64,
    /// Прослушиваний (от 30 секунд или до конца).
    pub plays: i64,
    pub tracks: i64,
    pub artists: i64,
    /// Артистов, которых до этого года не слушали.
    pub new_artists: i64,
    /// Дней, когда что-то играло.
    pub days: i64,
    /// Дней в году на сегодня (для «N из M дней»).
    pub days_in_range: i64,
    /// Лайков за год.
    pub likes: i64,
    /// Сессий «Моей волны» и радио за год.
    pub wave_sessions: i64,
    pub top_tracks: Vec<RewindTrack>,
    pub top_artists: Vec<RewindArtist>,
    pub top_albums: Vec<RewindAlbum>,
    pub genres: Vec<RewindGenre>,
    pub moods: Vec<RewindMood>,
    /// Двенадцать месяцев по порядку.
    pub months: Vec<RewindMonth>,
    pub personality: RewindPersonality,
    #[schema(required = true)]
    pub first_track: Option<RewindFirst>,
    #[schema(required = true)]
    pub top_day: Option<RewindDay>,
    #[schema(required = true)]
    pub streak: Option<RewindStreak>,
    pub discoveries: Vec<RewindDiscovery>,
    #[schema(required = true)]
    pub track_of_year: Option<RewindTrackOfYear>,
    /// Плейлист «Твои лучшие треки {год}», если уже сохранён.
    #[schema(required = true)]
    pub playlist_id: Option<uuid::Uuid>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RewindYear {
    pub year: i32,
    pub minutes: i64,
}

/// `GET /api/rewind`: годы, за которые есть прослушивания (новые первыми), и текущий год слушателя.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RewindYears {
    /// Год итогов: открытый событием, а у администратора при закрытом — текущий.
    pub current: i32,
    /// Сезон итогов текущего года (с 1 декабря).
    pub season: bool,
    pub years: Vec<RewindYear>,
    /// Итоги открыты: карточки и истории видны. Закрытые видит только администратор (предпросмотр).
    pub open: bool,
    /// Стиль историй этого слушателя.
    pub style: RewindStyle,
    /// Итоги года `current` уже посмотрены: карточка «Твои итоги готовы» больше не нужна.
    pub seen: bool,
    /// Когда итоги откроются сами (режим «Автоматически», пока закрыто) — для напоминания в приложении.
    #[schema(required = true)]
    pub opens_at: Option<DateTime<Utc>>,
}

/// Стиль историй: `wave` — «Волна года», `spread` — «Разворот», `record` — «Пластинка».
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum RewindStyle {
    Wave,
    Spread,
    Record,
}

/// Стиль в настройках сервера: один для всех или `random` — случайный для каждого слушателя.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum RewindStyleSetting {
    Wave,
    Spread,
    Record,
    Random,
}

/// Когда итоги открыты: `auto` — с 1 декабря по 15 января (в декабре — за этот год, в январе — за
/// прошлый), `open` — открыты сейчас за выбранный год, `closed` — закрыты.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum RewindMode {
    Auto,
    Open,
    Closed,
}

/// `GET /api/admin/rewind`: «Итоги года» как событие сервера.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RewindEvent {
    pub style: RewindStyleSetting,
    pub mode: RewindMode,
    /// Год для режима «Открыто сейчас».
    #[schema(required = true)]
    pub year: Option<i32>,
    /// Какой год открыт прямо сейчас; `null` — закрыто.
    #[schema(required = true)]
    pub open_year: Option<i32>,
    /// Ближайшее автоматическое открытие (режим «Автоматически», пока закрыто).
    #[schema(required = true)]
    pub opens_at: Option<DateTime<Utc>>,
    /// Когда закроется само (режим «Автоматически», пока открыто).
    #[schema(required = true)]
    pub closes_at: Option<DateTime<Utc>>,
    /// Годы с прослушиваниями на сервере — для выбора года (новые первыми).
    pub years: Vec<i32>,
    pub updated_at: DateTime<Utc>,
}

/// `PATCH /api/admin/rewind`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RewindEventPatch {
    #[serde(default)]
    pub style: Option<RewindStyleSetting>,
    #[serde(default)]
    pub mode: Option<RewindMode>,
    /// Год для «Открыто сейчас»; без него — текущий.
    #[serde(default)]
    pub year: Option<i32>,
}

/// Звук слайда историй: что играет и с какого места.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RewindCue {
    /// Слайд: `intro`, `minutes`, `days`, `first`, `tracks`, `toy`, `artists`, `albums`, `sound`,
    /// `discoveries`, `personality`, `summary`. Слайда нет в списке — играет прежний трек.
    pub slide: String,
    pub track: Track,
    /// С какого места играть: припев или самая энергичная часть.
    pub start_ms: i64,
    /// Откуда место: `analysis` — по кривой энергии полного анализа, `estimate` — 30 % трека.
    pub start_from: String,
}

/// `GET /api/rewind/{year}/soundtrack`: музыка историй. Треки, которых ещё нет на сервере, ставятся
/// на скачивание вне очереди — к своему слайду они успевают.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RewindSoundtrack {
    pub year: i32,
    /// По порядку слайдов.
    pub cues: Vec<RewindCue>,
}

/// `GET /api/rewind/{year}/more`: «Ещё послушать» — новое для слушателя, похожее на любимое за год.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RewindMore {
    pub items: Vec<Track>,
    /// Плейлист «Ещё послушать {год}», если уже сохранён.
    #[schema(required = true)]
    pub playlist_id: Option<uuid::Uuid>,
}

/// `POST /api/rewind/{year}/more`: сохранить показанное плейлистом.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SaveRewindMore {
    /// Треки по порядку, до 100.
    pub track_ids: Vec<uuid::Uuid>,
}
