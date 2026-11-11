"""Нормализация названий и имён для сравнения записей Deezer и YouTube."""

from __future__ import annotations

import re
import unicodedata
from dataclasses import dataclass, field

from rapidfuzz import fuzz

# Русская и украинская кириллица → латиница. Точность не важна: схема нужна только
# для нечёткого сравнения «Пошлая Молли» ↔ «Poshlaya Molly».
_TRANSLIT = str.maketrans(
    {
        "а": "a",
        "б": "b",
        "в": "v",
        "г": "g",
        "д": "d",
        "е": "e",
        "ё": "e",
        "ж": "zh",
        "з": "z",
        "и": "i",
        "й": "y",
        "к": "k",
        "л": "l",
        "м": "m",
        "н": "n",
        "о": "o",
        "п": "p",
        "р": "r",
        "с": "s",
        "т": "t",
        "у": "u",
        "ф": "f",
        "х": "h",
        "ц": "ts",
        "ч": "ch",
        "ш": "sh",
        "щ": "sch",
        "ъ": "",
        "ы": "y",
        "ь": "",
        "э": "e",
        "ю": "yu",
        "я": "ya",
        "і": "i",
        "ї": "yi",
        "є": "ye",
        "ґ": "g",
        "ў": "u",
    }
)

_CYRILLIC = re.compile(r"[Ѐ-ӿ]")
_APOSTROPHES = re.compile(r"['’‘`´ʼ]")
_NON_WORD = re.compile(r"[\W_]+")


def has_cyrillic(text: str) -> bool:
    return bool(_CYRILLIC.search(text))


def transliterate(text: str) -> str:
    return text.translate(_TRANSLIT)


def fold(text: str) -> str:
    """Регистр, ё→е, диакритика, пунктуация — всё, что не меняет смысл названия."""
    text = unicodedata.normalize("NFKC", text).casefold().replace("ё", "е")
    text = "".join(c for c in unicodedata.normalize("NFKD", text) if not unicodedata.combining(c))
    text = text.replace("&", " and ")
    text = _APOSTROPHES.sub("", text)
    text = _NON_WORD.sub(" ", text)
    return " ".join(text.split())


# --- Разбор названия -------------------------------------------------------

_SEPARATOR = re.compile(r"\s+[-–—~]{1,2}\s+|\s*[|｜]\s*|\s+//\s+")
_BRACKET = re.compile(r"\(([^()]*)\)|\[([^\[\]]*)\]|\{([^{}]*)\}|【([^【】]*)】")
_QUOTES = re.compile(r"[«»“”„\"]")

_FEAT_BRACKET = re.compile(
    r"^\s*(?:feat\.?|ft\.?|featuring|with|при\s+уч\.?|при\s+участии|п\.\s*у\.)\s+(?P<names>.+)$",
    re.IGNORECASE,
)
_FEAT_INLINE = re.compile(
    r"\s+(?:feat\.?|ft\.?|featuring|при участии)\s+(?P<names>.+)$", re.IGNORECASE
)
_PROD = re.compile(r"^(?:prod|produced|beat)(?:\s+by)?\b")
_NAME_SPLIT = re.compile(
    r"\s*(?:,|;|&|\+|/|\s(?:x|and|и|feat\.?|ft\.?|with|vs\.?)\s)\s*",
    re.IGNORECASE,
)

# Слова, которые не меняют запись: «(Official Video)», «- Remastered 2011», «[HD]».
# Скобка или хвост только из таких слов выбрасывается целиком.
_NOISE_WORDS = frozenset(
    (
        *("official", "officiel", "music", "video", "audio", "lyric", "lyrics"),
        *("visualizer", "visualiser", "clip", "videoclip", "hd", "hq", "4k", "8k", "uhd"),
        *("1080p", "720p", "60fps", "full", "explicit", "clean", "dirty", "uncensored"),
        *("censored", "remaster", "remastered", "remasterd", "digitally", "digital", "mono"),
        *("stereo", "deluxe", "edition", "expanded", "anniversary", "bonus", "track", "album"),
        *("single", "version", "original", "premiere", "new", "with", "in"),
        *("премьера", "клипа", "клип", "трека", "песни", "песня", "официальный"),
        *("официальное", "официальная", "видео", "видеоклип", "аудио", "текст", "ремастер"),
        *("ремастеринг", "версия"),
    )
)
_YEAR = re.compile(r"^(?:19|20)\d\d$")

