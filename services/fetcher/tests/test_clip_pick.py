"""Отбор клипа: только официальное видео, в приоритете — с канала автора. Выдачи — настоящие
(поиск YouTube через yt-dlp, ноябрь 2026), сокращены до нужных полей."""

from __future__ import annotations

from pathlib import Path
from typing import Any

from dalywave_fetcher.clip import ClipMaker, VideoSupport
from dalywave_fetcher.clip_pick import (
    ClipTrack,
    Pick,
    assess,
    better,
    channel_is_artist,
    choose,
    entry,
)
from dalywave_fetcher.config import Settings
from dalywave_fetcher.download import Downloader, DownloadError


def item(
    video_id: str,
    title: str,
    channel: str,
    duration: float,
    views: int = 0,
    verified: bool = False,
) -> dict[str, Any]:
    return {
        "id": video_id,
        "ie_key": "Youtube",
        "title": title,
        "channel": channel,
        "duration": duration,
        "view_count": views,
        "channel_is_verified": verified or None,
    }


def reasons(picks: list[Pick]) -> dict[str, list[str]]:
    return {p.video_id: p.reasons for p in picks}


def rejected_for(picks: list[Pick], video_id: str, prefix: str) -> bool:
    return any(r.startswith(prefix) for r in reasons(picks)[video_id])


# Вместо клипа «Sleep Well» играла фанатская Minecraft-анимация: у неё в названии
# «Music Video», и старый отбор поставил её выше клипа с канала CG5
SLEEP_WELL = [
    item(
        "TbWV7gCPiYc",
        "CG5 - Sleep Well (from Poppy Playtime: Chapter 3)",
        "CG5",
        208,
        321_169_270,
        True,
    ),
    item(
        "28I_6qvI3Dk",
        '"Sleep Well" (from Poppy Playtime: Chapter 3) by CG5 - OFFICIAL MUSIC VIDEO',
        "Mob Entertainment",
        208,
        116_533_734,
        True,
    ),
    item(
        "lsZkA0SZHN8",
        "CG5 - Sleep Well (Official Live Performance)",
        "CG5",
        207,
        39_123_513,
        True,
    ),
    item("H9q9QZgWF5o", "CG5 - SLEEP WELL (Lyrics)", "7clouds Trance", 208, 8_128_753),
    item(
        "rC35_gNfdU0",
        '"Sleep Well" | Animated Poppy Playtime Minecraft Music Video (Song By @CG5 )',
        "WilliamaYT and Level Up Productions",
        208,
        1_573_503,
    ),
    item(
        "8y2kZ3lG3uQ",
        "TADC - Sleep Well [CG5] [TADC AMV] [SPOILERS!] [EPILEPSY WARNING!]",
        "josse13579",
        208,
        207_748,
    ),
    item(
        "6w9xubgB1oM",
        "CG5 - Sleep Well ft. Mob Entertainment (Poppy Playtime: Chapter 3) | Official Audio",
        "RosendoTv",
        208,
        2_096_488,
    ),
    item(
        "7gwPbINaNC4",
        "SLEEP WELL (CG5) ANIMATION MAP COMPLETE . POPPY PLAYTIME CHAPTER 3 . SMILING CRITTERS",
        "Karl_Animatez",
        207,
        1_279_465,
    ),
]
SLEEP_WELL_TRACK = ClipTrack("Sleep Well", ["CG5", "Chi-Chi", "Kathy-Chan", "Cami-Cat"], 208)


