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
    pub current: i32,
    /// Сезон итогов текущего года (с 1 декабря).
    pub season: bool,
    pub years: Vec<RewindYear>,
}
