import pytest

from dalywave_fetcher.normalize import (
    clean_channel,
    fold,
    name_match,
    parse_title,
    split_names,
    text_similarity,
    transliterate,
)


def test_fold_drops_case_punctuation_and_yo() -> None:
    assert fold("Don't Stop — Me Now!") == "dont stop me now"
    assert fold("Ёлки-Палки") == "елки палки"
    assert fold("Beyoncé & JAY-Z") == "beyonce and jay z"


def test_transliterate() -> None:
    assert transliterate("пошлая молли") == "poshlaya molli"
    assert transliterate("щука и ёж") == "schuka i ezh"


@pytest.mark.parametrize(
    ("title", "base", "versions", "markers"),
    [
        ("ZAKHELO (Slowed)", "zakhelo", ("slowed",), {"slowed"}),
        (
            "MONTAGEM BATCHI (ULTRA SLOWED)",
            "montagem batchi",
            ("ultra slowed",),
            {"slowed", "ultra"},
        ),
        (
            "Murder In My Mind (Slowed + Reverb)",
            "murder in my mind",
            ("slowed reverb",),
            {"slowed", "reverb"},
        ),
        ("Don't Be Mad (Sped Up)", "dont be mad", ("sped up",), {"sped_up"}),
        ("Bohemian Rhapsody - Remastered 2011", "bohemian rhapsody", (), set()),
        ("Bohemian Rhapsody (2011 Remaster)", "bohemian rhapsody", (), set()),
        ("Bohemian Rhapsody (Live Aid)", "bohemian rhapsody", ("live aid",), {"live"}),
        (
            "Shine On You Crazy Diamond (Pts. 1-5)",
            "shine on you crazy diamond",
            ("pts 1 5",),
            set(),
        ),
        ("Группа крови (Английская версия)", "группа крови", ("англииская",), {"alt"}),
        ("Song [Official Lyric Video] (HD)", "song", (), set()),
        ("Song (Премьера клипа, 2021)", "song", (), set()),
        ("Song (prod. by Someone)", "song", (), set()),
        ("Paint It Black - Cover", "paint it black", ("cover",), {"cover"}),
        ("Song (Official Live Video)", "song", ("live",), {"live"}),
        ("Montagem Tomada (Ao Vivo)", "montagem tomada", ("ao vivo",), {"live"}),
        ("ZAKHELO (Hyper Slowed)", "zakhelo", ("hyper slowed",), {"slowed", "hyper"}),
        (
            "MONTAGEM BATCHI (Super Ultra Slow)",
            "montagem batchi",
            ("super ultra slow",),
            {"slowed", "super"},
        ),
    ],
)
def test_parse_title(title: str, base: str, versions: tuple[str, ...], markers: set[str]) -> None:
    parts = parse_title(title)
    assert parts.base == base
    assert parts.versions == versions
    assert parts.markers == markers


def test_live_word_inside_title_is_not_a_marker() -> None:
    assert parse_title("Live Forever").markers == frozenset()
    assert parse_title("Live Forever (Live at Knebworth)").markers == {"live"}
    assert parse_title("Cover Me").markers == frozenset()


def test_featured_artists_are_extracted() -> None:
    parts = parse_title("Sunflower (feat. Swae Lee)")
    assert parts.base == "sunflower"
    assert parts.featured == ("swae lee",)
    assert parse_title("Song ft. A, B & C").featured == ("a", "b", "c")
    assert parse_title("Song (with lyrics)").featured == ()


def test_video_title_artist_prefix_is_stripped() -> None:
    parts = parse_title(
        "Post Malone, Swae Lee - Sunflower (Spider-Man: Into the Spider-Verse) (Official Video)",
        ["Post Malone"],
    )
    assert parts.base == "sunflower"
    assert parts.versions == ("spider man into the spider verse",)
    assert parts.prefix_artists == ("post malone", "swae lee")
    # Без совпадения с артистом левая часть остаётся названием.
    assert parse_title("Song - Remastered 2011", ["Other"]).base == "song"


def test_cyrillic_prefix_with_quotes() -> None:
    parts = parse_title("КИНО - «Группа крови» (Official Audio)", ["Кино"])
    assert parts.base == "группа крови"
    assert parts.markers == frozenset()


def test_split_names() -> None:
    assert split_names("A, B & C feat. D") == ["a", "b", "c", "d"]
    assert split_names("Пошлая Молли и Кто-то") == ["пошлая молли", "кто то"]


def test_name_match() -> None:
    assert name_match(fold("Пошлая Молли"), fold("Poshlaya Molly"))
    assert name_match(fold("Кино"), fold("KINO"))
    assert name_match("the weeknd", "weeknd")
    assert name_match(fold("mzlff"), fold("MZLFF"))
    assert not name_match("queen", "queensryche")
    assert not name_match("kino", "nino rota")


def test_text_similarity_handles_scripts() -> None:
    assert text_similarity(fold("Нон стоп"), fold("Non stop")) > 0.85
    assert text_similarity("love", "love me do") < 0.9
    assert text_similarity("same", "same") == 1.0


def test_clean_channel() -> None:
    assert clean_channel("QueenVEVO") == "Queen"
    assert clean_channel("Kordhell - Topic") == "Kordhell"
    assert clean_channel("Rushex Official") == "Rushex"