# Метки версий. «Anywhere» — однозначные слова, их ищем во всём названии; остальные
# считаются меткой, только если стоят в скобках или после разделителя: «Live Forever»
# и «Cover Me» — названия песен, а не концертная запись и не кавер.
_MARKERS_ANYWHERE: tuple[tuple[str, re.Pattern[str]], ...] = (
    ("remix", re.compile(r"\b(?:remix\w*|rmx|ремикс\w*|rework|bootleg|vip\s+mix)\b")),
    ("sped_up", re.compile(r"\b(?:sped\s*up|speed\s*up|speeded\s*up|spedup|ускорен\w*)\b")),
    (
        "slowed",
        re.compile(
            r"\b(?:slowed|slow\s+(?:and\s+)?reverb|(?:ultra|super|hyper|extra|mega)\s+"
            r"(?:ultra\s+)?slow|замедлен\w*)\b"
        ),
    ),
    ("reverb", re.compile(r"\breverb\w*\b")),
    ("nightcore", re.compile(r"\bnightcore\b")),
    ("8d", re.compile(r"\b8d(?:\s+audio)?\b")),
    ("bass_boosted", re.compile(r"\bbass\s*boost(?:ed)?\b")),
    ("karaoke", re.compile(r"\b(?:karaoke|караоке|минусовка|backing\s+track)\b")),
    ("instrumental", re.compile(r"\b(?:instrumental|инструментал\w*|без\s+слов)\b")),
    ("acapella", re.compile(r"\b(?:a\s*capp?ella|acapp?ella|акапелла)\b")),
    ("loop", re.compile(r"\b(?:\d+\s+hours?|\d+\s+час\w*|loop(?:ed)?)\b")),
    ("cover", re.compile(r"\b(?:cover\s+(?:by|version)|кавер\w*|originally\s+performed)\b")),
    (
        "live",
        re.compile(
            r"\blive\s+(?:at|from|in|on|session|version|performance|concert)\b"
            r"|\bконцертн\w+\s+запис\w*"
        ),
    ),
)
_MARKERS_VERSION: tuple[tuple[str, re.Pattern[str]], ...] = (
    (
        "live",
        re.compile(r"\b(?:live|концерт\w*|лайв|unplugged|in\s+concert|ao\s+vivo|en\s+vivo)\b"),
    ),
    ("cover", re.compile(r"\b(?:cover|кавер\w*|tribute)\b")),
    ("acoustic", re.compile(r"\b(?:acoustic|акустик\w*|unplugged)\b")),
    ("edit", re.compile(r"\b(?:edit|radio\s+version|short\s+version)\b")),
    ("extended", re.compile(r"\b(?:extended|long\s+version)\b")),
    ("demo", re.compile(r"\b(?:demo|демо)\b")),
    ("reprise", re.compile(r"\breprise\b")),
    ("mashup", re.compile(r"\bmash\s*up\b")),
    ("alt", re.compile(r"\b(?:alternat\w+|\w+ish\s+version|верси\w+)\b")),
)
_INTENSITY = re.compile(
    r"\b(ultra|super|extra|mega|hyper)\s+(?:ultra\s+)?(?:slowed|slow|sped\s*up|speed\s*up)\b"
)

# Метка с одной стороны почти наверняка означает другую запись.
STRONG_MARKERS = frozenset(
    {
        "live",
        "remix",
        "sped_up",
        "slowed",
        "reverb",
        "nightcore",
        "8d",
        "bass_boosted",
        "karaoke",
        "instrumental",
        "acapella",
        "loop",
        "cover",
    }
)


@dataclass(frozen=True)
class TitleParts:
    """Название, разложенное на части. Все строки уже прошли `fold`."""

    base: str
    versions: tuple[str, ...] = ()
    featured: tuple[str, ...] = ()
    markers: frozenset[str] = field(default_factory=frozenset)
    # Имена из префикса «Артист - Песня» в названиях видео.
    prefix_artists: tuple[str, ...] = ()

    @property
    def version(self) -> str:
        return " ".join(self.versions)


def split_names(text: str) -> list[str]:
    """«A, B & C feat. D» → ['a', 'b', 'c', 'd']."""
    names = (fold(part) for part in _NAME_SPLIT.split(f" {text} "))
    return [n for n in names if n]


def _strip_noise(folded: str) -> str:
    words = [w for w in folded.split() if w not in _NOISE_WORDS and not _YEAR.match(w)]
    return " ".join(words)


