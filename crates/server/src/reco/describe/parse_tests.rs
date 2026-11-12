//! Разбор живых формулировок: как люди на самом деле просят музыку.

use dalywave_protocol::{
    describe::DescribeBrief,
    wave::{WaveActivity, WaveCharacter, WaveLanguage, WaveMood},
};

use super::parse::{MINUTES, parse};

fn p(text: &str) -> DescribeBrief {
    parse(text).brief
}

fn like(b: &DescribeBrief) -> Vec<&str> {
    b.like.iter().map(|a| a.said.as_str()).collect()
}

fn without(b: &DescribeBrief) -> Vec<&str> {
    b.without.iter().map(|a| a.said.as_str()).collect()
}

#[test]
fn gym_phonk_without_slow_songs() {
    let b = p("40 минут фонка для зала, без медляков");
    assert_eq!(b.minutes, Some(40));
    assert_eq!(b.count, None);
    assert_eq!(b.genres, vec!["phonk"]);
    assert_eq!(b.activity, WaveActivity::Sport);
    assert_eq!(b.scene.as_deref(), Some("для зала"));
    assert!(b.no_slow);
    assert_eq!(
        b.mood,
        WaveMood::Any,
        "«медляков» после «без» — не настроение"
    );
    assert!(b.focused);
    assert_eq!(b.sound.as_deref(), Some("фонка для зала"));
}

#[test]
fn calm_russian_indie_in_the_rain() {
    let b = p("спокойный инди под дождь, только русское");
    assert_eq!(b.mood, WaveMood::Calm);
    assert_eq!(b.genres, vec!["indie"]);
    assert_eq!(b.language, WaveLanguage::Ru);
    assert_eq!(b.scene.as_deref(), Some("под дождь"));
    assert_eq!(b.activity, WaveActivity::Any);
    assert_eq!(b.sound.as_deref(), Some("спокойный инди под дождь"));
    assert!(b.focused);
}

#[test]
fn like_an_artist_but_less_known() {
    let b = p("как Скриптонит, но менее известное");
    assert_eq!(like(&b), vec!["Скриптонит"]);
    assert_eq!(b.character, WaveCharacter::Rare);
    assert!(b.focused);
    assert_eq!(b.sound, None, "имя артиста в поиск по звучанию не идёт");
}

#[test]
fn durations_in_many_forms() {
    let m = |t: &str| p(t).minutes;
    assert_eq!(m("музыка на час"), Some(60));
    assert_eq!(m("часик чего-нибудь бодрого"), Some(60));
    assert_eq!(m("полчаса джаза"), Some(30));
    assert_eq!(m("полтора часа техно"), Some(90));
    assert_eq!(m("1,5 часа техно"), Some(90));
    assert_eq!(m("2 часа в дорогу"), Some(120));
    assert_eq!(m("пару часов для работы"), Some(120));
    assert_eq!(m("минут на сорок рока"), Some(40));
    assert_eq!(m("сорок минут"), Some(40));
    assert_eq!(m("двадцать пять минут"), Some(25));
    assert_eq!(m("45мин"), Some(45));
    assert_eq!(m("на 3 минуты"), Some(10), "не короче 10 минут");
    assert_eq!(m("10 часов"), Some(240), "не длиннее 4 часов");
    assert_eq!(m("часа на два"), Some(120));
    let c = |t: &str| p(t).count;
    assert_eq!(c("20 треков русского рэпа"), Some(20));
    assert_eq!(c("десять песен для сна"), Some(10));
    assert_eq!(c("треков 15 лоуфая"), Some(15));
    assert_eq!(c("1000 треков"), Some(60));
    assert_eq!(c("спокойное"), None);
    // эпоха — не число треков
    let b = p("хиты 90-х");
    assert_eq!(b.count, None);
    assert_eq!(b.minutes, None);
    assert_eq!(b.character, WaveCharacter::Pop);
    assert_eq!(b.sound.as_deref(), Some("90-х"));
}

#[test]
fn languages() {
    assert_eq!(p("русский рок").language, WaveLanguage::Ru);
    assert_eq!(p("рэп на русском").language, WaveLanguage::Ru);
    assert_eq!(p("только русскоязычное").language, WaveLanguage::Ru);
    assert_eq!(p("зарубежный инди").language, WaveLanguage::Foreign);
    assert_eq!(p("что-то на английском").language, WaveLanguage::Foreign);
    assert_eq!(p("иностранное").language, WaveLanguage::Foreign);
    assert_eq!(p("без русского").language, WaveLanguage::Foreign);
    assert_eq!(p("не на русском").language, WaveLanguage::Foreign);
    assert_eq!(p("без слов для работы").language, WaveLanguage::None);
    assert_eq!(p("без вокала").language, WaveLanguage::None);
    let b = p("инструментальный хип-хоп");
    assert_eq!(b.language, WaveLanguage::None);
    assert_eq!(b.genres, vec!["rap"]);
    assert!(b.sound.as_deref().unwrap().contains("инструментальный"));
    assert_eq!(p("белорусский рок").language, WaveLanguage::Any);
}

