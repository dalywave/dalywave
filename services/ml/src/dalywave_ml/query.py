"""Русский запрос → английский текст для CLAP.

CLAP обучен на английских подписях, а токенизатор RoBERTa режет кириллицу на байты — русский
запрос он «не понимает». Запросы к волне и поиску короткие и крутятся вокруг настроений,
жанров, инструментов и занятий, поэтому перевод — словарь основ: «спокойн» ловит
«спокойный/спокойная/спокойное…». Латиница (lo-fi, phonk, имена) проходит как есть,
незнакомые русские слова отбрасываются и возвращаются в `unknown`, чтобы словарь можно было
пополнять. Модель-переводчик (opus-mt-ru-en, ~300 МБ) не нужна для такого словаря.
"""

from __future__ import annotations

import re
from dataclasses import dataclass, field

# Фразы проверяются раньше отдельных слов (длинные первыми); части через «|» добавляются
# по отдельности, а части с «for/with/…» уходят в конец запроса.
PHRASES: list[tuple[str, str]] = [
    ("без слов", "instrumental|without vocals"),
    ("без вокала", "instrumental|without vocals"),
    ("без голоса", "instrumental|without vocals"),
    ("с вокалом", "with vocals"),
    ("женский вокал", "female vocals"),
    ("женским вокалом", "female vocals"),
    ("мужской вокал", "male vocals"),
    ("мужским вокалом", "male vocals"),
    ("для работы", "for focus and work"),
    ("на работе", "for focus and work"),
    ("для учебы", "for studying"),
    ("для учёбы", "for studying"),
    ("для концентрации", "for concentration"),
    ("для сна", "for sleep"),
    ("перед сном", "for falling asleep"),
    ("под дождь", "for a rainy day"),
    ("в дождь", "for a rainy day"),
    ("в дорогу", "for a road trip"),
    ("в дороге", "for a road trip"),
    ("за рулем", "for driving"),
    ("за рулём", "for driving"),
    ("для тренировки", "for a workout"),
    ("для спорта", "for a workout"),
    ("в зал", "for a gym workout"),
    ("для бега", "for running"),
    ("для пробежки", "for running"),
    ("для вечеринки", "for a party"),
    ("на вечеринку", "for a party"),
    ("для медитации", "for meditation"),
    ("для йоги", "for yoga"),
    ("для чтения", "for reading"),
    ("для отдыха", "for relaxing"),
    ("на фон", "for background listening"),
    ("на фоне", "for background listening"),
    ("по утрам", "for the morning"),
    ("с утра", "for the morning"),
    ("ночью", "for the night"),
    ("80-х", "80s"),
    ("80х", "80s"),
    ("90-х", "90s"),
    ("90х", "90s"),
    ("2000-х", "2000s"),
    ("нулевых", "2000s"),
    ("драм-н-бейс", "drum and bass"),
    ("драм н бейс", "drum and bass"),
    ("хип-хоп", "hip hop"),
    ("хип хоп", "hip hop"),
    ("лоу-фай", "lo-fi"),
    ("ло-фай", "lo-fi"),
    ("поп-рок", "pop rock"),
    ("пост-рок", "post-rock"),
    ("пост-панк", "post-punk"),
]

