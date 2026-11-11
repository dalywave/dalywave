"""Прогон эталонного набора и калибровка порога отпечатков."""

from __future__ import annotations

import itertools
import json
import platform
import statistics
import sys
import time
from collections import Counter
from collections.abc import Callable
from datetime import UTC, datetime
from pathlib import Path
from typing import Any

from .acquire import AcquireResult, Fetcher
from .download import tool_versions
from .verify import DEFAULT_THRESHOLD, VerifyError, align, fingerprint

STAGES = ("deezer", "match", "preview", "download", "verify", "total")
AUDIO_SUFFIXES = (".opus", ".m4a")


def _log(msg: str) -> None:
    print(msg, file=sys.stderr, flush=True)


def load_set(path: Path) -> list[dict[str, Any]]:
    return list(json.loads(path.read_text(encoding="utf-8"))["tracks"])


def _row(entry: dict[str, Any], result: AcquireResult) -> dict[str, Any]:
    data = result.to_json(max_candidates=3)
    chosen = data["chosen"]
    first = result.attempts[0] if result.attempts else None
    errors = [a.error.code for a in result.attempts if a.error]
    if result.error and result.error["code"] not in errors:
        errors.append(result.error["code"])
    row: dict[str, Any] = {
        "deezerId": entry["deezerId"],
        "expected": {
            "artist": entry.get("artist"),
            "title": entry.get("title"),
            "durationS": result.track.duration_s if result.track else entry.get("durationS"),
        },
        "tags": entry.get("tags", []),
        "status": result.status,
        "viableCandidates": len(result.match.viable()) if result.match else 0,
        "attempts": len(result.attempts),
        "firstAttemptVerified": bool(first and first.verify and first.verify.verified),
        "errors": errors,
        "timingsMs": result.timings_ms,
        "chosen": None,
        "attemptsDetail": data["attempts"],
        "topCandidates": data["candidates"],
        "steps": data["steps"],
    }
    if chosen:
        c, d, v = chosen["candidate"], chosen["download"], chosen["verify"]
        row["chosen"] = {
            "videoId": c["videoId"],
            "title": c["title"] or d["title"],
            "artists": c["artists"],
            "album": c["album"],
            "durationMs": d["durationMs"],
            "source": c["source"],
            "kind": c["kind"],
            "score": c["score"],
            "similarity": v["similarity"] if v else None,
            "offsetS": v["offsetS"] if v else None,
            "itag": d["itag"],
            "codec": d["codec"],
            "abrKbps": d["abrKbps"],
            "sampleRate": d["sampleRate"],
            "sizeBytes": d["sizeBytes"],
            "container": d["container"],
            "originalContainer": d["originalContainer"],
            "client": d["client"],
            "poToken": d["poToken"],
            "downloadTimingsMs": d["timingsMs"],
        }
    return row


def summarize(rows: list[dict[str, Any]]) -> dict[str, Any]:
    total = len(rows)
    statuses = Counter(r["status"] for r in rows)
    chosen = [r["chosen"] for r in rows if r["chosen"]]
    downloaded = sum(1 for r in rows if any(a["download"] for a in r["attemptsDetail"]))

    def rate(n: int) -> float:
        return round(n / total, 3) if total else 0.0

    timings = {}
    for stage in STAGES:
        values = [r["timingsMs"].get(stage, 0) for r in rows if r["timingsMs"]]
        if values:
            timings[stage] = {
                "median": int(statistics.median(values)),
                "max": max(values),
                "mean": int(statistics.mean(values)),
            }
    for key, name in (("extract", "ytExtract"), ("adWait", "ytAdWait"), ("transfer", "ytTransfer")):
        values = [c["downloadTimingsMs"].get(key, 0) for c in chosen]
        if values:
            timings[name] = {"median": int(statistics.median(values)), "max": max(values)}
    abr = [c["abrKbps"] for c in chosen if c["abrKbps"]]
    found = sum(1 for r in rows if r["viableCandidates"] > 0)
    wrong_top = sum(
        1
        for r in rows
        if r["attemptsDetail"]
        and r["attemptsDetail"][0]["verify"]
        and r["attemptsDetail"][0]["verify"]["verified"] is False
    )
    return {
        "total": total,
        "found": found,
        "foundRate": rate(found),
        "downloaded": downloaded,
        "verified": statuses.get("verified", 0),
        "verifiedRate": rate(statuses.get("verified", 0)),
        "statuses": dict(statuses),
        "firstAttemptVerified": sum(1 for r in rows if r["firstAttemptVerified"]),
        # Лучший кандидат скачан, но отпечаток не совпал: без проверки это была бы ошибка.
        "topCandidateRejectedByFingerprint": wrong_top,
        "errors": dict(Counter(e for r in rows for e in r["errors"])),
        "itags": dict(Counter(c["itag"] for c in chosen)),
        "codecs": dict(Counter(c["codec"] for c in chosen)),
        "abrKbps": {
            "median": round(statistics.median(abr), 1) if abr else None,
            "min": min(abr, default=None),
            "max": max(abr, default=None),
        },
        "clients": dict(Counter(c["client"] for c in chosen)),
        "poToken": sum(1 for c in chosen if c["poToken"]),
        "sources": dict(Counter(f"{c['source']}/{c['kind']}" for c in chosen)),
        "timingsMs": timings,
    }