#[test]
fn activities_and_scenes() {
    let a = |t: &str| p(t).activity;
    assert_eq!(a("в качалку"), WaveActivity::Sport);
    assert_eq!(a("для тренировки"), WaveActivity::Sport);
    assert_eq!(a("на пробежку"), WaveActivity::Sport);
    assert_eq!(a("для работы"), WaveActivity::Work);
    assert_eq!(a("для учёбы"), WaveActivity::Work);
    assert_eq!(a("сосредоточиться"), WaveActivity::Work);
    assert_eq!(a("перед сном"), WaveActivity::Sleep);
    assert_eq!(a("чтобы засыпать"), WaveActivity::Sleep);
    assert_eq!(a("бодрое утро"), WaveActivity::Wake);
    assert_eq!(a("на вечеринку"), WaveActivity::Party);
    assert_eq!(a("танцевальное"), WaveActivity::Party);
    assert_eq!(a("в дорогу"), WaveActivity::Road);
    assert_eq!(a("за рулём"), WaveActivity::Road);
    assert_eq!(a("расслабиться"), WaveActivity::Rest);
    assert_eq!(a("чилл на фон"), WaveActivity::Rest);
    assert_eq!(p("в качалку").scene.as_deref(), Some("в качалку"));
    assert_eq!(
        p("для долгой дороги").scene.as_deref(),
        Some("для долгой дороги")
    );
    assert_eq!(p("тренировка").scene.as_deref(), Some("для тренировки"));
    let b = p("ночная прогулка по городу");
    assert_eq!(b.activity, WaveActivity::Any);
    assert!(b.focused);
    assert_eq!(b.scene.as_deref(), Some("по городу"));
}

#[test]
fn moods() {
    let m = |t: &str| p(t).mood;
    assert_eq!(m("бодрое"), WaveMood::Energetic);
    assert_eq!(m("что-то качовое"), WaveMood::Energetic);
    assert_eq!(m("жёсткий рок"), WaveMood::Energetic);
    assert_eq!(m("спокойная музыка"), WaveMood::Calm);
    assert_eq!(m("медляки"), WaveMood::Calm);
    assert_eq!(m("под дождь"), WaveMood::Calm);
    assert_eq!(m("весёлое летнее"), WaveMood::Happy);
    assert_eq!(m("позитивчик"), WaveMood::Happy);
    assert_eq!(m("грустные песни"), WaveMood::Sad);
    assert_eq!(m("меланхолия"), WaveMood::Sad);
    assert_eq!(m("не грустное"), WaveMood::Happy);
    assert_eq!(m("не слишком громкое"), WaveMood::Calm);
    assert_eq!(m("грустное, но бодрое"), WaveMood::Sad, "первое названное");
    assert_eq!(m("hardstyle"), WaveMood::Any, "жанр, а не настроение");
    assert_eq!(p("hardstyle").genres, vec!["hardstyle"]);
}

#[test]
fn characters() {
    let c = |t: &str| p(t).character;
    assert_eq!(c("что-то новое"), WaveCharacter::New);
    assert_eq!(c("незнакомое"), WaveCharacter::New);
    assert_eq!(c("что я не слышал"), WaveCharacter::New);
    assert_eq!(c("без повторов"), WaveCharacter::New);
    assert_eq!(c("редкое"), WaveCharacter::Rare);
    assert_eq!(c("малоизвестные артисты"), WaveCharacter::Rare);
    assert_eq!(c("менее популярное"), WaveCharacter::Rare);
    assert_eq!(c("не мейнстрим"), WaveCharacter::Rare);
    assert_eq!(c("андеграунд"), WaveCharacter::Rare);
    assert_eq!(c("мало кто знает"), WaveCharacter::Rare);
    assert_eq!(c("без хитов"), WaveCharacter::Rare);
    assert_eq!(c("популярное"), WaveCharacter::Pop);
    assert_eq!(c("хиты"), WaveCharacter::Pop);
    assert_eq!(c("самое известное"), WaveCharacter::Pop);
    assert_eq!(c("из лайков"), WaveCharacter::Fav);
    assert_eq!(c("моё любимое"), WaveCharacter::Fav);
    assert_eq!(c("знакомое"), WaveCharacter::Fav);
    assert_eq!(c("новогодние песни"), WaveCharacter::Mix);
}

