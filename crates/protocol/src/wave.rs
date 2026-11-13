//! «Моя волна» и рекомендации: сессии волны с настройками, выдача с объяснением «почему этот трек»,
//! реакции, похожие треки и поиск словами по звучанию.
//!
//! Сессия помнит, что уже выдано, что заходит и что пропускается: `next` выдаёт следующие треки
//! с учётом всех реакций. Радио — та же сессия с затравкой (трек, артист, альбом, плейлист, слова).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::catalog::Track;

/// Занятие: задаёт границы энергии и темпа и ход сессии («Засыпаю» снижает энергию, «Просыпаюсь» — поднимает).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum WaveActivity {
    #[default]
    Any,
    /// Просыпаюсь.
    Wake,
    /// В дороге.
    Road,
    /// Работаю.
    Work,
    /// Тренируюсь: темп бега — `tempo`.
    Sport,
    /// Отдыхаю.
    Rest,
    /// Засыпаю.
    Sleep,
    /// Вечеринка.
    Party,
}

names!(WaveActivity {
    Any => "any",
    Wake => "wake",
    Road => "road",
    Work => "work",
    Sport => "sport",
    Rest => "rest",
    Sleep => "sleep",
    Party => "party",
});

/// Характер: сколько знакомого и нового, популярное или редкое.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum WaveCharacter {
    /// Любимое.
    Fav,
    /// Знакомое и новое.
    #[default]
    Mix,
    /// Незнакомое.
    New,
    /// Популярное.
    Pop,
    /// Редкое.
    Rare,
}

names!(WaveCharacter { Fav => "fav", Mix => "mix", New => "new", Pop => "pop", Rare => "rare" });

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum WaveMood {
    #[default]
    Any,
    /// Бодрое.
    Energetic,
    /// Весёлое.
    Happy,
    /// Спокойное.
    Calm,
    /// Грустное.
    Sad,
}

names!(WaveMood {
    Any => "any",
    Energetic => "energetic",
    Happy => "happy",
    Calm => "calm",
    Sad => "sad",
});

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum WaveLanguage {
    #[default]
    Any,
    /// Русский.
    Ru,
    /// Иностранный.
    Foreign,
    /// Без слов.
    None,
}

names!(WaveLanguage { Any => "any", Ru => "ru", Foreign => "foreign", None => "none" });

/// От чего радио.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum SeedKind {
    Track,
    Artist,
    Album,
    Playlist,
    /// Описание словами: «спокойный инструментал для работы».
    Text,
    /// Год слушателя (`text` — «2026»): звучание его самых слушаемых треков.
    Year,
}

names!(SeedKind {
    Track => "track",
    Artist => "artist",
    Album => "album",
    Playlist => "playlist",
    Text => "text",
    Year => "year",
});

/// Затравка: `id` — для трека, артиста, альбома и плейлиста, `text` — для описания словами и года.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WaveSeed {
    #[serde(rename = "type")]
    pub kind: SeedKind,
    #[serde(default)]
    pub id: Option<Uuid>,
    #[serde(default)]
    pub text: Option<String>,
}

/// Настройки волны; не переданные — по умолчанию: любое занятие, знакомое и новое, любое настроение и язык.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WaveSettings {
    #[serde(default)]
    pub activity: WaveActivity,
    #[serde(default)]
    pub character: WaveCharacter,
    #[serde(default)]
    pub mood: WaveMood,
    #[serde(default)]
    pub language: WaveLanguage,
    /// Радио от трека, артиста, альбома, плейлиста или описания словами.
    #[serde(default)]
    pub seed: Option<WaveSeed>,
    /// Темп бега, шагов в минуту (140–200), для «Тренируюсь»: темп треков подстраивается под него.
    #[serde(default)]
    pub tempo: Option<u16>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NewWaveSession {
    #[serde(default)]
    pub settings: WaveSettings,
}

/// Готова ли волна к запуску.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum WavePrepState {
    /// Первые треки под настройки ещё скачиваются: спросить сессию снова (или дождаться
    /// `acquisition.updated` по трекам очереди). Дольше полутора минут не длится.
    Preparing,
    /// Можно играть очередь.
    #[default]
    Ready,
}