def _classify(content: str) -> tuple[str, str, str]:
    """Скобка или хвост названия → (тип, свёрнутый текст, текст без шума).

    Тип: feat / prod / noise / version. Для feat вместо текста — имена как есть:
    запятые между ними ещё нужны.
    """
    if m := _FEAT_BRACKET.match(content):
        names = m.group("names")
        if _strip_noise(fold(names)):
            return "feat", names, names
    folded = fold(content)
    if _PROD.match(folded):
        return "prod", folded, ""
    cleaned = _strip_noise(folded)
    if not cleaned:
        return "noise", folded, ""
    return "version", folded, cleaned


def _detect_markers(full: str, versions: list[str]) -> set[str]:
    markers = {name for name, rx in _MARKERS_ANYWHERE if rx.search(full)}
    for version in versions:
        markers.update(name for name, rx in _MARKERS_VERSION if rx.search(version))
    markers.update(m.group(1) for m in _INTENSITY.finditer(full))
    return markers


def parse_title(title: str, artists: list[str] | tuple[str, ...] = ()) -> TitleParts:
    """Разбирает название на основу, версии, приглашённых артистов и метки.

    `artists` нужны для названий видео вида «Артист - Песня»: левая часть, похожая на
    имя артиста, отбрасывается.
    """
    text = _QUOTES.sub(" ", unicodedata.normalize("NFKC", title))
    raw_versions: list[str] = []
    versions: list[str] = []
    featured: list[str] = []

    def take(content: str) -> None:
        kind, folded, cleaned = _classify(content)
        if kind == "feat":
            featured.extend(split_names(cleaned))
        elif kind in ("version", "noise"):
            raw_versions.append(folded)
            if cleaned:
                versions.append(cleaned)

    # Сначала скобки, изнутри наружу.
    while m := _BRACKET.search(text):
        take(next(g for g in m.groups() if g is not None))
        text = text[: m.start()] + " " + text[m.end() :]

    segments = [s.strip() for s in _SEPARATOR.split(text) if s and s.strip()]
    prefix_artists: list[str] = []
    if len(segments) > 1 and artists and _looks_like_artist(segments[0], artists):
        prefix_artists = split_names(segments[0])
        segments = segments[1:]
    base_raw = segments[0] if segments else ""
    for tail in segments[1:]:
        take(tail)

    if m := _FEAT_INLINE.search(base_raw):
        featured.extend(split_names(m.group("names")))
        base_raw = base_raw[: m.start()]

    return TitleParts(
        base=fold(base_raw),
        versions=tuple(versions),
        featured=tuple(dict.fromkeys(featured)),
        markers=frozenset(_detect_markers(fold(title), raw_versions)),
        prefix_artists=tuple(prefix_artists),
    )


def _looks_like_artist(segment: str, artists: list[str] | tuple[str, ...]) -> bool:
    names = split_names(segment)
    folded_artists = [fold(a) for a in artists if a]
    return any(name_match(n, a) for n in names for a in folded_artists)


# --- Сходство --------------------------------------------------------------


def _ratio(a: str, b: str) -> float:
    if not a or not b:
        return 0.0
    # token_set даёт 100 для «love» ↔ «love me do», поэтому он с понижающим весом.
    return (
        max(
            fuzz.ratio(a, b),
            fuzz.token_sort_ratio(a, b),
            0.85 * fuzz.token_set_ratio(a, b),
        )
        / 100.0
    )


def text_similarity(a: str, b: str) -> float:
    """Сходство двух уже свёрнутых строк 0..1 с учётом транслитерации."""
    best = _ratio(a, b)
    if best < 1.0 and (has_cyrillic(a) or has_cyrillic(b)):
        best = max(best, _ratio(transliterate(a), transliterate(b)))
    return best


def name_match(a: str, b: str) -> bool:
    """Одно и то же имя артиста? Строки — после `fold`."""
    if not a or not b:
        return False
    if a == b:
        return True
    ta, tb = transliterate(a), transliterate(b)
    if ta == tb:
        return True
    if max(fuzz.ratio(a, b), fuzz.ratio(ta, tb)) >= 88:
        return True
    # «the weeknd» ↔ «weeknd»: короткое имя целиком входит в длинное.
    short, long_ = sorted((ta, tb), key=len)
    return len(short) >= 4 and f" {short} " in f" {long_} "


_CHANNEL_NOISE = re.compile(r"\s*(?:-\s*topic|vevo|official|music|channel|tv)\s*$", re.IGNORECASE)


def clean_channel(name: str) -> str:
    """«ArtistVEVO», «Artist - Topic», «Artist Official» → «Artist»."""
    prev = None
    while prev != name:
        prev = name
        name = _CHANNEL_NOISE.sub("", name).strip()
    return name