# Основы слов (после замены ё→е). Чем длиннее основа, тем раньше она проверяется.
STEMS: dict[str, str] = {
    # настроение и характер
    "спокойн": "calm",
    "тих": "quiet",
    "расслабл": "relaxing",
    "расслабон": "relaxing",
    "умиротвор": "peaceful",
    "медленн": "slow",
    "быстр": "fast",
    "весел": "happy",
    "радост": "joyful",
    "позитив": "positive",
    "грустн": "sad",
    "грусть": "sad",
    "печальн": "sad",
    "тоскл": "melancholic",
    "тоск": "melancholic",
    "меланхол": "melancholic",
    "депрессив": "depressive",
    "бодр": "upbeat energetic",
    "энергичн": "energetic",
    "заряжающ": "energetic",
    "мощн": "powerful",
    "агрессивн": "aggressive",
    "зл": "angry",
    "ярост": "furious",
    "мрачн": "dark",
    "темн": "dark",
    "светл": "bright",
    "романтич": "romantic",
    "нежн": "tender",
    "мечтат": "dreamy",
    "атмосферн": "atmospheric",
    "эпичн": "epic",
    "эпик": "epic",
    "драйвов": "driving",
    "драйв": "driving",
    "ностальг": "nostalgic",
    "летн": "summer",
    "зимн": "winter",
    "осенн": "autumn",
    "весенн": "spring",
    "ночн": "night",
    "утренн": "morning",
    "вечерн": "evening",
    "громк": "loud",
    "мягк": "soft",
    "тяжел": "heavy",
    "легк": "light",
    "сонн": "sleepy",
    "чилл": "chill",
    "чил": "chill",
    "вайб": "vibe",
    "грув": "groovy",
    "танцев": "dance",
    "клубн": "club",
    "праздничн": "festive",
    "новогодн": "christmas",
    "красив": "beautiful",
    "странн": "weird experimental",
    "эксперимент": "experimental",
    "минималист": "minimal",
    "кинематограф": "cinematic",
    "киношн": "cinematic",
    # жанры
    "инструментал": "instrumental",
    "акустич": "acoustic",
    "акустик": "acoustic",
    "электронн": "electronic",
    "электроник": "electronic",
    "рок": "rock",
    "попс": "pop",
    "поп": "pop",
    "рэп": "rap",
    "реп": "rap",
    "хипхоп": "hip hop",
    "хип-хоп": "hip hop",
    "драм-н-бейс": "drum and bass",
    "лоу-фай": "lo-fi",
    "джаз": "jazz",
    "блюз": "blues",
    "классическ": "classical",
    "классик": "classical",
    "метал": "metal",
    "панк": "punk",
    "инди": "indie",
    "фонк": "phonk",
    "техно": "techno",
    "хаус": "house",
    "транс": "trance",
    "эмбиент": "ambient",
    "амбиент": "ambient",
    "лоуфай": "lo-fi",
    "лофай": "lo-fi",
    "шансон": "chanson",
    "фолк": "folk",
    "народн": "folk",
    "кантри": "country",
    "регги": "reggae",
    "диско": "disco",
    "фанк": "funk",
    "соул": "soul",
    "гиперпоп": "hyperpop",
    "драмнбейс": "drum and bass",
    "дабстеп": "dubstep",
    "трэп": "trap",
    "треп": "trap",
    "синтвейв": "synthwave",
    "синтипоп": "synth-pop",
    "саундтрек": "soundtrack",
    "оркестр": "orchestral",
    "эстрад": "pop",
    "шугейз": "shoegaze",
    "гранж": "grunge",
    "эмо": "emo",
    "дрилл": "drill",
    "р-н-б": "r&b",
    "рнб": "r&b",
    "госпел": "gospel",
    "латин": "latin",
    "кпоп": "k-pop",
    "аниме": "anime",
    "детск": "children's",
    "колыбельн": "lullaby",
    # инструменты и голос
    "гитар": "guitar",
    "пианино": "piano",
    "фортепиан": "piano",
    "рояль": "piano",
    "скрипк": "violin",
    "виолонч": "cello",
    "барабан": "drums",
    "ударн": "drums",
    "басов": "bass",
    "бас": "bass",
    "синтезатор": "synthesizer",
    "синт": "synth",
    "саксофон": "saxophone",
    "труб": "trumpet",
    "флейт": "flute",
    "орган": "organ",
    "вокал": "vocals",
    "голос": "voice",
    "хор": "choir",
    "женск": "female",
    "мужск": "male",
    # занятия и обстановка
    "работ": "work",
    "учеб": "study",
    "учит": "study",
    "сон": "sleep",
    "сна": "sleep",
    "засып": "falling asleep",
    "просып": "waking up",
    "утр": "morning",
    "вечер": "evening",
    "дожд": "rain",
    "дорог": "road trip",
    "машин": "driving",
    "спорт": "workout",
    "тренир": "workout",
    "бег": "running",
    "вечеринк": "party",
    "тусовк": "party",
    "медитац": "meditation",
    "йог": "yoga",
    "кофе": "coffee",
    "кафе": "cafe",
    "праздник": "celebration",
    "лето": "summer",
    "зима": "winter",
    "осень": "autumn",
    "весна": "spring",
    "море": "sea",
    "морск": "sea",
    "пляж": "beach",
    "город": "city",
    "космос": "space",
    "космич": "space",
    "игров": "video game",
    "видеоигр": "video game",
    # язык и время
    "русск": "russian",
    "английск": "english",
    "англоязычн": "english",
    "зарубежн": "foreign",
    "иностранн": "foreign",
    "стар": "old",
    "ретро": "retro",
    "винтаж": "vintage",
    "современн": "modern",
    "нов": "new",
    # служебные существительные
    "музык": "music",
    "песн": "song",
    "трек": "track",
    "бит": "beat",
    "мелоди": "melody",
}