def test_sleep_well_takes_the_clip_from_the_artist_channel() -> None:
    best, picks = choose(SLEEP_WELL, SLEEP_WELL_TRACK)
    assert best is not None and best.video_id == "TbWV7gCPiYc"
    assert best.source == "artist"
    # Фанатская анимация, AMV и «карта» — фанатское; текст, звук и выступление — не клип
    assert rejected_for(picks, "rC35_gNfdU0", "fan:")
    assert "not_official" in reasons(picks)["rC35_gNfdU0"]
    assert rejected_for(picks, "8y2kZ3lG3uQ", "fan:")
    assert rejected_for(picks, "7gwPbINaNC4", "fan:")
    assert rejected_for(picks, "H9q9QZgWF5o", "version:")
    assert rejected_for(picks, "6w9xubgB1oM", "version:")
    assert rejected_for(picks, "lsZkA0SZHN8", "version:")
    # Клип издателя игры — официальный, но ниже канала автора
    mob = next(p for p in picks if p.video_id == "28I_6qvI3Dk")
    assert mob.ok and mob.source == "official" and mob.score < best.score
    # Годные — первыми, по убыванию оценки
    assert [p.ok for p in picks] == sorted((p.ok for p in picks), reverse=True)


def test_without_the_artist_channel_the_publisher_clip_is_taken() -> None:
    best, _ = choose(SLEEP_WELL, SLEEP_WELL_TRACK, exclude={"TbWV7gCPiYc"})
    assert best is not None and best.video_id == "28I_6qvI3Dk"


def test_lady_gaga_official_video_not_behind_the_scenes_or_reuploads() -> None:
    items = [
        item(
            "qrO4YZeyl0I",
            "Lady Gaga - Bad Romance (Official Music Video)",
            "Lady Gaga",
            309,
            2_095_611_200,
            True,
        ),
        item("BpjQro8BB_0", "[4K] Lady Gaga - Bad Romance (Music Video)", "MVIDEO4K", 308, 40_918),
        item(
            "OXm6_v0rMCY",
            "Lady Gaga - Bad Romance (Gaga Live Sydney Monster Hall)",
            "Lady Gaga",
            285,
            31_365_306,
            True,
        ),
        item(
            "X2tR1VHCOEc",
            "Cruella - Bad Romance - Lady Gaga - music video",
            "Mela Bird",
            301,
            125_079_772,
        ),
        item(
            "R-Loodl2cgA",
            "Lady Gaga - Bad Romance (Official Behind the Scenes)",
            "Lady Gaga",
            223,
            12_197_424,
            True,
        ),
    ]
    best, picks = choose(items, ClipTrack("Bad Romance", ["Lady Gaga"], 294))
    assert best is not None and best.video_id == "qrO4YZeyl0I"
    assert "not_official" in reasons(picks)["BpjQro8BB_0"]
    assert "not_official" in reasons(picks)["X2tR1VHCOEc"]
    assert rejected_for(picks, "OXm6_v0rMCY", "version:")
    assert rejected_for(picks, "R-Loodl2cgA", "version:")


def test_label_channel_and_kpop_mv() -> None:
    # Клип — на канале лейбла (Deezer: JYP Entertainment); у группы — выступления и танцы
    items = [
        item("OvioeS1ZZ7o", 'Stray Kids "MANIAC" M/V', "JYP Entertainment", 203, 360_300_854),
        item(
            "APB-AKpnKT4",
            'Stray Kids "MANIAC" Performance | MTV Fresh Out',
            "Stray Kids",
            199,
            123_095_142,
            True,
        ),
        item(
            "Ip28E1XItvY",
            'Stray Kids "MANIAC" Dance Practice Video',
            "Stray Kids",
            193,
            19_757_978,
            True,
        ),
        item("XhM1B0rg8SY", 'Stray Kids "MANIAC" M/V Reaction', "Stray Kids", 349, 8_003_083, True),
    ]
    track = ClipTrack("MANIAC", ["Stray Kids"], 202, label="JYP Entertainment")
    best, picks = choose(items, track)
    assert best is not None and best.video_id == "OvioeS1ZZ7o"
    assert best.source == "label" and best.official_title
    for video_id in ("APB-AKpnKT4", "Ip28E1XItvY", "XhM1B0rg8SY"):
        assert rejected_for(picks, video_id, "version:"), video_id
    # Без лейбла и без галочки канал лейбла — просто чужой канал
    best, _ = choose(items, ClipTrack("MANIAC", ["Stray Kids"], 202))
    assert best is None


