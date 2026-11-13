"""Отбор клипа трека из выдачи YouTube: только официальное видео, в приоритете — с канала автора.

Чистые функции без сети: `assess` оценивает один ролик, `choose` — выдачу поиска.

Откуда ролик (`source`), по убыванию веса:
- `artist` — канал самого исполнителя (любого из исполнителей трека): имя канала совпадает с именем
  артиста после нормализации и транслитерации; официальный канал артиста YouTube (значок ноты) обычно
  называется так же;
- `official` — проверенный канал (галочка или нота: yt-dlp отдаёт их одним признаком), в названии есть
  исполнитель и пометка официального клипа («Official Video», «M/V», «Клип»): канал лейбла или
  издателя под другим именем;
- `vevo` — VEVO-канал исполнителя;
- `label` — канал лейбла альбома (из данных Deezer), в названии есть исполнитель.

Без источника ролик не клип: перезалив, фанатское видео. Отсев — фанатское (анимация не с канала
автора, Minecraft, Roblox, Gacha, AMV, MMD, edit…), не клип (текст, звук, визуализатор,
выступление, реакция, кавер, slowed/sped up, nightcore, час подряд…), другая длина (±15 %, клипу
с вступлением — до минуты длиннее). Нет подходящего — клипа нет: показывается обложка.
"""

from __future__ import annotations

import math
import re
from dataclasses import dataclass, field
from typing import Any

from rapidfuzz import fuzz

from .normalize import (
    STRONG_MARKERS,
    clean_channel,
    fold,
    name_match,
    parse_title,
    split_names,
    text_similarity,
    transliterate,
)

MIN_TITLE_SIMILARITY = 0.82
# Длина ролика относительно трека: ±15 %; клип с вступлением — до минуты длиннее
DURATION_SHARE = 0.15
INTRO_EXTRA_S = 60.0
# Вес источника: разрыв между соседними больше всех бонусов вместе — клип с канала автора
# выигрывает у «Official Music Video» издателя, а не наоборот
SOURCE_WEIGHT = {"artist": 4.0, "official": 3.0, "vevo": 2.4, "label": 1.8}
# Сколько кандидатов запоминает сервер (их видит администратор в «Клип»)
KEEP_CANDIDATES = 8

# Не клип: текст, звук, выступление, реакция, переделки. Мимо, если слово есть в названии трека
_NOT_A_CLIP = re.compile(
    r"\b(?:lyrics?|lyric\s+video|letra|paroles|official\s+audio|audio|visuali[sz]er|"
    r"teaser|trailer|snippet|preview|reaction|reacts?|karaoke|cover|live|remix|"
    r"sped\s*up|slowed|reverb|nightcore|8d|bass\s+boosted|performance|dance\s+practice|"
    r"choreography|relay\s+dance|behind\s+the\s+scenes|making\s+of|making\s+film|interview|"
    r"instrumental|acapella|a\s+cappella|mashup|extended|loop|1\s+hour|10\s+hours?|"
    r"sub\s+espanol|subtitulado|legendado|перевод|текст|слова|караоке|кавер|реакция|лайв|"
    r"тизер|выступление|концерт|live\s+session|сниппет)\b"
)
# Фанатское видео — откуда бы ни было
_FAN = re.compile(
    r"\b(?:amv|gmv|pmv|mmd|fan\s*made|fanmade|fan\s+video|fan\s+animation|fan\s+clip|"
    r"fan\s+edit|edit|edits|animatic|minecraft|roblox|gacha(?:\s+(?:life|club))?|sfm|"
    r"vrchat|speedpaint|meme|tiktok|fortnite|lego|sims|фан|фанатск\w*|майнкрафт|роблокс)\b"
)
# Анимация — фанатская, если ролик не с канала автора (у автора бывает и анимационный клип)
_ANIMATION = re.compile(r"\b(?:animation|animated|animate[sd]?|анимаци\w*|мультик)\b")
_OFFICIAL = re.compile(
    r"\b(?:official\s+(?:music\s+)?video|music\s+video|official\s+mv|m\s+v|mv|official\s+clip|"
    r"videoclip|video\s+oficial|clip\s+officiel|клип|премьера\s+клипа|официальный\s+клип|"
    r"официальное\s+видео)\b"
)
_LABEL_NOISE = re.compile(
    r"\b(?:records?|recordings|music|musik|entertainment|ent|label|labels|group|inc|llc|ltd|"
    r"publishing|productions?|media|official|рекордс|рекордз|мьюзик|лейбл)\b"
)