names!(WavePrepState { Preparing => "preparing", Ready => "ready" });

/// Сессия волны (или радио — с затравкой).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct WaveSession {
    pub id: Uuid,
    pub settings: WaveSettings,
    /// Подпись: «Моя волна», «Радио: Lowborn», «По запросу «…»».
    pub title: String,
    /// Сколько треков уже выдано.
    pub served: u32,
    pub started_at: DateTime<Utc>,
    /// `preparing` — первые треки под настройки ещё скачиваются; `ready` — очередь можно играть.
    pub state: WavePrepState,
    /// Сколько треков начала уже на сервере …
    pub ready: u32,
    /// … и сколько нужно, чтобы начать.
    pub need: u32,
    /// Начало волны в порядке игры: подобрано под настройки, готовое — первым. Дальше — `next`.
    pub queue: Vec<WaveItem>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, IntoParams)]
#[serde(rename_all = "camelCase")]
#[into_params(parameter_in = Query)]
pub struct NextQuery {
    /// Сколько треков: 1–30, по умолчанию 10.
    pub count: Option<u32>,
}

/// Почему трек здесь.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ReasonKind {
    /// «Похоже на Lowborn»: сосед по звучанию грани вкуса.
    Similar,
    /// Похоже на то, что заходит в этой сессии.
    Session,
    /// Радио: похоже на затравку.
    Seed,
    /// По описанию словами.
    Query,
    /// «Новое для тебя»: ни трека, ни артиста ещё не слушал.
    NewToYou,
    /// «Давно не слушали».
    Forgotten,
    /// «Из любимого».
    Favorite,
    /// «Вы часто слушаете».
    Often,
    /// Новинка артиста из подписок.
    NewRelease,
    /// Популярное (характер «Популярное»).
    Popular,
    /// Редкая находка (характер «Редкое»).
    Rare,
}

names!(ReasonKind {
    Similar => "similar",
    Session => "session",
    Seed => "seed",
    Query => "query",
    NewToYou => "new_to_you",
    Forgotten => "forgotten",
    Favorite => "favorite",
    Often => "often",
    NewRelease => "new_release",
    Popular => "popular",
    Rare => "rare",
});

/// Подпись под названием трека: вид для иконки и готовый текст по-русски.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Reason {
    pub kind: ReasonKind,
    pub text: String,
}

/// Трек выдачи: почему он здесь и данные для стыков (темп, тональность по Камелоту, энергия).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct WaveItem {
    pub track: Track,
    pub reason: Reason,
    #[schema(required = true)]
    pub bpm: Option<f32>,
    /// 8A, 11B …
    #[schema(required = true)]
    pub key_camelot: Option<String>,
    /// 0..1.
    #[schema(required = true)]
    pub energy: Option<f32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct WaveItems {
    pub items: Vec<WaveItem>,
}

/// Реакция на трек волны.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum WaveFeedbackKind {
    /// Лайк — и трек в медиатеке.
    Like,
    /// «Не нравится»: трек больше не рекомендуется, похожее реже.
    Dislike,
    Skip,
    /// «Не сейчас»: пропуск без штрафа треку.
    NotNow,
    /// «Больше такого».
    MoreLikeThis,
    /// Дослушан.
    Completed,
}

names!(WaveFeedbackKind {
    Like => "like",
    Dislike => "dislike",
    Skip => "skip",
    NotNow => "not_now",
    MoreLikeThis => "more_like_this",
    Completed => "completed",
});

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WaveFeedback {
    pub track_id: Uuid,
    pub kind: WaveFeedbackKind,
    /// Где был трек в момент реакции: ранний пропуск весит больше позднего.
    #[serde(default)]
    pub position_ms: Option<i32>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, IntoParams)]
#[serde(rename_all = "camelCase")]
#[into_params(parameter_in = Query)]
pub struct SimilarQuery {
    /// 1–50, по умолчанию 20.
    pub limit: Option<u32>,
}