def test_feat_and_vevo() -> None:
    items = [
        item(
            "JQbjS0_ZfJ0",
            "Kendrick Lamar, SZA - All The Stars",
            "Kendrick Lamar",
            235,
            775_140_399,
            True,
        ),
        item(
            "CJnXXxcm-zU",
            "All the Stars- Kendrick Lamar, SZA: Black Panther Music Video",
            "JakeHeeWonKiSungJayNoo",
            237,
            653_174,
        ),
        item(
            "yvOh7vVqlaE",
            "Kendrick Lamar, SZA - All The Stars (Lyrics)",
            "SyrebralVibes",
            231,
            74_305_128,
            True,
        ),
    ]
    best, picks = choose(items, ClipTrack("All The Stars", ["Kendrick Lamar", "SZA"], 232))
    assert best is not None and best.video_id == "JQbjS0_ZfJ0" and best.source == "artist"
    assert "not_official" in reasons(picks)["CJnXXxcm-zU"]
    # Канал второго исполнителя — тоже канал автора; VEVO исполнителя — официальный
    sza = assess(
        entry(item("s", "SZA - All The Stars (Official Video)", "SZAVEVO", 233)),  # type: ignore[arg-type]
        ClipTrack("All The Stars", ["Kendrick Lamar", "SZA"], 232),
    )
    assert sza.ok and sza.source == "vevo"


def test_russian_artists_and_transliterated_channels() -> None:
    items = [
        item(
            "sdnbZkKvVDY",
            "Макс Корж — Мотылёк (official video)",
            "Max Korzh",
            254,
            100_186_097,
            True,
        ),
        item(
            "e-Ci-qTjKqU",
            'Макс Корж - Мотылек (LIVE) Киев. Стадион "Динамо"',
            "Макс Корж - OFFICIAL FANPAGE",
            208,
            648_048,
            True,
        ),
        item(
            "DdXm1DoKolk", "Макс Корж - Мотылек", "Макс Корж - OFFICIAL FANPAGE", 238, 300_761, True
        ),
        item(
            "UB1tBu7bD0s",
            "Макс Корж-Мотылёк (официальный клип)",
            "Александр Козловский",
            179,
            134,
        ),
    ]
    best, picks = choose(items, ClipTrack("Мотылёк", ["Макс Корж"], 250))
    assert best is not None and best.video_id == "sdnbZkKvVDY" and best.source == "artist"
    # Фан-страница — не канал артиста, хоть и с галочкой и его именем
    assert "not_official" in reasons(picks)["DdXm1DoKolk"]
    assert "not_official" in reasons(picks)["UB1tBu7bD0s"]
    assert channel_is_artist("ПОШЛАЯ МОЛЛИ", ["Пошлая Молли"])
    assert channel_is_artist("Poshlaya Molly", ["Пошлая Молли"])
    assert channel_is_artist("MIYAGI & ANDY PANDA", ["Miyagi", "Andy Panda"])
    assert not channel_is_artist("Hajime Fan Clips", ["Miyagi", "Andy Panda"])