#[test]
fn references() {
    assert_eq!(like(&p("в духе Rushex")), vec!["Rushex"]);
    assert_eq!(like(&p("в стиле Скриптонита")), vec!["Скриптонита"]);
    assert_eq!(like(&p("похожее на Kai Angel")), vec!["Kai Angel"]);
    assert_eq!(like(&p("типа Мияги и Эндшпиль")), vec!["Мияги", "Эндшпиль"]);
    assert_eq!(
        like(&p("как Скриптонит, Мияги")),
        vec!["Скриптонит", "Мияги"]
    );
    assert_eq!(
        like(&p("что-то вроде The Weeknd для вечеринки")),
        vec!["The Weeknd"]
    );
    let b = p("как Земфира, но новое");
    assert_eq!(like(&b), vec!["Земфира"]);
    assert_eq!(b.character, WaveCharacter::New);
    // «в стиле фонк» — жанр
    let b = p("в стиле фонк");
    assert!(b.like.is_empty());
    assert_eq!(b.genres, vec!["phonk"]);
    assert_eq!(b.sound.as_deref(), Some("фонк"));
    assert!(p("как в 90-х").like.is_empty());
    assert!(p("как можно бодрее").like.is_empty());
    let b = p("фонк как Kordhell для зала");
    assert_eq!(like(&b), vec!["Kordhell"]);
    assert_eq!(b.genres, vec!["phonk"]);
    assert_eq!(b.activity, WaveActivity::Sport);
}

#[test]
fn exclusions() {
    let b = p("рэп без мата");
    assert!(b.no_explicit);
    assert_eq!(b.genres, vec!["rap"]);
    assert!(p("без нецензурщины").no_explicit);
    assert!(p("без плохих слов").no_explicit);
    assert_eq!(p("без плохих слов").language, WaveLanguage::Any);
    assert!(p("без explicit").no_explicit);
    assert!(p("не медленное").no_slow);
    assert!(p("без всяких баллад").no_slow);
    let b = p("без попсы");
    assert_eq!(b.avoid_genres, vec!["pop"]);
    assert!(b.genres.is_empty());
    assert_eq!(p("рок, но не попса").avoid_genres, vec!["pop"]);
    assert_eq!(p("без рэпа и попсы").avoid_genres, vec!["rap", "pop"]);
    assert_eq!(p("кроме рэпа").avoid_genres, vec!["rap"]);
    let b = p("русский рэп без Моргенштерна");
    assert_eq!(without(&b), vec!["Моргенштерна"]);
    assert_eq!(b.language, WaveLanguage::Ru);
    assert_eq!(b.genres, vec!["rap"]);
    let b = p("бодрое без Басты и Тимати, без мата");
    assert_eq!(without(&b), vec!["Басты", "Тимати"]);
    assert!(b.no_explicit);
    assert_eq!(b.mood, WaveMood::Energetic);
}

#[test]
fn plain_requests_lean_on_taste() {
    let b = p("40 минут для зала");
    assert!(!b.focused, "без жанра и своих слов — из вкуса");
    assert_eq!(b.activity, WaveActivity::Sport);
    let b = p("бодрая музыка для работы");
    assert!(!b.focused);
    assert_eq!(b.mood, WaveMood::Energetic);
    assert_eq!(b.activity, WaveActivity::Work);
    assert!(p("что-нибудь спокойное").sound.is_some());
    assert!(!p("включи что-нибудь спокойное пожалуйста").focused);
    assert!(
        p("музыка как в аниме").focused,
        "незнакомое слово — конкретика"
    );
}

#[test]
fn genres_in_the_wild() {
    let g = |t: &str| p(t).genres;
    assert_eq!(g("лоу фай для учебы"), vec!["lofi"]);
    assert_eq!(g("lo-fi"), vec!["lofi"]);
    assert_eq!(g("хип хоп 2000-х"), vec!["rap"]);
    assert_eq!(g("драм н бейс"), vec!["dnb"]);
    assert_eq!(g("рэпчик"), vec!["rap"]);
    assert_eq!(g("русский рок и пост-панк"), vec!["rock", "postpunk"]);
    assert_eq!(g("r&b на вечер"), vec!["rnb"]);
    assert_eq!(g("эмбиент для медитации"), vec!["ambient"]);
    assert_eq!(g("классика для чтения"), vec!["classical"]);
    assert_eq!(g("электронщина"), vec!["electronic"]);
    assert_eq!(g("популярное"), Vec::<String>::new());
    assert_eq!(g("трэп"), vec!["trap"]);
}

#[test]
fn phrase_for_the_title() {
    assert_eq!(
        parse("что-нибудь спокойное на вечер").phrase,
        "спокойное на вечер"
    );
    assert_eq!(parse("спокойный вечер").phrase, "спокойный вечер");
    assert_eq!(parse("хочу музыку для зала").phrase, "для зала");
}

#[test]
fn junk_does_not_break_anything() {
    for t in [
        "",
        " ",
        ",,,",
        "!!!",
        "без",
        "как",
        "не",
        "40",
        "минут",
        "в стиле",
        "как ,",
        "без ,",
        "🔥🔥🔥",
        "a",
        "без без без",
        "как как как",
        "1,5,5 часа",
    ] {
        let b = p(t);
        assert!(
            b.minutes
                .is_none_or(|m| (MINUTES.0..=MINUTES.1).contains(&m)),
            "{t}"
        );
    }
    let _ = p(&"очень ".repeat(200));
}