/// «Умное перемешивание»: похожее на весь список (альбом, плейлист), чтобы подмешать в очередь.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SuggestRequest {
    /// Треки списка, до 500: по их звучанию ищется похожее, сами они не предлагаются.
    pub track_ids: Vec<Uuid>,
    /// Что ещё не предлагать: уже подмешанное и убранное из очереди, до 500.
    #[serde(default)]
    pub exclude: Vec<Uuid>,
    /// 1–50, по умолчанию 20.
    pub limit: Option<u32>,
}

/// Поиск словами по звучанию: «грустный инди под дождь».
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SoundSearch {
    pub text: String,
    /// Сколько треков в каждом списке: 1–50, по умолчанию 20.
    #[serde(default)]
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SoundSearchResults {
    /// Запрос, как его понял сервис анализа (английский для CLAP).
    pub query: String,
    /// Запрос переведён словарём.
    pub translated: bool,
    /// Незнакомые словарю слова: в поиск они не попали.
    pub unknown_words: Vec<String>,
    /// Ближайшее по звучанию из медиатеки: лайки, плейлисты, прослушанное, скачанное.
    pub library: Vec<Track>,
    /// Ближайшее из индекса каталога: то, что ещё не слушал.
    pub catalog: Vec<Track>,
}

/// Сохранённое сочетание настроек волны: запускает волну в одно нажатие.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct WavePreset {
    pub id: Uuid,
    pub name: String,
    /// Имя иконки Phosphor без префикса: `laptop`, `moon-stars`.
    pub icon: String,
    /// Без затравки: радио не сохраняется.
    pub settings: WaveSettings,
    /// Порядок карточек — по возрастанию.
    pub position: f64,
    pub created_at: DateTime<Utc>,
}

/// Сочетания по порядку карточек.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct WavePresetList {
    pub items: Vec<WavePreset>,
}

/// Новое сочетание — в конец ряда.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NewWavePreset {
    /// 1–40 символов.
    pub name: String,
    pub icon: String,
    pub settings: WaveSettings,
}

/// Правка сочетания: переданные поля меняются.
#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WavePresetPatch {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub settings: Option<WaveSettings>,
}

/// Новый порядок: названные сочетания — первыми в этом порядке, остальные — после, как были.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WavePresetOrder {
    pub ids: Vec<Uuid>,
}

/// `GET /api/wave/why`: какой трек объяснить.
#[derive(Debug, Clone, Default, Serialize, Deserialize, IntoParams)]
#[serde(rename_all = "camelCase")]
#[into_params(parameter_in = Query)]
pub struct WhyQuery {
    pub track_id: Uuid,
    /// Сессия волны, из которой трек; без неё — последний показ трека.
    pub session_id: Option<Uuid>,
}

/// О чём строка объяснения — для значка.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum WhyKind {
    /// Похоже на артиста, которого слушатель любит.
    Artist,
    /// Похоже на то, что зашло в этой сессии или недавно.
    Recent,
    /// В духе грани вкуса.
    Facet,
    /// Знакомый трек: лайк, часто слушает, давно не слушал.
    Familiar,
    /// Новое для слушателя.
    New,
    /// Время суток или устройство.
    Moment,
    /// Затравка радио или описание словами.
    Seed,
    /// Новинка из подписок.
    Release,
    /// Разведка: волна пробует новое, чтобы не ходить по кругу.
    Explore,
    /// Сеть вкуса: похожее обычно дослушивают.
    Learned,
}

names!(WhyKind {
    Artist => "artist",
    Recent => "recent",
    Facet => "facet",
    Familiar => "familiar",
    New => "new",
    Moment => "moment",
    Seed => "seed",
    Release => "release",
    Explore => "explore",
    Learned => "learned",
});

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct WhyLine {
    pub kind: WhyKind,
    /// Готовый текст по-русски: «Похоже на Rushex, которого ты часто слушаешь».
    pub text: String,
}

/// «Почему этот трек» — понятными словами, по тому, что на самом деле повлияло на выбор.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct WhyTrack {
    pub track: Track,
    /// Главное — первым.
    pub lines: Vec<WhyLine>,
    /// Только администратору: оценки и сходства, версия сети, грань — как в разделе «Рекомендации».
    #[schema(value_type = Option<Object>, required = true)]
    pub details: Option<serde_json::Value>,
}

