import pytest

from dalywave_ml.query import translate


@pytest.mark.parametrize(
    ("text", "expected"),
    [
        ("спокойный инструментал для работы", "calm instrumental music for focus and work"),
        ("грустный инди под дождь", "sad indie music for a rainy day"),
        ("Бодрое без слов", "upbeat energetic instrumental music without vocals"),
        ("lo-fi для учёбы", "lo-fi music for studying"),
        ("агрессивный фонк в зал", "aggressive phonk music for a gym workout"),
        ("русский рэп 2000-х", "russian rap 2000s music"),
        ("хип-хопа на фоне", "hip hop music for background listening"),
        ("Мечтательная музыка ночью", "dreamy music for the night"),
        ("танцевальная электроника 90х", "dance electronic 90s music"),
    ],
)
def test_translation(text, expected):
    q = translate(text)
    assert q.translated
    assert q.text == expected


def test_english_passes_through():
    q = translate("calm piano for sleep")
    assert not q.translated
    assert q.text == "calm piano for sleep"


def test_unknown_words_reported_and_original_kept():
    q = translate("Земфира")
    assert not q.translated
    assert q.text == "Земфира"
    assert q.unknown == ["земфира"]


def test_word_forms_share_stem():
    for form in ("спокойный", "спокойная", "спокойное", "спокойные", "спокойного"):
        assert translate(form).text == "calm music"


def test_phrase_is_not_matched_inside_word():
    # «на фоне» — фраза, а «на фонограмме» — нет.
    assert "background" not in translate("рок на фонограмме").text


def test_stopwords_and_latin_mix():
    q = translate("включи что-нибудь типа phonk и trap")
    assert q.text == "phonk trap music"
    assert q.unknown == []