@dataclass(frozen=True)
class ClipTrack:
    """Что ищем: трек с исполнителями, длиной и лейблом альбома (Deezer)."""

    title: str
    artists: list[str]
    duration_s: float
    label: str | None = None


@dataclass
class Pick:
    video_id: str
    title: str
    channel: str
    duration_s: float
    views: int = 0
    verified: bool = False
    score: float = 0.0
    source: str | None = None
    official_title: bool = False
    reasons: list[str] = field(default_factory=list)

    @property
    def ok(self) -> bool:
        return not self.reasons

    @property
    def rank(self) -> float:
        return SOURCE_WEIGHT.get(self.source or "", 0.0)

    def to_json(self) -> dict[str, Any]:
        return {
            "videoId": self.video_id,
            "title": self.title,
            "channel": self.channel,
            "durationS": self.duration_s,
            "views": self.views,
            "verified": self.verified,
            "score": round(self.score, 3),
            "source": self.source,
            "rejected": self.reasons or None,
        }


def entry(item: dict[str, Any]) -> Pick | None:
    """Ролик из выдачи yt-dlp (плоской или полной)."""
    video_id = item.get("id")
    if not video_id or item.get("ie_key") not in (None, "Youtube"):
        return None
    return Pick(
        video_id=str(video_id),
        title=str(item.get("title") or ""),
        channel=str(item.get("channel") or item.get("uploader") or ""),
        duration_s=float(item.get("duration") or 0),
        views=int(item.get("view_count") or 0),
        verified=bool(item.get("channel_is_verified")),
    )


def _clean_label(label: str) -> str:
    return " ".join(_LABEL_NOISE.sub(" ", fold(label)).split())


def same_name(a: str, b: str) -> bool:
    """Строгое совпадение имён (уже после `fold`): равны, равны в транслите или почти равны.
    Без «короткое входит в длинное»: канал «Miyagi Fan Clips» — не канал Miyagi."""
    if not a or not b:
        return False
    # «Макс Корж» → «maks korzh», а канал — «Max Korzh»
    ta, tb = (transliterate(s).replace("ks", "x") for s in (a, b))
    return a == b or ta == tb or max(fuzz.ratio(a, b), fuzz.ratio(ta, tb)) >= 88


def channel_is_artist(channel: str, artists: list[str]) -> bool:
    """Канал исполнителя: «CG5», «LadyGagaVEVO», «Miyagi & Andy Panda» (общий канал дуэта)."""
    folded_artists = [a for name in artists for a in split_names(name)]
    cleaned = clean_channel(channel)
    whole = fold(cleaned)
    names = [whole, *split_names(cleaned)]
    return any(same_name(n, a) for n in names for a in folded_artists)


def channel_source(
    pick: Pick, track: ClipTrack, artist_in_title: bool, official: bool
) -> str | None:
    """Чей канал: исполнителя, проверенный, VEVO, лейбла — или ничей."""
    channel = pick.channel.strip()
    if not channel or channel.casefold().endswith("- topic"):
        return None
    by_name = channel_is_artist(channel, track.artists)
    if by_name and channel.casefold().replace(" ", "").endswith("vevo"):
        return "vevo"
    if by_name:
        return "artist"
    if pick.verified and artist_in_title and official:
        return "official"
    if track.label and artist_in_title:
        label = _clean_label(track.label)
        own = _clean_label(channel)
        if label and own and (name_match(label, own) or label == own):
            return "label"
    return None


