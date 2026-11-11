"""Данные для дизайн-макетов: тестовые артисты и похожие, альбомы, треки, обложки, превью.

Запуск: python3 -I design/fixtures/build.py
Результат: design/fixtures/data.json, data.js (window.DW_DATA), img/, audio/ — img и audio не коммитятся.
"""

from __future__ import annotations

import json
import pathlib
import time
import urllib.parse
import urllib.request

ROOT = pathlib.Path(__file__).resolve().parent
IMG = ROOT / "img"
AUDIO = ROOT / "audio"

# Тестовые артисты (id Deezer) и похожие на них — по поиску
SEED = {
    67614242: "5opka",
    52101022: "Mzlff",
    195491497: "Rushex",
    13828465: "Lowborn",
    13172915: "Mytch",
    181771087: "Parker The Bandit",
}
SIMILAR = [
    "Пошлая Молли", "SQWOZ BAB", "дора", "МУККА", "Noize MC", "Дайте танк (!)",
    "Валентин Стрыкало", "кис-кис", "Электрофорез", "Sayfalse", "DJ FKU", "CHASHKAKEFIRA",
    "Bohnes", "Poe the Passenger", "Call Me Karizma", "AViVA", "Adam Jensen", "VOILÀ",
    "Demi the Daredevil", "Molchat Doma", "2rbina 2rista", "Pepel Nahudi",
]

_last = [0.0]


def api(path: str) -> dict:
    # Deezer: около 50 запросов за 5 секунд
    wait = 0.12 - (time.monotonic() - _last[0])
    if wait > 0:
        time.sleep(wait)
    _last[0] = time.monotonic()
    with urllib.request.urlopen("https://api.deezer.com/" + path, timeout=30) as r:
        data = json.load(r)
    if isinstance(data, dict) and "error" in data:
        raise RuntimeError(f"{path}: {data['error']}")
    return data


def download(url: str | None, dest: pathlib.Path) -> str | None:
    if not url:
        return None
    if not dest.exists():
        with urllib.request.urlopen(url, timeout=60) as r:
            dest.write_bytes(r.read())
    return str(dest.relative_to(ROOT))


def track_out(t: dict, album: dict | None = None) -> dict:
    alb = album or t.get("album") or {}
    return {
        "id": t["id"],
        "title": t["title"],
        "titleShort": t.get("title_short") or t["title"],
        "version": t.get("title_version") or "",
        "artistId": t["artist"]["id"],
        "artist": t["artist"]["name"],
        "albumId": alb.get("id"),
        "album": alb.get("title"),
        "durationMs": int(t.get("duration") or 0) * 1000,
        "explicit": bool(t.get("explicit_lyrics")),
        "rank": t.get("rank"),
        "isrc": t.get("isrc"),
        "position": t.get("track_position"),
    }


def main() -> None:
    IMG.mkdir(exist_ok=True)
    AUDIO.mkdir(exist_ok=True)

    artist_ids: dict[int, bool] = {aid: True for aid in SEED}
    for name in SIMILAR:
        found = api("search/artist?limit=1&q=" + urllib.parse.quote(name)).get("data", [])
        if found:
            artist_ids.setdefault(found[0]["id"], False)

    artists, albums, tracks = [], {}, {}
    for aid, seed in artist_ids.items():
        a = api(f"artist/{aid}")
        pic = download(a.get("picture_xl"), IMG / f"artist-{aid}.jpg")
        top = api(f"artist/{aid}/top?limit=8").get("data", [])
        related = [r["id"] for r in api(f"artist/{aid}/related?limit=10").get("data", [])]
        artists.append({
            "id": aid, "name": a["name"], "picture": pic, "fans": a.get("nb_fan"), "albumsCount": a.get("nb_album"),
            "seed": seed, "topTrackIds": [t["id"] for t in top], "relatedIds": related,
        })
        for t in top:
            tracks.setdefault(t["id"], track_out(t))
        # Свежие релизы артиста: у этих артистов много синглов, берём разные типы
        rel = api(f"artist/{aid}/albums?limit=40").get("data", [])
        rel.sort(key=lambda x: x.get("release_date") or "", reverse=True)
        picked = [x for x in rel if x.get("record_type") == "album"][:2] + [x for x in rel if x.get("record_type") != "album"][:2]
        for alb in picked[: (4 if seed else 2)]:
            albums.setdefault(alb["id"], {"id": alb["id"], "seed": seed})
        print(f"{a['name']}: {len(top)} top, {len(picked)} releases")

    # Полные данные альбомов с треками
    for alb_id in list(albums):
        d = api(f"album/{alb_id}")
        cover = download(d.get("cover_xl"), IMG / f"album-{alb_id}.jpg")
        album_tracks = [track_out(t, d) for t in d.get("tracks", {}).get("data", [])]
        for t in album_tracks:
            t["position"] = album_tracks.index(t) + 1
            tracks.setdefault(t["id"], t)
        albums[alb_id] = {
            "id": alb_id, "title": d["title"], "artistId": d["artist"]["id"], "artist": d["artist"]["name"],
            "cover": cover, "type": d.get("record_type"), "releaseDate": d.get("release_date"),
            "label": d.get("label"), "genres": [g["name"] for g in d.get("genres", {}).get("data", [])],
            "durationMs": int(d.get("duration") or 0) * 1000, "fans": d.get("fans"), "explicit": bool(d.get("explicit_lyrics")),
            "trackIds": [t["id"] for t in album_tracks], "seed": albums[alb_id]["seed"],
        }

    # Обложка для каждого трека — обложка его альбома (если альбом в выборке), иначе маленькая из Deezer
    for t in tracks.values():
        alb = albums.get(t["albumId"])
        if alb:
            t["cover"] = alb["cover"]
        else:
            d = api(f"track/{t['id']}")
            t["cover"] = download(d.get("album", {}).get("cover_big"), IMG / f"album-{t['albumId']}.jpg")
            t["isrc"] = t.get("isrc") or d.get("isrc")

    # Превью для живой волны: по треку от разных тестовых артистов
    previews = []
    for aid in list(SEED)[:5]:
        artist = next(a for a in artists if a["id"] == aid)
        for tid in artist["topTrackIds"][:1]:
            d = api(f"track/{tid}")
            if d.get("preview"):
                previews.append({"trackId": tid, "file": download(d["preview"], AUDIO / f"{tid}.mp3"), "bpm": d.get("bpm") or None})

    data = {
        "generatedAt": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        "me": {"name": "Захар"},
        "artists": artists,
        "albums": list(albums.values()),
        "tracks": list(tracks.values()),
        "previews": previews,
    }
    (ROOT / "data.json").write_text(json.dumps(data, ensure_ascii=False, indent=1))
    (ROOT / "data.js").write_text("window.DW_DATA = " + json.dumps(data, ensure_ascii=False) + ";\n")
    print(f"artists={len(artists)} albums={len(albums)} tracks={len(tracks)} previews={len(previews)}")


if __name__ == "__main__":
    main()