def test_no_official_clip_means_no_clip() -> None:
    # Только звук на канале исполнителя и перезаливы — клипа нет, показывается обложка
    items = [
        item(
            "wyufEub7bM0",
            "LOWBORN // Light Me Up (feat. VISTA) // Official Audio",
            "LOWBORN",
            194,
            11_707,
            True,
        ),
        item("dr63ZWfpeZg", "Lowborn, VISTA - Light Me Up | Sub Español | HD", "LEGION", 194, 289),
        item(
            "w4JEhuRN4jI",
            "LOWBORN [Nightcore] - Light Me Up (feat. VISTA)",
            "Night Steel",
            169,
            569,
        ),
    ]
    best, picks = choose(items, ClipTrack("Light Me Up", ["Lowborn", "vista"], 194))
    assert best is None
    assert rejected_for(picks, "wyufEub7bM0", "version:")
    # Перезалив на проверенном канале без пометки клипа — не официальный (Kosandra у Black Beats)
    kosandra = [
        item(
            "1OzGrXpuw54",
            "Miyagi & Andy Panda - Кассандра (Kosandra)",
            "Black Beats",
            221,
            177_123_252,
            True,
        ),
        item(
            "mueaELCxHH8", "Miyagi & Andy Panda - Kosandra КЛИП 2020", "Hajime Fan Clips", 222, 874
        ),
    ]
    best, picks = choose(kosandra, ClipTrack("Kosandra", ["Miyagi", "Andy Panda"], 220))
    assert best is None
    assert "not_official" in reasons(picks)["1OzGrXpuw54"]


def test_duration_within_fifteen_percent_or_a_minute_of_intro() -> None:
    track = ClipTrack("Skibidi", ["Little Big"], 190)

    def judged(duration: float) -> Pick:
        pick = entry(
            item("x", "LITTLE BIG – SKIBIDI (official music video)", "Little Big", duration)
        )
        assert pick is not None
        return assess(pick, track)

    assert judged(193).ok
    assert judged(165).ok  # −13 %
    assert "duration" in judged(150).reasons  # −21 %: нарезка
    assert judged(245).ok  # +55 с: вступление клипа
    assert "duration" in judged(290).reasons
    assert judged(193).score > judged(245).score


def test_animation_from_the_artist_is_a_clip() -> None:
    # Анимационный клип на канале автора — официальный; та же анимация у фаната — нет
    own = entry(item("a", "CG5 - Sleep Well (Animated Music Video)", "CG5", 208, 1000, True))
    fan = entry(item("f", "CG5 - Sleep Well (Animated Music Video)", "Animator", 208, 1000))
    assert own is not None and fan is not None
    assert assess(own, SLEEP_WELL_TRACK).ok
    assert rejected_for([assess(fan, SLEEP_WELL_TRACK)], "f", "fan:")


def test_a_better_channel_replaces_the_cut_clip() -> None:
    best, picks = choose(SLEEP_WELL, SLEEP_WELL_TRACK)
    assert best is not None
    fan = next(p for p in picks if p.video_id == "rC35_gNfdU0")
    mob = next(p for p in picks if p.video_id == "28I_6qvI3Dk")
    assert better(best, fan)  # фанатское — заменить
    assert better(best, mob)  # издатель ниже канала автора — заменить
    assert not better(best, best)
    assert better(best, None)


# ───────────── режимы задания ─────────────


class FakeMaker(ClipMaker):
    """Поиск — из списка, сведения о ролике — из словаря, нарезка — без сети и ffmpeg."""

    def __init__(self, items: list[dict[str, Any]], infos: dict[str, dict[str, Any]]) -> None:
        super().__init__(Downloader(Settings(data_dir=Path("data"))))
        self.items = items
        self.infos = infos
        self.queries: list[str] = []
        self.cut: list[str] = []

    def support(self) -> VideoSupport:
        return VideoSupport("libx264")

    def search(self, query: str) -> list[dict[str, Any]]:
        self.queries.append(query)
        return self.items

    def video_info(self, video_id: str) -> Pick:
        if video_id not in self.infos:
            raise DownloadError("unavailable", "Video unavailable")
        pick = entry({**self.infos[video_id], "id": video_id})
        assert pick is not None
        return pick

    def _cut(self, pick: Pick, *_: Any, **__: Any) -> dict[str, Any]:
        self.cut.append(pick.video_id)
        return {"status": "ready", "videoId": pick.video_id, "source": pick.source}