def run_bench(
    fetcher: Fetcher,
    set_path: Path,
    out_path: Path,
    *,
    label: str,
    audio_dir: Path,
    delay_s: float = 5.0,
    only: list[int] | None = None,
    log: Callable[[str], None] = _log,
) -> dict[str, Any]:
    entries = [e for e in load_set(set_path) if not only or e["deezerId"] in only]
    started_at = datetime.now(UTC)
    rows = []
    for i, entry in enumerate(entries, 1):
        if i > 1:
            time.sleep(delay_s)
        track_dir = audio_dir / label / str(entry["deezerId"])
        result = fetcher.acquire(entry["deezerId"], track_dir, keep_rejected=True)
        (track_dir / "result.json").write_text(
            json.dumps(result.to_json(max_candidates=20), ensure_ascii=False, indent=2),
            encoding="utf-8",
        )
        row = _row(entry, result)
        rows.append(row)
        c = row["chosen"]
        detail = (
            f"{c['similarity']} {c['source']}/{c['kind']} itag {c['itag']} {c['codec']} "
            f"{c['abrKbps']} kbps {c['client']}"
            if c
            else ",".join(row["errors"]) or "—"
        )
        log(
            f"[{i}/{len(entries)}] {entry['deezerId']} {entry['artist']} — {entry['title']}: "
            f"{row['status']} {detail} {row['timingsMs'].get('total', 0) / 1000:.1f} s"
        )
    report = {
        "label": label,
        "startedAt": started_at.isoformat(timespec="seconds"),
        "finishedAt": datetime.now(UTC).isoformat(timespec="seconds"),
        "environment": {
            "host": platform.node(),
            "platform": f"{platform.system()} {platform.machine()}",
            "python": platform.python_version(),
            "tools": tool_versions(fetcher.settings),
            "setup": fetcher.settings.describe(),
            "delayS": delay_s,
            "threshold": DEFAULT_THRESHOLD,
        },
        "summary": summarize(rows),
        "tracks": rows,
    }
    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    out_path.with_suffix(".md").write_text(to_markdown(report), encoding="utf-8")
    return report


def _fmt_s(ms: int | None) -> str:
    return "—" if ms is None else f"{ms / 1000:.1f}"