STOPWORDS = {
    "и",
    "а",
    "но",
    "или",
    "в",
    "во",
    "на",
    "с",
    "со",
    "к",
    "ко",
    "по",
    "под",
    "над",
    "для",
    "без",
    "из",
    "от",
    "до",
    "о",
    "об",
    "про",
    "что",
    "чтобы",
    "как",
    "очень",
    "немного",
    "чуть",
    "что-нибудь",
    "что-то",
    "нибудь",
    "то",
    "это",
    "мне",
    "меня",
    "я",
    "хочу",
    "хочется",
    "давай",
    "включи",
    "поставь",
    "найди",
    "можно",
    "пожалуйста",
    "типа",
    "вроде",
    "такой",
    "такая",
    "такое",
    "такие",
    "похожее",
    "похожая",
    "похожий",
    "похожие",
    "чем",
    "бы",
    "же",
}

_CONTEXT_PREFIXES = ("for ", "with ", "as ", "without ")
_NOUNS = {"music", "song", "track", "beat", "melody", "soundtrack", "lullaby"}
_WORD = re.compile(r"[a-zа-я0-9][a-zа-я0-9&'+\-]*", re.IGNORECASE)
_CYRILLIC = re.compile(r"[а-я]")
_STEMS_SORTED = sorted(STEMS.items(), key=lambda kv: -len(kv[0]))
_PHRASES_SORTED = sorted(enumerate(PHRASES), key=lambda kv: -len(kv[1][0]))


@dataclass
class Query:
    original: str
    text: str
    translated: bool
    unknown: list[str] = field(default_factory=list)


def _normalize(text: str) -> str:
    return text.lower().replace("ё", "е").strip()


def _lookup(word: str) -> str | None:
    for stem, english in _STEMS_SORTED:
        # Слишком длинное окончание — скорее другое слово с той же основой.
        if word.startswith(stem) and len(word) - len(stem) <= 6:
            return english
    return None


def translate(text: str) -> Query:
    original = text.strip()
    norm = _normalize(original)
    if not _CYRILLIC.search(norm):
        return Query(original, original, translated=False)
    for i, (phrase, _) in _PHRASES_SORTED:
        norm = re.sub(rf"(?<![\wа-я]){re.escape(_normalize(phrase))}(?![\wа-я])", f" §{i}§ ", norm)
    mods: list[str] = []
    ctx: list[str] = []
    unknown: list[str] = []

    def add(item: str) -> None:
        target = ctx if item.startswith(_CONTEXT_PREFIXES) else mods
        if item not in mods and item not in ctx:
            target.append(item)

    for n, piece in enumerate(re.split(r"§(\d+)§", norm)):
        if n % 2 == 1:
            for item in PHRASES[int(piece)][1].split("|"):
                add(item)
            continue
        for word in _WORD.findall(piece):
            if word in STOPWORDS:
                continue
            if not _CYRILLIC.search(word):
                add(word)
                continue
            english = _lookup(word)
            if english:
                add(english)
            else:
                unknown.append(word)
    if not mods and not ctx:
        return Query(original, original, translated=False, unknown=unknown)
    words = " ".join(mods).split()
    if not _NOUNS.intersection(words):
        mods.append("music")
    return Query(original, " ".join(mods + ctx), translated=True, unknown=unknown)