def make(maker: FakeMaker, **kw: Any) -> dict[str, Any]:
    return maker.make(
        title="Sleep Well",
        artists=["CG5", "Chi-Chi", "Kathy-Chan", "Cami-Cat"],
        duration_ms=208_000,
        hook_start_ms=60_000,
        length_ms=12_000,
        **kw,
    )


FAN_INFO = {
    "rC35_gNfdU0": item(
        "rC35_gNfdU0",
        '"Sleep Well" | Animated Poppy Playtime Minecraft Music Video (Song By @CG5 )',
        "WilliamaYT and Level Up Productions",
        208,
    )
}


def test_auto_cuts_the_artist_clip_and_reports_candidates() -> None:
    maker = FakeMaker(SLEEP_WELL, {})
    result = make(maker)
    assert result["status"] == "ready" and maker.cut == ["TbWV7gCPiYc"]
    # Клип с канала автора нашёлся первым запросом — второй не нужен
    assert len(maker.queries) == 1
    assert len(result["candidates"]) == 8
    assert result["candidates"][0]["videoId"] == "TbWV7gCPiYc"
    assert result["candidates"][0]["source"] == "artist"


def test_search_mode_only_lists_candidates() -> None:
    maker = FakeMaker(SLEEP_WELL, {})
    result = make(maker, mode="search")
    assert result["status"] == "none" and maker.cut == []
    assert len(maker.queries) == 2
    assert {c["videoId"] for c in result["candidates"]} >= {"TbWV7gCPiYc", "28I_6qvI3Dk"}


def test_pinned_mode_cuts_the_chosen_video_without_judging() -> None:
    maker = FakeMaker([], FAN_INFO)
    result = make(maker, mode="pinned", video_id="rC35_gNfdU0")
    assert result["status"] == "ready" and maker.cut == ["rC35_gNfdU0"]
    assert maker.queries == []


def test_review_replaces_a_fan_video_and_keeps_the_right_one() -> None:
    # Старый клип — фанатская анимация: нашёлся клип с канала автора — перерезать
    maker = FakeMaker(SLEEP_WELL, FAN_INFO)
    result = make(maker, mode="review", current_video_id="rC35_gNfdU0")
    assert result["status"] == "ready" and maker.cut == ["TbWV7gCPiYc"]
    # Старый клип и есть лучший — оставить, не тратя нарезку
    maker = FakeMaker(SLEEP_WELL, {})
    result = make(maker, mode="review", current_video_id="TbWV7gCPiYc")
    assert result["status"] == "kept" and maker.cut == []
    assert result["videoId"] == "TbWV7gCPiYc" and result["source"] == "artist"
    # Фанатский клип, а официального нет — клип убрать
    maker = FakeMaker([SLEEP_WELL[4]], FAN_INFO)
    result = make(maker, mode="review", current_video_id="rC35_gNfdU0")
    assert result["status"] == "none" and maker.cut == []


def blind() -> list[dict[str, Any]]:
    """Выдача без каналов и просмотров — так YouTube отдаёт её время от времени."""
    return [{**i, "channel": "", "view_count": 0, "channel_is_verified": None} for i in SLEEP_WELL]


def test_search_without_channels_asks_the_videos_and_never_decides_none() -> None:
    # Живой прогон: перепроверка по такой выдаче сочла бы клип CG5 «не официальным»
    maker = FakeMaker(blind(), {i["id"]: i for i in SLEEP_WELL})
    result = make(maker)
    assert result["status"] == "ready" and maker.cut == ["TbWV7gCPiYc"]
    assert result["details"]["filledChannels"]
    # Канал не узнать — не «клипа нет», а сбой: задание повторится, хороший клип не уберётся
    maker = FakeMaker(blind(), {})
    result = make(maker, mode="review", current_video_id="TbWV7gCPiYc")
    assert result["status"] == "error" and result["error"].startswith("search_degraded")
    assert maker.cut == []