def to_markdown(report: dict[str, Any]) -> str:
    s = report["summary"]
    env = report["environment"]
    t = s["timingsMs"]
    lines = [
        f"# Прогон `{report['label']}`",
        "",
        (
            f"{report['startedAt']} → {report['finishedAt']}, {env['platform']}, "
            f"yt-dlp {env['tools']['ytDlp']}, {env['tools']['deno'] or 'deno —'}."
        ),
        f"Настройки: `{json.dumps(env['setup'], ensure_ascii=False)}`, пауза {env['delayS']} с.",
        "",
        "| Показатель | Значение |",
        "|---|---|",
        f"| Найдено (есть кандидат ≥ 0,6) | {s['found']} из {s['total']} |",
        f"| Скачано | {s['downloaded']} из {s['total']} |",
        f"| Проверено отпечатком | {s['verified']} из {s['total']} |",
        f"| Статусы | {', '.join(f'{k}: {v}' for k, v in s['statuses'].items())} |",
        f"| Верно с первой попытки | {s['firstAttemptVerified']} |",
        f"| Лучший кандидат отбит отпечатком | {s['topCandidateRejectedByFingerprint']} |",
        f"| Ошибки | {', '.join(f'{k}: {v}' for k, v in s['errors'].items()) or 'нет'} |",
        f"| Форматы (itag) | {', '.join(f'{k}: {v}' for k, v in s['itags'].items())} |",
        (
            f"| Битрейт, кбит/с | медиана {s['abrKbps']['median']}, "
            f"{s['abrKbps']['min']}–{s['abrKbps']['max']} |"
        ),
        f"| Клиенты плеера | {', '.join(f'{k}: {v}' for k, v in s['clients'].items())} |",
        f"| С PO-токеном | {s['poToken']} |",
        "",
        "Время по этапам, секунды (медиана / максимум):",
        "",
        "| Этап | Медиана | Максимум |",
        "|---|---|---|",
    ]
    for stage, values in t.items():
        lines.append(f"| {stage} | {_fmt_s(values['median'])} | {_fmt_s(values['max'])} |")
    lines += [
        "",
        (
            "| # | Трек Deezer | Длит. | Статус | Выбрано на YouTube | Длит. | Источник |"
            " Сходство | Формат | Клиент | Время, с |"
        ),
        "|---|---|---|---|---|---|---|---|---|---|---|",
    ]
    for i, r in enumerate(report["tracks"], 1):
        e = r["expected"]
        c = r["chosen"]
        if c:
            got = f"{c['title']} — {', '.join(c['artists'][:2])} (`{c['videoId']}`)"
            got_len = _fmt_s(c["durationMs"])
            src = f"{c['source']}/{c['kind']}"
            sim = "—" if c["similarity"] is None else f"{c['similarity']:.3f}"
            fmt = f"{c['itag']} {c['codec']} {c['abrKbps']}"
            client = f"{c['client']}{' +pot' if c['poToken'] else ''}"
        else:
            got, got_len, src, sim, fmt, client = (
                (", ".join(r["errors"]) or "—"),
                "",
                "",
                "",
                "",
                "",
            )
        lines.append(
            f"| {i} | {e['artist']} — {e['title']} | {e['durationS']} | {r['status']} | {got} | "
            f"{got_len} | {src} | {sim} | {fmt} | {client} | {_fmt_s(r['timingsMs'].get('total'))} |"
        )
    return "\n".join(lines) + "\n"


# --- Калибровка порога -------------------------------------------------------


def fetch_hard_negatives(
    fetcher: Fetcher,
    spec: Path,
    out_dir: Path,
    *,
    delay_s: float = 5.0,
    log: Callable[[str], None] = _log,
) -> list[dict[str, Any]]:
    """Скачивает другие записи тех же песен (концерты, ремиксы, sped up) и превью к ним.

    Уже скачанное не качается повторно.
    """
    pairs = []
    downloaded = False
    for entry in json.loads(spec.read_text(encoding="utf-8"))["pairs"]:
        track_dir = out_dir / str(entry["deezerId"])
        preview = track_dir / f"{entry['deezerId']}.preview.mp3"
        if not preview.is_file():
            fetcher.deezer.download_preview(fetcher.deezer.track(entry["deezerId"]), preview)
        files = [f for f in track_dir.glob(f"{entry['videoId']}.*") if f.suffix in AUDIO_SUFFIXES]
        if files:
            path = files[0]
        else:
            if downloaded:
                time.sleep(delay_s)
            path = fetcher.downloader.download(entry["videoId"], track_dir, entry["videoId"]).path
            downloaded = True
            log(f"трудный негатив: {entry['note']}")
        pairs.append({**entry, "preview": preview, "file": path})
    return pairs


