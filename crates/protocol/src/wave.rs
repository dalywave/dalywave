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
}

names!(SeedKind {
    Track => "track",
    Artist => "artist",
    Album => "album",
    Playlist => "playlist",
    Text => "text",
});

/// Затравка: `id` — для трека, артиста, альбома и плейлиста, `text` — для описания словами.
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