// ───────────── «Как подобран»: карта вкуса ─────────────
// Координаты — в системе карты звучания (UMAP по векторам звучания, примерно 0..1); масштаб и рамку
// выбирает приложение.

/// Грань вкуса на карте: облако с подписью.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ExplainFacet {
    pub id: Uuid,
    pub label: String,
    pub x: f32,
    pub y: f32,
    /// Радиус облака.
    pub r: f32,
    /// Доля вкуса, 0..1.
    pub weight: f32,
}

/// Трек сессии до выбранного: линия того, что играло.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ExplainStep {
    pub track_id: Uuid,
    pub title: String,
    pub artists: String,
    pub x: f32,
    pub y: f32,
    /// Чем кончился: дослушан, пропущен, лайк…; `null` — реакции не было.
    pub outcome: Option<WaveFeedbackKind>,
}

/// Что стало с вариантом подбора.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExplainStage {
    /// Прошёл отбор и оценён.
    Ranked,
    /// Выбран в эту выдачу (сам выбранный трек — в `chosen`).
    Picked,
    /// Отсеян: уже звучал, отдыхает, далеко от вкуса, вне настроек.
    Dropped,
}

/// Вариант этого подбора.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ExplainCandidate {
    pub track_id: Uuid,
    pub x: f32,
    pub y: f32,
    pub stage: ExplainStage,
    /// Номер грани в `facets`, из которой пришёл вариант.
    pub facet: Option<u32>,
    /// Оценка подбора (у отсеянных — нет): чем выше, тем ярче точка.
    pub score: Option<f32>,
}

/// Ближайший соперник выбранного.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ExplainRival {
    pub track_id: Uuid,
    pub title: String,
    pub artists: String,
    pub x: f32,
    pub y: f32,
}

/// «Как подобран»: всё для карты вкуса у трека, который подобрала волна.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct WaveExplain {
    pub track: Track,
    pub x: f32,
    pub y: f32,
    /// Грань, из которой пришёл трек (номер в `facets`).
    pub facet: Option<u32>,
    pub facets: Vec<ExplainFacet>,
    /// Последние треки сессии до этого, по порядку.
    pub session: Vec<ExplainStep>,
    /// Варианты подбора с координатами; следа нет (старый показ) — пусто.
    pub candidates: Vec<ExplainCandidate>,
    pub rivals: Vec<ExplainRival>,
    /// Причины коротко — для чипов: главное первым.
    pub reasons: Vec<WhyLine>,
    /// Сколько вариантов рассматривал подбор (с отсеянными); следа нет — `null`.
    pub considered: Option<u32>,
    /// Остальной каталог вокруг — «пыль» карты: x0, y0, x1, y1, …
    pub dust: Vec<f32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_match_the_interface() {
        let s: WaveSettings = serde_json::from_value(serde_json::json!({
            "activity": "sleep", "character": "new", "mood": "calm", "language": "none",
            "seed": {"type": "text", "text": "дождь"}
        }))
        .unwrap();
        assert_eq!(s.activity, WaveActivity::Sleep);
        assert_eq!(s.character, WaveCharacter::New);
        assert_eq!(s.language, WaveLanguage::None);
        assert_eq!(s.seed.unwrap().kind, SeedKind::Text);
        let empty: WaveSettings = serde_json::from_value(serde_json::json!({})).unwrap();
        assert_eq!(empty.character, WaveCharacter::Mix);
        assert_eq!(empty.mood, WaveMood::Any);
        assert!(serde_json::from_value::<WaveSettings>(serde_json::json!({"speed": 1})).is_err());
        for k in ReasonKind::ALL {
            assert_eq!(serde_json::to_value(k).unwrap(), k.as_str());
        }
        for k in WaveFeedbackKind::ALL {
            assert_eq!(serde_json::to_value(k).unwrap(), k.as_str());
        }
        assert_eq!(serde_json::to_value(WaveLanguage::None).unwrap(), "none");
    }
}