def _stats(items: list[dict[str, Any]]) -> dict[str, Any]:
    values = sorted(i["sim"] for i in items if i["sim"] is not None)
    if not values:
        return {"n": 0}
    return {
        "n": len(values),
        "min": values[0],
        "p05": values[int(0.05 * (len(values) - 1))],
        "median": statistics.median(values),
        "p95": values[int(0.95 * (len(values) - 1))],
        "max": values[-1],
    }


def calibrate(
    audio_dirs: list[Path], hard_pairs: list[dict[str, Any]] | None = None
) -> dict[str, Any]:
    """Сходство отпечатков на парах трёх видов.

    Позитивы — превью и выбранный файл того же трека. Негативы — превью и выбранные
    файлы других треков набора. Трудные негативы — превью и другая запись той же
    песни (`hard_pairs`). Отдельно — файлы отклонённых попыток: их надо смотреть
    глазами, это может быть и верная запись, которую проверка не узнала.
    """
    previews: dict[str, Path] = {}
    chosen: dict[str, Path] = {}
    rejected: dict[str, list[Path]] = {}
    for audio_dir in audio_dirs:
        for track_dir in sorted(p for p in audio_dir.iterdir() if p.is_dir()):
            key = f"{audio_dir.name}/{track_dir.name}"
            preview = track_dir / f"{track_dir.name}.preview.mp3"
            if preview.is_file():
                previews[key] = preview
            result_file = track_dir / "result.json"
            chosen_name = None
            if result_file.is_file():
                data = json.loads(result_file.read_text(encoding="utf-8"))
                if data.get("file") and data.get("status") == "verified":
                    chosen_name = Path(data["file"]["path"]).name
            for f in sorted(f for f in track_dir.iterdir() if f.suffix in AUDIO_SUFFIXES):
                if f.name == chosen_name:
                    chosen[key] = f
                elif chosen_name is not None:
                    rejected.setdefault(key, []).append(f)

    prints: dict[Path, Any] = {}

    def similarity(preview: Path, track: Path) -> float | None:
        try:
            for path in (preview, track):
                if path not in prints:
                    prints[path] = fingerprint(path)
            res = align(prints[preview].items, prints[track].items)
        except VerifyError:
            return None
        return None if res is None else round(res.similarity, 4)

    positives = [
        {"track": key, "file": path.name, "sim": similarity(previews[key], path)}
        for key, path in chosen.items()
        if key in previews
    ]
    rejected_pairs = [
        {"track": key, "file": p.name, "sim": similarity(previews[key], p)}
        for key, paths in rejected.items()
        if key in previews
        for p in paths
    ]
    negatives = [
        {"preview": key_a, "file": key_b, "sim": similarity(preview, path)}
        for (key_a, preview), (key_b, path) in itertools.product(previews.items(), chosen.items())
        if key_a.split("/")[-1] != key_b.split("/")[-1]
    ]
    hard = [
        {
            "deezerId": p["deezerId"],
            "videoId": p["videoId"],
            "note": p.get("note"),
            "sim": similarity(Path(p["preview"]), Path(p["file"])),
        }
        for p in hard_pairs or []
    ]

    pos_stats, neg_stats, hard_stats = _stats(positives), _stats(negatives), _stats(hard)

    def midpoint(worst: dict[str, Any]) -> float | None:
        if not pos_stats.get("n") or not worst.get("n"):
            return None
        return round((pos_stats["min"] + worst["max"]) / 2, 3)

    return {
        "threshold": DEFAULT_THRESHOLD,
        "positives": pos_stats,
        "negatives": neg_stats,
        "hardNegatives": hard_stats,
        "rejectedAttempts": _stats(rejected_pairs),
        # Середина между худшим позитивом и лучшим негативом каждой группы.
        "midpoint": {"negatives": midpoint(neg_stats), "hardNegatives": midpoint(hard_stats)},
        "positivePairs": sorted(positives, key=lambda i: i["sim"] or 0),
        "hardNegativePairs": sorted(hard, key=lambda i: -(i["sim"] or 0)),
        "rejectedAttemptPairs": sorted(rejected_pairs, key=lambda i: -(i["sim"] or 0)),
        "topNegativePairs": sorted(negatives, key=lambda i: -(i["sim"] or 0))[:10],
    }