def assess(pick: Pick, track: ClipTrack) -> Pick:
    """Годится ли ролик в клип трека: причины отказа — в `reasons`, сила — в `score`, чей канал —
    в `source`."""
    artists = track.artists
    expected = parse_title(track.title, artists)
    got = parse_title(pick.title, artists)
    video_folded = fold(pick.title)
    track_folded = fold(track.title)
    folded_artists = [a for name in artists for a in split_names(name)]

    similarity = text_similarity(expected.base, got.base) if expected.base else 0.0
    if similarity < MIN_TITLE_SIMILARITY and f" {expected.base} " not in f" {video_folded} ":
        pick.reasons.append("title")

    if pick.channel.casefold().endswith("- topic"):
        pick.reasons.append("topic_channel")
    in_title = any(name_match(p, a) for p in got.prefix_artists for a in folded_artists) or any(
        len(a) >= 3 and f" {a} " in f" {video_folded} " for a in folded_artists
    )
    pick.official_title = bool(_OFFICIAL.search(video_folded))
    pick.source = channel_source(pick, track, in_title, pick.official_title)
    if not (pick.source in ("artist", "vevo") or in_title):
        pick.reasons.append("artist")

    extra = set(got.markers & STRONG_MARKERS) - set(expected.markers)
    words = {m.group(0) for m in _NOT_A_CLIP.finditer(video_folded)}
    words -= {m.group(0) for m in _NOT_A_CLIP.finditer(track_folded)}
    if extra or words:
        pick.reasons.append("version:" + ",".join(sorted(extra | words)))

    fan = {m.group(0) for m in _FAN.finditer(video_folded)}
    fan -= {m.group(0) for m in _FAN.finditer(track_folded)}
    if pick.source not in ("artist", "vevo", "official"):
        fan |= {m.group(0) for m in _ANIMATION.finditer(video_folded)}
    if fan:
        pick.reasons.append("fan:" + ",".join(sorted(fan)))

    if pick.source is None:
        pick.reasons.append("not_official")

    duration_s = track.duration_s
    delta = pick.duration_s - duration_s
    if not pick.duration_s:
        pick.reasons.append("duration_unknown")
    elif duration_s and not (
        abs(delta) <= duration_s * DURATION_SHARE or 0 < delta <= INTRO_EXTRA_S
    ):
        pick.reasons.append("duration")

    score = pick.rank + 0.5 * similarity
    if pick.official_title:
        score += 0.4
    if pick.views:
        score += min(0.3, math.log10(pick.views + 1) / 30)
    if pick.duration_s and duration_s:
        score -= min(0.5, abs(delta) / duration_s)
    pick.score = score
    return pick


def choose(
    items: list[dict[str, Any]],
    track: ClipTrack,
    exclude: set[str] | frozenset[str] = frozenset(),
) -> tuple[Pick | None, list[Pick]]:
    """Лучший официальный клип из выдачи и все кандидаты (годные по оценке, затем отвергнутые)."""
    picks: list[Pick] = []
    seen: set[str] = set()
    for item in items:
        pick = entry(item)
        if pick is None or pick.video_id in seen:
            continue
        seen.add(pick.video_id)
        assess(pick, track)
        if pick.video_id in exclude:
            pick.reasons.append("excluded")
        picks.append(pick)
    # Годные — первыми; из отвергнутых другие песни — в самом конце: администратору они ни к чему
    picks.sort(key=lambda p: (p.ok, "title" not in p.reasons, p.score), reverse=True)
    best = picks[0] if picks and picks[0].ok else None
    return best, picks


def unknown_channel(pick: Pick) -> bool:
    """Ролик без канала в выдаче, который иначе годился бы: официальность не проверить."""
    blocking = {r.split(":")[0] for r in pick.reasons}
    return not pick.channel.strip() and blocking <= {"not_official", "artist"}


def better(candidate: Pick, current: Pick | None) -> bool:
    """Заменить ли нарезанный клип найденным: текущий не годится или найденный — с канала выше рангом."""
    if current is None or not current.ok:
        return True
    return candidate.video_id != current.video_id and candidate.rank > current.rank
